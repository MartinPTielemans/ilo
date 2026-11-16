#include "browser_core/platform/linux/ui_text_input.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/i18n/rtl.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/window.h"
#include "ui/aura/client/focus_client.h"
#include "ui/aura/window.h"
#include "ui/base/ime/composition_text.h"
#include "ui/base/ime/input_method.h"
#include "ui/base/ime/text_input_flags.h"
#include "ui/events/event.h"
#include "ui/events/keycodes/keyboard_codes.h"
#include "ui/gfx/range/range.h"
#include "ui/views/widget/widget.h"

namespace browser_core::platform {

namespace {

aura::Window* FocusedWindowIn(views::Widget* widget) {
  auto* client = aura::client::GetFocusClient(widget->GetNativeView());
  return client ? client->GetFocusedWindow() : nullptr;
}

}  // namespace

UITextInput::UITextInput(views::Widget* widget) : widget_(widget) {}

// The window detached it as its widget went (SetKeyboard(false)).
UITextInput::~UITextInput() = default;

void UITextInput::SetCaret(std::optional<gfx::Rect> caret) {
  const bool moved = caret_ && caret && *caret_ != *caret;
  caret_ = caret;
  Update();
  ui::InputMethod* ime = widget_->GetInputMethod();
  if (moved && attached_ && ime && ime->GetTextInputClient() == this) {
    ime->OnCaretBoundsChanged(this);
  }
}

void UITextInput::SetKeyboard(bool ui) {
  keyboard_ = ui;
  // The field that took the keys went before its caret came.
  if (!ui && hold_ == Hold::kCaret) {
    EndHold();
    return;
  }
  Update();
}

void UITextInput::HoldForPage(bool hold, bool claimed) {
  if (hold) {
    ui::InputMethod* ime = widget_->GetInputMethod();
    ui::TextInputClient* page = ime ? ime->GetTextInputClient() : nullptr;
    // A page with no field focused gets the keys themselves, which the hold has. One composing
    // keeps the input method too: the page would commit its composition as it lost it, which v3
    // can't reset, so the composition would carry on here and the page would get it twice.
    if (attached_ || !page || page->GetTextInputType() == ui::TEXT_INPUT_TYPE_NONE ||
        page->HasCompositionText()) {
      return;
    }
    hold_ = Hold::kAnswer;
    Update();
    return;
  }
  if (hold_ == Hold::kNone) {
    return;
  }
  if (claimed && keyboard_ && !caret_) {
    hold_ = Hold::kCaret;
    return;
  }
  EndHold();
}

void UITextInput::EndHold() {
  hold_ = Hold::kNone;
  if (ToField()) {
    Update();
    return;
  }
  base::WeakPtr<ui::TextInputClient> page = page_;
  const std::u16string kept = std::exchange(kept_, std::u16string());
  const std::u16string composition = std::exchange(composition_, std::u16string());
  composition_kept_ = false;
  Update();
  ui::InputMethod* ime = widget_->GetInputMethod();
  if (!page || !ime || ime->GetTextInputClient() != page.get()) {
    return;
  }
  if (!kept.empty()) {
    page->InsertText(kept, ui::TextInputClient::InsertTextCursorBehavior::kMoveCursorAfterText);
  }
  if (!composition.empty()) {
    ui::CompositionText text;
    text.text = composition;
    text.selection = gfx::Range(composition.size());
    page->SetCompositionText(text);
  }
}

void UITextInput::Update() {
  if (hold_ != Hold::kNone && ToField()) {
    hold_ = Hold::kNone;
  }
  const bool wanted = ToField() || hold_ != Hold::kNone;
  if (wanted && !attached_) {
    Attach();
  } else if (!wanted && attached_) {
    Detach();
  }
  if (attached_ && ToField()) {
    GiveKept();
  }
}

void UITextInput::GiveKept() {
  if (!kept_.empty()) {
    Committed(std::exchange(kept_, std::u16string()));
  }
  if (std::exchange(composition_kept_, false)) {
    Composed(composition_);
  }
}

void UITextInput::Attach() {
  ui::InputMethod* ime = widget_->GetInputMethod();
  if (!ime) {
    return;
  }
  attached_ = true;
  page_window_.RemoveAll();
  page_.reset();
  if (ui::TextInputClient* page = ime->GetTextInputClient()) {
    // The input method drops what the page composed as the field it composes for changes, where a
    // page keeps it as it loses the focus.
    page->ConfirmCompositionText(/*keep_selection=*/false);
    page_ = page->AsWeakPtr();
    if (aura::Window* window = FocusedWindowIn(widget_)) {
      page_window_.Add(window);
    }
  }
  ime->SetFocusedTextInputClient(this);
  ime->OnCaretBoundsChanged(this);
}

void UITextInput::Detach() {
  attached_ = false;
  // What the UI's field composed stays in it, as a page's field keeps it losing the focus. Told
  // after the call that detached it, which may be the app's own.
  if (!composition_.empty()) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&UITextInput::Committed, weak_factory_.GetWeakPtr(),
                                  std::exchange(composition_, std::u16string())));
  }
  ui::InputMethod* ime = widget_->GetInputMethod();
  // A page that took the focus took the input method already.
  if (!ime || ime->GetTextInputClient() != this) {
    return;
  }
  const bool page_focused = page_ && !page_window_.windows().empty() &&
                            FocusedWindowIn(widget_) == page_window_.windows().front();
  ime->SetFocusedTextInputClient(page_focused ? page_.get() : nullptr);
  page_.reset();
  page_window_.RemoveAll();
}

