// Which permissions a page in an agent's Space is refused, what the activity entry of a refusal
// carries, and what becomes of a question the browser would have put to the user for such a page,
// apart so browser_core_unittests tests the rules alone. The answer is where the page is, and for
// a copy whether the user holds the tab: no page and no agent's call says which.

#ifndef BROWSER_CORE_AGENT_PERMISSION_RULE_H_
#define BROWSER_CORE_AGENT_PERMISSION_RULE_H_

#include <bitset>
#include <cstddef>

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
  // A question of no kind above that the browser would have put to the user.
  kOther,
  // A kind that gives a page nothing of the user's device or person, such as a screen wake lock
  // or a frame's own storage access: answered as in any tab.
  kLeftAsToday,
};
inline constexpr size_t kKinds = static_cast<size_t>(Kind::kLeftAsToday) + 1;

// Whether the browser refuses |kind| to a page in |tab|, whatever the profile has saved for its
// site. Nothing in a tab of the user's and nothing left as today. In an agent's Space every other
// kind, held or not, but a write to the clipboard, which the user's own copy needs at a step they
// do themselves.
bool Refuses(Kind kind, const agent_space_rule::Tab& tab);

// The action of a refusal's activity entry is this and the kind's word, and its reason the one
// code for every kind. The app words the entry from the kind; neither is ever shown.
inline constexpr char kEntryAction[] = "request_permission ";
inline constexpr char kEntryReason[] = "agent_space";
const char* CodeWord(Kind kind);

// Whether a handle on a file or a folder made for a page in |tab| is refused. One in the site's
// own storage is no file of the user's and is the page's in any tab; any other is refused in an
// agent's Space, held or not, as the picker for one is.
bool RefusesFileHandle(bool site_storage, const agent_space_rule::Tab& tab);

// The kinds a document's refusals have an entry for: one per document and kind, however often the
// page asks.
class EntriesMade {
 public:
  // Whether this is the document's first refusal of |kind|, which makes its entry.
  bool First(Kind kind);

 private:
  std::bitset<kKinds> made_;
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

}  // namespace browser_core::agent_permission_rule

#endif  // BROWSER_CORE_AGENT_PERMISSION_RULE_H_
