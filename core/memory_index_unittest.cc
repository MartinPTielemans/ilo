#include "browser_core/memory_index.h"

#include <algorithm>
#include <cmath>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "base/files/file_util.h"
#include "base/files/scoped_temp_dir.h"
#include "base/logging.h"
#include "base/rand_util.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/time/time.h"
#include "base/timer/elapsed_timer.h"
#include "browser_core/memory_keep_out.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// Stands in for the OS-held key: XOR, so the file's bytes differ from the text, behind a mark
// that says which key sealed it.
class FakeSealer : public MemoryIndex::Sealer {
 public:
  explicit FakeSealer(char mark) : mark_(mark) {}

  std::optional<std::vector<uint8_t>> Seal(std::string_view plain) const override {
    std::vector<uint8_t> sealed{static_cast<uint8_t>(mark_)};
    for (char c : plain) {
      sealed.push_back(static_cast<uint8_t>(c) ^ 0x5A);
    }
    return sealed;
  }

  std::optional<std::string> Open(base::span<const uint8_t> sealed) const override {
    if (sealed.empty() || sealed[0] != static_cast<uint8_t>(mark_)) {
      return std::nullopt;
    }
    std::string plain;
    for (uint8_t c : sealed.subspan(1u)) {
      plain.push_back(static_cast<char>(c ^ 0x5A));
    }
    return plain;
  }

 private:
  const char mark_;
};

base::Time Day(int day) {
  return base::Time::UnixEpoch() + base::Days(20000 + day);
}

class MemoryIndexTest : public testing::Test {
 protected:
  void SetUp() override {
    ASSERT_TRUE(dir_.CreateUniqueTempDir());
    path_ = dir_.GetPath().AppendASCII("Memory");
    index_ = std::make_unique<MemoryIndex>(std::make_unique<FakeSealer>('A'));
    ASSERT_TRUE(index_->Open(path_));
  }

  std::vector<std::string> Found(std::string_view query) {
    std::vector<std::string> urls;
    for (const MemoryMatch& match : index_->Search(query, 5)) {
      urls.push_back(match.url);
    }
    return urls;
  }

  base::ScopedTempDir dir_;
  base::FilePath path_;
  std::unique_ptr<MemoryIndex> index_;
};

TEST_F(MemoryIndexTest, FindsAPageByItsWordsTheLastAsTyped) {
  ASSERT_TRUE(index_->Add("https://a.test/koji", "a.test", "Koji at home",
                          "Koji is a mould.\nFermentation takes two days at 30 °C.", Day(1)));
  ASSERT_TRUE(index_->Add("https://b.test/ferns", "b.test", "Ferns",
                          "Ferns grow in the shade.", Day(2)));
  EXPECT_EQ(Found("fermentation"), std::vector<std::string>{"https://a.test/koji"});
  // "fer" is both pages' first letters; only one has a word that starts "ferm".
  EXPECT_EQ(Found("ferm"), std::vector<std::string>{"https://a.test/koji"});
  EXPECT_EQ(Found("fer"), (std::vector<std::string>{"https://b.test/ferns", "https://a.test/koji"}));
  EXPECT_EQ(Found("TWO days"), std::vector<std::string>{"https://a.test/koji"});
  EXPECT_TRUE(Found("two weeks").empty());
  EXPECT_TRUE(Found("").empty());
  std::vector<MemoryMatch> matches = index_->Search("fermentation two", 5);
  ASSERT_EQ(matches.size(), 1u);
  EXPECT_EQ(matches[0].title, "Koji at home");
  EXPECT_EQ(matches[0].snippet, "Fermentation takes two days at 30 °C.");
  EXPECT_EQ(matches[0].visited, Day(1));
}

TEST_F(MemoryIndexTest, FindsJapaneseByItsWords) {
  ASSERT_TRUE(index_->Add("https://a.test/tenki", "a.test", "天気",
                          "今日の天気は晴れです。", Day(1)));
  EXPECT_EQ(Found("天気"), std::vector<std::string>{"https://a.test/tenki"});
}

