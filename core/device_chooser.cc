#include "browser_core/device_chooser.h"

#include <map>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/logging.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "build/build_config.h"
#include "chrome/browser/platform_util.h"
#include "chrome/grit/generated_resources.h"
#include "components/url_formatter/elide_url.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "ui/base/l10n/l10n_util.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

std::map<bc_id, std::unique_ptr<DeviceChooser>>& Choosers() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<DeviceChooser>>> choosers;
  return *choosers;
}

// Whether |title| is Chrome's chooser title |id|, whatever site it names.
bool IsTitle(const std::u16string& title, int id) {
  const std::u16string marked = l10n_util::GetStringFUTF16(id, u"\x1f");
  const size_t mark = marked.find(u'\x1f');
  if (mark == std::u16string::npos) {
    return false;
  }
  const std::u16string_view before = std::u16string_view(marked).substr(0, mark);
  const std::u16string_view after = std::u16string_view(marked).substr(mark + 1);
  return title.size() > before.size() + after.size() && title.starts_with(before) &&
         title.ends_with(after);
}

// What the page asks for, which Chrome's controllers tell apart only by their titles, and
// Bluetooth's by its Re-scan.
int KindOf(const permissions::ChooserController& controller) {
  if (controller.ShouldShowReScanButton()) {
    return BC_DEVICE_BLUETOOTH;
  }
  const std::u16string title = controller.GetTitle();
  if (IsTitle(title, IDS_SERIAL_PORT_CHOOSER_PROMPT)) {
    return BC_DEVICE_SERIAL;
  }
  if (IsTitle(title, IDS_HID_CHOOSER_PROMPT)) {
    return BC_DEVICE_HID;
  }
  return BC_DEVICE_USB;
}

const char* KindName(int kind) {
  switch (kind) {
    case BC_DEVICE_SERIAL:
      return "serial";
    case BC_DEVICE_HID:
      return "hid";
    case BC_DEVICE_BLUETOOTH:
      return "bluetooth";
  }
  return "usb";
}

// Chrome's option text, "Arduino Uno (cu.usbmodem14101)", as a name and what's in its brackets.
std::pair<std::string, std::string> Split(const std::u16string& option) {
  const std::string text = base::UTF16ToUTF8(option);
  const size_t open = text.rfind(" (");
  if (open == std::string::npos || open == 0 || !text.ends_with(')')) {
    return {text, ""};
  }
  return {text.substr(0, open), text.substr(open + 2, text.size() - open - 3)};
}

}  // namespace

// static
base::OnceClosure DeviceChooser::Show(content::RenderFrameHost* owner,
                                      std::unique_ptr<permissions::ChooserController> controller) {
  content::WebContents* contents = content::WebContents::FromRenderFrameHost(owner);
  const bc_id tab = contents ? Tab::IdOf(contents) : 0;
  const int kind = KindOf(*controller);
  if (!tab) {
    controller->Cancel();
    return base::DoNothing();
  }
  if (const Space* space = Space::Holding(tab); space && space->agent()) {
    // A device is the user's alone to give, and an agent's Space has no user watching it, so the
    // page hears a cancel, as from Chrome's Cancel, and the activity log says why.
    LOG(INFO) << "browser_core: device chooser " << tab << " " << KindName(kind)
              << " ended: an agent's tab";
    if (Host().agent_activity) {
      const std::string action = std::string("request_device ") + KindName(kind);
      const bc_agent_activity entry = {
          .agent = space->agent()->id.c_str(),
          .tab = tab,
          .action = action.c_str(),
          .refused = "The page asked for a device, which only the user can give, so the request "
                     "ended as cancelled.",
      };
      Host().agent_activity(Host().context, &entry);
    }
    controller->Cancel();
    return base::DoNothing();
  }
  // A newer request takes the tab's card, as Chrome's bubble for the older one would close.
  End(tab, nullptr, base::BindOnce(&permissions::ChooserController::Close));
  auto chooser = base::WrapUnique(new DeviceChooser(contents, tab, kind, std::move(controller)));
  base::WeakPtr<DeviceChooser> weak = chooser->weak_factory_.GetWeakPtr();
  Choosers()[tab] = std::move(chooser);
  LOG(INFO) << "browser_core: device chooser " << tab << " " << KindName(kind) << " shown";
  return base::BindOnce(
      [](bc_id tab, base::WeakPtr<DeviceChooser> chooser) {
        if (chooser) {
          End(tab, chooser.get(), {});
        }
      },
      tab, weak);
}

// static
void DeviceChooser::Choose(bc_id tab, const char* id) {
  auto found = Choosers().find(tab);
  if (found == Choosers().end() || !id) {
    return;
  }
  const DeviceChooser& chooser = *found->second;
  for (size_t index = 0; index < chooser.controller_->NumOptions(); ++index) {
    if (chooser.IdOf(index) == id) {
      LOG(INFO) << "browser_core: device chooser " << tab << " chose " << index;
      End(tab, &chooser,
          base::BindOnce(
              [](size_t index, permissions::ChooserController* controller) {
                controller->Select({index});
              },
              index));
      return;
    }
  }
}

// static
void DeviceChooser::Cancel(bc_id tab) {
  LOG(INFO) << "browser_core: device chooser " << tab << " cancelled";
  End(tab, nullptr, base::BindOnce(&permissions::ChooserController::Cancel));
}

// static
void DeviceChooser::Scan(bc_id tab) {
  auto found = Choosers().find(tab);
  if (found == Choosers().end() || found->second->kind_ != BC_DEVICE_BLUETOOTH) {
    return;
  }
  found->second->controller_->RefreshOptions();
}

