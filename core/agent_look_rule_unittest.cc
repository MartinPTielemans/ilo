#include "browser_core/agent_look_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

TEST(AgentLookRuleTest, AYesWithholdsThePageItAsked) {
  EXPECT_EQ(AnswerOfLook(true, LookedDocument::kShown), LookAnswer::kWithhold);
}

TEST(AgentLookRuleTest, AYesAfterTheTabLeftItsDocumentWithholdsNothing) {
  EXPECT_EQ(AnswerOfLook(true, LookedDocument::kLeft), LookAnswer::kNothing);
}

TEST(AgentLookRuleTest, AYesForADocumentInTheCacheKeepsItThere) {
  EXPECT_EQ(AnswerOfLook(true, LookedDocument::kCached), LookAnswer::kKeepFromCache);
}

TEST(AgentLookRuleTest, ARendererGoneIsAYesForThePage) {
  EXPECT_EQ(AnswerOfLook(true, LookedDocument::kRendererGone), LookAnswer::kWithhold);
}

TEST(AgentLookRuleTest, ANoNeverClearsOrWithholds) {
  for (LookedDocument where : {LookedDocument::kShown, LookedDocument::kCached,
                               LookedDocument::kLeft, LookedDocument::kRendererGone}) {
    EXPECT_EQ(AnswerOfLook(false, where), LookAnswer::kNothing);
  }
}

}  // namespace
}  // namespace browser_core
