// What a page in an agent's Space is refused, and which Space lists a tab a page or the browser
// opened, apart so browser_core_unittests tests the rules alone. The answer is where the page is
// and nothing else: no page and no agent's call says which.

#ifndef BROWSER_CORE_AGENT_SPACE_RULE_H_
#define BROWSER_CORE_AGENT_SPACE_RULE_H_

#include <stddef.h>
#include <stdint.h>

#include <string>
#include <string_view>
#include <vector>

#include "base/functional/callback.h"

namespace browser_core::agent_space_rule {

// The tab a page is in when it asks.
struct Tab {
  // In an agent's Space: a page of the agent's own, or a tab of the user's handed to it.
  bool agents = false;
  // The user holds it from the agent, as at a handoff or once they take it over.
  bool held = false;
};

// What a page asks of the browser that is the user's to give.
enum class Ask {
  // The clipboard's content, by a paste.
  kPaste,
  // A write to the clipboard, by a copy.
  kCopy,
  // The picker for a file or a folder of the user's.
  kFilePicker,
  // A USB, HID, serial or Bluetooth device the user gave the page's site.
  kDevice,
  // A link of the page's opened in a preview over the user's window: by a click that leaves a
  // pinned tab's site, a click with ⇧, or the row of the page's menu.
  kLinkPreview,
  // Picture in Picture: a video's or a document's window floating on the user's screen, and the
  // video at the app's media card.
  kPictureInPicture,
};

// Whether the browser refuses |ask| from a page in |tab|. Nothing in a tab of the user's. In an
// agent's Space the picker, a device and Picture in Picture always, and a paste, a copy and a link
// preview unless the user holds the tab, so their own paste, copy and click work at a step they do
// themselves.
bool Refuses(Ask ask, const Tab& tab);

// Whether the page's menu has its row that opens a link in a preview, for a link that could open
// in one (|web_link|): never where the click itself would open none.
bool MenuHasLinkPreviewRow(bool web_link, const Tab& tab);

// A tab a page or the browser opened, as its window comes to list it.
struct Opened {
  // It was made for a Space, which every tab a page in an agent's Space opens is for that Space
  // from its making, and that Space is still there.
  bool made_for_a_space = false;
  bool its_space_is_there = false;
  // Its opener is in a Space of the user's now.
  bool opener_in_users_space = false;
  // The window it opened in shows a Space of the user's.
  bool window_shows_users_space = false;
  // Its profile has a Space of the user's.
  bool profile_has_users_space = false;
};

// Where its window lists it.
enum class Listed {
  kInTheSpaceItWasMadeFor,
  kInItsOpenersSpace,
  kInTheSpaceShown,
  // The first Space of the user's in its profile.
  kInTheUsersFirstSpace,
  // Nowhere: it's closed.
  kClosed,
};

// A tab made for a Space is listed there, or closed once that Space is gone: where its opener is by
// then says nothing, so a tab an agent's page opened is never listed in a Space of the user's. Any
// other is listed in a Space of the user's, its opener's first, then the one the window shows, then
// its profile's first, and never in an agent's, whichever the window shows and wherever its opener
// has gone: closed if its profile has none.
Listed ListedIn(const Opened& opened);

// A Space, as a tab with nowhere else to be listed finds it.
struct Candidate {
  bool agents = false;
  // A window's own, as a Little window's or a Blank Window's: only that window's tabs are its.
  bool a_windows_own = false;
  bool of_the_tabs_profile = false;
};

// Whether such a tab may be listed in it: a Space of the user's, of the tab's own profile, that is
// no window's own. The first that may is the user's first Space (Listed::kInTheUsersFirstSpace).
bool TakesATabWithNowhereElse(const Candidate& space);

// An agent's Space as the core looks for the one a tab opens in: its agent's id and its profile's
// folder, in the order the Spaces were made.
struct AgentSpace {
  std::string_view agent;
  std::string_view profile;
};

// Which of |spaces| is the agent |agent|'s in the profile whose folder is |profile|, or -1 for
// none. An agent has one Space a profile, the first made there. An empty |profile| asks for the
// agent's Space as before profiles were named: the last made, wherever it is (#4376).
int AgentSpaceIn(const std::vector<AgentSpace>& spaces,
                 std::string_view agent,
                 std::string_view profile);

// Whether a tab may open in a Space of the profile whose folder is |profile| when Chrome gives the
// profile whose folder is |given|: only that one. Chrome gives the first profile for one that
// hasn't loaded, and a page opened there would use another profile's sign-ins.
bool OpensIn(std::string_view profile, std::string_view given);

// A profile as the pick for an agent's tab weighs it (#4376, bc_profile_pick_for_site).
struct PickCandidate {
  uint64_t profile = 0;
  // Its place among the profiles the app gave, which breaks a tie.
  size_t order = 0;
  // Read only where its history has loaded, and never a private session's, which keeps none.
  bool loaded = false;
  bool private_session = false;
  // Its last visit to the URL's site, microseconds since 1970, or 0 for none.
  int64_t last_visit = 0;
};

// The profile whose history visited the site last, of those loaded and not private, the first
// given on a tie; or 0 for none.
uint64_t Picked(const std::vector<PickCandidate>& candidates);

// The hosts a pick for a page at |host| reads each profile's history for, by the history's own
// index of addresses rather than every row: the page's host, its site, and the site's www host,
// each once.
std::vector<std::string> PickHosts(std::string_view host, std::string_view site);

// One profile's answer to a pick, which comes exactly once: the newest visit it read, in
// microseconds, or 0 if it's dropped unanswered, as a read is when its profile's history shuts
// down first. So a pick always ends, whatever becomes of each profile's read.
class PickAnswer {
 public:
  explicit PickAnswer(base::OnceCallback<void(int64_t)> answer);
  ~PickAnswer();
  PickAnswer(const PickAnswer&) = delete;
  PickAnswer& operator=(const PickAnswer&) = delete;

  void Answer(int64_t last_visit);

 private:
  base::OnceCallback<void(int64_t)> answer_;
};

}  // namespace browser_core::agent_space_rule

#endif  // BROWSER_CORE_AGENT_SPACE_RULE_H_