void UITextInput::Composed(const std::u16string& text) {
  if (!Host().ui_composed) {
    return;
  }
  const std::string utf8 = base::UTF16ToUTF8(text);
  for (Window* window : Window::All()) {
    if (window->widget() == widget_) {
      Host().ui_composed(Host().context, window->id(), utf8.c_str());
      return;
    }
  }
}

void UITextInput::Committed(const std::u16string& text) {
  if (!Host().ui_committed) {
    return;
  }
  const std::string utf8 = base::UTF16ToUTF8(text);
  for (Window* window : Window::All()) {
    if (window->widget() == widget_) {
      Host().ui_committed(Host().context, window->id(), utf8.c_str());
      return;
    }
  }
}

base::WeakPtr<ui::TextInputClient> UITextInput::AsWeakPtr() {
  return weak_factory_.GetWeakPtr();
}

void UITextInput::SetCompositionText(const ui::CompositionText& composition) {
  composition_ = composition.text;
  if (ToField()) {
    Composed(composition_);
  } else {
    composition_kept_ = true;
  }
}

size_t UITextInput::ConfirmCompositionText(bool) {
  const size_t length = composition_.size();
  if (length) {
    // InsertText clears composition_ first, so it gets the text by value.
    InsertText(std::exchange(composition_, std::u16string()),
               InsertTextCursorBehavior::kMoveCursorAfterText);
  }
  return length;
}

void UITextInput::ClearCompositionText() {
  if (!composition_.empty()) {
    composition_.clear();
    if (ToField()) {
      Composed(composition_);
    } else {
      composition_kept_ = false;
    }
  }
}

void UITextInput::InsertText(const std::u16string& text, InsertTextCursorBehavior) {
  composition_.clear();
  if (ToField()) {
    Committed(text);
  } else {
    kept_ += text;
    composition_kept_ = false;
  }
}

void UITextInput::InsertChar(const ui::KeyEvent& event) {
  // A key the input method let through reaches the app's UI as a key too, which types it; only
  // the character it committed for a key it took (VKEY_PROCESSKEY) is the UI's to insert here.
  if (event.key_code() == ui::VKEY_PROCESSKEY && event.GetCharacter()) {
    InsertText(std::u16string(1, event.GetCharacter()),
               InsertTextCursorBehavior::kMoveCursorAfterText);
  }
}

ui::TextInputType UITextInput::GetTextInputType() const {
  // Held for the field, it's one before the field has its caret.
  return caret_ || hold_ != Hold::kNone ? ui::TEXT_INPUT_TYPE_TEXT : ui::TEXT_INPUT_TYPE_NONE;
}

ui::TextInputMode UITextInput::GetTextInputMode() const {
  return ui::TEXT_INPUT_MODE_DEFAULT;
}

base::i18n::TextDirection UITextInput::GetTextDirection() const {
  return base::i18n::UNKNOWN_DIRECTION;
}

int UITextInput::GetTextInputFlags() const {
  return ui::TEXT_INPUT_FLAG_NONE;
}

bool UITextInput::CanComposeInline() const {
  return true;
}

gfx::Rect UITextInput::GetCaretBounds() const {
  gfx::Rect caret = caret_.value_or(gfx::Rect());
  caret.Offset(
      widget_->GetNativeWindow()->GetRootWindow()->GetBoundsInScreen().OffsetFromOrigin());
  return caret;
}

gfx::Rect UITextInput::GetSelectionBoundingBox() const {
  return GetCaretBounds();
}

bool UITextInput::GetCompositionCharacterBounds(size_t, gfx::Rect*) const {
  return false;
}

bool UITextInput::HasCompositionText() const {
  return !composition_.empty();
}

ui::TextInputClient::FocusReason UITextInput::GetFocusReason() const {
  return FOCUS_REASON_OTHER;
}

// The field's text stays the app's: the input method gets no surrounding text.
bool UITextInput::GetTextRange(gfx::Range*) const {
  return false;
}

bool UITextInput::GetCompositionTextRange(gfx::Range*) const {
  return false;
}

bool UITextInput::GetEditableSelectionRange(gfx::Range*) const {
  return false;
}

bool UITextInput::SetEditableSelectionRange(const gfx::Range&) {
  return false;
}

bool UITextInput::GetTextFromRange(const gfx::Range&, std::u16string*) const {
  return false;
}

void UITextInput::OnInputMethodChanged() {}

bool UITextInput::ChangeTextDirectionAndLayoutAlignment(base::i18n::TextDirection) {
  return false;
}

void UITextInput::ExtendSelectionAndDelete(size_t, size_t) {}

void UITextInput::EnsureCaretNotInRect(const gfx::Rect&) {}

bool UITextInput::IsTextEditCommandEnabled(ui::TextEditCommand) const {
  return false;
}

void UITextInput::SetTextEditCommandForNextKeyEvent(ui::TextEditCommand) {}

ukm::SourceId UITextInput::GetClientSourceForMetrics() const {
  return ukm::kInvalidSourceId;
}

bool UITextInput::ShouldDoLearning() {
  return false;
}

// GPUI tells the core only where the caret is, not the field's bounds, so neither is known.
void UITextInput::GetActiveTextInputControlLayoutBounds(std::optional<gfx::Rect>*,
                                                        std::optional<gfx::Rect>*) {}

bool UITextInput::SetCompositionFromExistingText(const gfx::Range&,
                                                 const std::vector<ui::ImeTextSpan>&) {
  return false;
}

}  // namespace browser_core::platform
