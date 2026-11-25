#ifndef BROWSER_CORE_AGENT_DEVICES_H_
#define BROWSER_CORE_AGENT_DEVICES_H_

#include <memory>

#include "content/public/browser/bluetooth_delegate.h"
#include "content/public/browser/hid_delegate.h"
#include "content/public/browser/serial_delegate.h"
#include "content/public/browser/usb_delegate.h"

namespace browser_core {

// Chrome's delegates for a page's USB, HID, serial and Bluetooth devices, which tell a page in an
// agent's Space that its site has none: a device the user gave a site is theirs to use, as a new
// one is theirs to give (device_chooser.h). The rule is agent_space_rule.h's, by the Space alone,
// and it's asked each time a page lists or opens one, so a tab is answered as where it is now.
std::unique_ptr<content::UsbDelegate> MakeUsbDelegate();
std::unique_ptr<content::HidDelegate> MakeHidDelegate();
std::unique_ptr<content::SerialDelegate> MakeSerialDelegate();
std::unique_ptr<content::BluetoothDelegate> MakeBluetoothDelegate();

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_DEVICES_H_
