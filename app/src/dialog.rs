//! The dialog that asks before something hard to take back, such as deleting a Space with its
//! tabs, or before quitting: a card over the window, which dims, as Arc's (docs/ux-spec.md ›
//! Spaces › Renaming and deleting, and Warn before quitting). GPUI can't draw over the page's
//! native view, so it's a window of its own that the host shows over all of the browser window,
//! as its child, with the keyboard. What it asks of its window goes through `Host`. It also asks
//! Chrome's questions, such as whether to add an extension, which wait their turn behind the
//! dialog that shows and which Chrome can take back, the account's, such as whether to add a
//! device, which wait and go the same way, and a page's own, such as its alert(), over
//! its tab's card alone, which come and go with the tab (docs/ux-spec.md › Page dialogs).

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::Duration,
};
use web_time::Instant;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, Div, Entity, FocusHandle, Focusable, FontWeight,
    Global, Hsla, Image, KeyBinding, MouseButton, Pixels, Point, SharedString, Stateful, Task,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions,
    accesskit::{Role, Toggled},
    actions, canvas, div, img, point,
    prelude::*,
    px, rgb, rgba, size, svg,
};

use crate::{
    palette::{self, STATE_PRESSED},
    settling::Settling,
    shell::cubic_bezier,
    text_input::TextInput,
};

actions!(dialog, [Confirm, Cancel, NextField]);

use crate::elevation::Level;
use crate::tokens::dialog::ACTION;
use crate::tokens::dialog::ACTION_HOVERED;
use crate::tokens::dialog::ACTION_LEDGE;
use crate::tokens::dialog::ALSO_GAP;
use crate::tokens::dialog::BUTTON_HEIGHT;
use crate::tokens::dialog::BUTTON_INSET;
use crate::tokens::dialog::BUTTON_RADIUS;
use crate::tokens::dialog::BUTTONS_TOP;
use crate::tokens::dialog::BUTTONS_UNDER_TITLE;
use crate::tokens::dialog::CANCEL_BORDER;
use crate::tokens::dialog::CANCEL_FILL;
use crate::tokens::dialog::CANCEL_HOVERED;
use crate::tokens::dialog::CANCEL_TEXT;
use crate::tokens::dialog::CHIP_END;
use crate::tokens::dialog::CHIP_GAP;
use crate::tokens::dialog::CHIP_HEIGHT;
use crate::tokens::dialog::CHIP_RADIUS;
use crate::tokens::dialog::DETAIL;
use crate::tokens::dialog::DETAIL_LINE;
use crate::tokens::dialog::DETAIL_TOP;
use crate::tokens::dialog::DIM;
use crate::tokens::dialog::DIM_OPACITY;
use crate::tokens::dialog::ESC_FILL;
use crate::tokens::dialog::ESC_TEXT;
use crate::tokens::dialog::ICON;
use crate::tokens::dialog::ICON_GAP;
use crate::tokens::dialog::LABEL_X;
use crate::tokens::dialog::LIST_GAP;
use crate::tokens::dialog::LIST_TOP;
use crate::tokens::dialog::RADIUS;
use crate::tokens::dialog::RETURN_BORDER;
use crate::tokens::dialog::RETURN_FILL;
use crate::tokens::dialog::RETURN_GLYPH;
use crate::tokens::dialog::RETURN_WIDTH;
use crate::tokens::dialog::SUBLINE;
use crate::tokens::dialog::SUBLINE_INDENT;
use crate::tokens::dialog::SUBLINE_TEXT;
use crate::tokens::dialog::TEXT;
use crate::tokens::dialog::TEXT_INSET;
use crate::tokens::dialog::TITLE_LINE;
use crate::tokens::dialog::WIDTH;
use crate::tokens::dialog::{
    CHECK_BOX, CHECK_GAP, CHECK_GLYPH, CHECK_RADIUS, CHECK_TOP, FIELD_GAP, FIELD_PLACEHOLDER,
    FIELD_TEXT, FIELDS_TOP,
};
use crate::tokens::text;
/// Arc's motion: over 0.275 s the window dims and the card comes down 32 pt into place,
/// ease-in-out, opaque after the first 0.135 s. It leaves the way it came, fading all the way.
pub(crate) const MOVE: f32 = 0.275;
const FADE_IN: f32 = 0.135;
/// Linux gives the browser window the keyboard back as the dialog starts leaving, and the Mac and
/// Windows once it has hidden: a page's question is answered then.
const PAGE_ANSWERED_AS_IT_LEAVES: bool = cfg!(target_os = "linux");
const DROP: f32 = 32.;

/// What a dialog asks.
pub struct Question {
    /// Shown over the message, such as the extension's.
    pub icon: Option<Arc<Image>>,
    pub message: SharedString,
    /// Under the message, or nothing if empty.
    pub detail: SharedString,
    /// Lines listed under the detail, each with its smaller lines under it.
    pub list: Vec<(SharedString, Vec<SharedString>)>,
    /// The button that does it, which ↩ chooses. With none, only Cancel answers.
    pub action: SharedString,
    pub cancel: SharedString,
    pub tone: Tone,
    /// A button at the left for another answer, and what it does once the dialog has gone.
    pub also: Option<(SharedString, Box<dyn FnOnce(&mut App)>)>,
    /// Fields under the detail, such as a prompt's, the first with the keyboard.
    pub fields: Vec<Field>,
    /// A checkbox under them, by its label, such as "Always allow…".
    pub check: Option<SharedString>,
    /// What ↩ answers.
    pub on_return: Return,
}

/// What ↩ answers in a dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Return {
    /// The action, whose button carries the ↩ chip.
    Action,
    /// Cancel, the action taking a click, as for a page opening an app, so a key press the page
    /// timed can't choose it, as Chrome has it.
    Cancel,
    /// Nothing, the action taking a click, for one nothing can undo that reaches past this device.
    Nothing,
}

