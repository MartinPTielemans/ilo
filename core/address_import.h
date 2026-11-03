#ifndef BROWSER_CORE_ADDRESS_IMPORT_H_
#define BROWSER_CORE_ADDRESS_IMPORT_H_

#include <stddef.h>

#include "browser_core/public/browser_core.h"

class Profile;

namespace browser_core {

// Adds another browser's saved |addresses| to |profile|'s once they've loaded, skipping each it
// has, then tells the app's addresses_added how many it added for |id|. False for a private
// profile or one whose user turned addresses off.
bool AddImportedAddresses(Profile* profile, bc_id id, const bc_address* addresses, size_t count);

}  // namespace browser_core

#endif  // BROWSER_CORE_ADDRESS_IMPORT_H_
