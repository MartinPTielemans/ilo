#include "browser_core/dialog_keys.h"

#include <cstdio>
#include <cstdlib>
#include <map>
#include <memory>
#include <string>

#include "base/functional/bind.h"
#include "base/location.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/platform/keyboard.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/ui/tabs/public/tab_dialog_manager.h"
#include "components/input/native_web_keyboard_event.h"
#include "content/public/browser/keyboard_event_processing_result.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_delegate.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "ui/base/accelerators/accelerator.h"
#include "ui/base/mojom/ui_base_types.mojom-shared.h"
#include "ui/events/event.h"
#include "ui/events/event_handler.h"
#include "ui/events/keycodes/keyboard_codes.h"
#include "ui/gfx/native_ui_types.h"
#include "ui/views/focus/focus_manager.h"
#include "ui/views/view.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_delegate.h"
#include "ui/views/widget/widget_observer.h"

#if !BUILDFLAG(IS_MAC)
#include "ui/aura/window.h"
#endif

namespace browser_core::dialog_keys {
namespace {

class DialogKeys;

// Whether the test driver runs ilo, which KeyGate's lines are for alone, as the Mac's host's.
bool Driven() {
  static const bool driven = getenv("BROWSER_DRIVE") != nullptr;
  return driven;
}

// The key a tab's dialog is asking the app about, while it asks.
const input::NativeWebKeyboardEvent* g_asking = nullptr;

// "ctrl-shift-key 0x31": a key held with ⌃ or ⌘ by its code, never what it types.
std::string Chord(bool control, bool alt, bool shift, bool command, int code) {
  char text[64];
  snprintf(text, sizeof(text), "%s%s%s%skey 0x%x", control ? "ctrl-" : "", alt ? "alt-" : "",
           shift ? "shift-" : "", command ? "cmd-" : "", static_cast<unsigned>(code));
  return text;
}

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
// it didn't use (tab_key_unhandled), as the core's page gives them. And its presses.
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
    // What the handler hears, first: a shortcut it never sees is lost before the dialog's root
    // view, and one it hears only as a key going up or as text is let go below (#3717).
    if (Driven() && event->phase() == ui::EP_PRETARGET &&
        (event->flags() & (ui::EF_CONTROL_DOWN | ui::EF_COMMAND_DOWN))) {
      fprintf(stderr, "browser: tab dialog's handler heard %s %s%s\n",
              Chord(event->IsControlDown(), event->IsAltDown(), event->IsShiftDown(),
                    event->IsCommandDown(), event->key_code())
                  .c_str(),
              event->type() == ui::EventType::kKeyPressed ? "going down" : "going up",
              event->is_char() ? ", as text" : "");
    }
    if (event->type() != ui::EventType::kKeyPressed) {
      return;
    }
    // A page's keys are asked about as they go down, not as the characters they type. On the Mac a
    // key AppKit made text comes as its character alone, so it's asked about as the key (#3230).
    std::unique_ptr<ui::KeyEvent> pressed;
#if BUILDFLAG(IS_MAC)
    if (event->is_char()) {
      pressed = platform::KeyBehindCharacter(*event);
    }
#endif
    const ui::KeyEvent& down = pressed ? *pressed : *event;
    const bool before = event->phase() == ui::EP_PRETARGET;
    if (down.is_char()) {
      LetGo(before, down, "a char");
      return;
    }
    if (DialogKeeps(down)) {
      LetGo(before, down, "the dialog keeps it");
      return;
    }
    if (!before && event->handled()) {
      return;
    }
    content::WebContents* page = ShownPage(window_);
    if (!page || !page->GetDelegate()) {
      LetGo(before, down, "no shown page");
      return;
    }
    const input::NativeWebKeyboardEvent key(down);
    if (before) {
      g_asking = &key;
      const content::KeyboardEventProcessingResult result =
          page->GetDelegate()->PreHandleKeyboardEvent(page, key);
      KeyGate(key, result == content::KeyboardEventProcessingResult::HANDLED ? "asked: handled"
                   : result == content::KeyboardEventProcessingResult::NOT_HANDLED
                       ? "asked: not handled"
                   : result == content::KeyboardEventProcessingResult::NOT_HANDLED_IS_SHORTCUT
                       ? "asked: not handled, a shortcut"
                       : "asked: another answer");
      g_asking = nullptr;
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

  // A press on the dialog: one on an agent's tab's, such as answering its warning before http,
  // acts on the agent's tab, so the app takes the tab over as a press in its page does (#3243).
  // The dialog still gets it.
  void OnMouseEvent(ui::MouseEvent* event) override {
    if (event->type() == ui::EventType::kMousePressed && event->phase() == ui::EP_PRETARGET) {
      Window::DialogPressed(window_, dialog_);
    }
  }
  // A finger on a touch screen, which views gives the dialog as a touch, not a mouse press.
  void OnTouchEvent(ui::TouchEvent* event) override {
    if (event->type() == ui::EventType::kTouchPressed && event->phase() == ui::EP_PRETARGET) {
      Window::DialogPressed(window_, dialog_);
    }
  }

  // The page |page| took the keyboard: if this dialog shows over it, the dialog takes it back, as
  // Chrome's dialog manager does on a key or wheel the page ignored. A click the page ignores isn't
  // reported, and on the Mac AppKit makes the clicked page view first responder all the same, as
  // Windows gives a pane's HWND the keyboard (platform/win/window_views.cc). Only |page|'s own
  // dialog: an agent's tab a tile shows may have one in the window over another tab's page.
  void PageFocused(content::WebContents* page) {
    tabs::TabDialogManager* dialogs = Window::DialogsOf(page);
    if (!dialog_->IsVisible() || ShownPage(window_) != page || !dialogs ||
        !dialogs->IsDialogManaged(dialog_)) {
      return;
    }
    // Posted, so it's after AppKit's click and once the window says it's active: AppKit may tell
    // the page before the window, and a window that isn't active doesn't take the keyboard.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(
            [](base::WeakPtr<views::Widget> dialog, base::WeakPtr<views::Widget> window) {
              if (!dialog || !window || dialog->IsClosed() || !dialog->IsVisible() ||
                  !window->IsActive()) {
                return;
              }
              if (views::WidgetDelegate* delegate = dialog->widget_delegate()) {
                if (views::View* first = delegate->GetInitiallyFocusedView()) {
                  first->RequestFocus();
                }
              }
              dialog->Activate();
              fprintf(stderr, "browser: tab dialog took the keyboard back\n");
            },
            dialog_->GetWeakPtr(), window_->GetWeakPtr()));
  }

#if !BUILDFLAG(IS_MAC)
  void PageViewFocused(gfx::NativeView view) {
    content::WebContents* page = ShownPage(window_);
    if (!page || !view) {
      return;
    }
    // Aura focuses the window holding the page when the page's own view, ignoring input under the
    // dialog, won't take it, as the pane's on Windows.
    gfx::NativeView page_view = page->GetNativeView();
    if (page_view->Contains(view) || view->Contains(page_view)) {
      PageFocused(page);
    }
  }
#endif

