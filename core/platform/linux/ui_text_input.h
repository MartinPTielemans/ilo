// A text field of the app's UI as the browser window's input method sees it. Under Wayland the
// compositor sends a client's input method to one of its text inputs, the one Chromium's window
// holds, so the UI's fields compose through Chromium's input method, as a page's do
// (bc_window_ui_text_input), and the app draws the composition.

#ifndef BROWSER_CORE_PLATFORM_LINUX_UI_TEXT_INPUT_H_
#define BROWSER_CORE_PLATFORM_LINUX_UI_TEXT_INPUT_H_

#include <optional>
#include <string>
#include <vector>

#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "ui/aura/window_tracker.h"
#include "ui/base/ime/text_input_client.h"
#include "ui/gfx/geometry/rect.h"

namespace views {
class Widget;
}

namespace browser_core::platform {

class UITextInput : public ui::TextInputClient {
 public:
  explicit UITextInput(views::Widget* widget);
  UITextInput(const UITextInput&) = delete;
  UITextInput& operator=(const UITextInput&) = delete;
  ~UITextInput() override;

  // The UI's focused field's caret, in DIPs from the window's top left, or none focused.
  void SetCaret(std::optional<gfx::Rect> caret);
  // Whether the keys are the UI's or an overlay's of it, rather than a page's.
  void SetKeyboard(bool ui);
  // Whether the input method's client is the UI's field.
  bool attached() const { return attached_; }

  // ui::TextInputClient:
  base::WeakPtr<ui::TextInputClient> AsWeakPtr() override;
  void SetCompositionText(const ui::CompositionText& composition) override;
  size_t ConfirmCompositionText(bool keep_selection) override;
  void ClearCompositionText() override;
  void InsertText(const std::u16string& text, InsertTextCursorBehavior cursor_behavior) override;
  void InsertChar(const ui::KeyEvent& event) override;
  ui::TextInputType GetTextInputType() const override;
  ui::TextInputMode GetTextInputMode() const override;
  base::i18n::TextDirection GetTextDirection() const override;
  int GetTextInputFlags() const override;
  bool CanComposeInline() const override;
  gfx::Rect GetCaretBounds() const override;
  gfx::Rect GetSelectionBoundingBox() const override;
  bool GetCompositionCharacterBounds(size_t index, gfx::Rect* rect) const override;
  bool HasCompositionText() const override;
  FocusReason GetFocusReason() const override;
  bool GetTextRange(gfx::Range* range) const override;
  bool GetCompositionTextRange(gfx::Range* range) const override;
  bool GetEditableSelectionRange(gfx::Range* range) const override;
  bool SetEditableSelectionRange(const gfx::Range& range) override;
  bool GetTextFromRange(const gfx::Range& range, std::u16string* text) const override;
  void OnInputMethodChanged() override;
  bool ChangeTextDirectionAndLayoutAlignment(base::i18n::TextDirection direction) override;
  void ExtendSelectionAndDelete(size_t before, size_t after) override;
  void EnsureCaretNotInRect(const gfx::Rect& rect) override;
  bool IsTextEditCommandEnabled(ui::TextEditCommand command) const override;
  void SetTextEditCommandForNextKeyEvent(ui::TextEditCommand command) override;
  ukm::SourceId GetClientSourceForMetrics() const override;
  bool ShouldDoLearning() override;
  void GetActiveTextInputControlLayoutBounds(std::optional<gfx::Rect>* control_bounds,
                                             std::optional<gfx::Rect>* selection_bounds) override;
  bool SetCompositionFromExistingText(const gfx::Range& range,
                                      const std::vector<ui::ImeTextSpan>& spans) override;

 private:
  // The input method composes for the UI's field while it's focused and has the keys, and for the
  // page that had them before once it hasn't.
  void Update();
  void Attach();
  void Detach();
  void Composed(const std::u16string& text);
  void Committed(const std::u16string& text);

  raw_ptr<views::Widget> widget_;
  std::optional<gfx::Rect> caret_;
  bool keyboard_ = false;
  bool attached_ = false;
  std::u16string composition_;
  // The page's field the input method composed for before, and the page's window, which has it
  // back if it still has the focus: an overlay lends the keys without taking the page's focus.
  base::WeakPtr<ui::TextInputClient> page_;
  aura::WindowTracker page_window_;
  base::WeakPtrFactory<UITextInput> weak_factory_{this};
};

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_LINUX_UI_TEXT_INPUT_H_
