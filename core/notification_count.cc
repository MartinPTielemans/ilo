#include "browser_core/notification_count.h"

namespace browser_core {

bool NotificationCount::Posted(const std::string& id) {
  if (!ids_.insert(id).second) {
    return false;
  }
  ++count_;
  return true;
}

bool NotificationCount::Shown() {
  if (count_ == 0) {
    return false;
  }
  count_ = 0;
  ids_.clear();
  return true;
}

}  // namespace browser_core
