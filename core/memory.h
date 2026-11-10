#ifndef BROWSER_CORE_MEMORY_H_
#define BROWSER_CORE_MEMORY_H_

#include <cstdint>
#include <string>

#include "browser_core/public/browser_core.h"

class Profile;

namespace content {
class WebContents;
}  // namespace content

namespace browser_core {

// Registers the profile pref that says whether the user turned the memory on (bc_memory_state).
void RegisterMemoryPrefs();

// Opens |profile|'s memory if the user turned it on, and forgets what its history forgets. Never
// for a private window's profile.
void SetUpMemory(Profile* profile);

// Watches |contents| for pages to keep: each page once, when it has been still kMemoryIdle since
// it loaded or behind another tab kMemoryBehind, so never on a tab switch, a new tab, the command
// bar's open or launch.
void WatchForMemory(content::WebContents* contents);

// See bc_memory_*.
int MemoryState(bc_id profile);
void SetMemoryState(bc_id profile, int state);
void SearchMemory(bc_id profile, uint64_t request, const std::string& query);
void ForgetMemorySite(bc_id profile, const std::string& site);
void ForgetMemory(bc_id profile);
void KeepOutOfMemory(bc_id profile, const std::string& site, bool out);
void ReadMemoryKeptOut(bc_id profile, uint64_t request);

}  // namespace browser_core

#endif  // BROWSER_CORE_MEMORY_H_
