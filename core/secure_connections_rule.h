// A profile's Always use secure connections and what becomes of an agent's page at Chrome's warning
// before http, apart so browser_core_unittests tests the rules alone. The warning's Continue to
// site is the user's: no page and no agent's call presses it, and a tab that isn't the user's to
// answer in never has the warning.

#ifndef BROWSER_CORE_SECURE_CONNECTIONS_RULE_H_
#define BROWSER_CORE_SECURE_CONNECTIONS_RULE_H_

#include <cstdint>

#include "browser_core/agent_space_rule.h"

namespace browser_core::secure_connections_rule {

// What a profile does with a page that answers only over http, as Chrome's setting has it.
enum class Mode {
  // Loads it over http without a word, after trying https.
  kOff,
  // Warns before it on a public site: what a profile that never chose has.
  kBalanced,
  // Warns before it everywhere but on this computer. Chrome's stricter choice, which no row of
  // ours sets and none takes away while it stays on.
  kStrict,
};

// Whether the profile always uses secure connections, as the setting's row shows it.
bool IsOn(Mode mode);

// The mode the profile has once the row is turned |on| or off from |now|. On from off is the
// balanced one; on while it warns already changes nothing, so the stricter choice stays; off is
// off from either.
Mode After(Mode now, bool on);

// Whether Chrome's warning is never shown for a page of |tab|: in an agent's Space unless the
// user holds the tab, where its Continue to site is theirs as in a tab of their own.
bool NoWarning(const agent_space_rule::Tab& tab);

// What becomes of a tab whose page was stopped so, for the page asked for never opened in it.
enum class PutBack {
  // It goes back to the page it had.
  kBack,
  // It had none, as a tab the agent just opened: it closes.
  kClosed,
};
PutBack PutBackTo(bool has_a_page_behind);

// The activity entry of a page stopped so, one for each: this action and this reason, both codes
// the app words and neither ever shown, and as its site (bc_agent_activity) the host of the page
// that never opened, since the tab is put back on another or closed.
inline constexpr char kEntryAction[] = "open_page";
inline constexpr char kEntryReason[] = "not_secure";

// The entry's action for a page the user went to and left at Chrome's warning when they handed
// the tab back: no step of the agent's asked for that page. The app ends no step on either
// entry, so the two differ only for who reads the log.
inline constexpr char kHandedBackAction[] = "handed_back_page";

// The navigation an agent's open or navigate started in a tab, for the job that waits on its
// page: the navigation's id, 0 for none, and whether the call was an open.
struct Asked {
  int64_t load = 0;
  bool opened = false;
};

// What the next job in the tab has of |kept|, which is left empty either way: all of it if the job
// |waits| for a page to settle, as the app asks right after its open or its navigate, and nothing
// if it is any other job. So a load is one job's at most, and no later job has an older load's.
Asked TakenBy(Asked& kept, bool waits);

// Whether the job that has |asked| is the one whose page the stopped navigation |taken| was to
// open: that job alone is told its page didn't open. A job that asked for no load never is, as a
// click whose page then went to http, nor is any job for a page left at a Hand Back, which was no
// job's (0).
bool AskedFor(const Asked& asked, int64_t taken);

// What that job is told, as the app says it of a tab that couldn't open or couldn't go.
const char* NotOpened(bool opened);

// The job running in a tab as the tab goes: whether it is a click whose press went to the page,
// and whether it is the wait the app asks right after its open or its navigate, with the load it
// has.
struct Running {
  bool clicked = false;
  bool waits = false;
  Asked asked;
};

// What a job is told as its tab goes.
enum class AtClose {
  // That the agent's session with the tab ended, as at any tab's end.
  kTabEnded,
  // That its click went and the tab then closed: a result, so the click isn't done again.
  kClickWent,
  // That the page its open opened went on by itself, with BC_AGENT_FAILURE_WENT_ON_NOT_SECURE.
  kWentOn,
};

// What |job| is told of a tab that is gone. Only where the core closed it itself, as the page it
// went on to was stopped with none behind it (|closed_as_not_secure|), and only the job |running|
// then, is told more than the tab's end: a click that went, since the agent would do it again, and
// an open's wait, since the page it waited for was there and left by itself. A job still queued
// never began; a click not yet sent didn't go; a navigate has the page it left behind it, so its
// tab is put back and never closed so. Every close but that one tells every job the tab's end.
AtClose ToldAtClose(bool closed_as_not_secure, bool running, const Running& job);

// kClickWent's answer: the object the header gives agent_page_done for it.
inline constexpr char kClosedAnswer[] = R"({"closed":"not_secure"})";

}  // namespace browser_core::secure_connections_rule

#endif  // BROWSER_CORE_SECURE_CONNECTIONS_RULE_H_
