#include "browser_core/tab_recent_rule.h"

#include "url/origin.h"

namespace browser_core::tab_recent {

int SourceOf(const Asker& tab) {
  // The site's own origin: not another scheme, port or host under it, such as gist.github.com.
  const bool on_site =
      tab.committed.is_valid() &&
      url::Origin::Create(tab.committed).IsSameOriginWith(GURL(live_folder::kSite));
  return tab.kept && tab.users && on_site ? kPullRequests : 0;
}

std::optional<live_folder::Outcome> OutcomeFor(int source,
                                               std::string_view asked,
                                               const std::optional<Asker>& now,
                                               const live_folder::Kept& kept) {
  if (!source || !now || SourceOf(*now) != source) {
    return std::nullopt;
  }
  return live_folder::OutcomeOf(kept.answers, asked, now->profile);
}

}  // namespace browser_core::tab_recent
