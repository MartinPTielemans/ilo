#ifndef BROWSER_CORE_CHECKED_H_
#define BROWSER_CORE_CHECKED_H_

namespace browser_core {

// Whether the checks run the app, under their driver (BROWSER_DRIVE) or check.sh (BROWSER_CHECK),
// read once. A line naming a site or an address the user goes to is written only then, as the
// app's log! lines are: Chromium's ERROR lines reach stderr in any build, and a desktop can keep
// an app's stderr (#3725).
bool Checked();

}  // namespace browser_core

#endif  // BROWSER_CORE_CHECKED_H_
