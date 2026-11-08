#ifndef BROWSER_CORE_BLUETOOTH_PAIR_H_
#define BROWSER_CORE_BLUETOOTH_PAIR_H_

#include <optional>
#include <string>

#include "base/functional/callback.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/bluetooth_delegate.h"
#include "content/public/browser/web_contents_observer.h"

namespace content {
class RenderFrameHost;
}

namespace browser_core {

// A Bluetooth device a page pairs with asking the user for its PIN, or whether to pair, which the
// app shows in its card in place of Chrome's dialogs (tab_pair). One a tab: a newer one ends the
// older as cancelled. It goes with its page, as Chrome's tab-modal dialog does.
class BluetoothPair : public content::WebContentsObserver {
 public:
  // The answer: the PIN typed, "" to pair without one, or none for a cancel.
  using Answer = base::OnceCallback<void(std::optional<std::string> pin)>;

  // g_browser_core_bluetooth_pair.
  static void Show(content::RenderFrameHost* frame,
                   const std::u16string& device,
                   content::BluetoothDelegate::PairPromptCallback callback,
                   content::BluetoothDelegate::PairingKind kind,
                   const std::optional<std::u16string>& passkey);
  // See bc_tab_pair, bc_tab_cancel_pair and bc_tab_pair_for_testing.
  static void Pair(bc_id tab, const char* pin);
  static void Cancel(bc_id tab);
  static void ShowForTesting(bc_id tab, int kind);

  BluetoothPair(const BluetoothPair&) = delete;
  BluetoothPair& operator=(const BluetoothPair&) = delete;
  ~BluetoothPair() override;

 private:
  BluetoothPair(content::WebContents* contents,
                bc_id tab,
                std::string device,
                int kind,
                std::optional<std::string> passkey,
                Answer answer);

  // Asks the app, for the tab |contents| shows, unless an agent's Space holds it.
  static void Ask(content::WebContents* contents,
                  std::string device,
                  int kind,
                  std::optional<std::string> passkey,
                  Answer answer);
  // Ends the tab's prompt with |pin|, none for a cancel, and tells the app.
  static void End(bc_id tab, std::optional<std::string> pin);

  // content::WebContentsObserver: the prompt goes with its page.
  void PrimaryPageChanged(content::Page& page) override;
  void WebContentsDestroyed() override;

  const bc_id tab_;
  const std::string device_;
  // A BC_PAIR_ value.
  const int kind_;
  const std::optional<std::string> passkey_;
  Answer answer_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_BLUETOOTH_PAIR_H_
