#include "browser_core/boost_colors.h"

#include "base/strings/stringprintf.h"

namespace browser_core {

std::string BoostSheet(const bc_boost_colors* colors, std::string_view css) {
  // A search of the Boost's own CSS, which the core already holds, on each frame it's shown in.
  if (!colors || css.find("--ilo-space-") == std::string_view::npos) {
    return std::string(css);
  }
  return base::StringPrintf(
             ":root{--ilo-space-background: #%06x; --ilo-space-text: #%06x; "
             "--ilo-space-accent: #%06x; --ilo-space-accent-text: #%06x}\n",
             colors->background & 0xffffff, colors->text & 0xffffff, colors->accent & 0xffffff,
             colors->accent_text & 0xffffff) +
         std::string(css);
}

}  // namespace browser_core
