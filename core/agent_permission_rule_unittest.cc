#include "browser_core/agent_permission_rule.h"

#include <set>
#include <string>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::agent_permission_rule {
namespace {

using agent_space_rule::Tab;

constexpr Tab kUsers{};
constexpr Tab kAgents{.agents = true};
constexpr Tab kHeld{.agents = true, .held = true};

Kind KindAt(size_t at) {
  return static_cast<Kind>(at);
}

TEST(AgentPermissionRule, AUsersTabIsRefusedNothing) {
  for (size_t at = 0; at < kKinds; ++at) {
    EXPECT_FALSE(Refuses(KindAt(at), kUsers)) << at;
  }
}

TEST(AgentPermissionRule, AnAgentsSpaceIsRefusedEveryKindButThoseLeftAsToday) {
  for (size_t at = 0; at < kKinds; ++at) {
    EXPECT_EQ(Refuses(KindAt(at), kAgents), KindAt(at) != Kind::kLeftAsToday) << at;
  }
}

// The user holding the tab changes one answer: their own copy writes the clipboard.
TEST(AgentPermissionRule, AHeldTabIsRefusedAsOneNotHeldButForACopy) {
  for (size_t at = 0; at < kKinds; ++at) {
    const Kind kind = KindAt(at);
    if (kind == Kind::kClipboardWrite) {
      EXPECT_FALSE(Refuses(kind, kHeld));
      continue;
    }
    EXPECT_EQ(Refuses(kind, kHeld), Refuses(kind, kAgents)) << at;
  }
}

// The app words the entry from these, so they are spelled here once more.
TEST(AgentPermissionRule, TheKindsTheAppHasASentenceForKeepTheirWords) {
  EXPECT_STREQ(CodeWord(Kind::kCamera), "camera");
  EXPECT_STREQ(CodeWord(Kind::kCameraPanTiltZoom), "camera_pan_tilt_zoom");
  EXPECT_STREQ(CodeWord(Kind::kMicrophone), "microphone");
  EXPECT_STREQ(CodeWord(Kind::kLocation), "location");
  EXPECT_STREQ(CodeWord(Kind::kClipboardRead), "clipboard_read");
  EXPECT_STREQ(CodeWord(Kind::kClipboardWrite), "clipboard_write");
  EXPECT_STREQ(CodeWord(Kind::kScreenCapture), "screen_capture");
  EXPECT_STREQ(CodeWord(Kind::kNotifications), "notifications");
  EXPECT_STREQ(CodeWord(Kind::kFilePicker), "file_picker");
  EXPECT_STREQ(kEntryAction, "request_permission ");
  EXPECT_STREQ(kEntryReason, "agent_space");
}

// A code word is one word of small letters, and no two refused kinds share one.
TEST(AgentPermissionRule, EveryRefusedKindHasACodeWordOfItsOwn) {
  std::set<std::string> words;
  for (size_t at = 0; at < kKinds; ++at) {
    if (KindAt(at) == Kind::kLeftAsToday) {
      continue;
    }
    const std::string word = CodeWord(KindAt(at));
    ASSERT_FALSE(word.empty()) << at;
    for (char letter : word) {
      EXPECT_TRUE((letter >= 'a' && letter <= 'z') || letter == '_') << word;
    }
    EXPECT_TRUE(words.insert(word).second) << word;
  }
}

TEST(AgentPermissionRule, ADocumentHasOneEntryPerKind) {
  EntriesMade made;
  EXPECT_TRUE(made.First(Kind::kCamera));
  EXPECT_FALSE(made.First(Kind::kCamera));
  EXPECT_TRUE(made.First(Kind::kMicrophone));
  EXPECT_FALSE(made.First(Kind::kCamera));
  EXPECT_FALSE(made.First(Kind::kMicrophone));
  // Another document starts over.
  EntriesMade next;
  EXPECT_TRUE(next.First(Kind::kCamera));
}

TEST(AgentPermissionRule, AUsersTabKeepsItsHandlesOnFiles) {
  EXPECT_FALSE(RefusesFileHandle(/*site_storage=*/false, kUsers));
  EXPECT_FALSE(RefusesFileHandle(/*site_storage=*/true, kUsers));
}

TEST(AgentPermissionRule, AHandleOnAUsersFileIsRefusedInAnAgentsSpace) {
  EXPECT_TRUE(RefusesFileHandle(/*site_storage=*/false, kAgents));
}

// As the picker for a file is: the user holding the tab opens none of their files to its page.
TEST(AgentPermissionRule, AHeldTabIsRefusedAHandleOnAUsersFileToo) {
  EXPECT_TRUE(RefusesFileHandle(/*site_storage=*/false, kHeld));
}

TEST(AgentPermissionRule, ASitesOwnStorageIsItsPagesInAnyTab) {
  EXPECT_FALSE(RefusesFileHandle(/*site_storage=*/true, kAgents));
  EXPECT_FALSE(RefusesFileHandle(/*site_storage=*/true, kHeld));
}

// A worker that is no frame's is in no tab, which the wiring gives the rule as a tab of no Space.
TEST(AgentPermissionRule, AFrameInNoTabKeepsItsHandles) {
  EXPECT_FALSE(RefusesFileHandle(/*site_storage=*/false, Tab{}));
}

TEST(AgentPermissionRule, AUsersTabIsAskedAsEver) {
  EXPECT_EQ(TakesRequest(/*link_handler_offer=*/false, kUsers), Taken::kNo);
  EXPECT_EQ(TakesRequest(/*link_handler_offer=*/true, kUsers), Taken::kNo);
}

TEST(AgentPermissionRule, AQuestionForAPageInAnAgentsSpaceIsCancelled) {
  EXPECT_EQ(TakesRequest(/*link_handler_offer=*/false, kAgents), Taken::kCancelled);
}

TEST(AgentPermissionRule, ASitesOfferToHandleALinkIsDroppedUnanswered) {
  EXPECT_EQ(TakesRequest(/*link_handler_offer=*/true, kAgents), Taken::kDropped);
}

TEST(AgentPermissionRule, AHeldTabIsNotAskedEither) {
  EXPECT_EQ(TakesRequest(/*link_handler_offer=*/false, kHeld), Taken::kCancelled);
  EXPECT_EQ(TakesRequest(/*link_handler_offer=*/true, kHeld), Taken::kDropped);
}

}  // namespace
}  // namespace browser_core::agent_permission_rule
