#include <cmath>
#include <optional>
#include <string>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/task/sequenced_task_runner.h"
#include "base/time/time.h"
#include "browser_core/platform/battery.h"
#include "components/dbus/thread_linux/dbus_thread_linux.h"
#include "dbus/bus.h"
#include "dbus/message.h"
#include "dbus/object_path.h"
#include "dbus/object_proxy.h"
#include "dbus/property.h"

namespace browser_core::platform {

namespace {

using BatteryState = base::BatteryLevelProvider::BatteryState;

// UPower's display device: the computer's batteries as one, as a desktop's battery icon shows them.
constexpr char kUPower[] = "org.freedesktop.UPower";
constexpr char kDisplayDevice[] = "/org/freedesktop/UPower/devices/DisplayDevice";
constexpr char kDevice[] = "org.freedesktop.UPower.Device";

// Its Type for a battery, and the States in which it powers the computer.
constexpr uint32_t kBatteryType = 2;
constexpr uint32_t kDischarging = 2;
constexpr uint32_t kEmpty = 3;
constexpr uint32_t kPendingDischarge = 6;

// What UPower last said, which the provider reports. None until it has said, or without UPower or
// a battery, so Battery Saver never turns on.
std::optional<BatteryState>& Last() {
  static base::NoDestructor<std::optional<BatteryState>> last;
  return *last;
}

// Samples as UPower says the display device changed, on its PropertiesChanged signal: nothing
// polls, so an idle browser on mains never wakes for it.
class UPowerSource : public base::SamplingEventSource {
 public:
  bool Start(SamplingEventCallback callback) override {
    sample_ = std::move(callback);
    // The sampler starts this before Chrome's threads are made, and the bus needs its own.
    base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&UPowerSource::Connect, weak_.GetWeakPtr()));
    return true;
  }
  base::TimeDelta GetSampleInterval() override { return base::Minutes(1); }

 private:
  void Connect() {
    scoped_refptr<dbus::Bus> bus = dbus_thread_linux::GetSharedSystemBus();
    // Without UPower, quietly no battery.
    device_ = bus->GetObjectProxyWithOptions(kUPower, dbus::ObjectPath(kDisplayDevice),
                                             dbus::ObjectProxy::IGNORE_SERVICE_UNKNOWN_ERRORS);
    device_->ConnectToSignal(dbus::kPropertiesInterface, dbus::kPropertiesChanged,
                             base::BindRepeating(&UPowerSource::Changed, weak_.GetWeakPtr()),
                             base::DoNothing());
    // A UPower started again says nothing of what it reads until it changes.
    device_->SetNameOwnerChangedCallback(base::BindRepeating(
        [](base::WeakPtr<UPowerSource> self, const std::string&, const std::string& owner) {
          if (self && !owner.empty()) {
            self->Read();
          }
        },
        weak_.GetWeakPtr()));
    Read();
  }

  // UPower changes the device's rate and times as it discharges; only what Battery Saver reads is
  // worth reading again.
  void Changed(dbus::Signal* signal) {
    dbus::MessageReader reader(signal);
    std::string interface;
    dbus::MessageReader changed(nullptr);
    if (!reader.PopString(&interface) || interface != kDevice || !reader.PopArray(&changed)) {
      return;
    }
    while (changed.HasMoreData()) {
      dbus::MessageReader entry(nullptr);
      std::string name;
      if (!changed.PopDictEntry(&entry) || !entry.PopString(&name)) {
        return;
      }
      if (name == "Percentage" || name == "State" || name == "IsPresent" || name == "Type") {
        Read();
        return;
      }
    }
  }

  void Read() {
    dbus::MethodCall call(dbus::kPropertiesInterface, dbus::kPropertiesGetAll);
    dbus::MessageWriter(&call).AppendString(kDevice);
    device_->CallMethod(&call, dbus::ObjectProxy::TIMEOUT_USE_DEFAULT,
                        base::BindOnce(&UPowerSource::Got, weak_.GetWeakPtr()));
  }

  void Got(dbus::Response* response) {
    double percentage = 0;
    uint32_t state = 0;
    uint32_t type = 0;
    bool present = false;
    dbus::MessageReader reader(response);
    dbus::MessageReader properties(nullptr);
    if (response && reader.PopArray(&properties)) {
      while (properties.HasMoreData()) {
        dbus::MessageReader entry(nullptr);
        std::string name;
        if (!properties.PopDictEntry(&entry) || !entry.PopString(&name)) {
          break;
        }
        if (name == "Percentage") {
          entry.PopVariantOfDouble(&percentage);
        } else if (name == "State") {
          entry.PopVariantOfUint32(&state);
        } else if (name == "Type") {
          entry.PopVariantOfUint32(&type);
        } else if (name == "IsPresent") {
          entry.PopVariantOfBool(&present);
        }
      }
    }
    std::optional<BatteryState> read;
    if (present && type == kBatteryType) {
      read.emplace();
      read->battery_count = 1;
      read->is_external_power_connected =
          state != kDischarging && state != kEmpty && state != kPendingDischarge;
      // Hundredths of a percent, as UPower gives a fraction.
      read->current_capacity = static_cast<uint64_t>(std::llround(percentage * 100));
      read->full_charged_capacity = 10000;
      read->charge_unit = base::BatteryLevelProvider::BatteryLevelUnit::kRelative;
      read->capture_time = base::TimeTicks::Now();
    }
    std::optional<BatteryState>& last = Last();
    const bool same = last.has_value() == read.has_value() &&
                      (!read || (last->current_capacity / 100 == read->current_capacity / 100 &&
                                 last->is_external_power_connected ==
                                     read->is_external_power_connected));
    last = read;
    if (!same) {
      // check.sh reads this line.
      if (read) {
        LOG(ERROR) << "browser: UPower says " << read->current_capacity / 100 << "% "
                   << (read->is_external_power_connected ? "plugged in" : "on battery");
      } else {
        LOG(ERROR) << "browser: UPower says no battery";
      }
    }
    if (sample_) {
      sample_.Run();
    }
  }

  SamplingEventCallback sample_;
  // The bus owns it.
  raw_ptr<dbus::ObjectProxy> device_ = nullptr;
  base::WeakPtrFactory<UPowerSource> weak_{this};
};

class UPowerProvider : public base::BatteryLevelProvider {
 public:
  void GetBatteryState(
      base::OnceCallback<void(const std::optional<BatteryState>&)> callback) override {
    std::move(callback).Run(Last());
  }
};

}  // namespace

Battery OsBattery() {
  return {std::make_unique<UPowerSource>(), std::make_unique<UPowerProvider>()};
}

}  // namespace browser_core::platform
