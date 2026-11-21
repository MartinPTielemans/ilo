#include "browser_core/native_ad_block_lists.h"

#include <string>
#include <vector>

#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/files/scoped_temp_dir.h"
#include "base/test/bind.h"
#include "base/test/mock_log.h"
#include "base/test/scoped_path_override.h"
#include "base/test/task_environment.h"
#include "base/test/test_future.h"
#include "base/time/time.h"
#include "browser_core/mojo_once_for_testing.h"
#include "chrome/common/chrome_paths.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/weak_wrapper_shared_url_loader_factory.h"
#include "services/network/test/test_url_loader_factory.h"
#include "testing/gmock/include/gmock/gmock.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

using ::testing::_;
using ::testing::AnyNumber;
using ::testing::ElementsAre;
using ::testing::HasSubstr;
using ::testing::IsEmpty;

constexpr char kKey[] = "urlhaus-1";
constexpr char kFirst[] = "https://first.example/list.txt";
constexpr char kSecond[] = "https://second.example/list.txt";
constexpr char kThird[] = "https://third.example/list.txt";

// A list of 100 rules and |extra|, under a header saying it was updated at |updated|, as
// URLhaus's says it.
std::string Dated(const std::string& updated, const std::string& extra = "") {
  std::string list = "! Title: A list\n! Updated: " + updated + "\n";
  for (int rule = 0; rule < 100; ++rule) {
    list += "||ads" + std::to_string(rule) + ".example^\n";
  }
  return list + extra;
}

// An update of one list, on the loader's answers given here, in a user data folder of its own.
class NativeAdBlockListsTest : public testing::Test {
 protected:
  // The test loader factory answers through Mojo.
  static void SetUpTestSuite() { StartMojoOnceForTesting(); }

  void SetUp() override {
    ASSERT_TRUE(shipped_.CreateUniqueTempDir());
    factory_.SetInterceptor(base::BindLambdaForTesting(
        [this](const network::ResourceRequest& request) { asked_.push_back(request.url.spec()); }));
    EXPECT_CALL(log_, Log(_, _, _, _, _)).Times(AnyNumber());
  }

  // Expects |times| refusals in the log from here on.
  void ExpectRefusals(int times) {
    EXPECT_CALL(log_, Log(logging::LOGGING_WARNING, _, _, _, HasSubstr("refused a download")))
        .Times(times);
    log_.StartCapturingLogs();
  }

  // Holds |text| as the copy the browser came with, the list having no downloaded copy.
  base::FilePath HoldShipped(const std::string& text) {
    const base::FilePath file = shipped_.GetPath().AppendASCII("urlhaus-filter-online.txt");
    EXPECT_TRUE(base::WriteFile(file, text));
    return file;
  }

  // Holds |text| as the list's downloaded copy, two days old, so a day's update_after has passed.
  base::FilePath HoldDownloaded(const std::string& text) {
    const base::FilePath file = DownloadedAdBlockList(kKey);
    EXPECT_TRUE(base::CreateDirectory(file.DirName()));
    EXPECT_TRUE(base::WriteFile(file, text));
    const base::Time then = base::Time::Now() - base::Days(2);
    EXPECT_TRUE(base::TouchFile(file, then, then));
    return file;
  }

  // Updates the list held at |held| from the three addresses, and gives the keys it downloaded.
  std::vector<std::string> Update(const base::FilePath& held) {
    asked_.clear();
    base::test::TestFuture<std::vector<std::string>> done;
    UpdateNativeAdBlockLists(
        {{kKey, {GURL(kFirst), GURL(kSecond), GURL(kThird)}, base::Days(1), held}},
        factory_.GetSafeWeakWrapper(), done.GetCallback());
    return done.Take();
  }

  std::string DownloadedCopy() {
    std::string text;
    EXPECT_TRUE(base::ReadFileToString(DownloadedAdBlockList(kKey), &text));
    return text;
  }

  base::test::TaskEnvironment task_environment_;
  base::ScopedPathOverride user_data_{chrome::DIR_USER_DATA};
  base::ScopedTempDir shipped_;
  network::TestURLLoaderFactory factory_;
  base::test::MockLog log_;
  // The addresses the update asked, in order.
  std::vector<std::string> asked_;
};

