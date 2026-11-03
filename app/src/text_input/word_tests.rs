//! A field's word and line keys and its double- and triple-click, as the user presses them (#1385).

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

struct Shows(Entity<TextInput>);

impl Render for Shows {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone())
    }
}

/// A field holding `text`, focused, with the caret at its end, taking `keys`' platform's keys.
struct Field {
    input: Entity<TextInput>,
    cx: VisualTestContext,
}

impl Field {
    fn new(text: &str, keys: Keys, secure: bool, cx: &mut TestAppContext) -> Self {
        cx.update(|cx| bind_keys_for(keys, cx));
        let (shows, cx) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| TextInput::new("Search or Enter URL…", cx).secure(secure));
            window.focus(&input.focus_handle(cx), cx);
            Shows(input)
        });
        let input = cx.read(|cx| shows.read(cx).0.clone());
        cx.simulate_input(text);
        cx.run_until_parked();
        Self { input, cx: cx.clone() }
    }

    fn mac(text: &str, cx: &mut TestAppContext) -> Self {
        Self::new(text, Keys::Mac, false, cx)
    }

    /// Presses `keys` and gives the selection, as `start..end`, with the caret first where it's
    /// at the start.
    fn press(&mut self, keys: &str) -> (usize, usize) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
        self.selected()
    }

    fn selected(&self) -> (usize, usize) {
        self.input.read_with(&self.cx, |input, _| {
            let range = input.selection();
            match input.selection_reversed {
                true => (range.end, range.start),
                false => (range.start, range.end),
            }
        })
    }

    fn text(&self) -> String {
        self.input.read_with(&self.cx, |input, _| input.text().to_string())
    }

    /// Where `fraction` of the way across the character at `index` is in the window.
    fn over(&mut self, index: usize, fraction: f32) -> Point<Pixels> {
        self.input.update_in(&mut self.cx, |input, _, _| {
            let (bounds, line) = (input.last_bounds.unwrap(), input.last_layout.clone().unwrap());
            let (left, right) = (line.x_for_index(index), line.x_for_index(index + 1));
            let x = bounds.left() + left + (right - left) * fraction - input.scroll;
            point(x, bounds.center().y)
        })
    }

    /// Clicks `clicks` times in a row over the middle of the character at `index`.
    fn click(&mut self, index: usize, clicks: usize) -> Range<usize> {
        let position = self.over(index, 0.5);
        self.press_at(position, clicks, Modifiers::default());
        self.release(position, clicks);
        self.input.read_with(&self.cx, |input, _| input.selection())
    }

    fn press_at(&mut self, position: Point<Pixels>, clicks: usize, modifiers: Modifiers) {
        let button = MouseButton::Left;
        let (click_count, first_mouse) = (clicks, false);
        let down = MouseDownEvent { button, position, modifiers, click_count, first_mouse };
        self.cx.simulate_event(down);
        self.cx.run_until_parked();
    }

    fn release(&mut self, position: Point<Pixels>, click_count: usize) {
        let (button, modifiers) = (MouseButton::Left, Modifiers::default());
        self.cx.simulate_event(MouseUpEvent { button, position, modifiers, click_count });
        self.cx.run_until_parked();
    }

    fn drag_to(&mut self, position: Point<Pixels>) -> (usize, usize) {
        let pressed_button = Some(MouseButton::Left);
        let modifiers = Modifiers::default();
        self.cx.simulate_event(MouseMoveEvent { position, pressed_button, modifiers });
        self.cx.run_until_parked();
        self.selected()
    }

    /// Starts an input method composing `text` at the caret, as Japanese input does before the
    /// user commits it.
    fn compose(&mut self, text: &str) {
        self.input.update_in(&mut self.cx, |input, window, cx| {
            input.replace_and_mark_text_in_range(None, text, None, window, cx)
        });
        self.cx.run_until_parked();
    }

    fn commit(&mut self) {
        self.input.update_in(&mut self.cx, |input, window, cx| input.unmark_text(window, cx));
        self.cx.run_until_parked();
    }
}

