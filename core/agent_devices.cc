#include "browser_core/agent_devices.h"

#include <vector>

#include "browser_core/agent_space_page.h"
#include "chrome/browser/bluetooth/chrome_bluetooth_delegate.h"
#include "chrome/browser/bluetooth/chrome_bluetooth_delegate_impl_client.h"
#include "chrome/browser/hid/chrome_hid_delegate.h"
#include "chrome/browser/serial/chrome_serial_delegate.h"
#include "chrome/browser/usb/chrome_usb_delegate.h"
#include "third_party/blink/public/mojom/bluetooth/web_bluetooth.mojom.h"

namespace browser_core {

namespace {

// A worker with no page asks with no frame, and is in no tab.
bool Refused(content::RenderFrameHost* frame) {
  return frame && RefusesInAgentSpace(agent_space_rule::Ask::kDevice, frame);
}

class UsbDelegate : public ChromeUsbDelegate {
 public:
  bool HasDevicePermission(content::BrowserContext* browser_context,
                           content::RenderFrameHost* frame,
                           const url::Origin& origin,
                           const device::mojom::UsbDeviceInfo& device_info) override {
    return !Refused(frame) &&
           ChromeUsbDelegate::HasDevicePermission(browser_context, frame, origin, device_info);
  }
};

class HidDelegate : public ChromeHidDelegate {
 public:
  bool HasDevicePermission(content::BrowserContext* browser_context,
                           content::RenderFrameHost* frame,
                           const url::Origin& origin,
                           const device::mojom::HidDeviceInfo& device) override {
    return !Refused(frame) &&
           ChromeHidDelegate::HasDevicePermission(browser_context, frame, origin, device);
  }
};

class SerialDelegate : public ChromeSerialDelegate {
 public:
  bool HasPortPermission(content::RenderFrameHost* frame,
                         const device::mojom::SerialPortInfo& port) override {
    return !Refused(frame) && ChromeSerialDelegate::HasPortPermission(frame, port);
  }
};

class BluetoothDelegate : public ChromeBluetoothDelegate {
 public:
  using ChromeBluetoothDelegate::ChromeBluetoothDelegate;

  bool HasDevicePermission(content::RenderFrameHost* frame,
                           const blink::WebBluetoothDeviceId& device_id) override {
    return !Refused(frame) && ChromeBluetoothDelegate::HasDevicePermission(frame, device_id);
  }
  std::vector<blink::mojom::WebBluetoothDevicePtr> GetPermittedDevices(
      content::RenderFrameHost* frame) override {
    if (Refused(frame)) {
      return {};
    }
    return ChromeBluetoothDelegate::GetPermittedDevices(frame);
  }
};

}  // namespace

std::unique_ptr<content::UsbDelegate> MakeUsbDelegate() {
  return std::make_unique<UsbDelegate>();
}

std::unique_ptr<content::HidDelegate> MakeHidDelegate() {
  return std::make_unique<HidDelegate>();
}

std::unique_ptr<content::SerialDelegate> MakeSerialDelegate() {
  return std::make_unique<SerialDelegate>();
}

std::unique_ptr<content::BluetoothDelegate> MakeBluetoothDelegate() {
  return std::make_unique<BluetoothDelegate>(std::make_unique<ChromeBluetoothDelegateImplClient>());
}

}  // namespace browser_core
