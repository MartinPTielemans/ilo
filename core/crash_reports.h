#ifndef BROWSER_CORE_CRASH_REPORTS_H_
#define BROWSER_CORE_CRASH_REPORTS_H_

#include <string>

class Profile;

namespace browser_core::crash_reports {

// Crashpad writes a minidump of every crash, in the profile folder's Crashpad
// (engine/patches/ours/chromium-crashpad-start.patch), and its handler never sends one. The core
// sends them, only while the user has Send Crash Reports on (bc_set_send_crash_reports), to the
// collector the app names with --crash-upload-url: each report not yet sent, one at a time, once
// the browser has started and when the user turns it on, deleting each once the collector has it.
// One that doesn't go waits for the next launch: no timer retries it. Turning it off deletes every
// report not yet sent. While it's off, the launch after the browser crashed offers once to send
// that crash's report (bc_host's crash_report_offered).

// In the browser process, before Chromium starts Crashpad: its reports go in the profile folder the
// app gave, not Chromium's default one, which another Chromium on the machine shares.
void PlaceReports();

// As each profile starts: writes its Preferences now, with the mark Chrome's ExitTypeService has
// just set there that its session crashed, which Chrome would write only 10 s after the change. A
// crash sooner, as at startup, would otherwise leave the last session's mark, and the next launch
// wouldn't offer its report. Only with a collector, since the mark is only read for the offer.
void ProfileStarted(Profile* profile);

// In each process, once Chromium has started Crashpad there: its reports leave out the memory that
// pointers on the stack lead to, which can be a page's, and Chrome's dumps of processes that didn't
// crash, which record URLs. The page's URL that Chrome would record, the core's content client
// leaves out (main_delegate.cc).
void LeaveOutPages();

// In each process once Chrome has set its crash keys there, and in the browser process again once
// it has set them with about:flags' switches: the switches they list leave out those naming the
// user's folders, such as the profile's, whose path has the user's name.
void LeaveOutFolders();

// The collector's host, or empty with none, when nothing is sent and Settings doesn't offer to.
std::string CollectorHost();

// The id of the last report the collector took while Send Crash Reports was on, as 32 hex digits,
// which a Sentry collector files it under, or empty: before one was taken, and once the switch is
// turned off, which deletes it.
std::string LastReport();

// Whether the user has Send Crash Reports on. Turning it on sends the reports not yet sent, and off
// deletes them.
bool Sending();
void SetSending(bool send);

// Once the app has started: sends the reports not yet sent, if the user has said to, once the
// browser has finished starting, or else offers to send the report of a crash the time before.
// With --crash-after-start, crashes the browser instead, once Crashpad's handler is up and the
// profile's crashed mark is on disk, so the checks see it write its minidump and the next launch
// offer it (app/check/crash.sh). The prod app doesn't pass that switch on.
void Started();

// The user's answer to the offer: send that report, though Send Crash Reports is off, or delete it.
void Answer(bool send);

}  // namespace browser_core::crash_reports

#endif  // BROWSER_CORE_CRASH_REPORTS_H_