/// ⌥← and ⌥→ go a word, by the stops a Mac field makes, and with ⇧ the caret's end of the
/// selection goes a word while the other stays.
#[gpui::test]
fn option_arrows_move_by_words_and_with_shift_select_them(cx: &mut TestAppContext) {
    let mut field = Field::mac("example.com/some/path", cx);
    assert_eq!(field.press("alt-left"), (17, 17));
    assert_eq!(field.press("alt-left"), (12, 12));
    assert_eq!(field.press("alt-left"), (0, 0));
    assert_eq!(field.press("alt-left"), (0, 0), "at the start it stays");
    assert_eq!(field.press("alt-right"), (11, 11));
    assert_eq!(field.press("alt-right"), (16, 16));
    assert_eq!(field.press("shift-alt-right"), (16, 21));
    assert_eq!(field.press("shift-alt-left"), (16, 17));
    assert_eq!(field.press("shift-alt-left"), (16, 12), "past where it began, it turns");
    assert_eq!(field.press("shift-alt-left"), (16, 0));
    // A move without ⇧ goes a word from the selection's end the way it goes.
    assert_eq!(field.press("alt-right"), (21, 21));
    assert_eq!(field.press("shift-alt-left shift-alt-left alt-left"), (0, 0));
    assert_eq!(field.text(), "example.com/some/path");
}

/// ⇧⌘← and ⇧⌘→ select to an end, as ⇧Home and ⇧End do, and ⌃A and ⌃E go there, with ⇧
/// selecting, as Emacs's keys do in any Mac field.
#[gpui::test]
fn cmd_shift_arrows_and_emacs_keys_go_to_the_ends(cx: &mut TestAppContext) {
    let mut field = Field::mac("hello world", cx);
    assert_eq!(field.press("ctrl-a"), (0, 0));
    assert_eq!(field.press("ctrl-e"), (11, 11));
    assert_eq!(field.press("alt-left"), (6, 6));
    assert_eq!(field.press("shift-cmd-left"), (6, 0));
    assert_eq!(field.press("shift-cmd-right"), (6, 11), "from where the selection began");
    assert_eq!(field.press("ctrl-shift-a"), (6, 0));
    assert_eq!(field.press("ctrl-shift-e"), (6, 11));
    assert_eq!(field.press("shift-home"), (6, 0));
    assert_eq!(field.press("shift-end"), (6, 11));
}

/// ⌥⌦ deletes to the end of the word after the caret, or the selection where there is one.
#[gpui::test]
fn option_forward_delete_deletes_the_word_after_the_caret(cx: &mut TestAppContext) {
    let mut field = Field::mac("example.com/some/path", cx);
    field.press("ctrl-a alt-delete");
    assert_eq!(field.text(), "/some/path");
    field.press("alt-delete");
    assert_eq!(field.text(), "/path");
    field.press("alt-right alt-delete");
    assert_eq!(field.text(), "/path", "at the end there's nothing after the caret");
    field.press("cmd-a alt-delete");
    assert_eq!(field.text(), "");
}

/// On the Mac a selection made whole, by a double-click or Select All, grows the way the first key
/// goes, where a selection made by keys moves the end the caret's at.
#[gpui::test]
fn a_whole_selection_grows_the_way_the_key_goes(cx: &mut TestAppContext) {
    let mut field = Field::mac("one two three", cx);
    assert_eq!(field.click(5, 2), 4..7);
    assert_eq!(field.press("shift-alt-left"), (7, 0));
    assert_eq!(field.click(5, 2), 4..7);
    assert_eq!(field.press("shift-alt-right"), (4, 13));
    assert_eq!(field.press("cmd-a shift-left"), (13, 0), "it can't grow past the start");
    assert_eq!(field.press("shift-right"), (13, 1), "now it's the caret's end that moves");
}

/// A double-click selects the word under it, or the run of spaces or the mark, and a
/// triple-click the whole field, as its one line.
#[gpui::test]
fn a_double_click_selects_a_word_and_a_triple_click_the_line(cx: &mut TestAppContext) {
    let mut field = Field::mac("hello world  foo", cx);
    assert_eq!(field.click(7, 2), 6..11);
    assert_eq!(field.click(11, 2), 11..13);
    assert_eq!(field.click(14, 3), 0..16);
    assert!(field.click(2, 1).is_empty(), "one click puts the caret down");
    // Over the right half of hello's "o" it's still hello's, not the space after it.
    let position = field.over(4, 0.9);
    field.press_at(position, 2, Modifiers::default());
    field.release(position, 2);
    assert_eq!(field.selected(), (0, 5));
    let mut url = Field::mac("https://www.example.com/a?b=c", cx);
    assert_eq!(url.click(9, 2), 8..23);
    assert_eq!(url.click(5, 2), 5..6);
}

