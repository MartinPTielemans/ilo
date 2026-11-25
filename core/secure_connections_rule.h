// A profile's Always use secure connections and what becomes of an agent's page at Chrome's warning
// before http, apart so browser_core_unittests tests the rules alone. The warning's Continue to
// site is the user's: no page and no agent's call presses it, and a tab that isn't the user's to
// answer in never has the warning.

#ifndef BROWSER_CORE_SECURE_CONNECTIONS_RULE_H_
#define BROWSER_CORE_SECURE_CONNECTIONS_RULE_H_

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

}  // namespace browser_core::secure_connections_rule

#endif  // BROWSER_CORE_SECURE_CONNECTIONS_RULE_H_
