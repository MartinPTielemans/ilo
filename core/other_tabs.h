#ifndef BROWSER_CORE_OTHER_TABS_H_
#define BROWSER_CORE_OTHER_TABS_H_

#include <string>
#include <vector>

#include "base/time/time.h"

namespace sessions {
struct SessionTab;
}

namespace browser_core {

// The tabs open on the account's other computers, as the app hears of them (bc_other_tab, #1234).

// One of the account's other computers, as sync last brought it: its name and system as Settings ›
// Sync lists them, when it last changed its tabs, and its tabs, the most recently used first.
struct OtherComputer {
  std::string name;
  std::string os;
  base::Time seen;
  std::vector<const sessions::SessionTab*> tabs;
};

struct OtherTab {
  std::string url;
  std::string title;
  std::string computer;
  std::string os;
  base::Time seen;
  bool operator==(const OtherTab&) const = default;
};

// The tabs of |computers|, which come most recently seen first, in their order: each tab's page
// as it shows it now, http and https only, as only those open here.
std::vector<OtherTab> OtherTabsOf(const std::vector<OtherComputer>& computers);

}  // namespace browser_core

#endif  // BROWSER_CORE_OTHER_TABS_H_
