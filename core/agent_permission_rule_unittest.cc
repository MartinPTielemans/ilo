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

// The user holding the tab changes three answers: their own copy writes the clipboard, and a print
// and the choice of a certificate are theirs to make.
TEST(AgentPermissionRule, AHeldTabIsRefusedAsOneNotHeldButForWhatTheUserDoesThemselves) {
  for (size_t at = 0; at < kKinds; ++at) {
    const Kind kind = KindAt(at);
    if (kind == Kind::kClipboardWrite || kind == Kind::kPrint || kind == Kind::kClientCertificate) {
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

TEST(AgentPermissionRule, AUsersTabHasTheColorPicker) {
  EXPECT_FALSE(Refuses(Kind::kColorPicker, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceHasNoColorPicker) {
  EXPECT_TRUE(Refuses(Kind::kColorPicker, kAgents));
}

// It reads the user's screen, not the page: the user holding the tab opens none for its page.
TEST(AgentPermissionRule, AHeldTabHasNoColorPickerEither) {
  EXPECT_TRUE(Refuses(Kind::kColorPicker, kHeld));
}

TEST(AgentPermissionRule, AUsersTabPrints) {
  EXPECT_FALSE(Refuses(Kind::kPrint, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceIsRefusedAPrint) {
  EXPECT_TRUE(Refuses(Kind::kPrint, kAgents));
}

// While the user holds the tab a print works as in a tab of theirs, their own command's and the
// page's.
TEST(AgentPermissionRule, AHeldTabPrints) {
  EXPECT_FALSE(Refuses(Kind::kPrint, kHeld));
}

TEST(AgentPermissionRule, AUsersTabIsShownTheCertificateChooser) {
  EXPECT_FALSE(RefusesClientCertificate(/*any_to_offer=*/true, kUsers));
}

TEST(AgentPermissionRule, ALoadInAnAgentsSpaceThatAsksACertificateIsEnded) {
  EXPECT_TRUE(RefusesClientCertificate(/*any_to_offer=*/true, kAgents));
}

// While the user holds the tab the choice is theirs, as in a tab of their own.
TEST(AgentPermissionRule, AHeldTabIsShownTheCertificateChooser) {
  EXPECT_FALSE(RefusesClientCertificate(/*any_to_offer=*/true, kHeld));
}

// No chooser is drawn where the user has none to offer: the load goes on in any tab.
TEST(AgentPermissionRule, ALoadWithNoCertificateToOfferGoesOnInAnyTab) {
  EXPECT_FALSE(RefusesClientCertificate(/*any_to_offer=*/false, kUsers));
  EXPECT_FALSE(RefusesClientCertificate(/*any_to_offer=*/false, kAgents));
  EXPECT_FALSE(RefusesClientCertificate(/*any_to_offer=*/false, kHeld));
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