/// A field in a dialog.
pub struct Field {
    pub placeholder: SharedString,
    /// What it starts with, all selected.
    pub text: SharedString,
    /// It shows dots for what's typed, and won't copy it, as a password field.
    pub secure: bool,
}

/// What the user answered: the action or not, what was in the fields, and the checkbox.
pub struct Reply {
    pub accept: bool,
    pub texts: Vec<String>,
    pub checked: bool,
}

impl std::fmt::Debug for Reply {
    // What was typed, such as a password, stays out of logs and test output.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Reply {{ accept: {}, fields: {}, checked: {} }}",
            self.accept,
            self.texts.len(),
            self.checked
        )
    }
}

/// The action's colour, by what it does.
#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    /// Red, as it's hard to take back, such as deleting a Space.
    Destructive,
    /// The Space's accent, as it's easily taken back or takes nothing back, such as quitting or
    /// adding an extension.
    Accent,
}

/// What the user chose.
#[derive(Clone, Copy, PartialEq)]
enum Answer {
    Action,
    Also,
    Cancel,
}

/// What the dialog asks of its window.
pub trait Host {
    /// Shows the dialog's window over all of browser window `window`, the one whose question it
    /// is, else the one in front, or over `over` in it, a tab's card, with the keyboard.
    fn show(&self, window: Option<u64>, over: Option<Bounds<Pixels>>);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
    /// The dialog was answered and leaves, so no key typed from now may be lost to it: Linux gives
    /// the browser window the keyboard back at once (overlays::dialog_leaving), and the Mac and
    /// Windows hold the keys until it has hidden (host_hold_keys).
    fn hold_keys(&self) {}
    /// Gives the keys held to the window with the keyboard, the browser window once it's hidden
    /// (host_release_held_keys).
    fn give_held_keys(&self) {}
}

/// Where the motion is: how far the window has dimmed and the card has faded in, from 0 to 1,
/// and how far above its place the card is.
#[derive(Clone, Copy)]
struct Pose {
    dim: f32,
    opacity: f32,
    raised: f32,
}

impl Pose {
    /// `seconds` after the dialog was asked for.
    fn arriving(seconds: f32) -> Self {
        let t = (seconds / MOVE).min(1.);
        Pose {
            dim: t,
            opacity: (seconds / FADE_IN).min(1.),
            raised: DROP * (1. - ease_in_out(t)),
        }
    }

    /// `seconds` after the user answered, from where the motion was then.
    fn leaving(self, seconds: f32) -> Self {
        let t = (seconds / MOVE).min(1.);
        Pose {
            dim: self.dim * (1. - t),
            opacity: self.opacity * (1. - t),
            raised: self.raised + (DROP - self.raised) * ease_in_out(t),
        }
    }
}

pub(crate) fn ease_in_out(t: f32) -> f32 {
    cubic_bezier(0.42, 0., 0.58, 1., t)
}

/// Who asked a question.
#[derive(Clone, Copy, PartialEq)]
enum Source {
    /// The browser's own, such as deleting a Space.
    User,
    /// Chrome's, such as adding an extension, which Chrome can take back.
    Chrome,
    /// The account's, such as adding a device to it, which the core can take back, as Chrome's.
    Account,
    /// A page's, over its tab's card, which goes when the tab does.
    Page,
}

impl Source {
    /// Its action grants something, so it works only after the dialog's moment on screen.
    fn grants(self) -> bool {
        matches!(self, Source::Chrome | Source::Account)
    }
}

/// A question to show, and what's told of its answer once the dialog has gone.
struct Request {
    question: Question,
    source: Source,
    /// The browser window whose question it is, as one behind another can ask from its context
    /// menu (#1432), else None for the app's, which go over the one in front.
    window: Option<u64>,
    /// The tab's card a page's question goes over, and the radius of its corners.
    over: Option<(Bounds<Pixels>, f32)>,
    /// A page's question's tab and id, by which it's asked again as its card moves.
    page: Option<(u64, u64)>,
    answered: Box<dyn FnOnce(Reply, &mut App)>,
}

struct Asked {
    /// The question, its other answer taken out into `also` and `also_chosen`.
    question: Question,
    also: Option<SharedString>,
    also_chosen: Option<Box<dyn FnOnce(&mut App)>>,
    source: Source,
    over: Option<(Bounds<Pixels>, f32)>,
    page: Option<(u64, u64)>,
    fields: Vec<Entity<TextInput>>,
    checked: bool,
    answered: Option<Box<dyn FnOnce(Reply, &mut App)>>,
    /// When its window was first on screen, where the motion starts.
    shown: Option<Instant>,
    settled: bool,
    /// Its moment on screen before the action of Chrome's question, which grants something, works.
    settling: Settling,
    /// Once the user has answered, or Chrome took it back: when, and where the motion was.
    answered_at: Option<(Instant, Pose)>,
    _leave: Option<Task<()>>,
    /// The button under the pointer, and the one the last frame drew so.
    hovered: Option<&'static str>,
    drawn_hover: Option<&'static str>,
    /// The button held down, and the one the last frame drew so.
    pressed: Option<&'static str>,
    drawn_press: Option<&'static str>,
    /// Where each button was said to be drawn, in the browser window.
    buttons_drawn: Rc<RefCell<HashMap<&'static str, Point<Pixels>>>>,
    /// Where the card, the checkbox and its label were drawn last, in the dialog's window.
    check_drawn: Rc<Cell<CheckDrawn>>,
}

/// The card's bounds, its checkbox's and the checkbox's label's, as drawn.
#[derive(Clone, Copy, Default, Debug)]
struct CheckDrawn {
    card: Bounds<Pixels>,
    check: Bounds<Pixels>,
    label: Bounds<Pixels>,
}

