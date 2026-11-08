#include "browser_core/other_tabs.h"

#include "base/strings/utf_string_conversions.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "components/sessions/core/session_types.h"
#include "url/gurl.h"

namespace browser_core {

std::vector<OtherTab> OtherTabsOf(const std::vector<OtherComputer>& computers) {
  std::vector<OtherTab> tabs;
  for (const OtherComputer& computer : computers) {
    for (const sessions::SessionTab* tab : computer.tabs) {
      if (!tab || tab->navigations.empty()) {
        continue;
      }
      const sessions::SerializedNavigationEntry& page =
          tab->navigations[tab->normalized_navigation_index()];
      const GURL& url = page.virtual_url();
      if (!url.is_valid() || !url.SchemeIsHTTPOrHTTPS()) {
        continue;
      }
      tabs.push_back({.url = url.spec(),
                      .title = base::UTF16ToUTF8(page.title()),
                      .computer = computer.name,
                      .os = computer.os,
                      .seen = computer.seen});
    }
  }
  return tabs;
}

}  // namespace browser_core
