#include "browser_core/idle_check.h"

#include <algorithm>
#include <cstdint>
#include <deque>
#include <map>
#include <optional>
#include <utility>
#include <vector>

#include "base/functional/callback.h"
#include "base/time/time.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr base::TimeDelta kAfter = base::Hours(2);

// The core as a check sees it: tabs with when each was last seen, and a queue of posted tasks
// that runs only when the test says.
class FakeHost : public IdleCheck::Host {
 public:
  void Post(base::OnceClosure task) override { tasks.push_back(std::move(task)); }
  bool On() override { return on; }
  std::vector<uint64_t> Tabs() override {
    std::vector<uint64_t> tabs;
    for (const auto& [tab, when] : seen) {
      tabs.push_back(tab);
    }
    return tabs;
  }
  std::optional<base::Time> LastSeen(uint64_t tab) override {
    auto it = seen.find(tab);
    return it == seen.end() ? std::nullopt : std::make_optional(it->second);
  }
  // Each tab once, as the check that follows another takes the tabs that one unloaded again.
  void Unload(uint64_t tab) override {
    if (std::find(unloaded.begin(), unloaded.end(), tab) == unloaded.end()) {
      unloaded.push_back(tab);
    }
  }
  void Checked() override { ++checked; }

  // As check.sh's age does.
  void Age(base::TimeDelta by) {
    for (auto& [tab, when] : seen) {
      when -= by;
    }
  }
  void RunOne() {
    base::OnceClosure task = std::move(tasks.front());
    tasks.pop_front();
    std::move(task).Run();
  }
  void RunAll() {
    while (!tasks.empty()) {
      RunOne();
    }
  }

  bool on = true;
  std::map<uint64_t, base::Time> seen;
  std::deque<base::OnceClosure> tasks;
  std::vector<uint64_t> unloaded;
  int checked = 0;
};

// tabs' sign-in step on the Mac (#2388): a switch asks for a check, the tabs age before its task
// starts, and the switch after that finds the check still posted. The check has to see the ageing.
TEST(IdleCheckTest, TabsThatAgeBeforeAPostedCheckStartsAreUnloaded) {
  FakeHost host;
  IdleCheck check(host, kAfter);
  const base::Time now = base::Time::Now();
  host.seen = {{4, now}, {5, now}};
  check.Ask();
  host.Age(kAfter);
  // The next switch leaves tab 5 for a new tab 6, so 5 was just seen.
  host.seen[5] = base::Time::Now();
  host.seen[6] = base::Time::Now();
  check.Ask();
  ASSERT_EQ(host.tasks.size(), 1u);
  host.RunAll();
  EXPECT_EQ(host.unloaded, std::vector<uint64_t>{4});
  EXPECT_EQ(host.checked, 1);
}

TEST(IdleCheckTest, EachTaskUnloadsOneTabAndTheCheckEndsAfterTheLast) {
  FakeHost host;
  IdleCheck check(host, kAfter);
  const base::Time long_ago = base::Time::Now() - kAfter;
  host.seen = {{1, long_ago}, {2, long_ago}, {3, long_ago}};
  check.Ask();
  for (size_t unloaded = 1; unloaded <= 3; ++unloaded) {
    host.RunOne();
    EXPECT_EQ(host.unloaded.size(), unloaded);
    EXPECT_EQ(host.checked, 0);
  }
  host.RunAll();
  EXPECT_EQ(host.unloaded.size(), 3u);
  EXPECT_EQ(host.checked, 1);
}

TEST(IdleCheckTest, ATabSeenBeforeItsTurnKeepsItsPage) {
  FakeHost host;
  IdleCheck check(host, kAfter);
  const base::Time long_ago = base::Time::Now() - kAfter;
  host.seen = {{1, long_ago}, {2, long_ago}};
  check.Ask();
  host.RunOne();
  ASSERT_EQ(host.unloaded.size(), 1u);
  const uint64_t other = host.unloaded[0] == 1 ? 2 : 1;
  host.seen[other] = base::Time::Now();
  host.RunAll();
  EXPECT_EQ(host.unloaded.size(), 1u);
}

TEST(IdleCheckTest, AClosedTabIsPassedOver) {
  FakeHost host;
  IdleCheck check(host, kAfter);
  const base::Time long_ago = base::Time::Now() - kAfter;
  host.seen = {{1, long_ago}, {2, long_ago}};
  check.Ask();
  host.RunOne();
  ASSERT_EQ(host.unloaded.size(), 1u);
  host.seen.erase(host.unloaded[0] == 1 ? 2 : 1);
  host.RunAll();
  EXPECT_EQ(host.unloaded.size(), 1u);
  EXPECT_EQ(host.checked, 1);
}

// The check took its tabs before tab 3 was idle, so another follows it, and only that one says
// it's done.
TEST(IdleCheckTest, AnAskWhileACheckRunsChecksAgainAfterIt) {
  FakeHost host;
  IdleCheck check(host, kAfter);
  const base::Time now = base::Time::Now();
  host.seen = {{1, now - kAfter}, {2, now - kAfter}, {3, now}};
  check.Ask();
  host.RunOne();
  host.seen[3] = now - kAfter;
  check.Ask();
  host.RunAll();
  EXPECT_EQ(host.unloaded.size(), 3u);
  EXPECT_EQ(host.unloaded.back(), 3u);
  EXPECT_EQ(host.checked, 1);
}