/// Puts the bounds it's drawn at into `drawn` by `put`.
fn drawn_into(
    drawn: &Rc<Cell<CheckDrawn>>,
    put: fn(&mut CheckDrawn, Bounds<Pixels>),
) -> impl IntoElement + use<> {
    let drawn = drawn.clone();
    canvas(
        move |bounds, _, _| {
            let mut now = drawn.get();
            put(&mut now, bounds);
            drawn.set(now);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl Asked {
    /// How long the dialog has been arriving, in seconds.
    fn arrived_for(&self) -> f32 {
        self.shown.map_or(0., |shown| shown.elapsed().as_secs_f32())
    }

    fn pose(&self) -> Pose {
        match self.answered_at {
            Some((at, from)) => from.leaving(at.elapsed().as_secs_f32()),
            None => Pose::arriving(self.arrived_for()),
        }
    }

    /// Where it's drawn: as its motion has it, or in its place under Reduce Motion, fading.
    fn drawn(&self, cx: &App) -> Pose {
        let pose = self.pose();
        Pose {
            raised: crate::motion::offset(pose.raised, cx),
            ..pose
        }
    }
}

pub struct Dialog {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    asked: Option<Asked>,
    /// Chrome's question, waiting for the dialog that shows to go.
    next: Option<Request>,
    /// The account's, waiting the same way, after Chrome's.
    next_account: Option<Request>,
    /// A page's, waiting the same way, after Chrome's.
    next_page: Option<Request>,
}

struct DialogWindow(WindowHandle<Dialog>);

impl Global for DialogWindow {}

/// Opens the dialog's window, hidden, for `host` to show over the browser window, ahead of time
/// as the command bar's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<Dialog>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(WIDTH)));
    open_at(host, bounds, false, cx)
}

/// Opens the dialog's window at `bounds`, shown if `show`, which the preview does.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<Dialog>> {
    bind_keys(cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The dim and the card are drawn.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        let dialog = cx.new(|cx| Dialog::new(host, cx));
        // Asked to close, as Windows asks the window with the keyboard on Alt+F4, it cancels as Esc
        // does and stays, as the app shows it again for the next question (#1362).
        let asked = dialog.downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            asked
                .update(cx, |dialog, cx| dialog.answer(Answer::Cancel, cx))
                .ok();
            false
        });
        dialog
    });
    match window {
        Ok(window) => {
            cx.set_global(DialogWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the dialog's window: {error}");
            None
        }
    }
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Confirm, Some("Dialog")),
        KeyBinding::new("escape", Cancel, Some("Dialog")),
        KeyBinding::new("tab", NextField, Some("Dialog")),
    ]);
}

/// A dialog of a window's own, for a window the browser window's dialog can't go over, as
/// Settings is on Linux. Its view draws it over all of its content (`cover`), and draws again as
/// it changes.
pub fn in_window<V: 'static>(host: Rc<dyn Host>, cx: &mut Context<V>) -> Entity<Dialog> {
    bind_keys(cx);
    let dialog = cx.new(|cx| Dialog::new(host, cx));
    cx.observe(&dialog, |_, _, cx| cx.notify()).detach();
    dialog
}

/// Asks as `ask` does, in `dialog`, a window's own (`in_window`), which has the keyboard, with ↩
/// answering `on_return`.
pub fn ask_in(
    dialog: &Entity<Dialog>,
    message: String,
    detail: String,
    action: &'static str,
    on_return: Return,
    confirmed: Box<dyn FnOnce(&mut App)>,
    window: &mut Window,
    cx: &mut App,
) {
    let question = Question {
        on_return,
        ..destructive(message, detail, action)
    };
    let confirmed = Box::new(move |_, cx: &mut App| confirmed(cx));
    dialog.update(cx, |dialog, cx| {
        dialog.pose(None, question, confirmed, window, cx)
    });
}

/// Whether a window's own `dialog` is asking, or leaving after the answer.
pub fn asks(dialog: &Entity<Dialog>, cx: &App) -> bool {
    dialog.read(cx).asked.is_some()
}

/// What a window draws over all of its content while its own `dialog` asks: the dialog, which
/// takes every click, as the window waits for the answer.
pub fn cover(dialog: &Entity<Dialog>, cx: &App) -> Option<AnyElement> {
    asks(dialog, cx).then(|| {
        div()
            .id("dialog-cover")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .child(dialog.clone())
            .into_any_element()
    })
}

/// Whether the dialog is asking, or leaving after the answer.
pub fn asking(cx: &App) -> bool {
    cx.try_global::<DialogWindow>()
        .and_then(|dialog| dialog.0.read(cx).ok())
        .is_some_and(|dialog| dialog.asked.is_some())
}

/// Whether `pose` asks now: there's a dialog to ask in, or one to be made as it's first asked, and
/// it isn't asking another question.
pub fn can_pose(cx: &App) -> bool {
    (cx.try_global::<DialogWindow>().is_some() || crate::lazy_window::pending::<Dialog>(cx))
        && !asking(cx)
}

/// Asks `message`, with `detail` under it, over browser `window` or the one in front, and calls
/// `confirmed` if the user chooses `action` over Cancel, once the dialog has gone. The action is
/// red, as what it does is hard to take back. Nothing while another dialog shows.
pub fn ask(
    window: Option<u64>,
    message: String,
    detail: String,
    action: &'static str,
    confirmed: Box<dyn FnOnce(&mut App)>,
    cx: &mut App,
) {
    ask_returning(
        window,
        message,
        detail,
        action,
        Return::Action,
        confirmed,
        cx,
    );
}

/// `ask`, with ↩ answering `on_return`, as `ask_in` has it.
pub fn ask_returning(
    window: Option<u64>,
    message: String,
    detail: String,
    action: &'static str,
    on_return: Return,
    confirmed: Box<dyn FnOnce(&mut App)>,
    cx: &mut App,
) {
    let question = Question {
        on_return,
        ..destructive(message, detail, action)
    };
    pose(window, question, Box::new(move |_, cx| confirmed(cx)), cx);
}

