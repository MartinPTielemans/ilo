#include "browser_core/platform/battery.h"

#include <windows.h>

#include <powersetting.h>
#include <powrprof.h>

#include <vector>

#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/task/sequenced_task_runner.h"
#include "base/time/time.h"

namespace browser_core::platform {

namespace {

// winnt.h's GUID_BATTERY_PERCENTAGE_REMAINING and GUID_ACDC_POWER_SOURCE, which are defined in
// uuid.lib, not linked here.
constexpr GUID kPercentageRemaining = {
    0xa7ad8041, 0xb45a, 0x4cae, {0x87, 0xa3, 0xee, 0xcb, 0xb4, 0x68, 0xa9, 0xe1}};
constexpr GUID kPowerSource = {
    0x5d3e9a59, 0xe9d5, 0x4b00, {0xa6, 0xbd, 0xff, 0x34, 0xff, 0x51, 0x65, 0x48}};

class NoticeSource;

// Where Windows' callbacks, on a thread of its own, hand a notice to the source. It outlives the
// source, as a callback may still be running as the source goes, and then posts to nothing.
struct Notices {
  scoped_refptr<base::SequencedTaskRunner> runner;
  base::WeakPtr<NoticeSource> source;
};

Notices& TheNotices() {
  static base::NoDestructor<Notices> notices;
  return *notices;
}

// Samples as Windows says the remaining charge changed, or the computer was plugged in or out,
// in place of Chromium's sampling each minute: an idle browser on mains never wakes for it.
// Windows says both once as each is registered, so the first sample follows at once.
class NoticeSource : public base::SamplingEventSource {
 public:
  ~NoticeSource() override {
    for (HPOWERNOTIFY handle : handles_) {
      PowerSettingUnregisterNotification(handle);
    }
  }

  bool Start(SamplingEventCallback callback) override {
    sample_ = std::move(callback);
    TheNotices() = {base::SequencedTaskRunner::GetCurrentDefault(), weak_.GetWeakPtr()};
    DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS params = {&NoticeSource::Notice, nullptr};
    for (const GUID* setting : {&kPercentageRemaining, &kPowerSource}) {
      HPOWERNOTIFY handle = nullptr;
      if (PowerSettingRegisterNotification(setting, DEVICE_NOTIFY_CALLBACK, &params, &handle) ==
          ERROR_SUCCESS) {
        handles_.push_back(handle);
      }
    }
    return !handles_.empty();
  }
  base::TimeDelta GetSampleInterval() override { return base::Minutes(1); }

 private:
  static ULONG CALLBACK Notice(PVOID, ULONG type, PVOID) {
    if (type == PBT_POWERSETTINGCHANGE) {
      const Notices& notices = TheNotices();
      notices.runner->PostTask(FROM_HERE, base::BindOnce(&NoticeSource::Sample, notices.source));
    }
    return ERROR_SUCCESS;
  }

  void Sample() {
    if (!told_) {
      told_ = true;
      // check.sh reads this line.
      LOG(ERROR) << "browser: battery sampled on Windows' notice";
    }
    sample_.Run();
  }

  SamplingEventCallback sample_;
  std::vector<HPOWERNOTIFY> handles_;
  bool told_ = false;
  base::WeakPtrFactory<NoticeSource> weak_{this};
};

}  // namespace

Battery OsBattery() {
  return {std::make_unique<NoticeSource>(), base::BatteryLevelProvider::Create()};
}

}  // namespace browser_core::platform
