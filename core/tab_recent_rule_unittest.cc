#include "browser_core/tab_recent_rule.h"

#include <optional>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/test/task_environment.h"
#include "base/time/time.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::tab_recent {
namespace {

using live_folder::Answer;
using live_folder::Item;
using live_folder::Kept;
using live_folder::Kind;

// A pinned tab of the user's on the site, on the profile "Default".
Asker Pinned(std::string_view url = "https://github.com/pulls") {
  return {.kept = true, .users = true, .committed = GURL(url), .profile = "Default"};
}

TEST(TabRecentRule, OnlyAPinnedTabOrFavoriteOfTheUsersOnTheSiteAsks) {
  EXPECT_EQ(SourceOf(Pinned()), kPullRequests);
  EXPECT_EQ(SourceOf(Pinned("https://github.com/")), kPullRequests);
  EXPECT_EQ(SourceOf(Pinned("https://github.com/acme/web/pull/482")), kPullRequests);

  // A Today tab, and one in an agent's Space or a private window.
  Asker today = Pinned();
  today.kept = false;
  EXPECT_EQ(SourceOf(today), 0);
  Asker agents = Pinned();
  agents.users = false;
  EXPECT_EQ(SourceOf(agents), 0);

  // Only the site's own origin.
  EXPECT_EQ(SourceOf(Pinned("http://github.com/pulls")), 0);
  EXPECT_EQ(SourceOf(Pinned("https://gist.github.com/")), 0);
  EXPECT_EQ(SourceOf(Pinned("https://github.com:8443/")), 0);
  EXPECT_EQ(SourceOf(Pinned("https://github.com.example.org/")), 0);
  EXPECT_EQ(SourceOf(Pinned("https://example.org/?https://github.com/")), 0);
  EXPECT_EQ(SourceOf(Pinned("about:blank")), 0);
  EXPECT_EQ(SourceOf(Pinned("")), 0);

  // A tab whose profile isn't loaded still asks, and fails (OutcomeFor).
  Asker unloaded = Pinned();
  unloaded.profile.clear();
  EXPECT_EQ(SourceOf(unloaded), kPullRequests);
}

// The card's asks on a made-up clock, through the fetch the Live Folder's share.
class TabRecentAnswers : public testing::Test {
 protected:
  // Asks for the card of the tab |who| on |profile|, counting each time the site would be asked.
  void Ask(int who, const std::string& profile = "Default") {
    fetches_.Ask({profile, kPullRequests}, /*refresh=*/false,
                 base::BindOnce([](int* asked) { ++*asked; }, base::Unretained(&asked_)),
                 base::BindOnce([](std::vector<std::pair<int, Kept>>* answered, int who,
                                   const Kept& kept) { answered->emplace_back(who, kept); },
                                base::Unretained(&answered_), who));
  }

  // The site's answer for |profile|: one row with |number| in each list.
  void Ends(uint32_t number, const std::string& profile = "Default") {
    Answer answer{.signed_in = true, .items = {Item{.repo = "acme/web", .number = number}}};
    fetches_.Ended({profile, kPullRequests}, {answer, answer});
  }

  base::test::TaskEnvironment clock_{base::test::TaskEnvironment::TimeSource::MOCK_TIME};
  live_folder::SharedFetches fetches_;
  int asked_ = 0;
  std::vector<std::pair<int, Kept>> answered_;
};

TEST_F(TabRecentAnswers, ACardInsideTheFoldersFloorGetsWhatTheSiteGaveIt) {
  // The Live Folder's ask, then the card's, on the same profile: the site is asked once.
  Ask(1);
  Ends(7);
  const base::Time given = base::Time::Now();
  clock_.FastForwardBy(live_folder::kFloor - base::Seconds(1));
  Ask(2);
  clock_.RunUntilIdle();
  EXPECT_EQ(asked_, 1);
  ASSERT_EQ(answered_.size(), 2u);
  const Kept& kept = answered_[1].second;
  EXPECT_EQ(kept.answered, given);

  const std::optional<live_folder::Outcome> outcome =
      OutcomeFor(kPullRequests, "Default", Pinned(), kept);
  ASSERT_TRUE(outcome);
  EXPECT_EQ(outcome->kind, Kind::kAnswered);
  ASSERT_EQ(outcome->items.size(), 2u);
  EXPECT_EQ(outcome->items[0].number, 7u);
}

TEST_F(TabRecentAnswers, OnlyATabThatStillAsksIsAnswered) {
  Ask(1);
  Ends(7);
  ASSERT_EQ(answered_.size(), 1u);
  const Kept& kept = answered_[0].second;

  // Closed, unpinned, gone off the site or into an agent's Space while it waited: nobody asked.
  EXPECT_FALSE(OutcomeFor(kPullRequests, "Default", std::nullopt, kept));
  Asker unpinned = Pinned();
  unpinned.kept = false;
  EXPECT_FALSE(OutcomeFor(kPullRequests, "Default", unpinned, kept));
  EXPECT_FALSE(OutcomeFor(kPullRequests, "Default", Pinned("https://example.org/"), kept));
  Asker agents = Pinned();
  agents.users = false;
  EXPECT_FALSE(OutcomeFor(kPullRequests, "Default", agents, kept));
  // Nor a source it never asked.
  EXPECT_FALSE(OutcomeFor(0, "Default", Pinned(), kept));

  // Still on the site, on another page of it: answered.
  const std::optional<live_folder::Outcome> moved =
      OutcomeFor(kPullRequests, "Default", Pinned("https://github.com/acme/web"), kept);
  ASSERT_TRUE(moved);
  EXPECT_EQ(moved->kind, Kind::kAnswered);
}

TEST_F(TabRecentAnswers, RowsReachNoTabOnAnotherProfile) {
  Ask(1);
  Ends(7);
  const Kept& kept = answered_[0].second;

  // A favorite whose profile changed while it waited, or whose profile is no longer loaded, fails
  // rather than show rows from the session it asked with.
  Asker elsewhere = Pinned();
  elsewhere.profile = "Profile 1";
  const std::optional<live_folder::Outcome> moved =
      OutcomeFor(kPullRequests, "Default", elsewhere, kept);
  ASSERT_TRUE(moved);
  EXPECT_EQ(moved->kind, Kind::kFailed);
  EXPECT_TRUE(moved->items.empty());
  Asker unloaded = Pinned();
  unloaded.profile.clear();
  EXPECT_EQ(OutcomeFor(kPullRequests, "Default", unloaded, kept)->kind, Kind::kFailed);

  // A tab that asked with its profile not loaded fails on no lists, even once it has loaded.
  const Kept none{.answered = base::Time::Now()};
  EXPECT_EQ(OutcomeFor(kPullRequests, "", Pinned(), none)->kind, Kind::kFailed);

  // Another profile's card has a fetch and a floor of its own.
  Ask(2, "Profile 1");
  EXPECT_EQ(asked_, 2);
}

}  // namespace
}  // namespace browser_core::tab_recent