/// A drag from a double-click selects whole words, both ways from the one it started on.
#[gpui::test]
fn a_drag_from_a_double_click_selects_by_words(cx: &mut TestAppContext) {
    let mut field = Field::mac("hello world  foo", cx);
    let world = field.over(7, 0.5);
    field.press_at(world, 2, Modifiers::default());
    let foo = field.over(14, 0.5);
    assert_eq!(field.drag_to(foo), (6, 16));
    let hello = field.over(1, 0.5);
    assert_eq!(field.drag_to(hello), (11, 0));
    field.release(hello, 2);
    assert_eq!(field.drag_to(foo), (11, 0), "after the release it's done");
}

/// A password field gives away no word lengths: its word moves go to its ends, ⌥⌫ deletes all of
/// it, and a double-click selects all of it, as a Mac's does.
#[gpui::test]
fn a_secure_field_moves_and_selects_whole(cx: &mut TestAppContext) {
    let mut field = Field::new("hunter2 is mine", Keys::Mac, true, cx);
    assert_eq!(field.press("alt-left"), (0, 0));
    assert_eq!(field.press("alt-right"), (15, 15));
    assert_eq!(field.click(3, 2), 0..15);
    field.press("alt-right alt-backspace");
    assert_eq!(field.text(), "");
}

/// While an input method composes, the word and line keys it leaves the field change nothing, and
/// once it commits they go by the words it wrote.
#[gpui::test]
fn an_input_methods_composition_keeps_its_text_from_word_keys(cx: &mut TestAppContext) {
    let mut field = Field::mac("abc ", cx);
    field.compose("てきすと");
    let composing = |field: &Field| {
        field.input.read_with(&field.cx, |input, _| (input.selection(), input.marked_range.clone()))
    };
    let before = composing(&field);
    let keys = "alt-left alt-right shift-alt-left shift-alt-right shift-cmd-left shift-cmd-right \
        ctrl-a ctrl-e ctrl-shift-a shift-home cmd-left left right shift-left alt-delete";
    for key in keys.split_whitespace() {
        field.press(key);
        assert_eq!((field.text(), composing(&field)), ("abc てきすと".into(), before.clone()), "{key}");
    }
    assert!(field.click(1, 2).is_empty(), "a double-click is one click while it composes");
    field.compose("テキスト");
    field.commit();
    assert_eq!(field.text(), "abc テキスト");
    assert_eq!(field.press("cmd-right alt-left"), (4, 4));
    assert_eq!(field.press("alt-left alt-right"), (3, 3));
}

/// On Linux Ctrl moves and deletes by words, to a word's end going right, as GTK's and Chrome's
/// fields there do, and Alt+← stays the browser's Back.
#[gpui::test]
fn ctrl_moves_by_words_on_linux(cx: &mut TestAppContext) {
    let mut field = Field::new("hello world  foo", Keys::Linux, false, cx);
    assert_eq!(field.press("ctrl-left"), (13, 13));
    assert_eq!(field.press("ctrl-left"), (6, 6));
    assert_eq!(field.press("ctrl-right"), (11, 11));
    assert_eq!(field.press("ctrl-shift-right"), (11, 16));
    assert_eq!(field.press("ctrl-shift-left ctrl-shift-left"), (11, 6));
    assert_eq!(field.press("alt-left"), (11, 6), "Alt+← isn't the field's");
    field.press("home ctrl-delete");
    assert_eq!(field.text(), " world  foo");
    field.press("end ctrl-backspace");
    assert_eq!(field.text(), " world  ");
}

/// On Windows Ctrl+→ goes to the start of the next word and Ctrl+Delete deletes up to it, as
/// Windows' own fields and Chrome's there do.
#[gpui::test]
fn ctrl_right_goes_to_the_next_words_start_on_windows(cx: &mut TestAppContext) {
    let mut field = Field::new("hello world  foo", Keys::Windows, false, cx);
    assert_eq!(field.press("home ctrl-right"), (6, 6));
    assert_eq!(field.press("ctrl-right"), (13, 13));
    assert_eq!(field.press("ctrl-right"), (16, 16));
    assert_eq!(field.press("ctrl-left"), (13, 13));
    assert_eq!(field.press("home ctrl-shift-right"), (0, 6));
    field.press("home ctrl-delete");
    assert_eq!(field.text(), "world  foo");
}
