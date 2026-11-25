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

// The user holding the tab changes these answers: their own copy writes the clipboard, and a
// print, the choice of a certificate, of a device to cast to, of an account to sign in with and of
// where a share goes, and the answer to an extension's install are theirs to make.
TEST(AgentPermissionRule, AHeldTabIsRefusedAsOneNotHeldButForWhatTheUserDoesThemselves) {
  for (size_t at = 0; at < kKinds; ++at) {
    const Kind kind = KindAt(at);
    if (kind == Kind::kClipboardWrite || kind == Kind::kPrint || kind == Kind::kClientCertificate ||
        kind == Kind::kCast || kind == Kind::kExtensionInstall || kind == Kind::kFederatedSignIn ||
        kind == Kind::kShare || kind == Kind::kAppInstall) {
      EXPECT_FALSE(Refuses(kind, kHeld));
      continue;
    }
    EXPECT_EQ(Refuses(kind, kHeld), Refuses(kind, kAgents)) << at;
  }
}

// Every state a tab can be in when a frame of it asks: a user's, an agent's as the agent drives
// it and as the user holds it, handed by the user or opened by the agent, on the origin it was
// given on and off it, held by the user's own press or for them, and with a call that came with
// it or none.
template <typename Each>
void ForEveryState(Each each) {
  for (const Tab& tab : {kUsers, kAgents, kHeld}) {
    for (int bits = 0; bits < 32; ++bits) {
      each(tab, Handed{.handed = (bits & 1) != 0,
                       .on_given_origin = (bits & 2) != 0,
                       .users_press = (bits & 4) != 0,
                       .camera = (bits & 8) != 0,
                       .microphone = (bits & 16) != 0});
    }
  }
}

constexpr Handed kTakenOver{.handed = true, .on_given_origin = true, .users_press = true};

bool IsTheUsersOwnCall(Kind kind, const Tab& tab, const Handed& handed) {
  return (kind == Kind::kCamera || kind == Kind::kMicrophone) && tab.agents && tab.held &&
         handed.handed && handed.on_given_origin && handed.users_press;
}

TEST(AgentPermissionRule, AHandedTabTheUserTookOverAnswersItsCameraAndMicrophoneAsSaved) {
  EXPECT_FALSE(RefusesAsk(Kind::kCamera, kHeld, kTakenOver));
  EXPECT_FALSE(RefusesAsk(Kind::kMicrophone, kHeld, kTakenOver));
}

// Driven by the agent, held for the user at a handoff, off the origin it was given on, or a page
// the agent opened: each alone leaves the ask refused, for every kind.
TEST(AgentPermissionRule, AnAskIsRefusedAsEverInEveryStateButThatOne) {
  ForEveryState([](const Tab& tab, const Handed& handed) {
    for (size_t at = 0; at < kKinds; ++at) {
      const Kind kind = KindAt(at);
      if (IsTheUsersOwnCall(kind, tab, handed)) {
        EXPECT_FALSE(RefusesAsk(kind, tab, handed)) << at;
        continue;
      }
      EXPECT_EQ(RefusesAsk(kind, tab, handed), Refuses(kind, tab))
          << at << " " << tab.agents << tab.held << handed.handed << handed.on_given_origin
          << handed.users_press;
    }
  });
}

TEST(AgentPermissionRule, AHandedTabTheAgentDrivesIsRefusedTheCameraAndTheMicrophone) {
  const Handed driven{.handed = true, .on_given_origin = true};
  EXPECT_TRUE(RefusesAsk(Kind::kCamera, kAgents, driven));
  EXPECT_TRUE(RefusesAsk(Kind::kMicrophone, kAgents, driven));
}

// An agent's own call makes a handoff, so the hold it makes opens nothing.
TEST(AgentPermissionRule, AHandedTabHeldAtAHandoffIsRefusedTheCameraAndTheMicrophone) {
  const Handed handed_off{.handed = true, .on_given_origin = true};
  EXPECT_TRUE(RefusesAsk(Kind::kCamera, kHeld, handed_off));
  EXPECT_TRUE(RefusesAsk(Kind::kMicrophone, kHeld, handed_off));
}

