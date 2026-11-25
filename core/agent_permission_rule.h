// Which permissions a page in an agent's Space is refused, what the activity entry of a refusal
// carries, and what becomes of a question the browser would have put to the user for such a page,
// apart so browser_core_unittests tests the rules alone. The answer is where the page is, and for
// a copy whether the user holds the tab: no page and no agent's call says which.

#ifndef BROWSER_CORE_AGENT_PERMISSION_RULE_H_
#define BROWSER_CORE_AGENT_PERMISSION_RULE_H_

#include <bitset>
#include <cstddef>
#include <string_view>

#include "browser_core/agent_space_rule.h"

namespace browser_core::agent_permission_rule {

// A permission a page asks for, as the core names it.
enum class Kind {
  // The kinds whose activity entry has a sentence of its own in the app.
  kCamera,
  kCameraPanTiltZoom,
  kMicrophone,
  kLocation,
  kClipboardRead,
  kClipboardWrite,
  kScreenCapture,
  kNotifications,
  kFilePicker,
  // Every other kind refused, which the app says in one sentence.
  kMidi,
  kLocalFonts,
  kWindowManagement,
  kIdleDetection,
  kSensors,
  kLocalNetwork,
  kCapturedSurfaceControl,
  kSpeakerSelection,
  kKeyboardLock,
  kPointerLock,
  kAutomaticFullscreen,
  kVr,
  kAr,
  kHandTracking,
  kWebAppInstallation,
  kNfc,
  kSmartCard,
  kWebPrinting,
  // A handle on a file or a folder of the user's that the site was given before, which needs no
  // picker.
  kFileHandle,
  // The system's colour picker, which hands a page the colour of the screen where the user clicks.
  kColorPicker,
  // A print of the page, which draws a preview over it and the system's print sheet after.
  kPrint,
  // The chooser for a certificate of the user's, which a server asks for as a load starts.
  kClientCertificate,
  // The list of the user's devices to cast to, which a page asks for as much as the user does.
  kCast,
  // The question whether to add an extension, which is put on the user's window.
  kExtensionInstall,
  // The sheet that offers the user's accounts with another site to sign in with.
  kFederatedSignIn,
  // The system's sheet to share what a page hands it, with the user's apps and people in it.
  kShare,
  // The dialog that asks whether to install the page as an app of its own.
  kAppInstall,
  // A question of no kind above that the browser would have put to the user.
  kOther,
  // A kind that gives a page nothing of the user's device or person, such as a screen wake lock
  // or a frame's own storage access: answered as in any tab.
  kLeftAsToday,
};
inline constexpr size_t kKinds = static_cast<size_t>(Kind::kLeftAsToday) + 1;

// Whether the browser refuses |kind| to a page in |tab|, whatever the profile has saved for its
// site. Nothing in a tab of the user's and nothing left as today. In an agent's Space every other
// kind, held or not, but those the user's own doing needs at a step they do themselves: a write to
// the clipboard, for their copy, and a print and the choice of a certificate, which a held tab has
// as a tab of theirs does.
bool Refuses(Kind kind, const agent_space_rule::Tab& tab);

// A tab of the user's that an agent has (handed.h), as a frame in it asks for the camera or the
// microphone. The core reads each of these itself: no page and no agent's call sets one.
struct Handed {
  // An agent has it from the user. A page an agent opened itself is never one.
  bool handed = false;
  // The frame that asks and the tab's page are both on the origin the tab's page was on at the
  // give, to the scheme, the host and the port. A page the agent took the tab to elsewhere is as
  // one it opened, until the tab is back there, and another site's frame in the page is no part
  // of what the user gave.
  bool on_given_origin = false;
  // The user took the tab over by a press of their own. A hold made for them, at a handoff or for
  // a passkey, isn't one: an agent's own call makes a handoff.
  bool users_press = false;
  // The tab's page was using the camera, or the microphone, as the tab last passed into the
  // agent's hands: at the give, and each time the user handed it back.
  bool camera = false;
  bool microphone = false;
};

// Whether a page in |tab| is refused its ask for |kind|. As Refuses, but for the camera and the
// microphone in a handed tab the user took over by their own press, on the origin it was given
// on: there the ask is answered by what the profile has saved for the site, as in a tab of theirs,
// so the camera they turn on in a call they handed over comes on. They are asked nothing for it:
// the question of a site they never allowed is taken as any other (TakesRequest). Pan-tilt-zoom,
// the screen and every other kind are Refuses' in every state.
bool RefusesAsk(Kind kind, const agent_space_rule::Tab& tab, const Handed& handed);

// Whether a page in |tab| has its permission for |kind| read as refused, which is what a call
// under way is ended by when a setting of its site changes. As RefusesAsk, and read as saved too
// for a device the page was using as the tab last passed into the agent's hands, on the origin it
// was given on: a call the user handed over ends when they block its site, as in a tab of theirs,
// and at no other change. Nothing starts by this read: a page's ask is RefusesAsk's, whatever the
// status says.
bool RefusesStatus(Kind kind, const agent_space_rule::Tab& tab, const Handed& handed);

// The action of a refusal's activity entry is this and the kind's word, and its reason the one
// code for every kind. The app words the entry from the kind; neither is ever shown.
inline constexpr char kEntryAction[] = "request_permission ";
inline constexpr char kEntryReason[] = "agent_space";
const char* CodeWord(Kind kind);

// The reason of one entry apart: a question for the camera or the microphone was cancelled in a
// handed tab the user holds by their own press, on the origin it was given on. Their site is set
// to ask, and they are asked nothing for a page an agent has, so their way on is to take the tab
// back, which the app offers under this entry and no other. The app reads the code and shows it
// nowhere.
inline constexpr char kEntryReasonUsersHold[] = "agent_space_users_hold";

// The reason of the entry of a refusal of |kind|, which came by a question cancelled
// (|cancelled_question|, TakesRequest) or by a refusal before any question. The one code for
// every kind and state but that one.
const char* EntryReason(bool cancelled_question,
                        Kind kind,
                        const agent_space_rule::Tab& tab,
                        const Handed& handed);

// Whether a handle on a file or a folder made for a page in |tab| is refused. One in the site's
// own storage is no file of the user's and is the page's in any tab; any other is refused in an
// agent's Space, held or not, as the picker for one is.
bool RefusesFileHandle(bool site_storage, const agent_space_rule::Tab& tab);

// Whether a load for a page in |tab| that a server asks a certificate of is ended in place of the
// chooser. One with no certificate of the user's to offer draws no chooser and goes on as in any
// tab.
bool RefusesClientCertificate(bool any_to_offer, const agent_space_rule::Tab& tab);

// The kinds a document's refusals have an entry for: one per document and kind, however often the
// page asks. An entry with kEntryReasonUsersHold counts apart, so a page refused while the agent
// drove it still has that entry once the user takes the tab over.
class EntriesMade {
 public:
  // Whether this is the document's first refusal of |kind| with |reason|, EntryReason's, which
  // makes its entry.
  bool First(Kind kind, std::string_view reason = kEntryReason);

