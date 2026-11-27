#include "browser_core/page_in_field_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Now = PageInField::Now;

constexpr Now kTyping = {.agents = true, .editable = true, .focused = true, .key = true};

// Only an agent's tab says it, and only as it changes: a field focused in a user's tab is
// nothing the app hears.
TEST(PageInFieldRuleTest, OnlyAnAgentsTabSaysItAndOnlyAsItChanges) {
  PageInField users;
  EXPECT_EQ(users.Update({.editable = true, .focused = true, .key = true}), std::nullopt);

  PageInField field;
  EXPECT_EQ(field.Update(kTyping), true);
  EXPECT_EQ(field.Update(kTyping), std::nullopt);
  EXPECT_EQ(field.Update({.agents = true, .focused = true, .key = true}), false);
  EXPECT_EQ(field.Update({.agents = true, .focused = true, .key = true}), std::nullopt);
}

// The field is let go as the window stops being key or the UI takes the keyboard, so a cursor
// left in a page doesn't hold the thread once the user moves on.
TEST(PageInFieldRuleTest, TheWindowOrTheUIGoingLetsTheFieldGo) {
  PageInField field;
  EXPECT_EQ(field.Update(kTyping), true);
  EXPECT_EQ(field.Update({.agents = true, .editable = true, .focused = true}), false);
  EXPECT_EQ(field.Update(kTyping), true);
  EXPECT_EQ(field.Update({.agents = true, .editable = true, .key = true}), false);
  EXPECT_EQ(field.Update(kTyping), true);
  // Kept as the user's tab, it's no longer an agent's.
  EXPECT_EQ(field.Update({.editable = true, .focused = true, .key = true}), false);
}

// A tab closing in a field says false, and one that wasn't says nothing.
TEST(PageInFieldRuleTest, ClosingSaysFalseOnlyAfterTrue) {
  PageInField field;
  EXPECT_EQ(field.Closed(), std::nullopt);
  EXPECT_EQ(field.Update(kTyping), true);
  EXPECT_EQ(field.Closed(), false);
  EXPECT_EQ(field.Closed(), std::nullopt);
}

}  // namespace
}  // namespace browser_core