// static
void DeviceChooser::Settings(bc_id tab) {
  auto found = Choosers().find(tab);
  if (found == Choosers().end()) {
    return;
  }
  const DeviceChooser& chooser = *found->second;
  if (chooser.unauthorized_) {
#if BUILDFLAG(IS_MAC)
    chooser.controller_->OpenPermissionPreferences();
#endif
    return;
  }
  if (chooser.adapter_off_) {
    // Where Bluetooth is turned on; Linux has no one place for it.
#if BUILDFLAG(IS_MAC)
    platform_util::OpenExternal(GURL("x-apple.systempreferences:com.apple.BluetoothSettings"));
#elif BUILDFLAG(IS_WIN)
    platform_util::OpenExternal(GURL("ms-settings:bluetooth"));
#endif
  }
}

DeviceChooser::DeviceChooser(content::WebContents* contents,
                             bc_id tab,
                             int kind,
                             std::unique_ptr<permissions::ChooserController> controller)
    : content::WebContentsObserver(contents),
      tab_(tab),
      kind_(kind),
      // As the page's permission question names the site.
      site_(base::UTF16ToUTF8(url_formatter::FormatUrlForSecurityDisplay(
          contents->GetPrimaryMainFrame()->GetLastCommittedOrigin().GetURL(),
          url_formatter::SchemeDisplay::OMIT_CRYPTOGRAPHIC))),
      controller_(std::move(controller)) {
  controller_->set_view(this);
  ReportSoon();
}

DeviceChooser::~DeviceChooser() {
  if (controller_ && controller_->view() == this) {
    controller_->set_view(nullptr);
  }
}

// static
void DeviceChooser::End(bc_id tab,
                        const DeviceChooser* chooser,
                        base::OnceCallback<void(permissions::ChooserController*)> answer) {
  auto found = Choosers().find(tab);
  if (found == Choosers().end() || (chooser && found->second.get() != chooser)) {
    return;
  }
  // Out of the map first: the answer can come back here, through the closure Show returned.
  std::unique_ptr<DeviceChooser> ending = std::move(found->second);
  Choosers().erase(found);
  ending->controller_->set_view(nullptr);
  if (answer) {
    std::move(answer).Run(ending->controller_.get());
  }
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&DeviceChooser::ReportEnded, tab));
}

void DeviceChooser::OnOptionsInitialized() {
  initialized_ = true;
  if (kind_ != BC_DEVICE_BLUETOOTH) {
    refreshing_ = false;
  }
  ReportSoon();
}

void DeviceChooser::OnOptionAdded(size_t index) {
  ReportSoon();
}

void DeviceChooser::OnOptionRemoved(size_t index) {
  ReportSoon();
}

void DeviceChooser::OnOptionUpdated(size_t index) {
  ReportSoon();
}

void DeviceChooser::OnAdapterEnabledChanged(bool enabled) {
  adapter_off_ = !enabled;
  ReportSoon();
}

void DeviceChooser::OnAdapterAuthorizationChanged(bool authorized) {
  unauthorized_ = !authorized;
  ReportSoon();
}

void DeviceChooser::OnRefreshStateChanged(bool refreshing) {
  refreshing_ = refreshing;
  ReportSoon();
}

void DeviceChooser::PrimaryPageChanged(content::Page& page) {
  End(tab_, this, base::BindOnce(&permissions::ChooserController::Close));
}

void DeviceChooser::WebContentsDestroyed() {
  End(tab_, this, base::BindOnce(&permissions::ChooserController::Close));
}

std::string DeviceChooser::IdOf(size_t index) const {
  const std::u16string option = controller_->GetOption(index);
  size_t before = 0;
  for (size_t other = 0; other < index; ++other) {
    before += controller_->GetOption(other) == option;
  }
  return base::UTF16ToUTF8(option) + "#" + base::NumberToString(before);
}

int DeviceChooser::ScanState() const {
  if (unauthorized_) {
    return BC_SCAN_DENIED;
  }
  if (adapter_off_) {
    return BC_SCAN_OFF;
  }
  if (kind_ == BC_DEVICE_BLUETOOTH) {
    return refreshing_ ? BC_SCAN_SCANNING : BC_SCAN_STOPPED;
  }
  return initialized_ ? BC_SCAN_LIVE : BC_SCAN_SCANNING;
}

void DeviceChooser::ReportSoon() {
  if (reporting_) {
    return;
  }
  reporting_ = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&DeviceChooser::Report, weak_factory_.GetWeakPtr()));
}

void DeviceChooser::Report() {
  reporting_ = false;
  if (!Host().tab_chooser) {
    return;
  }
  const size_t count = controller_->NumOptions();
  // The strings first, as a device's point into them.
  std::vector<std::string> ids, names, details;
  for (size_t index = 0; index < count; ++index) {
    auto [name, detail] = Split(controller_->GetOption(index));
    if (controller_->IsPaired(index)) {
      detail = detail.empty() ? "Paired" : detail + " · Paired";
    }
    ids.push_back(IdOf(index));
    names.push_back(std::move(name));
    details.push_back(std::move(detail));
  }
  std::vector<bc_chooser_device> devices;
  for (size_t index = 0; index < count; ++index) {
    devices.push_back({
        .id = ids[index].c_str(),
        .name = names[index].c_str(),
        .detail = details[index].c_str(),
    });
  }
  Host().tab_chooser(Host().context, tab_, site_.c_str(), kind_, devices.data(), devices.size(),
                     ScanState());
}

// static
void DeviceChooser::ReportEnded(bc_id tab) {
  // A newer request may have taken the tab since.
  if (Choosers().contains(tab) || !Host().tab_chooser) {
    return;
  }
  Host().tab_chooser(Host().context, tab, nullptr, 0, nullptr, 0, 0);
}

}  // namespace browser_core