/// Whether to do `action`, in red, or cancel.
fn destructive(message: String, detail: String, action: &'static str) -> Question {
    Question {
        icon: None,
        message: message.into(),
        detail: detail.into(),
        list: Vec::new(),
        action: action.into(),
        cancel: "Cancel".into(),
        tone: Tone::Destructive,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: Return::Action,
    }
}

/// Asks `question` over browser `over` or the one in front, and calls `confirmed` with what its
/// fields hold if the user chooses its action, once the dialog has gone. Nothing while another
/// dialog shows.
pub fn pose(
    over: Option<u64>,
    question: Question,
    confirmed: Box<dyn FnOnce(Vec<String>, &mut App)>,
    cx: &mut App,
) {
    crate::lazy_window::ready::<Dialog>(cx);
    let Some(window) = cx.try_global::<DialogWindow>().map(|dialog| dialog.0) else {
        return;
    };
    window
        .update(cx, |dialog, window, cx| {
            dialog.pose(over, question, confirmed, window, cx)
        })
        .ok();
}

/// Asks Chrome's `question` once the dialog that shows has gone, in place of any of Chrome's
/// that hasn't been answered, and calls `answered` with the answer once the dialog has gone.
/// With no dialog to ask in, it's answered no at once, so that Chrome isn't left waiting.
pub fn ask_chrome(question: Question, answered: Box<dyn FnOnce(bool, &mut App)>, cx: &mut App) {
    ask_granting(Source::Chrome, question, answered, cx);
}

/// Takes back Chrome's question, unanswered: the dialog leaves if it shows it, and nothing is
/// told of an answer.
pub fn withdraw_chrome(cx: &mut App) {
    withdraw_granting(Source::Chrome, cx);
}

/// Asks the account's `question`, such as whether to add a device to it, as Chrome's are asked,
/// with its moment on screen before its action works. Chrome's and the account's wait for each
/// other, and neither takes the other's place.
pub fn ask_account(question: Question, answered: Box<dyn FnOnce(bool, &mut App)>, cx: &mut App) {
    ask_granting(Source::Account, question, answered, cx);
}

/// Takes back the account's question, unanswered, as `withdraw_chrome` does Chrome's.
pub fn withdraw_account(cx: &mut App) {
    withdraw_granting(Source::Account, cx);
}

/// Asks a question that grants something, Chrome's or the account's, once the dialog that shows
/// has gone, in place of any from `source` that hasn't been answered.
fn ask_granting(
    source: Source,
    question: Question,
    answered: Box<dyn FnOnce(bool, &mut App)>,
    cx: &mut App,
) {
    crate::lazy_window::ready::<Dialog>(cx);
    let Some(window) = cx.try_global::<DialogWindow>().map(|dialog| dialog.0) else {
        answered(false, cx);
        return;
    };
    let answered = Box::new(move |reply: Reply, cx: &mut App| answered(reply.accept, cx));
    let mut request = Some(Request {
        question,
        source,
        window: None,
        over: None,
        page: None,
        answered,
    });
    window
        .update(cx, |dialog, window, cx| {
            dialog.withdraw(source, cx);
            let request = request.take().expect("asked once");
            match (dialog.asked.is_some(), source) {
                (false, _) => dialog.show(request, window, cx),
                (true, Source::Account) => dialog.next_account = Some(request),
                (true, _) => dialog.next = Some(request),
            }
        })
        .ok();
    if let Some(request) = request {
        (request.answered)(
            Reply {
                accept: false,
                texts: Vec::new(),
                checked: false,
            },
            cx,
        );
    }
}

fn withdraw_granting(source: Source, cx: &mut App) {
    let Some(window) = cx.try_global::<DialogWindow>().map(|dialog| dialog.0) else {
        return;
    };
    window
        .update(cx, |dialog, _, cx| dialog.withdraw(source, cx))
        .ok();
}

/// For the drivers, as AppKit's alerts are chosen on the Mac: answers with the button `label` names
/// in the dialog that shows, as a click on it would. Returns whether it shows one.
pub fn choose(label: &str, cx: &mut App) -> bool {
    let Some(window) = cx.try_global::<DialogWindow>().map(|dialog| dialog.0) else {
        return false;
    };
    window
        .update(cx, |dialog, _, cx| dialog.choose(label, cx))
        .unwrap_or(false)
}

/// Asks a page's `question`, the dialog `id` of `tab`, over its tab's card, `over` in the window
/// with corners of `radius`, in place of any page's that shows or waits, once any other dialog has
/// gone, and calls `answered` with the answer once the dialog has gone. Asked again, it only moves
/// to `over`, with what was typed. A click on the card's dim does nothing, as the page waits.
pub fn ask_page(
    (tab, id): (u64, u64),
    question: Question,
    over: (Bounds<Pixels>, f32),
    answered: Box<dyn FnOnce(Reply, &mut App)>,
    cx: &mut App,
) {
    crate::lazy_window::ready::<Dialog>(cx);
    let Some(window) = cx.try_global::<DialogWindow>().map(|dialog| dialog.0) else {
        return;
    };
    let page = Some((tab, id));
    // Over the window in front, which the page's is before it asks (main.rs).
    let source = Source::Page;
    let request = Request {
        question,
        source,
        window: None,
        over: Some(over),
        page,
        answered,
    };
    window
        .update(cx, |dialog, window, cx| {
            if let Some(asked) = &mut dialog.asked
                && asked.page == page
                && asked.answered_at.is_none()
            {
                asked.over = Some(over);
                dialog.host.show(None, Some(over.0));
                cx.notify();
                return;
            }
            if let Some(next) = &mut dialog.next_page
                && next.page == page
            {
                next.over = Some(over);
                return;
            }
            dialog.withdraw(Source::Page, cx);
            match dialog.asked.is_some() {
                true => dialog.next_page = Some(request),
                false => dialog.show(request, window, cx),
            }
        })
        .ok();
}