TEST_F(MemoryIndexTest, APageKeptAgainReplacesItself) {
  ASSERT_TRUE(index_->Add("https://a.test/", "a.test", "A", "first words", Day(1)));
  ASSERT_TRUE(index_->Add("https://a.test/", "a.test", "A", "second words", Day(2)));
  EXPECT_EQ(index_->PageCount(), 1);
  EXPECT_TRUE(Found("first").empty());
  EXPECT_EQ(Found("second").size(), 1u);
}

TEST_F(MemoryIndexTest, ForgetsSitesPagesAndEverything) {
  ASSERT_TRUE(index_->Add("https://a.test/1", "a.test", "", "apples", Day(1)));
  ASSERT_TRUE(index_->Add("https://a.test/2", "a.test", "", "apples pears", Day(2)));
  ASSERT_TRUE(index_->Add("https://b.test/1", "b.test", "", "apples plums", Day(3)));
  ASSERT_TRUE(index_->Add("https://c.test/1", "c.test", "", "apples figs", Day(4)));
  index_->ForgetSite("a.test");
  EXPECT_EQ(Found("apples"), (std::vector<std::string>{"https://c.test/1", "https://b.test/1"}));
  // As history's Delete for one page, and Clear browsing data's last hour.
  index_->ForgetPages({"https://b.test/1"}, base::Time(), base::Time());
  EXPECT_EQ(Found("apples"), std::vector<std::string>{"https://c.test/1"});
  ASSERT_TRUE(index_->Add("https://b.test/1", "b.test", "", "apples plums", Day(3)));
  index_->ForgetPages({}, Day(4), Day(5));
  EXPECT_EQ(Found("apples"), std::vector<std::string>{"https://b.test/1"});
  index_->ForgetAll();
  EXPECT_EQ(index_->PageCount(), 0);
  EXPECT_TRUE(Found("apples").empty());
  EXPECT_TRUE(Found("plums").empty());
}

TEST_F(MemoryIndexTest, ASiteKeptOutLosesItsPagesAndComesBack) {
  ASSERT_TRUE(index_->Add("https://bank.test/statement", "bank.test", "", "balance", Day(1)));
  index_->KeepOut("bank.test", KeptOut::kBank, true);
  EXPECT_TRUE(index_->IsKeptOut("bank.test"));
  EXPECT_FALSE(index_->IsKeptOut("other.test"));
  EXPECT_TRUE(Found("balance").empty());
  index_->KeepOut("mine.test", KeptOut::kUser, true);
  EXPECT_EQ(index_->SitesKeptOut(),
            (std::vector<std::pair<std::string, KeptOut>>{{"bank.test", KeptOut::kBank},
                                                          {"mine.test", KeptOut::kUser}}));
  // Forgetting everything forgets pages; the sites kept out are a setting.
  index_->ForgetAll();
  EXPECT_EQ(index_->SitesKeptOut().size(), 2u);
  index_->KeepOut("bank.test", KeptOut::kBank, false);
  EXPECT_FALSE(index_->IsKeptOut("bank.test"));
}

TEST_F(MemoryIndexTest, TheFileHoldsNoWordOrAddressInTheClear) {
  ASSERT_TRUE(index_->Add("https://clinic.test/results", "clinic.test", "My results",
                          "haemoglobin within range", Day(1)));
  index_.reset();
  std::string bytes;
  ASSERT_TRUE(base::ReadFileToString(path_, &bytes));
  for (std::string_view clear : {"haemoglobin", "clinic.test", "results", "range"}) {
    EXPECT_EQ(bytes.find(clear), std::string::npos) << clear;
  }
}

