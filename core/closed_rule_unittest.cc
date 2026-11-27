#include "browser_core/closed_rule.h"

#include <algorithm>
#include <optional>
#include <string>
#include <vector>

#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/uuid.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::closed_rule {
namespace {

const base::Time kStart = base::Time::FromSecondsSinceUnixEpoch(1'800'000'000);

Closed Tab(uint64_t id, int minute, bool by_user, std::string profile = "Default") {
  return {.id = id, .at = kStart + base::Minutes(minute), .profile = profile, .by_user = by_user};
}

Closed Window(uint64_t id, int minute, std::string profile = "Default") {
  return {.id = id,
          .window = true,
          .at = kStart + base::Minutes(minute),
          .profile = profile,
          .by_user = true};
}

// What ⇧⌘T would bring back, by id, or 0.
uint64_t NextId(const std::vector<Closed>& newest_first, const std::string& profile = "Default") {
  std::optional<size_t> next = Next(newest_first, profile);
  return next ? newest_first[*next].id : 0;
}

TEST(ClosedRule, AfterTheTimerTakesATabTheUsersCloseComesBack) {
  // The user closed 1, then auto-archive took 2: ⇧⌘T brings back 1, and 2 stays archived.
  const std::vector<Closed> archive = {Tab(2, 10, /*by_user=*/false), Tab(1, 5, true)};
  EXPECT_EQ(NextId(archive), 1u);
  EXPECT_EQ(archive.size(), 2u);
}

TEST(ClosedRule, AClosedWindowComesBackWhenItClosedLast) {
  const std::vector<Closed> tabs = {Tab(1, 5, true)};
  const std::vector<Closed> windows = {Window(7, 9)};
  const std::vector<Closed> merged = Merged(tabs, windows);
  ASSERT_EQ(merged.size(), 2u);
  EXPECT_EQ(NextId(merged), 7u);
  EXPECT_TRUE(merged[*Next(merged, "Default")].window);
  // Once it's back, the tab closed before it.
  EXPECT_EQ(NextId(Merged(tabs, {})), 1u);
}

TEST(ClosedRule, AWindowsTabsArchivedAsItClosedComeAfterIt) {
  // A tab archived at the moment its window closed is merged before the window, so the window,
  // which brings its Space back, is what ⇧⌘T takes only when it closed later.
  const std::vector<Closed> merged = Merged({Tab(1, 9, true)}, {Window(7, 9)});
  EXPECT_EQ(merged[0].id, 1u);
  EXPECT_EQ(merged[1].id, 7u);
}

TEST(ClosedRule, AfterARelaunchTheLastTabClosedBeforeTheQuitComesBack) {
  // As the session file writes and reads the archive: who closed each, and its profile.
  const std::vector<Closed> before = {Tab(3, 12, false), Tab(2, 11, true), Tab(1, 10, true)};
  base::ListValue saved;
  for (const Closed& closed : before) {
    base::DictValue dict;
    dict.Set("archived", closed.at.InMillisecondsFSinceUnixEpoch());
    Save(closed, dict);
    saved.Append(std::move(dict));
  }
  std::optional<std::string> json = base::WriteJson(saved);
  ASSERT_TRUE(json);
  std::optional<base::Value> read = base::JSONReader::Read(*json, base::JSON_PARSE_RFC);
  ASSERT_TRUE(read && read->is_list());
  std::vector<Closed> after;
  // A launch gives each tab a new id.
  uint64_t id = 100;
  for (const base::Value& value : read->GetList()) {
    Closed& closed = after.emplace_back();
    closed.id = id++;
    closed.at = base::Time::FromMillisecondsSinceUnixEpoch(*value.GetDict().FindDouble("archived"));
    Load(value.GetDict(), closed);
  }
  ASSERT_EQ(after.size(), 3u);
  EXPECT_FALSE(after[0].by_user);
  EXPECT_EQ(after[1].profile, "Default");
  EXPECT_EQ(NextId(after), 101u);
}

TEST(ClosedRule, OneSavedBeforeWhoClosedItWasKeptIsLeftAlone) {
  base::DictValue old;
  old.Set("archived", kStart.InMillisecondsFSinceUnixEpoch());
  Closed closed;
  Load(old, closed);
  EXPECT_FALSE(closed.by_user);
  EXPECT_FALSE(Next({closed}, "").has_value());
}

TEST(ClosedRule, ThirtyClosedTabsComeBackOneByOne) {
  std::vector<Closed> archive;
  for (int minute = 30; minute >= 1; --minute) {
    archive.push_back(Tab(minute, minute, true));
  }
  for (uint64_t expected = 30; expected >= 1; --expected) {
    std::optional<size_t> next = Next(archive, "Default");
    ASSERT_TRUE(next) << "nothing came back after " << 30 - expected << " tabs";
    EXPECT_EQ(archive[*next].id, expected);
    // A tab reopened leaves the archive.
    archive.erase(archive.begin() + *next);
  }
  EXPECT_FALSE(Next(archive, "Default").has_value());
}

TEST(ClosedRule, TheTimersTabsStayInTheArchiveAndNeverComeBack) {
  std::vector<Closed> archive = {Tab(3, 3, false), Tab(2, 2, true), Tab(1, 1, false)};
  archive.erase(archive.begin() + *Next(archive, "Default"));
  EXPECT_FALSE(Next(archive, "Default").has_value());
  // The Library still lists both.
  ASSERT_EQ(archive.size(), 2u);
  EXPECT_EQ(archive[0].id, 3u);
  EXPECT_EQ(archive[1].id, 1u);
}

TEST(ClosedRule, ATabDeletedInTheLibraryLeavesTheListAndItsUndoPutsItBack) {
  std::vector<Closed> archive = {Tab(2, 2, true), Tab(1, 1, true)};
  // Delete in the Library takes 2 out of the archive, so ⇧⌘T brings back 1.
  const Closed deleted = archive.front();
  archive.erase(archive.begin());
  EXPECT_EQ(NextId(archive), 1u);
  // ⌘Z puts it back at its place, and it's next again.
  archive.insert(archive.begin(), deleted);
  EXPECT_EQ(NextId(archive), 2u);
}

TEST(ClosedRule, TheListFollowsTheWindowsProfile) {
  const std::vector<Closed> archive = {Tab(2, 2, true, "Profile 1"), Tab(1, 1, true)};
  EXPECT_EQ(NextId(archive, "Default"), 1u);
  EXPECT_EQ(NextId(archive, "Profile 1"), 2u);
  EXPECT_EQ(NextId(archive, "Profile 2"), 0u);
}

TEST(ClosedRule, TheTwentySixthCloseDropsTheOldestsHistoryAndKeepsItsPage) {
  // 26 the user closed, newest first, with the timer's among them, which don't count.
  std::vector<Closed> archive;
  for (int minute = 26; minute >= 1; --minute) {
    archive.push_back(Tab(minute, minute, true));
    archive.push_back(Tab(100 + minute, minute, false));
  }
  const std::vector<uint64_t> with_history = WithHistory(archive);
  EXPECT_EQ(with_history.size(), kWithHistory);
  EXPECT_FALSE(std::ranges::contains(with_history, 1u)) << "the oldest kept its history";
  EXPECT_TRUE(std::ranges::contains(with_history, 2u));
  EXPECT_TRUE(std::ranges::contains(with_history, 26u));
  // Its row stays, so once the 25 are back ⇧⌘T brings it back too, at its page.
  for (size_t reopened = 0; reopened < kWithHistory; ++reopened) {
    archive.erase(archive.begin() + *Next(archive, "Default"));
  }
  EXPECT_EQ(NextId(archive), 1u);
}

TEST(ClosedRule, AClosedWindowsRecordOutlastsARelaunch) {
  const ClosedWindow closed{.id = 7,
                            .closed = kStart,
                            .bounds = gfx::Rect(40, 60, 1200, 800),
                            .maximized = true,
                            .fullscreen = true,
                            .sidebar_width = 260,
                            .space = base::Uuid::GenerateRandomV4(),
                            .tab = base::Uuid::GenerateRandomV4(),
                            .profile = "Profile 1"};
  std::optional<std::string> json = base::WriteJson(SaveWindow(closed));
  ASSERT_TRUE(json);
  std::optional<base::Value> read = base::JSONReader::Read(*json, base::JSON_PARSE_RFC);
  ASSERT_TRUE(read && read->is_dict());
  std::optional<ClosedWindow> back = LoadWindow(read->GetDict());
  ASSERT_TRUE(back);
  // A launch gives it a new id.
  EXPECT_EQ(back->id, 0u);
  EXPECT_EQ(back->closed, kStart);
  EXPECT_EQ(back->bounds, closed.bounds);
  EXPECT_TRUE(back->maximized);
  EXPECT_TRUE(back->fullscreen);
  EXPECT_EQ(back->sidebar_width, 260.0);
  EXPECT_EQ(back->space, closed.space);
  EXPECT_EQ(back->tab, closed.tab);
  EXPECT_EQ(back->profile, "Profile 1");
  // One with no Space to show is none.
  base::DictValue spaceless = SaveWindow(closed);
  ASSERT_TRUE(spaceless.Remove("space"));
  EXPECT_FALSE(LoadWindow(spaceless));
}

}  // namespace
}  // namespace browser_core::closed_rule
