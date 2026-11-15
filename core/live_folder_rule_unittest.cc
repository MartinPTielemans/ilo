#include "browser_core/live_folder_rule.h"

#include <string>

#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/origin.h"

namespace browser_core::live_folder {
namespace {

// A row as the site's search writes it.
std::string Row(std::string_view number,
                std::string_view title,
                std::string_view owner = "harbor",
                std::string_view state = "open",
                std::string_view reviewable = "ready",
                std::string_view merged = "false") {
  return base::StrCat({R"({"number":)", number, R"(,"hl_title":")", title, R"(","state":")", state,
                       R"(","reviewable_state":")", reviewable, R"(","merged":)", merged,
                       R"(,"repo":{"repository":{"owner_login":")", owner, R"(","name":"web"}}})"});
}

std::string Search(std::string_view signed_in, std::string_view rows) {
  return base::StrCat({R"({"payload":{"blackbirdSearchRoute":{"logged_in":)", signed_in,
                       R"(,"results":[)", rows, "]}}}"});
}

TEST(LiveFolderRule, AsksTheSiteItselfForEachList) {
  for (List list : {List::kReviewRequests, List::kCreated}) {
    const GURL url = ListUrl(list);
    EXPECT_EQ(url::Origin::Create(url), url::Origin::Create(GURL(kSite)));
    EXPECT_EQ(url.path(), "/search");
  }
  EXPECT_NE(ListUrl(List::kReviewRequests).query().find("review-requested%3A%40me"),
            std::string::npos);
  EXPECT_NE(ListUrl(List::kCreated).query().find("author%3A%40me"), std::string::npos);
}

TEST(LiveFolderRule, ReadsRowsWithTheirStates) {
  const std::optional<Answer> answer =
      Parse(Search("true", base::StrCat({Row("7", "Fix the cache"), ",",
                                         // As the site writes a draft (2026-11-15).
                                         Row("8", "Draft", "harbor", "draft", "draft"), ",",
                                         Row("9", "Older draft", "harbor", "open", "draft"), ",",
                                         Row("10", "Merged", "harbor", "closed", "ready", "true"),
                                         ",", Row("11", "Closed", "harbor", "closed")})),
            List::kCreated);
  ASSERT_TRUE(answer);
  EXPECT_TRUE(answer->signed_in);
  // A merged or closed one isn't open, so it's left out.
  ASSERT_EQ(answer->items.size(), 3u);
  EXPECT_EQ(answer->items[0], (Item{.list = List::kCreated,
                                    .repo = "harbor/web",
                                    .number = 7,
                                    .title = "Fix the cache",
                                    .state = State::kOpen}));
  EXPECT_EQ(answer->items[1].state, State::kDraft);
  EXPECT_EQ(answer->items[2].state, State::kDraft);
}

TEST(LiveFolderRule, RowsReachOnlyASpaceOnTheProfileTheyWereAskedWith) {
  const std::optional<Answer> review =
      Parse(Search("true", Row("7", "Review")), List::kReviewRequests);
  const std::optional<Answer> created = Parse(Search("true", Row("8", "Own")), List::kCreated);
  ASSERT_TRUE(review && created);

  const Outcome same = OutcomeOf({review, created}, "Default", "Default");
  EXPECT_EQ(same.kind, Kind::kAnswered);
  ASSERT_EQ(same.items.size(), 2u);
  EXPECT_EQ(same.items[0].number, 7u);
  EXPECT_EQ(same.items[1].number, 8u);

  // Moved while it waited to a Space on another profile, or one that isn't loaded: no rows.
  const Outcome moved = OutcomeOf({review, created}, "Default", "Profile 1");
  EXPECT_EQ(moved.kind, Kind::kFailed);
  EXPECT_TRUE(moved.items.empty());
  EXPECT_EQ(OutcomeOf({review, created}, "Default", "").kind, Kind::kFailed);

  // On its own profile, a list that failed fails it, and a signed-out one makes it signed out.
  EXPECT_EQ(OutcomeOf({review, std::nullopt}, "Default", "Default").kind, Kind::kFailed);
  const std::optional<Answer> out = Parse(Search("false", ""), List::kCreated);
  const Outcome signed_out = OutcomeOf({review, out}, "Default", "Default");
  EXPECT_EQ(signed_out.kind, Kind::kSignedOut);
  EXPECT_TRUE(signed_out.items.empty());
}

TEST(LiveFolderRule, SignedOutHasNoRows) {
  const std::optional<Answer> answer = Parse(Search("false", Row("7", "x")), List::kCreated);
  ASSERT_TRUE(answer);
  EXPECT_FALSE(answer->signed_in);
  EXPECT_TRUE(answer->items.empty());
}

TEST(LiveFolderRule, AnAnswerThatIsntTheSearchFails) {
  EXPECT_FALSE(Parse("<html>Sign in</html>", List::kCreated));
  EXPECT_FALSE(Parse(R"({"payload":{}})", List::kCreated));
  EXPECT_FALSE(Parse(R"({"payload":{"blackbirdSearchRoute":{"logged_in":true}}})", List::kCreated));
  // Too big, though it would parse.
  const std::string big = Search("true", Row("7", std::string(kMaxBytes, 'a')));
  EXPECT_FALSE(Parse(big, List::kCreated));
  // Too deep.
  std::string deep = std::string(64, '[') + std::string(64, ']');
  EXPECT_FALSE(Parse(Search("true", deep), List::kCreated));
}

TEST(LiveFolderRule, LeavesOutRowsThatCantBeOnesAndKeepsTheCap) {
  std::string rows =
      base::StrCat({Row("0", "no number"), ",", Row("7", "bad owner", "a/b"), ",",
                    Row("8", "bad owner", ".."), ",", Row("9", "odd state", "harbor", "locked")});
  for (size_t i = 0; i < kMaxRows + 5; ++i) {
    base::StrAppend(&rows, {",", Row(base::NumberToString(i + 10), "fine")});
  }
  const std::optional<Answer> answer = Parse(Search("true", rows), List::kReviewRequests);
  ASSERT_TRUE(answer);
  ASSERT_EQ(answer->items.size(), kMaxRows);
  EXPECT_EQ(answer->items[0].number, 10u);
  EXPECT_EQ(answer->items[0].list, List::kReviewRequests);
}

TEST(LiveFolderRule, TitlesArePlainText) {
  EXPECT_EQ(PlainTitle("Keep <em>⌘L</em> in the &quot;split&quot; &amp; field"),
            "Keep ⌘L in the \"split\" & field");
  EXPECT_EQ(PlainTitle("a &lt;b&gt; c"), "a <b> c");
  EXPECT_EQ(PlainTitle("  two\n\tlines\x01 here  "), "two lines here");
  EXPECT_EQ(PlainTitle("<script>x</script>"), "x");
  EXPECT_EQ(PlainTitle("unknown &copy; stays"), "unknown &copy; stays");
  const std::string long_title = PlainTitle(std::string(kMaxTitle, 'a') + "é");
  EXPECT_EQ(long_title.size(), kMaxTitle);
  // Cut inside a character, the character goes whole.
  const std::string cut = PlainTitle(std::string(kMaxTitle - 1, 'a') + "é");
  EXPECT_EQ(cut.size(), kMaxTitle - 1);
}

}  // namespace
}  // namespace browser_core::live_folder