TEST(AgentPermissionRule, AHandedTabOffTheOriginItWasGivenOnIsRefusedThemTakenOverOrNot) {
  const Handed elsewhere{.handed = true, .users_press = true};
  EXPECT_TRUE(RefusesAsk(Kind::kCamera, kHeld, elsewhere));
  EXPECT_TRUE(RefusesAsk(Kind::kMicrophone, kHeld, elsewhere));
}

TEST(AgentPermissionRule, APageTheAgentOpenedIsRefusedThemTakenOverOrNot) {
  const Handed opened{.on_given_origin = true, .users_press = true};
  EXPECT_TRUE(RefusesAsk(Kind::kCamera, kHeld, opened));
  EXPECT_TRUE(RefusesAsk(Kind::kMicrophone, kHeld, opened));
  EXPECT_TRUE(RefusesAsk(Kind::kCamera, kHeld, {}));
  EXPECT_TRUE(RefusesAsk(Kind::kMicrophone, kHeld, {}));
}

// The camera's own movement, the screen and every other kind stay refused where the camera and
// the microphone are answered.
TEST(AgentPermissionRule, NoOtherKindIsAnsweredAsSavedInATabTheUserTookOver) {
  for (size_t at = 0; at < kKinds; ++at) {
    const Kind kind = KindAt(at);
    if (kind == Kind::kCamera || kind == Kind::kMicrophone) {
      continue;
    }
    EXPECT_EQ(RefusesAsk(kind, kHeld, kTakenOver), Refuses(kind, kHeld)) << at;
  }
  EXPECT_TRUE(RefusesAsk(Kind::kCameraPanTiltZoom, kHeld, kTakenOver));
  EXPECT_TRUE(RefusesAsk(Kind::kScreenCapture, kHeld, kTakenOver));
}

constexpr Handed kInACall{.handed = true, .on_given_origin = true, .camera = true,
                          .microphone = true};

bool IsTheCallHandedOver(Kind kind, const Tab& tab, const Handed& handed) {
  return tab.agents && handed.handed && handed.on_given_origin &&
         ((kind == Kind::kCamera && handed.camera) ||
          (kind == Kind::kMicrophone && handed.microphone));
}

// While the agent drives the tab and while the user holds it.
TEST(AgentPermissionRule, ACallHandedOverHasItsCameraAndMicrophoneReadAsSaved) {
  for (const Tab& tab : {kAgents, kHeld}) {
    EXPECT_FALSE(RefusesStatus(Kind::kCamera, tab, kInACall));
    EXPECT_FALSE(RefusesStatus(Kind::kMicrophone, tab, kInACall));
  }
}

TEST(AgentPermissionRule, ADeviceTheCallDidntUseReadsAsRefused) {
  const Handed camera_only{.handed = true, .on_given_origin = true, .camera = true};
  EXPECT_FALSE(RefusesStatus(Kind::kCamera, kAgents, camera_only));
  EXPECT_TRUE(RefusesStatus(Kind::kMicrophone, kAgents, camera_only));
  const Handed microphone_only{.handed = true, .on_given_origin = true, .microphone = true};
  EXPECT_TRUE(RefusesStatus(Kind::kCamera, kAgents, microphone_only));
  EXPECT_FALSE(RefusesStatus(Kind::kMicrophone, kAgents, microphone_only));
}

TEST(AgentPermissionRule, ACallReadsAsRefusedOffTheOriginItWasGivenOn) {
  const Handed elsewhere{.handed = true, .camera = true, .microphone = true};
  EXPECT_TRUE(RefusesStatus(Kind::kCamera, kAgents, elsewhere));
  EXPECT_TRUE(RefusesStatus(Kind::kMicrophone, kHeld, elsewhere));
}

TEST(AgentPermissionRule, APageTheAgentOpenedReadsAsRefused) {
  const Handed opened{.on_given_origin = true, .camera = true, .microphone = true};
  EXPECT_TRUE(RefusesStatus(Kind::kCamera, kAgents, opened));
  EXPECT_TRUE(RefusesStatus(Kind::kMicrophone, kHeld, opened));
}