/// Takes back the page's question, unanswered, as its tab no longer shows or the page no longer
/// asks: the dialog leaves if it shows it, and nothing is told of an answer.
pub fn withdraw_page(cx: &mut App) {
    let Some(window) = cx.try_global::<DialogWindow>().map(|dialog| dialog.0) else {
        return;
    };
    window
        .update(cx, |dialog, _, cx| dialog.withdraw(Source::Page, cx))
        .ok();
}

impl Dialog {
    fn new(host: Rc<dyn Host>, cx: &mut Context<Self>) -> Self {
        Dialog {
            host,
            focus: cx.focus_handle(),
            asked: None,
            next: None,
            next_account: None,
            next_page: None,
        }
    }

    fn pose(
        &mut self,
        over: Option<u64>,
        question: Question,
        confirmed: Box<dyn FnOnce(Vec<String>, &mut App)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.asked.is_some() {
            return;
        }
        let answered = Box::new(move |reply: Reply, cx: &mut App| {
            if reply.accept {
                confirmed(reply.texts, cx);
            }
        });
        let source = Source::User;
        let request = Request {
            question,
            source,
            window: over,
            over: None,
            page: None,
            answered,
        };
        self.show(request, window, cx);
    }

    fn show(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        let Request {
            mut question,
            source,
            window: asker,
            over,
            page,
            answered,
        } = request;
        // check.sh reads this line.
        log!("browser: dialog shown {}", question.message);
        let (also, also_chosen) = question.also.take().unzip();
        let palette = palette::current(cx);
        let fields: Vec<_> = question
            .fields
            .iter()
            .map(|field| {
                cx.new(|cx| {
                    TextInput::new(field.placeholder.clone(), cx)
                        .with_text(field.text.clone())
                        .secure(field.secure)
                        .placeholder_color(palette.on_overlay(FIELD_PLACEHOLDER))
                        .caret_color(palette.accent)
                        .selection_color(palette.selection())
                })
            })
            .collect();
        match fields.first() {
            Some(field) => window.focus(&field.focus_handle(cx), cx),
            None => window.focus(&self.focus, cx),
        }
        self.asked = Some(Asked {
            question,
            also,
            also_chosen,
            source,
            over,
            page,
            fields,
            checked: false,
            answered: Some(answered),
            shown: None,
            settled: false,
            settling: Settling::default(),
            answered_at: None,
            _leave: None,
            hovered: None,
            drawn_hover: None,
            pressed: None,
            drawn_press: None,
            buttons_drawn: Rc::default(),
            check_drawn: Rc::default(),
        });
        self.host.show(asker, over.map(|(over, _)| over));
        cx.notify();
    }

    /// The pointer came onto the button `name`, or left it.
    fn hover(&mut self, name: &'static str, hovered: bool, cx: &mut Context<Self>) {
        let Some(asked) = &mut self.asked else {
            return;
        };
        if hovered {
            asked.hovered = Some(name);
        } else if asked.hovered == Some(name) {
            asked.hovered = None;
        }
        cx.notify();
    }

    /// The button `name` was pressed, or let go of anywhere.
    fn press(&mut self, name: &'static str, pressed: bool, cx: &mut Context<Self>) {
        let Some(asked) = &mut self.asked else {
            return;
        };
        asked.pressed = pressed.then_some(name);
        cx.notify();
    }

    /// Drops the question from `source` that waits, and the one that shows leaves, both untold.
    fn withdraw(&mut self, source: Source, cx: &mut Context<Self>) {
        match source {
            Source::Page => self.next_page = None,
            Source::Account => self.next_account = None,
            _ => self.next = None,
        }
        let Some(asked) = &mut self.asked else {
            return;
        };
        if asked.source == source && asked.answered_at.is_none() {
            // check.sh reads this line.
            log!("browser: dialog withdrawn");
            self.leave(None, cx);
        }
    }

