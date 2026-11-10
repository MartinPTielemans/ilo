#ifndef BROWSER_CORE_MEMORY_KEEP_OUT_H_
#define BROWSER_CORE_MEMORY_KEEP_OUT_H_

#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "browser_core/memory_index.h"

namespace browser_core {

// Whether a site keeps itself out of the memory, by what it says of itself rather than by a list
// of sites anyone keeps: the schema.org types its page declares (JSON-LD, which Blink reads for
// the browser without running the page), and a .bank or .insurance domain, whose registries check
// who holds them. |types| are the page's entities' @type values, nested ones too. A bank or health
// site that says neither keeps out only the pages it sends with Cache-Control: no-store, each by
// itself, which memory.cc reads from the response.
std::optional<KeptOut> MemorySiteSays(std::string_view site, const std::vector<std::string>& types);

}  // namespace browser_core

#endif  // BROWSER_CORE_MEMORY_KEEP_OUT_H_
