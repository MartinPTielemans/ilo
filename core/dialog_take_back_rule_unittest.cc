#include "browser_core/dialog_take_back_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::dialog_take_back_rule {
namespace {

TEST(DialogTakeBackRule, ThePageUnderTheDialogStillHoldingItGivesItBack) {
  Turns turns;
  const uint64_t under = turns.Took();
  EXPECT_TRUE(turns.Holds(under));
}

TEST(DialogTakeBackRule, APressInThePaneBesideKeepsTheKeyboardThere) {
  // The window made key focuses the dialog's page, then the press focuses the page beside, both
  // before the dialog's turn comes.
  Turns turns;
  const uint64_t under = turns.Took();
  const uint64_t beside = turns.Took();
  EXPECT_FALSE(turns.Holds(under));
  EXPECT_TRUE(turns.Holds(beside));
}

TEST(DialogTakeBackRule, TheDialogsPagePressedAgainTakesItBack) {
  Turns turns;
  turns.Took();
  turns.Took();
  const uint64_t again = turns.Took();
  EXPECT_TRUE(turns.Holds(again));
}

}  // namespace
}  // namespace browser_core::dialog_take_back_rule
