#include "browser_core/agent_look_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

TEST(AgentLookRuleTest, AYesWithholdsThePageItAsked) {
  const LookAnswer answer = AnswerOfLook(true, LookedDocument::kShown);
  EXPECT_TRUE(answer.withhold);
  EXPECT_FALSE(answer.keep_from_cache);
}

TEST(AgentLookRuleTest, AYesAfterTheTabLeftItsDocumentWithholdsTheShownPage) {
  const LookAnswer answer = AnswerOfLook(true, LookedDocument::kLeft);
  EXPECT_TRUE(answer.withhold);
  EXPECT_FALSE(answer.keep_from_cache);
}

TEST(AgentLookRuleTest, AYesForADocumentInTheCacheWithholdsTheShownPageAndKeepsItThere) {
  const LookAnswer answer = AnswerOfLook(true, LookedDocument::kCached);
  EXPECT_TRUE(answer.withhold);
  EXPECT_TRUE(answer.keep_from_cache);
}

TEST(AgentLookRuleTest, ARendererGoneIsAYesForThePage) {
  const LookAnswer answer = AnswerOfLook(true, LookedDocument::kRendererGone);
  EXPECT_TRUE(answer.withhold);
  EXPECT_FALSE(answer.keep_from_cache);
}

TEST(AgentLookRuleTest, ANoNeverClearsOrWithholds) {
  for (LookedDocument where : {LookedDocument::kShown, LookedDocument::kCached,
                               LookedDocument::kLeft, LookedDocument::kRendererGone}) {
    const LookAnswer answer = AnswerOfLook(false, where);
    EXPECT_FALSE(answer.withhold);
    EXPECT_FALSE(answer.keep_from_cache);
  }
}

}  // namespace
}  // namespace browser_core
