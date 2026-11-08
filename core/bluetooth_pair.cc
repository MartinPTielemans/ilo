#include "browser_core/bluetooth_pair.h"

#include <algorithm>
#include <map>
#include <memory>
#include <utility>

#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

using content::BluetoothDelegate;

std::map<bc_id, std::unique_ptr<BluetoothPair>>& Pairs() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<BluetoothPair>>> pairs;
  return *pairs;
}

const char* KindName(int kind) {
  switch (kind) {
    case BC_PAIR_PIN:
      return "pin";
    case BC_PAIR_PASSKEY:
      return "passkey";
  }
  return "confirm";
}

// Tells the app the tab's prompt ended, unless a newer one has taken the tab since.
void ReportEnded(bc_id tab) {
  if (Pairs().contains(tab) || !Host().tab_pair) {
    return;
  }
  Host().tab_pair(Host().context, tab, nullptr, 0, nullptr);
}

}  // namespace

// static
void BluetoothPair::Show(content::RenderFrameHost* frame,
                         const std::u16string& device,
                         BluetoothDelegate::PairPromptCallback callback,
                         BluetoothDelegate::PairingKind kind,
                         const std::optional<std::u16string>& passkey) {
  // Chrome's dialogs: its credentials one for a PIN, its confirm one for the others.
  int pair;
  switch (kind) {
    case BluetoothDelegate::PairingKind::kProvidePin:
      pair = BC_PAIR_PIN;
      break;
    case BluetoothDelegate::PairingKind::kConfirmOnly:
      pair = BC_PAIR_CONFIRM;
      break;
    case BluetoothDelegate::PairingKind::kConfirmPinMatch:
      pair = passkey ? BC_PAIR_PASSKEY : BC_PAIR_CONFIRM;
      break;
    default:
      // Chrome asks for no other kind.
      std::move(callback).Run(
          BluetoothDelegate::PairPromptResult(BluetoothDelegate::PairPromptStatus::kCancelled));
      return;
  }
  Ask(content::WebContents::FromRenderFrameHost(frame), base::UTF16ToUTF8(device), pair,
      passkey ? std::optional<std::string>(base::UTF16ToUTF8(*passkey)) : std::nullopt,
      base::BindOnce(
          [](BluetoothDelegate::PairPromptCallback callback, std::optional<std::string> pin) {
            BluetoothDelegate::PairPromptResult result(
                pin ? BluetoothDelegate::PairPromptStatus::kSuccess
                    : BluetoothDelegate::PairPromptStatus::kCancelled);
            result.pin = pin.value_or("");
            std::move(callback).Run(result);
          },
          std::move(callback)));
}

// static
void BluetoothPair::ShowForTesting(bc_id tab, int kind) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents || (kind != BC_PAIR_PIN && kind != BC_PAIR_CONFIRM && kind != BC_PAIR_PASSKEY)) {
    return;
  }
  Ask(contents, "Check device", kind,
      kind == BC_PAIR_PASSKEY ? std::optional<std::string>("123456") : std::nullopt,
      base::BindOnce(
          [](bc_id tab, std::optional<std::string> pin) {
            // check.sh reads these lines.
            if (!pin) {
              LOG(ERROR) << "browser: tab " << tab << " pairing cancelled";
            } else if (pin->empty()) {
              LOG(ERROR) << "browser: tab " << tab << " paired";
            } else {
              LOG(ERROR) << "browser: tab " << tab << " paired " << *pin;
            }
          },
          tab));
}

// static
void BluetoothPair::Ask(content::WebContents* contents,
                        std::string device,
                        int kind,
                        std::optional<std::string> passkey,
                        Answer answer) {
  const bc_id tab = contents ? Tab::IdOf(contents) : 0;
  LOG(ERROR) << "browser: bluetooth pair " << tab << " " << KindName(kind) << " asked";
  if (!tab) {
    std::move(answer).Run(std::nullopt);
    return;
  }
  if (const Space* space = Space::Holding(tab); space && space->agent()) {
    // Pairing is the user's alone to agree to, and an agent's Space has no user watching it, as
    // its device chooser ends: the page's connect fails, and the activity log says why.
    LOG(ERROR) << "browser: bluetooth pair " << tab << " ended: an agent's tab";
    if (Host().agent_activity) {
      const bc_agent_activity entry = {
          .agent = space->agent()->id.c_str(),
          .tab = tab,
          .action = "pair_device bluetooth",
          .refused = "The page asked to pair with a Bluetooth device, which only the user can "
                     "agree to, so pairing ended as cancelled.",
      };
      Host().agent_activity(Host().context, &entry);
    }
    std::move(answer).Run(std::nullopt);
    return;
  }
  // A newer prompt takes the tab's card, as Chrome's dialog for the older one would close.
  End(tab, std::nullopt);
  Pairs()[tab] = base::WrapUnique(new BluetoothPair(contents, tab, std::move(device), kind,
                                                    std::move(passkey), std::move(answer)));
  const BluetoothPair& shown = *Pairs()[tab];
  LOG(ERROR) << "browser: bluetooth pair " << tab << " " << KindName(kind) << " shown";
  if (Host().tab_pair) {
    Host().tab_pair(Host().context, tab, shown.device_.c_str(), kind,
                    shown.passkey_ ? shown.passkey_->c_str() : nullptr);
  }
}

// static
void BluetoothPair::Pair(bc_id tab, const char* pin) {
  auto found = Pairs().find(tab);
  if (found == Pairs().end()) {
    return;
  }
  if (found->second->kind_ != BC_PAIR_PIN) {
    LOG(ERROR) << "browser: bluetooth pair " << tab << " agreed";
    End(tab, "");
    return;
  }
  // As Chrome's OK: a PIN needs a digit, and goes trimmed.
  const std::string typed(base::TrimWhitespaceASCII(pin ? pin : "", base::TRIM_ALL));
  if (!std::ranges::any_of(typed, [](char c) { return base::IsAsciiDigit(c); })) {
    return;
  }
  LOG(ERROR) << "browser: bluetooth pair " << tab << " pin given";
  End(tab, typed);
}

// static
void BluetoothPair::Cancel(bc_id tab) {
  if (Pairs().contains(tab)) {
    LOG(ERROR) << "browser: bluetooth pair " << tab << " cancelled";
  }
  End(tab, std::nullopt);
}

// static
void BluetoothPair::End(bc_id tab, std::optional<std::string> pin) {
  auto found = Pairs().find(tab);
  if (found == Pairs().end()) {
    return;
  }
  // Out of the map first: the answer can start another prompt for the tab.
  std::unique_ptr<BluetoothPair> ending = std::move(found->second);
  Pairs().erase(found);
  std::move(ending->answer_).Run(std::move(pin));
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&ReportEnded, tab));
}

BluetoothPair::BluetoothPair(content::WebContents* contents,
                             bc_id tab,
                             std::string device,
                             int kind,
                             std::optional<std::string> passkey,
                             Answer answer)
    : content::WebContentsObserver(contents),
      tab_(tab),
      device_(std::move(device)),
      kind_(kind),
      passkey_(std::move(passkey)),
      answer_(std::move(answer)) {}

BluetoothPair::~BluetoothPair() = default;

void BluetoothPair::PrimaryPageChanged(content::Page& page) {
  Cancel(tab_);
}

void BluetoothPair::WebContentsDestroyed() {
  Cancel(tab_);
}

}  // namespace browser_core