TEST(IdleCheckTest, NothingIsUnloadedWhileOffAndAskingAgainChecksAgain) {
  FakeHost host;
  IdleCheck check(host, kAfter);
  host.seen = {{1, base::Time::Now() - kAfter}};
  host.on = false;
  check.Ask();
  host.RunAll();
  EXPECT_TRUE(host.unloaded.empty());
  EXPECT_EQ(host.checked, 1);
  host.on = true;
  check.Ask();
  host.RunAll();
  EXPECT_EQ(host.unloaded, std::vector<uint64_t>{1});
  EXPECT_EQ(host.checked, 2);
}

// Memory short, as calls.sh raises it: each tab with when it was last seen and what the core
// knows of it.
struct PressureTabs {
  std::optional<uint64_t> Pick() {
    std::vector<std::pair<uint64_t, base::Time>> tabs;
    for (const auto& [tab, when] : seen) {
      tabs.emplace_back(tab, when);
    }
    return PickUnderPressure(tabs, [this](uint64_t tab) {
      asked.push_back(tab);
      return facts[tab];
    });
  }
  // As the pick's tab lets go of its page.
  void Unload(uint64_t tab) { facts[tab].has_page = false; }

  std::map<uint64_t, base::Time> seen;
  std::map<uint64_t, UnloadFacts> facts;
  std::vector<uint64_t> asked;
};

// The fault in #686: a tab with no page, such as one put back at launch and not yet shown, stayed
// the unseen longest, so every notification picked it and freed nothing.
TEST(PressurePickTest, ATabWithNoPageIsPassedOverAndEachNotificationFreesAnother) {
  const base::Time now = base::Time::Now();
  PressureTabs tabs;
  tabs.seen = {{1, now - base::Hours(3)}, {2, now - base::Hours(2)}, {3, now - base::Hours(1)}};
  tabs.facts[1].has_page = false;
  EXPECT_EQ(tabs.Pick(), 2u);
  tabs.Unload(2);
  EXPECT_EQ(tabs.Pick(), 3u);
  tabs.Unload(3);
  EXPECT_EQ(tabs.Pick(), std::nullopt);
}

TEST(PressurePickTest, ACallThatOnlyListensAnAgentsTabAndAPageThatAsksKeepTheirPages) {
  const base::Time now = base::Time::Now();
  PressureTabs tabs;
  tabs.seen = {{1, now - base::Hours(4)},
               {2, now - base::Hours(3)},
               {3, now - base::Hours(2)},
               {4, now - base::Hours(1)}};
  tabs.facts[1].in_call = true;
  tabs.facts[2].agents = true;
  tabs.facts[3].asks_before_leaving = true;
  EXPECT_EQ(tabs.Pick(), 4u);
  tabs.Unload(4);
  EXPECT_EQ(tabs.Pick(), std::nullopt);
}

TEST(PressurePickTest, TheTabShownAndOneChromesChecksKeepAreKept) {
  const base::Time now = base::Time::Now();
  PressureTabs tabs;
  tabs.seen = {{1, now - base::Hours(2)}, {2, now - base::Hours(1)}, {3, now}};
  tabs.facts[1].chrome_keeps = true;
  tabs.facts[3].shown = true;
  EXPECT_EQ(tabs.Pick(), 2u);
}

// Chrome's checks cost a look at the page, so the pick stops at the first tab that may go.
TEST(PressurePickTest, TabsAreAskedOldestFirstUntilOneMayGo) {
  const base::Time now = base::Time::Now();
  PressureTabs tabs;
  tabs.seen = {{7, now - base::Hours(1)}, {8, now - base::Hours(3)}, {9, now - base::Hours(2)}};
  tabs.facts[8].in_call = true;
  EXPECT_EQ(tabs.Pick(), 9u);
  EXPECT_EQ(tabs.asked, (std::vector<uint64_t>{8, 9}));
}

// The 2-hour check lets a tab with no page go too, which costs nothing, but nothing else keeps a
// tab the user would lose something in.
TEST(MayUnloadTest, OnlyATabNothingKeepsMayGo) {
  EXPECT_TRUE(MayUnload(UnloadFacts{}));
  EXPECT_TRUE(MayUnload(UnloadFacts{.has_page = false}));
  EXPECT_FALSE(MayUnload(UnloadFacts{.agents = true, .has_page = false}));
  for (bool UnloadFacts::*kept :
       {&UnloadFacts::closing, &UnloadFacts::shown, &UnloadFacts::private_window,
        &UnloadFacts::agents, &UnloadFacts::system_page, &UnloadFacts::devtools_open,
        &UnloadFacts::asks_before_leaving, &UnloadFacts::in_call, &UnloadFacts::chrome_keeps}) {
    UnloadFacts facts;
    facts.*kept = true;
    EXPECT_FALSE(MayUnload(facts));
  }
}

}  // namespace
}  // namespace browser_core
