#include "browser_core/crash_report_fields.h"

#include <algorithm>

#include "base/json/json_writer.h"
#include "base/values.h"

namespace browser_core::crash_reports {

namespace {

// The app's version, as the daily count takes it: digits and dots, so nothing
// else rides along.
bool IsVersion(std::string_view version) {
  return !version.empty() && version.size() <= 32 &&
         std::ranges::all_of(version, [](char c) { return (c >= '0' && c <= '9') || c == '.'; });
}

bool IsChannel(std::string_view channel) {
  return channel == "release" || channel == "nightly" || channel == "rework";
}

}  // namespace

std::string SentryFields(std::string_view release, std::string_view environment) {
  base::DictValue fields;
  fields.Set("sdk", base::DictValue()
                        .Set("name", "browser_core")
                        .Set("version", "1")
                        .Set("settings", base::DictValue().Set("infer_ip", "never")));
  if (IsVersion(release)) {
    fields.Set("release", release);
  }
  if (IsChannel(environment)) {
    fields.Set("environment", environment);
  }
  return base::WriteJson(fields).value_or("{}");
}

}  // namespace browser_core::crash_reports