    /// Moves the keyboard to the next field, round to the first.
    fn next_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asked) = &self.asked else {
            return;
        };
        let focused = asked
            .fields
            .iter()
            .position(|field| field.focus_handle(cx).is_focused(window));
        let next = focused.map_or(0, |focused| (focused + 1) % asked.fields.len().max(1));
        if let Some(field) = asked.fields.get(next) {
            window.focus(&field.focus_handle(cx), cx);
        }
    }

    /// The user clicked the checkbox or its label.
    fn toggle_check(&mut self, cx: &mut Context<Self>) {
        if let Some(asked) = &mut self.asked {
            asked.checked = !asked.checked;
            cx.notify();
        }
    }

    /// The user pressed ↩: what the question says it answers.
    fn confirm(&mut self, cx: &mut Context<Self>) {
        let Some(asked) = &self.asked else {
            return;
        };
        // An input method takes ↩ to finish what it composes.
        if asked.fields.iter().any(|field| field.read(cx).composing()) {
            return;
        }
        match asked.question.on_return {
            Return::Action => self.answer(Answer::Action, cx),
            Return::Cancel => self.answer(Answer::Cancel, cx),
            Return::Nothing => {}
        }
    }

    /// The button `label` names was chosen, if the dialog shows one.
    fn choose(&mut self, label: &str, cx: &mut Context<Self>) -> bool {
        let Some(asked) = &self.asked else {
            return false;
        };
        let answer = match label {
            _ if asked.question.action == label => Answer::Action,
            _ if asked.question.cancel == label => Answer::Cancel,
            _ if asked.also.as_ref().is_some_and(|also| also == label) => Answer::Also,
            _ => return false,
        };
        self.answer(answer, cx);
        true
    }

    /// The user chose the action, the other answer or Cancel. The dialog leaves, and then what
    /// was chosen happens.
    fn answer(&mut self, answer: Answer, cx: &mut Context<Self>) {
        let Some(asked) = &mut self.asked else {
            return;
        };
        let unsettled = asked.source.grants() && !asked.settling.settled(cx);
        if asked.answered_at.is_some()
            || (answer == Answer::Action && (asked.question.action.is_empty() || unsettled))
        {
            return;
        }
        // check.sh reads these lines.
        match answer {
            Answer::Action => log!("browser: dialog confirmed"),
            Answer::Also => {
                log!(
                    "browser: dialog chose {}",
                    asked.also.clone().unwrap_or_default()
                )
            }
            Answer::Cancel => log!("browser: dialog cancelled"),
        }
        self.leave(Some(answer), cx);
    }

    /// The dialog leaves, and then what was chosen happens, unless it was taken back.
    fn leave(&mut self, answer: Option<Answer>, cx: &mut Context<Self>) {
        let Some(asked) = &mut self.asked else {
            return;
        };
        asked.answered_at = Some((Instant::now(), asked.pose()));
        // Motion never blocks a key (ux-principles): one typed while it leaves, such as ⌥⌘N right
        // after Esc, waits for the browser window rather than reaching the dialog going (#1930).
        self.host.hold_keys();
        let (answered, also_chosen) = match answer {
            Some(Answer::Also) => (None, asked.also_chosen.take()),
            Some(answer) => {
                let reply = Reply {
                    accept: answer == Answer::Action,
                    texts: (asked.fields.iter())
                        .map(|field| field.read(cx).text().to_owned())
                        .collect(),
                    checked: asked.checked,
                };
                (
                    asked.answered.take().map(|answered| (answered, reply)),
                    None,
                )
            }
            None => (None, None),
        };
        // On Linux the page has the keyboard back as the dialog starts leaving (Host::hold_keys),
        // and Chromium drops its keys until its question is answered, so it's answered then: ⌘L
        // typed right after Esc opens the command bar (#1711, #1963). After this update, as the
        // core's answer can come back to the dialog.
        let answered = match answered {
            Some((answered, reply))
                if PAGE_ANSWERED_AS_IT_LEAVES && asked.source == Source::Page =>
            {
                cx.defer(move |cx| answered(reply, cx));
                None
            }
            answered => answered,
        };
        asked._leave = Some(cx.spawn(async move |dialog, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(MOVE))
                .await;
            let hidden = dialog.update(cx, |dialog, cx| {
                dialog.asked = None;
                dialog.host.hide();
                dialog.host.give_held_keys();
                // check.sh reads this line.
                log!("browser: dialog hidden");
                cx.notify();
            });
            if hidden.is_err() {
                return;
            }
            if let Some((answered, reply)) = answered {
                cx.update(|cx| answered(reply, cx));
            }
            if let Some(also_chosen) = also_chosen {
                cx.update(also_chosen);
            }
            cx.update(show_next);
        }));
        cx.notify();
    }
}

/// Shows Chrome's question that waits, else the account's, else a page's, if no other dialog has
/// come up first.
fn show_next(cx: &mut App) {
    let Some(window) = cx.try_global::<DialogWindow>().map(|dialog| dialog.0) else {
        return;
    };
    window
        .update(cx, |dialog, window, cx| {
            if dialog.asked.is_none()
                && let Some(request) = (dialog.next.take())
                    .or_else(|| dialog.next_account.take())
                    .or_else(|| dialog.next_page.take())
            {
                dialog.show(request, window, cx);
            }
        })
        .ok();
}

/// A button with its key's chip after its label.
fn button(id: &'static str, label: SharedString, chip: impl IntoElement) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(BUTTON_HEIGHT))
        .pl(px(LABEL_X))
        .pr(px(CHIP_END))
        .flex()
        .items_center()
        .gap(px(CHIP_GAP))
        .rounded(px(BUTTON_RADIUS))
        // macOS's control font, as Arc's buttons have.
        .text_size(px(text::BODY))
        .child(label)
        .child(chip)
}

