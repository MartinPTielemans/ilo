#include "browser_core/crash_report_fields.h"

#include <optional>
#include <string>
#include <string_view>
#include <utility>

#include "base/json/json_reader.h"
#include "base/values.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::crash_reports {
namespace {

base::DictValue Fields(std::string_view release, std::string_view environment) {
  std::optional<base::DictValue> fields =
      base::JSONReader::ReadDict(SentryFields(release, environment), base::JSON_PARSE_RFC);
  EXPECT_TRUE(fields);
  return fields ? std::move(*fields) : base::DictValue();
}

TEST(CrashReportFields, NameTheReleaseAndChannel) {
  for (const char* channel : {"release", "nightly", "rework"}) {
    base::DictValue fields = Fields("0.3.1", channel);
    ASSERT_TRUE(fields.FindString("release"));
    EXPECT_EQ(*fields.FindString("release"), "0.3.1");
    ASSERT_TRUE(fields.FindString("environment"));
    EXPECT_EQ(*fields.FindString("environment"), channel);
  }
}

TEST(CrashReportFields, KeepNoAddress) {
  // The dictionary outlives the pointer into it.
  base::DictValue fields = Fields("0.3.1", "rework");
  const std::string* infer_ip = fields.FindStringByDottedPath("sdk.settings.infer_ip");
  ASSERT_TRUE(infer_ip);
  EXPECT_EQ(*infer_ip, "never");
}

TEST(CrashReportFields, LeaveOutWhatIsntAVersionOrAChannel) {
  // A build that passes neither, and anything else a switch could carry.
  for (const char* release : {"", "0.3.1-dev", "/Users/someone", "1.2.3 rework"}) {
    EXPECT_FALSE(Fields(release, "release").FindString("release")) << release;
  }
  for (const char* channel : {"", "beta", "Rework", "release "}) {
    EXPECT_FALSE(Fields("0.3.1", channel).FindString("environment")) << channel;
  }
  EXPECT_TRUE(Fields("", "").FindDict("sdk"));
}

}  // namespace
}  // namespace browser_core::crash_reports