// The first address is a mirror that stopped where the held copy came from: nothing new, so the
// second is asked and its newer copy kept, and the third isn't asked (#3505).
TEST_F(NativeAdBlockListsTest, AFirstMirrorThatStoppedGivesTheNextItsTurn) {
  const base::FilePath held = HoldDownloaded(Dated("2026-10-14T00:10:05Z"));
  factory_.AddResponse(kFirst, Dated("2026-10-14T00:10:05Z"));
  factory_.AddResponse(kSecond, Dated("2026-11-19T00:10:18Z"));
  factory_.AddResponse(kThird, Dated("2026-11-19T00:10:18Z"));
  ExpectRefusals(0);
  EXPECT_THAT(Update(held), ElementsAre(kKey));
  EXPECT_THAT(asked_, ElementsAre(kFirst, kSecond));
  EXPECT_EQ(DownloadedCopy(), Dated("2026-11-19T00:10:18Z"));
}

// The common day: the list hasn't changed since the last update, so each address says the held
// copy's date. The held copy stays, with no line in the log, and the list isn't due again.
TEST_F(NativeAdBlockListsTest, NothingNewAtAnyAddressKeepsTheHeldCopyQuietly) {
  const std::string copy = Dated("2026-11-19T00:10:18Z", "||held.example^\n");
  const base::FilePath held = HoldDownloaded(copy);
  for (const char* address : {kFirst, kSecond, kThird}) {
    factory_.AddResponse(address, Dated("2026-11-19T00:10:18Z"));
  }
  ExpectRefusals(0);
  EXPECT_THAT(Update(held), IsEmpty()) << "nothing for the ad blocker to read again";
  EXPECT_THAT(asked_, ElementsAre(kFirst, kSecond, kThird));
  EXPECT_EQ(DownloadedCopy(), copy) << "the held copy's own text, not a mirror's";
  EXPECT_THAT(Update(held), IsEmpty());
  EXPECT_THAT(asked_, IsEmpty()) << "due again only once its update_after has passed";
}

// Right after a release, the copy the browser came with is current, and every address says its
// date. It's kept as the downloaded copy, its clock started, so the list isn't due again at the
// next look.
TEST_F(NativeAdBlockListsTest, AShippedCopyAsNewAsEveryAddressIsntDueAgain) {
  const std::string copy = Dated("2026-11-19T00:10:18Z", "||shipped.example^\n");
  const base::FilePath held = HoldShipped(copy);
  for (const char* address : {kFirst, kSecond, kThird}) {
    factory_.AddResponse(address, Dated("2026-11-19T00:10:18Z"));
  }
  ExpectRefusals(0);
  EXPECT_THAT(Update(held), IsEmpty());
  EXPECT_THAT(asked_, ElementsAre(kFirst, kSecond, kThird));
  EXPECT_EQ(DownloadedCopy(), copy);
  EXPECT_THAT(Update(held), IsEmpty());
  EXPECT_THAT(asked_, IsEmpty());
}

// A first address older than the held copy is a fault: refused, with a line in the log, and the
// second's newer copy kept.
TEST_F(NativeAdBlockListsTest, AFirstAddressOlderThanTheHeldCopyIsRefused) {
  const base::FilePath held = HoldDownloaded(Dated("2026-11-18T00:10:18Z"));
  factory_.AddResponse(kFirst, Dated("2026-10-14T00:10:05Z"));
  factory_.AddResponse(kSecond, Dated("2026-11-19T00:10:18Z"));
  factory_.AddResponse(kThird, Dated("2026-11-19T00:10:18Z"));
  ExpectRefusals(1);
  EXPECT_THAT(Update(held), ElementsAre(kKey));
  EXPECT_THAT(asked_, ElementsAre(kFirst, kSecond));
  EXPECT_EQ(DownloadedCopy(), Dated("2026-11-19T00:10:18Z"));
}

}  // namespace
}  // namespace browser_core
