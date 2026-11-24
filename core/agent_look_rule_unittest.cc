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

// A look the limit ends answers its call that the page wasn't checked, and withholds nothing; its
// own answer, when it comes, decides as any look's does.
TEST(AgentLookRuleTest, ALookTheLimitEndsIsUncheckedAndKeptForItsOwnAnswer) {
  const CutLook cut = CutOfLook(LookCut::kLate);
  EXPECT_EQ(cut.call, LookEnd::kUnchecked);
  EXPECT_TRUE(cut.kept);
  EXPECT_TRUE(AnswerOfLook(true, LookedDocument::kShown).withhold);
  const LookAnswer no = AnswerOfLook(false, LookedDocument::kShown);
  EXPECT_FALSE(no.withhold);
  EXPECT_FALSE(no.keep_from_cache);
}

TEST(AgentLookRuleTest, ASessionsEndAnswersItsCallsWithheldAndKeepsTheLook) {
  const CutLook cut = CutOfLook(LookCut::kSessionEnded);
  EXPECT_EQ(cut.call, LookEnd::kWithheld);
  EXPECT_TRUE(cut.kept);
}

TEST(AgentLookRuleTest, APageLetGoAnswersItsCallsWithheldAndKeepsNoLook) {
  const CutLook cut = CutOfLook(LookCut::kGone);
  EXPECT_EQ(cut.call, LookEnd::kWithheld);
  EXPECT_FALSE(cut.kept);
}

// Only a look cut for time answers that it wasn't checked, so no other call hears it may try
// again.
TEST(AgentLookRuleTest, OnlyALateLookIsUnchecked) {
  for (LookCut each : {LookCut::kSessionEnded, LookCut::kGone}) {
    EXPECT_NE(CutOfLook(each).call, LookEnd::kUnchecked);
  }
}

// A page keeps no more looks than the cap: past it a call hears it wasn't checked at once, and
// sends nothing a renderer that never answers would keep.
TEST(AgentLookRuleTest, APageKeepsNoMoreLooksThanTheCap) {
  for (size_t kept = 0; kept < kMostKeptLooks; ++kept) {
    EXPECT_TRUE(LooksAgain(kept)) << kept;
  }
  EXPECT_FALSE(LooksAgain(kMostKeptLooks));
  EXPECT_FALSE(LooksAgain(kMostKeptLooks + 100));
  EXPECT_EQ(CutOfLook(LookCut::kLate).call, LookEnd::kUnchecked);
}

}  // namespace
}  // namespace browser_core
