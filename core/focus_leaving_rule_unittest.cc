#include "browser_core/focus_leaving_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::focus_leaving_rule {
namespace {

TEST(FocusLeavingRule, TheUsersShownTabAsksTheApp) {
  int asked = 0;
  EXPECT_EQ(Leave(/*agent_tab=*/false, /*shown=*/true,
                  [&] {
                    ++asked;
                    return true;
                  }),
            Kept::kByApp);
  EXPECT_EQ(asked, 1);
}

TEST(FocusLeavingRule, ThePageKeepsItWhenTheAppDoesnt) {
  EXPECT_EQ(Leave(/*agent_tab=*/false, /*shown=*/true, [] { return false; }), Kept::kByPage);
}

TEST(FocusLeavingRule, AnAgentsTabNeverAsksTheApp) {
  // Shown, as when the user watches an agent's Space, and not: the user's keyboard and view stay
  // where they are either way.
  for (bool shown : {true, false}) {
    int asked = 0;
    EXPECT_EQ(Leave(/*agent_tab=*/true, shown,
                    [&] {
                      ++asked;
                      return true;
                    }),
              Kept::kAgentsPage);
    EXPECT_EQ(asked, 0);
  }
}

TEST(FocusLeavingRule, NorATabNoWindowShows) {
  int asked = 0;
  EXPECT_EQ(Leave(/*agent_tab=*/false, /*shown=*/false,
                  [&] {
                    ++asked;
                    return true;
                  }),
            Kept::kUnshown);
  EXPECT_EQ(asked, 0);
}

TEST(FocusLeavingRule, TabFromTheRingGivesTheUsersPageTheKeyboard) {
  int given = 0;
  EXPECT_TRUE(Enter(/*agent_tab=*/false, [&] { ++given; }));
  EXPECT_EQ(given, 1);
}

TEST(FocusLeavingRule, ButNeverAnAgentsTabTheUserDoesntHold) {
  // Watched in its Space, where a click takes it over first: the page's focused element stays
  // where the agent left it, and the user's keyboard isn't put in a page the agent acts in.
  int given = 0;
  EXPECT_FALSE(Enter(/*agent_tab=*/true, [&] { ++given; }));
  EXPECT_EQ(given, 0);
}

}  // namespace
}  // namespace browser_core::focus_leaving_rule
