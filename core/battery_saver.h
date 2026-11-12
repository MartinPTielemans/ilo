#ifndef BROWSER_CORE_BATTERY_SAVER_H_
#define BROWSER_CORE_BATTERY_SAVER_H_

namespace browser_core {

// Before the performance manager makes its own battery sampler: puts in one that reads the battery
// as the OS tells of its changes (platform/battery.h), where Chromium has none (Linux) or reads it
// on a timer (Windows). With --battery-for-testing, one whose battery the checks set instead
// (SetBatteryForTesting), so a run on mains can be below 20% on battery.
void InstallBatterySampler();

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