// The status is read as saved in two states and no other, for the camera and the microphone and
// no other kind.
TEST(AgentPermissionRule, TheStatusReadsAsRefusedAsEverInEveryOtherState) {
  ForEveryState([](const Tab& tab, const Handed& handed) {
    for (size_t at = 0; at < kKinds; ++at) {
      const Kind kind = KindAt(at);
      if (IsTheUsersOwnCall(kind, tab, handed) || IsTheCallHandedOver(kind, tab, handed)) {
        EXPECT_FALSE(RefusesStatus(kind, tab, handed)) << at;
        continue;
      }
      EXPECT_EQ(RefusesStatus(kind, tab, handed), Refuses(kind, tab))
          << at << " " << tab.agents << tab.held << handed.handed << handed.on_given_origin
          << handed.users_press << handed.camera << handed.microphone;
    }
  });
}

// What a call came with changes the status alone: a page's ask is refused wherever it was, with
// the call or without, so nothing starts in the agent's hands.
TEST(AgentPermissionRule, ACallHandedOverOpensNoAsk) {
  ForEveryState([](const Tab& tab, const Handed& handed) {
    Handed without = handed;
    without.camera = false;
    without.microphone = false;
    for (size_t at = 0; at < kKinds; ++at) {
      EXPECT_EQ(RefusesAsk(KindAt(at), tab, handed), RefusesAsk(KindAt(at), tab, without)) << at;
    }
  });
  EXPECT_TRUE(RefusesAsk(Kind::kCamera, kAgents, kInACall));
  EXPECT_TRUE(RefusesAsk(Kind::kMicrophone, kAgents, kInACall));
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

TEST(AgentPermissionRule, AUsersTabIsShownTheDevicesToCastTo) {
  EXPECT_FALSE(Refuses(Kind::kCast, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceIsShownNoDevicesToCastTo) {
  EXPECT_TRUE(Refuses(Kind::kCast, kAgents));
  EXPECT_STREQ(CodeWord(Kind::kCast), "cast");
}

// While the user holds the tab its page casts as one of theirs, by their command or its own ask.
TEST(AgentPermissionRule, AHeldTabIsShownTheDevicesToCastTo) {
  EXPECT_FALSE(Refuses(Kind::kCast, kHeld));
}

TEST(AgentPermissionRule, AUsersTabAsksWhetherToAddAnExtension) {
  EXPECT_FALSE(Refuses(Kind::kExtensionInstall, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceAsksNothingOfAnExtensionsInstall) {
  EXPECT_TRUE(Refuses(Kind::kExtensionInstall, kAgents));
  EXPECT_STREQ(CodeWord(Kind::kExtensionInstall), "extension_install");
}

// While the user holds the tab the question is theirs, as in a tab of their own.
TEST(AgentPermissionRule, AHeldTabAsksWhetherToAddAnExtension) {
  EXPECT_FALSE(Refuses(Kind::kExtensionInstall, kHeld));
}

TEST(AgentPermissionRule, AUsersTabIsOfferedTheUsersAccountsToSignInWith) {
  EXPECT_FALSE(Refuses(Kind::kFederatedSignIn, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceIsOfferedNoAccountToSignInWith) {
  EXPECT_TRUE(Refuses(Kind::kFederatedSignIn, kAgents));
  EXPECT_STREQ(CodeWord(Kind::kFederatedSignIn), "federated_sign_in");
}

// While the user holds the tab the choice of an account is theirs, as in a tab of their own.
TEST(AgentPermissionRule, AHeldTabIsOfferedTheUsersAccountsToSignInWith) {
  EXPECT_FALSE(Refuses(Kind::kFederatedSignIn, kHeld));
}

TEST(AgentPermissionRule, AUsersTabShares) {
  EXPECT_FALSE(Refuses(Kind::kShare, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceIsShownNoSheetToShareWith) {
  EXPECT_TRUE(Refuses(Kind::kShare, kAgents));
  EXPECT_STREQ(CodeWord(Kind::kShare), "share");
}

// While the user holds the tab a share is theirs to send, as in a tab of their own.
TEST(AgentPermissionRule, AHeldTabShares) {
  EXPECT_FALSE(Refuses(Kind::kShare, kHeld));
}

TEST(AgentPermissionRule, AUsersTabIsAskedWhetherToInstallAnApp) {
  EXPECT_FALSE(Refuses(Kind::kAppInstall, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceIsShownNoDialogToInstallAnApp) {
  EXPECT_TRUE(Refuses(Kind::kAppInstall, kAgents));
  EXPECT_STREQ(CodeWord(Kind::kAppInstall), "app_install");
}

// While the user holds the tab the install is theirs to answer, as in a tab of their own.
TEST(AgentPermissionRule, AHeldTabIsAskedWhetherToInstallAnApp) {
  EXPECT_FALSE(Refuses(Kind::kAppInstall, kHeld));
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

TEST(AgentPermissionRule, AUsersTabOpensASelectsSystemMenu) {
  EXPECT_FALSE(RefusesPagePopup(PagePopup::kSystemMenu, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceOpensNoSystemMenu) {
  EXPECT_TRUE(RefusesPagePopup(PagePopup::kSystemMenu, kAgents));
}

TEST(AgentPermissionRule, AHeldTabOpensASelectsSystemMenu) {
  EXPECT_FALSE(RefusesPagePopup(PagePopup::kSystemMenu, kHeld));
}

TEST(AgentPermissionRule, AUsersTabOpensAPagesPopupWindow) {
  EXPECT_FALSE(RefusesPagePopup(PagePopup::kWindow, kUsers));
}

TEST(AgentPermissionRule, APageInAnAgentsSpaceOpensNoPopupWindow) {
  EXPECT_TRUE(RefusesPagePopup(PagePopup::kWindow, kAgents));
}

TEST(AgentPermissionRule, AHeldTabOpensAPagesPopupWindow) {
  EXPECT_FALSE(RefusesPagePopup(PagePopup::kWindow, kHeld));
}

constexpr Markup kSelect{.tag = "SELECT"};
constexpr Markup kDate{.tag = "INPUT", .type = "date"};
constexpr Markup kColor{.tag = "INPUT", .type = "color"};

TEST(AgentPermissionRule, AnAgentsClickIsToldASelectStaysShut) {
  EXPECT_EQ(ShutFieldClicked(kSelect, kAgents), ShutField::kSelect);
  EXPECT_EQ(ShutFieldClicked({.tag = "SELECT", .size = "1"}, kAgents), ShutField::kSelect);
  EXPECT_EQ(ShutFieldClicked({.tag = "SELECT", .size = "none"}, kAgents), ShutField::kSelect);
}

TEST(AgentPermissionRule, AnAgentsClickIsToldADateOrTimeFieldOpensNoCalendar) {
  for (std::string_view type : {"date", "datetime-local", "month", "week", "time", "DATE"}) {
    EXPECT_EQ(ShutFieldClicked({.tag = "INPUT", .type = type}, kAgents), ShutField::kDateOrTime)
        << type;
  }
}

TEST(AgentPermissionRule, AnAgentsClickIsToldAColorFieldOpensNoPicker) {
  EXPECT_EQ(ShutFieldClicked(kColor, kAgents), ShutField::kColor);
  EXPECT_EQ(ShutFieldClicked({.tag = "INPUT", .type = "Color"}, kAgents), ShutField::kColor);
}

TEST(AgentPermissionRule, ASelectThatDrawsItsListInThePageGetsNoWord) {
  EXPECT_EQ(ShutFieldClicked({.tag = "SELECT", .multiple = true}, kAgents), ShutField::kNone);
  for (std::string_view size : {"2", "10", " +4", "02", "3 lines"}) {
    EXPECT_EQ(ShutFieldClicked({.tag = "SELECT", .size = size}, kAgents), ShutField::kNone) << size;
  }
  for (std::string_view size : {"", "0", "1", "01", "-3", "+1"}) {
    EXPECT_EQ(ShutFieldClicked({.tag = "SELECT", .size = size}, kAgents), ShutField::kSelect)
        << size;
  }
}

TEST(AgentPermissionRule, AnyOtherElementGetsNoWord) {
  for (std::string_view type : {"", "text", "datetime", "range", "file", "dates", "colour"}) {
    EXPECT_EQ(ShutFieldClicked({.tag = "INPUT", .type = type}, kAgents), ShutField::kNone) << type;
  }
  // Only the engine's own name for the element counts, never an attribute a page gave another.
  EXPECT_EQ(ShutFieldClicked({.tag = "DIV", .type = "date"}, kAgents), ShutField::kNone);
  EXPECT_EQ(ShutFieldClicked({.tag = "BUTTON", .type = "color"}, kAgents), ShutField::kNone);
  EXPECT_EQ(ShutFieldClicked({.tag = "select"}, kAgents), ShutField::kNone);
  EXPECT_EQ(ShutFieldClicked({.tag = ""}, kAgents), ShutField::kNone);
}

TEST(AgentPermissionRule, AHeldTabsClickGetsNoWord) {
  for (const Markup& element : {kSelect, kDate, kColor}) {
    EXPECT_EQ(ShutFieldClicked(element, kHeld), ShutField::kNone) << element.type;
  }
}

TEST(AgentPermissionRule, AClickInAUsersTabGetsNoWord) {
  for (const Markup& element : {kSelect, kDate, kColor}) {
    EXPECT_EQ(ShutFieldClicked(element, kUsers), ShutField::kNone) << element.type;
  }
}

TEST(AgentPermissionRule, EachShutFieldHasACodeWordOfItsOwn) {
  EXPECT_EQ(ShutWord(ShutField::kNone), "");
  EXPECT_EQ(ShutWord(ShutField::kSelect), "select");
  EXPECT_EQ(ShutWord(ShutField::kDateOrTime), "date_time");
  EXPECT_EQ(ShutWord(ShutField::kColor), "color");
}

constexpr DownloadQuestion kDownloadQuestions[] = {
    DownloadQuestion::kUsersSaveAs,
    DownloadQuestion::kUsersSetting,
    DownloadQuestion::kPlaceUnusable,
};

TEST(AgentPermissionRule, AUsersTabIsAskedWhereADownloadGoes) {
  for (DownloadQuestion question : kDownloadQuestions) {
    EXPECT_EQ(AnswersDownloadQuestion(question, kUsers), DownloadAnswer::kLeft);
  }
}

TEST(AgentPermissionRule, ADownloadInAnAgentsSpaceGoesOnWhereTheUsersSettingWouldAsk) {
  EXPECT_EQ(AnswersDownloadQuestion(DownloadQuestion::kUsersSetting, kAgents),
            DownloadAnswer::kGoesOn);
}

TEST(AgentPermissionRule, ADownloadInAnAgentsSpaceIsCancelledWhereItsPlaceCantTakeIt) {
  EXPECT_EQ(AnswersDownloadQuestion(DownloadQuestion::kPlaceUnusable, kAgents),
            DownloadAnswer::kCancelled);
}

TEST(AgentPermissionRule, TheUsersOwnSaveAsIsTheirsInAnAgentsSpace) {
  EXPECT_EQ(AnswersDownloadQuestion(DownloadQuestion::kUsersSaveAs, kAgents),
            DownloadAnswer::kLeft);
}

TEST(AgentPermissionRule, AHeldTabIsAskedWhereADownloadGoes) {
  for (DownloadQuestion question : kDownloadQuestions) {
    EXPECT_EQ(AnswersDownloadQuestion(question, kHeld), DownloadAnswer::kLeft);
  }
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

// The question reads nothing of a handed tab, so the state whose ask is answered as saved has its
// question taken as every other: the user is asked nothing for a page an agent has.
TEST(AgentPermissionRule, NoStateOfATabAnAgentHasLetsAQuestionBeDrawn) {
  ForEveryState([](const Tab& tab, const Handed&) {
    for (bool offer : {false, true}) {
      EXPECT_EQ(TakesRequest(offer, tab) == Taken::kNo, !tab.agents) << tab.agents << tab.held;
    }
  });
  EXPECT_EQ(TakesRequest(/*link_handler_offer=*/false, kHeld), Taken::kCancelled);
}

}  // namespace
}  // namespace browser_core::agent_permission_rule