  // views::WidgetObserver:
  void OnWidgetDestroying(views::Widget* widget) override {
    // Erasing deletes this, so it's the last thing done.
    Watched().erase(widget);
  }

 private:
  // Says why a shortcut's key wasn't asked about, once a key as it goes into the dialog, so a
  // shortcut that doesn't reach the app from a dialog names its gate (#3230). Only a key held
  // with ⌃ or ⌘: one typed as text in the dialog's field may be a password's.
  static void LetGo(bool before, const ui::KeyEvent& key, const char* why) {
    if (!before || !(key.flags() & (ui::EF_CONTROL_DOWN | ui::EF_COMMAND_DOWN))) {
      return;
    }
    fprintf(stderr, "browser: tab dialog let %s%s%s%skey 0x%x go: %s\n",
            key.IsControlDown() ? "ctrl-" : "", key.IsAltDown() ? "alt-" : "",
            key.IsShiftDown() ? "shift-" : "", key.IsCommandDown() ? "cmd-" : "",
            static_cast<unsigned>(key.key_code()), why);
  }

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

void KeyGate(const input::NativeWebKeyboardEvent& key, const char* why) {
  const int held = key.GetModifiers();
  if (!Driven() || &key != g_asking ||
      !(held & (blink::WebInputEvent::kControlKey | blink::WebInputEvent::kMetaKey))) {
    return;
  }
  fprintf(stderr, "browser: tab dialog's %s, type %d: %s\n",
          Chord(held & blink::WebInputEvent::kControlKey, held & blink::WebInputEvent::kAltKey,
                held & blink::WebInputEvent::kShiftKey, held & blink::WebInputEvent::kMetaKey,
                key.windows_key_code)
              .c_str(),
          static_cast<int>(key.GetType()), why);
}

void PageFocused(content::WebContents* page) {
  for (auto& watched : Watched()) {
    watched.second->PageFocused(page);
  }
}

#if !BUILDFLAG(IS_MAC)
void PageViewFocused(gfx::NativeView view) {
  for (auto& watched : Watched()) {
    watched.second->PageViewFocused(view);
  }
}
#endif

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
    // What a check waits on before it presses a key in a tab's dialog (#3230).
    fprintf(stderr, "browser: tab dialog's keys heard\n");
  }
}

}  // namespace browser_core::dialog_keys
