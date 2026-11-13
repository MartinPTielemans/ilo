#include "browser_core/dialog_keys.h"

#include <map>
#include <memory>

#include "base/memory/raw_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "components/input/native_web_keyboard_event.h"
#include "content/public/browser/keyboard_event_processing_result.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_delegate.h"
#include "ui/base/accelerators/accelerator.h"
#include "ui/base/mojom/ui_base_types.mojom-shared.h"
#include "ui/events/event.h"
#include "ui/events/event_handler.h"
#include "ui/events/keycodes/keyboard_codes.h"
#include "ui/views/focus/focus_manager.h"
#include "ui/views/view.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_delegate.h"
#include "ui/views/widget/widget_observer.h"

namespace browser_core::dialog_keys {
namespace {

class DialogKeys;

std::map<views::Widget*, std::unique_ptr<DialogKeys>>& Watched() {
  static base::NoDestructor<std::map<views::Widget*, std::unique_ptr<DialogKeys>>> watched;
  return *watched;
}

// The page shown in browser window |window|: a tab's dialog shows over its own tab's page alone.
content::WebContents* ShownPage(views::Widget* window) {
  for (Window* each : Window::All()) {
    if (each->widget() == window) {
      return Tab::FromId(each->selected());
    }
  }
  return nullptr;
}

// Hears the keys of |dialog| in its root view, as views gives them to it on every OS: before the
// dialog for ilo's reserved and Prefer ilo shortcuts (tab_key_before_page), and after it for those
// it didn't use (tab_key_unhandled), as the core's page gives them.
class DialogKeys : public ui::EventHandler, public views::WidgetObserver {
 public:
  DialogKeys(views::Widget* dialog, views::Widget* window) : dialog_(dialog), window_(window) {
    dialog->GetRootView()->AddPreTargetHandler(this);
    dialog->GetRootView()->AddPostTargetHandler(this);
    observation_.Observe(dialog);
  }
  DialogKeys(const DialogKeys&) = delete;
  DialogKeys& operator=(const DialogKeys&) = delete;
  ~DialogKeys() override {
    if (views::View* root = dialog_->GetRootView()) {
      root->RemovePreTargetHandler(this);
      root->RemovePostTargetHandler(this);
    }
  }

  // ui::EventHandler:
  void OnKeyEvent(ui::KeyEvent* event) override {
    // A page's keys are asked about as they go down, not as the characters they type.
    if (event->type() != ui::EventType::kKeyPressed || event->is_char() || DialogKeeps(*event)) {
      return;
    }
    const bool before = event->phase() == ui::EP_PRETARGET;
    if (!before && event->handled()) {
      return;
    }
    content::WebContents* page = ShownPage(window_);
    if (!page || !page->GetDelegate()) {
      return;
    }
    const input::NativeWebKeyboardEvent key(*event);
    if (before) {
      const content::KeyboardEventProcessingResult result =
          page->GetDelegate()->PreHandleKeyboardEvent(page, key);
      if (result != content::KeyboardEventProcessingResult::NOT_HANDLED &&
          result != content::KeyboardEventProcessingResult::NOT_HANDLED_IS_SHORTCUT) {
        event->StopPropagation();
      }
      return;
    }
    // The tab, the window and the dialog can go in it: nothing here is touched after.
    if (page->GetDelegate()->HandleKeyboardEvent(page, key)) {
      event->StopPropagation();
    }
  }

  // views::WidgetObserver:
  void OnWidgetDestroying(views::Widget* widget) override {
    // Erasing deletes this, so it's the last thing done.
    Watched().erase(widget);
  }

 private:
  // Return, Escape and Tab answer, cancel and move through the dialog, and its own shortcuts stay
  // its own: the keys its page would keep, as a page keeps ⌘F in Docs.
  bool DialogKeeps(const ui::KeyEvent& event) const {
    switch (event.key_code()) {
      case ui::VKEY_RETURN:
      case ui::VKEY_ESCAPE:
      case ui::VKEY_TAB:
        return true;
      default:
        break;
    }
    views::FocusManager* focus = dialog_->GetFocusManager();
    return focus && focus->IsAcceleratorRegistered(ui::Accelerator(event));
  }

  const raw_ptr<views::Widget> dialog_;
  const raw_ptr<views::Widget> window_;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

}  // namespace

void Watch(views::Widget* window) {
  if (!window) {
    return;
  }
  // A tab's dialog is modal to its tab (kChild): in the window, as a child widget, or in one of its
  // own that the window owns, or on the Mac a child NSWindow, which views lists alike.
  for (views::Widget* dialog : views::Widget::GetAllOwnedWidgets(window->GetNativeView())) {
    if (dialog == window || Watched().contains(dialog) || !dialog->GetRootView() ||
        !dialog->widget_delegate() ||
        dialog->widget_delegate()->GetModalType() != ui::mojom::ModalType::kChild) {
      continue;
    }
    Watched().emplace(dialog, std::make_unique<DialogKeys>(dialog, window));
  }
}

}  // namespace browser_core::dialog_keys
