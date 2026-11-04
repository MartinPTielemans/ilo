#ifndef BROWSER_CORE_BATTERY_SAVER_H_
#define BROWSER_CORE_BATTERY_SAVER_H_

namespace browser_core {

// Before the performance manager makes its own battery sampler: with --battery-for-testing, puts
// in one whose battery the checks set (SetBatteryForTesting), so a run on mains, or on Linux,
// where Chromium reads no battery, can be below 20% on battery.
void InstallBatteryForTesting();

// Once the browser has started: tells the app as Chrome's Battery Saver (Energy Saver) turns on or
// off, and each tab as its page is frozen or thawed (bc_host's battery_saver_changed, tab_frozen).
void WatchBatterySaver();

// Whether Battery Saver is on now.
bool BatterySaverActive();

// Turns it off until the computer is plugged in again, as Chrome's "Turn off for now" does.
void TurnOffBatterySaverForNow();

// Whether it turns on below 20% on battery, Chrome's default, or never.
bool BatterySaverEnabled();
void SetBatterySaverEnabled(bool enabled);

// The battery the check's sampler reports from now: |percent| charged and whether it's
// |plugged| in. Does nothing without --battery-for-testing.
void SetBatteryForTesting(int percent, bool plugged);

}  // namespace browser_core

#endif  // BROWSER_CORE_BATTERY_SAVER_H_