/// Says where the button `name` is drawn in the browser window once the card has settled, and
/// again if it moves, for check.sh to point at.
fn button_frame(name: &'static str, asked: &Asked) -> impl IntoElement + use<> {
    let (drawn, settled) = (asked.buttons_drawn.clone(), asked.settled);
    let origin = asked.over.map_or(Point::default(), |(over, _)| over.origin);
    canvas(
        move |bounds, _, _| {
            let at = origin + bounds.center();
            if settled && drawn.borrow_mut().insert(name, at) != Some(at) {
                // check.sh reads this line.
                log!(
                    "browser: dialog {name} at {:.0} {:.0}",
                    f32::from(at.x),
                    f32::from(at.y)
                );
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

/// A key's chip on a button.
fn chip() -> Div {
    div()
        .h(px(CHIP_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CHIP_RADIUS))
}

fn shadow(color: Hsla, y: f32, blur: f32, spread: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The window holds the card alone, so VoiceOver reads it as the alert it is, and the
        // focus GPUI reports is on it as the dialog comes.
        let a11y = window.is_a11y_active();
        let root = div()
            .id("dialog-window")
            .key_context("Dialog")
            .track_focus(&self.focus)
            .size_full()
            .font_family(crate::face::UI);
        let Some(asked) = &mut self.asked else {
            return root;
        };
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, so the motion starts there rather than skip its start.
        if asked.shown.is_none() && window.is_visible() {
            asked.shown = Some(Instant::now());
            if asked.source.grants() {
                asked.settling.start("dialog".into(), cx);
            }
        }
        let pose = asked.drawn(cx);
        let moving = asked.answered_at.is_some() || asked.arrived_for() < MOVE;
        if moving {
            window.request_animation_frame();
        } else if !asked.settled {
            asked.settled = true;
            // check.sh reads this line.
            log!("browser: dialog settled");
        }
        // check.sh reads this line. It comes as the button under the pointer is drawn so, as the
        // sidebar's hover line does, so a capture that follows shows it.
        if asked.hovered != asked.drawn_hover {
            asked.drawn_hover = asked.hovered;
            log!("browser: dialog hover {}", asked.hovered.unwrap_or("none"));
        }
        // As the hover line, for a capture of the button held down.
        if asked.pressed != asked.drawn_press {
            asked.drawn_press = asked.pressed;
            log!(
                "browser: dialog pressed {}",
                asked.pressed.unwrap_or("none")
            );
        }
        let question = &asked.question;
        let palette = palette::current(cx);
        let page = asked.source == Source::Page;
        // An alert has only its action.
        let cancel = (!question.cancel.is_empty()).then(|| {
            button(
                "dialog-cancel",
                question.cancel.clone(),
                chip()
                    .px(px(6.5))
                    .bg(palette.plate(ESC_FILL))
                    .text_size(px(text::SMALL))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.on_overlay(ESC_TEXT))
                    .child("ESC"),
            )
            // Its border is inside its edge, where Arc's label and chip are measured from.
            .pl(px(LABEL_X - 1.))
            .pr(px(CHIP_END - 1.))
            .border_1()
            .border_color(palette.plate(CANCEL_BORDER))
            .bg(palette.plate(CANCEL_FILL))
            .hover(move |style| style.bg(palette.plate(CANCEL_HOVERED)))
            .active(move |style| style.bg(palette.state(CANCEL_FILL, STATE_PRESSED)))
            .text_color(palette.on_overlay(CANCEL_TEXT))
            .relative()
            .child(button_frame("cancel", asked))
            .when(a11y, |button| {
                button
                    .role(Role::Button)
                    .aria_label(question.cancel.clone())
            })
            .on_hover(cx.listener(|dialog, hovered, _, cx| dialog.hover("cancel", *hovered, cx)))
            .on_click(cx.listener(|dialog, _, _, cx| dialog.answer(Answer::Cancel, cx)))
        });
        // A text-only button in Cancel's look, its label as far in from each side.
        let also = asked.also.clone().map(|label| {
            div()
                .id("dialog-also")
                .h(px(BUTTON_HEIGHT))
                .px(px(LABEL_X))
                .flex()
                .items_center()
                .rounded(px(BUTTON_RADIUS))
                .text_size(px(text::BODY))
                .border_1()
                .border_color(palette.plate(CANCEL_BORDER))
                .bg(palette.plate(CANCEL_FILL))
                .hover(move |style| style.bg(palette.plate(CANCEL_HOVERED)))
                .active(move |style| style.bg(palette.state(CANCEL_FILL, STATE_PRESSED)))
                .text_color(palette.on_overlay(CANCEL_TEXT))
                .when(a11y, |button| {
                    button.role(Role::Button).aria_label(label.clone())
                })
                .child(label)
                .relative()
                .child(button_frame("also", asked))
                .on_hover(cx.listener(|dialog, hovered, _, cx| dialog.hover("also", *hovered, cx)))
                .on_click(cx.listener(|dialog, _, _, cx| dialog.answer(Answer::Also, cx)))
        });
        let (fill, hovered, pressed, ledge, label): (Hsla, Hsla, Hsla, Hsla, Hsla) =
            match question.tone {
                Tone::Destructive => {
                    let hovered = rgb(ACTION_HOVERED).into();
                    (
                        rgb(ACTION).into(),
                        hovered,
                        hovered,
                        rgba(ACTION_LEDGE).into(),
                        rgb(TEXT).into(),
                    )
                }
                Tone::Accent => {
                    let accent = Hsla::from(palette.accent);
                    (
                        accent,
                        palette.accent_hovered(),
                        palette.accent_pressed(),
                        accent.opacity(0.2),
                        palette.accent_label.into(),
                    )
                }
            };
        let return_chip = (question.on_return == Return::Action).then(|| {
            chip()
                .w(px(RETURN_WIDTH))
                .border_1()
                .border_color(label.opacity(RETURN_BORDER))
                .bg(label.opacity(RETURN_FILL))
                .child(
                    svg()
                        .path("icons/corner-down-left.svg")
                        .size(px(RETURN_GLYPH))
                        .text_color(label),
                )
        });
        let action = (!question.action.is_empty()).then(|| {
            button(
                "dialog-confirm",
                question.action.clone(),
                div().children(return_chip),
            )
            // Without its chip, its label is as far in from each side.
            .when(question.on_return != Return::Action, |button| {
                button.gap_0().pr(px(LABEL_X))
            })
            .bg(fill)
            .hover(move |style| style.bg(hovered))
            .active(move |style| style.bg(pressed))
            .shadow(vec![shadow(ledge, 2., 0., 0.)])
            .text_color(label)
            .relative()
            .child(button_frame("confirm", asked))
            .when(a11y, |button| {
                button
                    .role(Role::Button)
                    .aria_label(question.action.clone())
            })
            .on_hover(cx.listener(|dialog, hovered, _, cx| dialog.hover("confirm", *hovered, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|dialog, _, _, cx| dialog.press("confirm", true, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|dialog, _, _, cx| dialog.press("confirm", false, cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|dialog, _, _, cx| dialog.press("confirm", false, cx)),
            )
            .on_click(cx.listener(|dialog, _, _, cx| dialog.answer(Answer::Action, cx)))
        });
        let list = question.list.iter().map(|(line, sublines)| {
            div()
                .flex()
                .flex_col()
                .child(line.clone())
                .children(sublines.iter().map(|subline| {
                    div()
                        .pl(px(SUBLINE_INDENT))
                        .line_height(px(SUBLINE))
                        .text_size(px(text::SMALL))
                        .text_color(palette.on_overlay(SUBLINE_TEXT))
                        .child(subline.clone())
                }))
        });
        let detail = (!question.detail.is_empty() || !question.list.is_empty()).then(|| {
            div()
                .mt(px(DETAIL_TOP))
                .pr(px(TEXT_INSET - BUTTON_INSET))
                .flex()
                .flex_col()
                .line_height(px(DETAIL_LINE))
                .text_size(px(text::BODY))
                .text_color(palette.on_overlay(DETAIL))
                .when(!question.detail.is_empty(), |text| {
                    text.child(question.detail.clone())
                })
                .when(!question.list.is_empty(), |text| {
                    text.child(
                        div()
                            .when(!question.detail.is_empty(), |list| list.mt(px(LIST_TOP)))
                            .flex()
                            .flex_col()
                            .gap(px(LIST_GAP))
                            .children(list),
                    )
                })
        });
        let fields = (!asked.fields.is_empty()).then(|| {
            div()
                .mt(px(FIELDS_TOP))
                .flex()
                .flex_col()
                .gap(px(FIELD_GAP))
                .children(asked.fields.iter().map(|field| {
                    div()
                        .h(px(BUTTON_HEIGHT))
                        .px(px(LABEL_X - 1.))
                        .flex()
                        .items_center()
                        .rounded(px(BUTTON_RADIUS))
                        .border_1()
                        .border_color(palette.plate(CANCEL_BORDER))
                        .bg(palette.plate(CANCEL_FILL))
                        .text_size(px(text::BODY))
                        .text_color(palette.on_overlay(FIELD_TEXT))
                        .child(field.clone())
                }))
        });
        let check = question.check.clone().map(|check| {
            let checked = asked.checked;
            // The label wraps inside the card as the detail does, a site's long name too, and the
            // box sits beside its first line.
            div()
                .id("dialog-check")
                .mt(px(CHECK_TOP))
                .pr(px(TEXT_INSET - BUTTON_INSET))
                .flex()
                .items_start()
                .gap(px(CHECK_GAP))
                .text_size(px(text::BODY))
                .line_height(px(DETAIL_LINE))
                .text_color(palette.on_overlay(DETAIL))
                .child(
                    div()
                        .relative()
                        .flex_none()
                        .mt(px((DETAIL_LINE - CHECK_BOX) / 2.))
                        .child(drawn_into(&asked.check_drawn, |drawn, at| drawn.check = at))
                        .size(px(CHECK_BOX))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(CHECK_RADIUS))
                        .border_1()
                        .border_color(palette.plate(CANCEL_BORDER))
                        .bg(if checked {
                            fill
                        } else {
                            palette.plate(CANCEL_FILL)
                        })
                        .when(checked, |tick| {
                            tick.child(
                                svg()
                                    .path("icons/check.svg")
                                    .size(px(CHECK_GLYPH))
                                    .text_color(label),
                            )
                        }),
                )
                .when(a11y, |row| {
                    let toggled = if checked {
                        Toggled::True
                    } else {
                        Toggled::False
                    };
                    row.role(Role::CheckBox)
                        .aria_label(check.clone())
                        .aria_toggled(toggled)
                })
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .child(drawn_into(&asked.check_drawn, |drawn, at| drawn.label = at))
                        .child(check),
                )
                .on_click(cx.listener(|dialog, _, _, cx| dialog.toggle_check(cx)))
        });
        let buttons_top = match detail.is_some() || fields.is_some() || check.is_some() {
            true => BUTTONS_TOP,
            false => BUTTONS_UNDER_TITLE,
        };
        let card = div()
            .id("dialog")
            // Clicks on the card stay on it, and one around it cancels.
            .occlude()
            .relative()
            .child(drawn_into(&asked.check_drawn, |drawn, at| drawn.card = at))
            .top(px(-pose.raised))
            .w(px(WIDTH))
            .pt(px(TEXT_INSET))
            .pl(px(TEXT_INSET))
            .pr(px(BUTTON_INSET))
            .pb(px(BUTTON_INSET))
            .flex()
            .flex_col()
            .rounded(px(RADIUS))
            .border_1()
            .border_color(Level::Modal.rim_in(&palette))
            .bg(palette.dialog)
            .shadow(Level::Modal.shadow())
            .opacity(pose.opacity)
            .children(
                question
                    .icon
                    .clone()
                    .map(|icon| img(icon).size(px(ICON)).mb(px(ICON_GAP))),
            )
            .child(
                div()
                    .pr(px(TEXT_INSET - BUTTON_INSET))
                    .line_height(px(TITLE_LINE))
                    .text_size(px(text::HEADING))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.on_overlay(1.))
                    .child(question.message.clone()),
            )
            .children(detail)
            .children(fields)
            .children(check)
            .child(
                div()
                    .mt(px(buttons_top))
                    .flex()
                    .gap(px(ALSO_GAP))
                    .children(also)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .justify_end()
                            .gap(px(CHIP_GAP))
                            .children(cancel)
                            .children(action),
                    ),
            );
        let radius = asked.over.map_or(0., |(_, radius)| radius);
        let root = root.when(a11y, |root| {
            let root = root
                .role(Role::AlertDialog)
                .aria_label(question.message.clone());
            root.when(!question.detail.is_empty(), |root| {
                root.aria_description(question.detail.clone())
            })
        });
        root.on_action(cx.listener(|dialog, _: &Confirm, _, cx| dialog.confirm(cx)))
            .on_action(cx.listener(|dialog, _: &Cancel, _, cx| dialog.answer(Answer::Cancel, cx)))
            .on_action(
                cx.listener(|dialog, _: &NextField, window, cx| dialog.next_field(window, cx)),
            )
            // A page waits for its answer, so a click on its card's dim does nothing.
            .when(!page, |root| {
                root.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|dialog, _, _, cx| dialog.answer(Answer::Cancel, cx)),
                )
            })
            // Over a tab's card, the dim has the card's corners.
            .rounded(px(radius))
            .flex()
            .items_center()
            .justify_center()
            .bg(Hsla::from(rgb(DIM)).opacity(DIM_OPACITY * pose.dim))
            .child(card)
    }
}

#[cfg(test)]
mod tests;