 private:
  std::bitset<kKinds> made_;
  std::bitset<kKinds> users_hold_made_;
};

// What becomes of a question the browser was about to put to the user for a page.
enum class Taken {
  // Asked as in any tab.
  kNo,
  // Ended as cancelled, before anything of it is shown or kept.
  kCancelled,
  // Left unanswered and dropped: a site's offer to handle a kind of link, whose page hears
  // nothing either way, as when the user ignores it.
  kDropped,
};

// A user's tab is asked as ever. A page in an agent's Space is never asked, held or not: the user
// answers no question for a page a thread holds.
Taken TakesRequest(bool link_handler_offer, const agent_space_rule::Tab& tab);

// What a page draws outside itself for a field of its own.
enum class PagePopup {
  // The system's menu for a select, where the system draws a select's list.
  kSystemMenu,
  // A window the page draws by itself: a select's list elsewhere, a date or time field's
  // calendar, a color field's picker.
  kWindow,
};

// A user's tab, and one of an agent's Space the user holds, opens them as ever. A page in an
// agent's Space the user doesn't hold opens neither, whoever asked: an agent's click, a key, or
// the page itself. Its select stays shut, and nothing is drawn over the user's screen for it.
bool RefusesPagePopup(PagePopup popup, const agent_space_rule::Tab& tab);

// A field whose list or picker is drawn outside the page.
enum class ShutField {
  kNone,
  // A select that shows one line and drops its list down.
  kSelect,
  // An input of a date or time type, with its calendar or clock.
  kDateOrTime,
  // An input of type color.
  kColor,
};

// An element as its markup has it: its tag's name as the engine gives it, in capitals for an HTML
// page's own, and the attributes that say which field it is, each "" if it has none.
struct Markup {
  std::string_view tag;
  std::string_view type;
  // Whether a select takes several options, which draws its list in the page.
  bool multiple = false;
  std::string_view size;
};

// What an agent's click is told it landed on, so that it doesn't spend steps on a list that stays
// shut: the field |element| is, on a page refused its list or picker (RefusesPagePopup). Nothing
// where the list would have opened, and nothing for a select that draws its list in the page. The
// answer is the element's kind as the markup has it, never a word of the page.
ShutField ShutFieldClicked(const Markup& element, const agent_space_rule::Tab& tab);

// The code word the click's answer names it by, "" for none.
std::string_view ShutWord(ShutField field);

// Why Chrome would ask the user where a download's file goes.
enum class DownloadQuestion {
  // The user's own command to save something under a name of their choosing.
  kUsersSaveAs,
  // The user's setting to be asked where each download goes.
  kUsersSetting,
  // The place it would go can't take it: not writable, full, or its name too long.
  kPlaceUnusable,
};

// What becomes of that question.
enum class DownloadAnswer {
  // Chrome's, asked as in any tab.
  kLeft,
  // The download goes on to the place Chrome suggested, with nothing asked.
  kGoesOn,
  // The download is cancelled.
  kCancelled,
};

// A user's tab, and one of an agent's Space the user holds, is asked as ever. For a page in an
// agent's Space the user doesn't hold nothing is asked: a download the user's setting alone would
// ask about goes where Chrome suggested, and one whose place can't take it is cancelled. The
// user's own command is theirs in any tab.
DownloadAnswer AnswersDownloadQuestion(DownloadQuestion question, const agent_space_rule::Tab& tab);

}  // namespace browser_core::agent_permission_rule

#endif  // BROWSER_CORE_AGENT_PERMISSION_RULE_H_