TEST_F(MemoryIndexTest, AnotherKeyOpensNothing) {
  ASSERT_TRUE(index_->Add("https://a.test/", "a.test", "", "apples", Day(1)));
  index_.reset();
  MemoryIndex other(std::make_unique<FakeSealer>('B'));
  EXPECT_FALSE(other.Open(path_));
  EXPECT_TRUE(other.Search("apples", 5).empty());
  MemoryIndex same(std::make_unique<FakeSealer>('A'));
  ASSERT_TRUE(same.Open(path_));
  EXPECT_EQ(same.Search("apples", 5).size(), 1u);
}

TEST(MemoryKeepOutTest, SitesSayWhatTheyAre) {
  EXPECT_EQ(MemorySiteSays("bank.test", {"WebPage", "BankOrCreditUnion"}), KeptOut::kBank);
  EXPECT_EQ(MemorySiteSays("a.test", {"https://schema.org/MedicalWebPage"}), KeptOut::kHealth);
  EXPECT_EQ(MemorySiteSays("a.test", {"schema:Hospital"}), KeptOut::kHealth);
  EXPECT_EQ(MemorySiteSays("example.bank", {}), KeptOut::kDomain);
  EXPECT_EQ(MemorySiteSays("example.insurance", {"WebSite"}), KeptOut::kDomain);
  EXPECT_EQ(MemorySiteSays("news.test", {"NewsArticle", "Organization"}), std::nullopt);
  EXPECT_EQ(MemorySiteSays("banking.test", {}), std::nullopt);
}

// Not a gate: logs what indexing and a query take on this machine, with Chromium's own SQLite,
// ICU and BoringSSL, on made-up pages whose words follow Zipf's law as a language's do.
// spikes/memory-2324/bench measures the same on real pages.
TEST_F(MemoryIndexTest, Timing) {
  std::vector<std::string> vocabulary;
  for (int i = 0; i < 50000; ++i) {
    vocabulary.push_back(base::StrCat({"w", base::NumberToString(i * 7919 % 99991)}));
  }
  auto word = [&] {
    // Zipf: the n-th word is 1/n as likely as the first.
    double u = base::RandDouble();
    return vocabulary[static_cast<size_t>(std::pow(vocabulary.size(), u)) - 1];
  };
  constexpr int kPages = 2000;
  std::vector<base::TimeDelta> adds;
  std::vector<std::string> queries;
  for (int page = 0; page < kPages; ++page) {
    std::string text;
    for (int i = 0; i < 700; ++i) {
      text += word();
      text += i % 12 == 11 ? "\n" : " ";
    }
    if (page % 20 == 0) {
      size_t at = text.find(' ', text.size() / 2) + 1;
      queries.push_back(text.substr(at, text.find(' ', at) - at));
    }
    base::ElapsedTimer timer;
    ASSERT_TRUE(index_->Add(base::StrCat({"https://s", base::NumberToString(page % 97), ".test/",
                                          base::NumberToString(page)}),
                            base::StrCat({"s", base::NumberToString(page % 97), ".test"}), "",
                            text, Day(page)));
    adds.push_back(timer.Elapsed());
  }
  std::vector<base::TimeDelta> searches;
  for (const std::string& query : queries) {
    base::ElapsedTimer timer;
    EXPECT_FALSE(index_->Search(query, 5).empty()) << query;
    searches.push_back(timer.Elapsed());
  }
  std::ranges::sort(adds);
  std::ranges::sort(searches);
  int64_t size = 0;
  base::GetFileSize(path_, &size);
  LOG(ERROR) << "browser: memory timing " << kPages << " pages, file " << size / 1000
             << " KB; add median " << adds[adds.size() / 2].InMicrosecondsF() / 1000 << " ms, p95 "
             << adds[adds.size() * 95 / 100].InMicrosecondsF() / 1000 << " ms; search median "
             << searches[searches.size() / 2].InMicrosecondsF() / 1000 << " ms, p95 "
             << searches[searches.size() * 95 / 100].InMicrosecondsF() / 1000 << " ms";
}

}  // namespace
}  // namespace browser_core
