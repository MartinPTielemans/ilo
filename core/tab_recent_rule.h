#ifndef BROWSER_CORE_TAB_RECENT_RULE_H_
#define BROWSER_CORE_TAB_RECENT_RULE_H_

#include <optional>
#include <string>
#include <string_view>

#include "browser_core/live_folder_rule.h"
#include "url/gurl.h"

// Which tabs' hover cards ask a site what's recent, and what each is answered (#3074), apart from
// the tabs and the network so browser_core_unittests tests it alone. The fetch is the Live
// Folder's (FetchSourceLists), so the site is asked at most once a live_folder::kFloor for a
// profile, whoever asks.
namespace browser_core::tab_recent {

// GitHub's pull requests, as the header's BC_LIVE_PULL_REQUESTS numbers them.
inline constexpr int kPullRequests = 1;

// A tab, as its card's ask reads it.
struct Asker {
  // Pinned in its Space, or a favorite.
  bool kept = false;
  // In a Space of the user's: not an agent's, nor a window's own, as a private window's is. A
  // favorite is in no Space, and only the user's show them.
  bool users = false;
  // The page it committed, which an unloaded tab is still at.
  GURL committed;
  // The directory of the profile its page is on while that's the user's own, loaded and its
  // Space's, else empty.
  std::string profile;
};

// The BC_LIVE_ source |tab|'s card asks, or 0 for a tab that asks nothing: one that isn't pinned or
// a favorite, isn't in a Space of the user's, or whose committed page isn't on a site with rows,
// which is https://github.com alone.
int SourceOf(const Asker& tab);

// What the card that asked |source| with the profile |asked| is answered from |kept|, for its tab
// as it is |now|, none once it's gone; or nullopt for no answer: the tab is gone or no longer asks
// that, as one unpinned or gone off the site doesn't, so nobody asked for this. On another
// profile, or one not loaded, it fails, as rows asked with one profile's session reach no tab on
// another.
std::optional<live_folder::Outcome> OutcomeFor(int source,
                                               std::string_view asked,
                                               const std::optional<Asker>& now,
                                               const live_folder::Kept& kept);

}  // namespace browser_core::tab_recent

#endif  // BROWSER_CORE_TAB_RECENT_RULE_H_
