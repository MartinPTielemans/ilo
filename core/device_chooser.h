#ifndef BROWSER_CORE_DEVICE_CHOOSER_H_
#define BROWSER_CORE_DEVICE_CHOOSER_H_

#include <memory>
#include <string>

#include "base/functional/callback_forward.h"
#include "base/memory/weak_ptr.h"
#include "browser_core/public/browser_core.h"
#include "components/permissions/chooser_controller.h"
#include "content/public/browser/web_contents_observer.h"

namespace content {
class RenderFrameHost;
}

namespace browser_core {

// A page's request for a device (navigator.serial, .hid, .usb and .bluetooth), which the app shows
// in its card in place of Chrome's chooser bubble (tab_chooser). Chrome's ChooserController still
// finds the devices and gives the page the one chosen; this is its view. One a tab: a tab's newer
// request ends its older one, as Chrome's bubble for it would close.
class DeviceChooser : public permissions::ChooserController::View,
                      public content::WebContentsObserver {
 public:
  // g_browser_core_device_chooser. Returns what ends the request without an answer, which Chrome's
  // code that asked runs once it no longer needs one.
  static base::OnceClosure Show(content::RenderFrameHost* owner,
                                std::unique_ptr<permissions::ChooserController> controller);
  // See bc_tab_choose_device and the calls after it.
  static void Choose(bc_id tab, const char* id);
  static void Cancel(bc_id tab);
  static void Scan(bc_id tab);
  static void Settings(bc_id tab);

  DeviceChooser(const DeviceChooser&) = delete;
  DeviceChooser& operator=(const DeviceChooser&) = delete;
  ~DeviceChooser() override;

 private:
  DeviceChooser(content::WebContents* contents,
                bc_id tab,
                int kind,
                std::unique_ptr<permissions::ChooserController> controller);

  // Ends the tab's request, if it's still |chooser|, with |answer| told to the controller first:
  // its Select, Cancel or Close. None drops it, which Chrome takes as a cancel.
  static void End(bc_id tab,
                  const DeviceChooser* chooser,
                  base::OnceCallback<void(permissions::ChooserController*)> answer);

  // permissions::ChooserController::View:
  void OnOptionsInitialized() override;
  void OnOptionAdded(size_t index) override;
  void OnOptionRemoved(size_t index) override;
  void OnOptionUpdated(size_t index) override;
  void OnAdapterEnabledChanged(bool enabled) override;
  void OnAdapterAuthorizationChanged(bool authorized) override;
  void OnRefreshStateChanged(bool refreshing) override;

  // content::WebContentsObserver: the request goes with its page, as Chrome's bubble does.
  void PrimaryPageChanged(content::Page& page) override;
  void WebContentsDestroyed() override;

  // The core's id for the option at |index|: its text, and how many before it share that text.
  std::string IdOf(size_t index) const;
  // A BC_SCAN_ value.
  int ScanState() const;

  // Tells the app from a task of its own: the controller reports inside the app's calls.
  void ReportSoon();
  void Report();
  // Tells the app the tab's request ended.
  static void ReportEnded(bc_id tab);

  const bc_id tab_;
  // A BC_DEVICE_ value.
  const int kind_;
  const std::string site_;
  std::unique_ptr<permissions::ChooserController> controller_;
  bool initialized_ = false;
  // Chrome shows its throbber until the list is first filled, or while Bluetooth looks.
  bool refreshing_ = true;
  bool adapter_off_ = false;
  bool unauthorized_ = false;
  bool reporting_ = false;
  base::WeakPtrFactory<DeviceChooser> weak_factory_{this};
};

}  // namespace browser_core

#endif  // BROWSER_CORE_DEVICE_CHOOSER_H_
