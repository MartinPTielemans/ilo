#include "browser_core/checked.h"

#include <memory>

#include "base/environment.h"

namespace browser_core {

bool Checked() {
  static const bool checked = [] {
    std::unique_ptr<base::Environment> env = base::Environment::Create();
    return env->HasVar("BROWSER_DRIVE") || env->HasVar("BROWSER_CHECK");
  }();
  return checked;
}

}  // namespace browser_core
