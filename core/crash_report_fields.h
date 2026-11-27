// The fields a Sentry collector takes beside a crash report's minidump, apart
// so browser_core_unittests tests them alone.

#ifndef BROWSER_CORE_CRASH_REPORT_FIELDS_H_
#define BROWSER_CORE_CRASH_REPORT_FIELDS_H_

#include <string>
#include <string_view>

namespace browser_core::crash_reports {

// Sentry's fields, as JSON: keep no IP address with the report, and the build's
// |release|, its version, and its |environment|, the channel (release, nightly
// or rework), so one channel's crashes never mix with another's. Either that
// isn't a version or a channel is left out.
std::string SentryFields(std::string_view release, std::string_view environment);

}  // namespace browser_core::crash_reports

#endif  // BROWSER_CORE_CRASH_REPORT_FIELDS_H_
