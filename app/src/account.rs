//! Settings › Sync: the browser's account on an ilo server, which syncs the Spaces, and the
//! first profile's bookmarks, passwords and open tabs, across the user's devices (the core's
//! bc_sync_ calls). Signed out, it offers to sign in, and the server to sign in to. Once signed in,
//! the account's first device shows the recovery words once. A new device waits for another to add
//! it, with the code both show, or takes the words. Signed in, it lists the devices. The core signs
//! in, keeps the keys and syncs; this view says where the account is and asks. Each state has its
//! way out: Cancel while it waits, Sign Out, Remove, and Not me on the other device. Agents get
//! none of it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    Global, KeyBinding, Pixels, Render, SharedString, Window, actions, canvas, div, prelude::*, px,
    svg,
};

use crate::dialog::{Question, Return, Tone};
use crate::settings_rows::{Look, button, group};
use crate::text_input::TextInput;
use crate::tokens::settings::{
    BOX_GAP, BUTTONS_TOP, HEADER_GAP, PADDING, PROFILE_ROW, SECTION_GAP, WORD_GAP, WORD_NUMBER,
    WORDS_COLUMNS,
};
use crate::tokens::text;

actions!(account, [Keep, Leave]);

/// How long Copy says Copied, before it's Copy again.
const COPIED: Duration = Duration::from_secs(2);
/// How long the words stay on the clipboard after Copy: long enough to paste them where they're
/// kept, then gone, as a password manager clears a password it copied.
pub const WORDS_KEPT: Duration = Duration::from_secs(90);
/// What the words' panel says while a copy of them is on the clipboard. The Mac and Windows keep a
/// copy from their cloud clipboards; Linux has no such mark, as KDE Connect and GSConnect share it.
#[cfg(not(target_os = "linux"))]
pub const COPIED_NOTE: &str =
    "Copied for this device only. The clipboard forgets them in a minute and a half.";
#[cfg(target_os = "linux")]
pub const COPIED_NOTE: &str = "Copied. The clipboard forgets them in a minute and a half.";
/// The metadata that has GPUI's Linux clipboard offer a copy with KDE's password manager hint,
/// which Klipper and the clipboard managers that follow it don't record (patches/
/// gpui-linux-embedded.patch).
pub const SECRET_HINT: &str = "x-kde-passwordManagerHint";

/// Where the account is, as the core says (BC_SYNC_).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Status {
    #[default]
    SignedOut,
    /// The sign-in window is open.
    SigningIn,
    /// Signed in on the account's first device, which makes its secret.
    FirstDevice,
    /// Signed in on a device that hasn't the secret yet.
    NeedsSecret,
    /// Waiting for another device to add this one.
    Pairing,
    On,
}

/// Why the last step didn't go as asked (BC_SYNC_PROBLEM_).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Problem {
    WrongWords,
    Declined,
    Expired,
    Removed,
    Server,
}

/// The account, as the core last told it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct State {
    pub status: Status,
    pub problem: Option<Problem>,
    pub server: SharedString,
    /// The server is the build's own, named by the app's name.
    pub default_server: bool,
    pub email: SharedString,
    /// While pairing, the code once a device has answered, else "".
    pub code: SharedString,
}

/// The account as the core last said it is, for a view of it that opens later, as Settings'
/// does, and for the first launch to ask whether to offer it.
#[derive(Default)]
pub struct Synced(pub State);

impl Global for Synced {}

/// The account as the core last said it is, signed out before it has said.
pub fn synced(cx: &App) -> State {
    cx.try_global::<Synced>().map(|synced| synced.0.clone()).unwrap_or_default()
}

/// What the view tells the window it's in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccountEvent {
    /// The account is signed in and syncing, with its recovery words put away if it showed them:
    /// a step that offered it goes on.
    Finished,
}

/// One of the account's devices.
#[derive(Clone, PartialEq, Debug)]
pub struct Device {
    pub id: SharedString,
    pub name: SharedString,
    pub os: SharedString,
    pub current: bool,
}

/// Another device asking this one to add it.
#[derive(Clone, PartialEq, Debug)]
pub struct Pairing {
    pub id: SharedString,
    /// The device's name and system, as it gives them: anyone who signed in can say anything.
    pub name: SharedString,
    pub os: SharedString,
    pub code: SharedString,
    /// When it asked, as `clock` gives it: the server's time, or this device's for a server that
    /// doesn't say.
    pub asked: SharedString,
    /// How long ago it signed in to the account, by the server's word, as `since` gives it; empty
    /// from a server that doesn't say.
    pub signed_in: SharedString,
}

/// What the view asks of the core, and of the window it's in.
pub trait Host {
    /// Signs in to `server` from now on, signing out first; false for an address it won't take.
    fn set_server(&self, server: &str) -> bool;
    fn sign_in(&self);
    /// Stops what the account waits for: the sign-in, or the request to the other devices.
    fn cancel(&self);
    fn sign_out(&self);
    /// The recovery words, made first on the account's first device, or "" if there are none.
    fn recovery_words(&self) -> String;
    /// False if `words` aren't 24 words of the list.
    fn enter_recovery_words(&self, words: &str) -> bool;
    /// Deletes what the account syncs on the server and makes this its first device again.
    fn start_over(&self);
    fn request_pairing(&self);
    fn read_devices(&self);
    fn remove_device(&self, id: &str);
    /// Puts the account's secret on this device's clipboard, marked for clipboard histories and
    /// cloud clipboards to skip where the system has a way, and takes it off after `kept` unless
    /// something replaced it.
    fn copy_secret(&self, text: &str, kept: Duration, cx: &mut App);
    /// Asks `message`, with `detail` under it, and calls `confirmed` if the user chooses `action`,
    /// in red, ↩ answering `on_return`.
    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        on_return: Return,
        confirmed: Box<dyn FnOnce(&mut App)>,
        window: &mut Window,
        cx: &mut App,
    );
}

/// The recovery words on screen, until Done.
struct Words {
    words: Vec<String>,
    copied: Option<Instant>,
}

/// A field the view shows: the server's address for Change…, or the recovery words.
struct Field {
    input: Entity<TextInput>,
    /// The address or the words typed weren't taken.
    refused: bool,
    /// It takes the keyboard once drawn.
    focus: bool,
}

pub struct Account {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    state: State,
    /// As the core last listed them, once asked while the account shows.
    devices: Option<Vec<Device>>,
    words: Option<Words>,
    server: Option<Field>,
    entering: Option<Field>,
    /// This device asked the others to add it since it signed in, which it does once by itself,
    /// and again when the user asks.
    asked: bool,
    /// Where each button was last drawn, by its name, for the tests to press.
    drawn: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    /// The colours it's drawn in: Settings', or the ground of the view it's shown in.
    look: Look,
    /// It's the first launch's step, whose title, line and button say what its signed-out box
    /// would.
    step: bool,
}

impl EventEmitter<AccountEvent> for Account {}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Keep, Some("AccountField")),
        KeyBinding::new("escape", Leave, Some("AccountField")),
    ]);
}

/// Puts `text` on GPUI's clipboard, marked with SECRET_HINT, and takes it off after `kept` if it's
/// still there, for a host that can't copy a secret itself (Linux).
pub fn copy_for(text: &str, kept: Duration, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
        text.to_owned(),
        SECRET_HINT.to_owned(),
    ));
    let text = text.to_owned();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(kept).await;
        cx.update(|cx| {
            if cx.read_from_clipboard().and_then(|item| item.text()) == Some(text) {
                cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
            }
        });
    })
    .detach();
}

/// The code as both devices show it, in two threes, so it's read and checked at a glance. A no-break
/// space keeps them on one line in a sentence that wraps.
pub fn grouped(code: &str) -> String {
    match code.len() == 6 && code.is_ascii() {
        true => format!("{}\u{a0}{}", &code[..3], &code[3..]),
        false => code.to_owned(),
    }
}

/// What the dialog over the window asks when another device asks to be added. It goes by what
/// this device knows, when the request came, and gives the name and system only as the device's
/// own word, since anyone signed in to the account can call itself anything (browser#642). The
/// code is the check. ↩ is Not me and Add takes a click, so a key meant for the page can't add a
/// device.
pub fn pairing_question(pairing: &Pairing) -> Question {
    let name = given(&pairing.name);
    let said = match (name.is_empty(), given(&pairing.os)) {
        (true, os) if os.is_empty() => String::new(),
        (true, os) => format!("It says it runs {os}, which nothing checks. "),
        (false, os) if os.is_empty() => format!("It calls itself “{name}”, which nothing checks. "),
        (false, os) => format!("It calls itself “{name}” on {os}, which nothing checks. "),
    };
    let code = grouped(&pairing.code);
    let signed_in = match pairing.signed_in.is_empty() {
        true => String::new(),
        false => format!("It signed in to your account {}. ", pairing.signed_in),
    };
    Question {
        icon: None,
        message: format!("Add the device that asked at {}?", pairing.asked).into(),
        detail: format!("{signed_in}{said}Add it only if the device in front of you shows {code}.")
            .into(),
        list: Vec::new(),
        action: "Add".into(),
        cancel: "Not me".into(),
        tone: Tone::Accent,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: Return::Cancel,
    }
}

/// What a device says of itself, as one short line: no breaks to fake a line of the dialog's own,
/// and no more than a name needs.
fn given(text: &str) -> String {
    const LONGEST: usize = 40;
    let line: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.chars().count() > LONGEST {
        true => format!("{}…", line.chars().take(LONGEST - 1).collect::<String>()),
        false => line,
    }
}

/// How long ago `past` milliseconds is, as "just now" or "5 minutes ago", for a time the user
/// weighs rather than reads off a clock: a device that signed in days ago isn't the new one in
/// their hand.
pub fn since(past: f64) -> String {
    let minutes = (past / 60_000.).floor().max(0.) as u64;
    let (count, unit) = match minutes {
        0 => return "just now".into(),
        1..60 => (minutes, "minute"),
        60..2880 => (minutes / 60, "hour"),
        _ => (minutes / 1440, "day"),
    };
    format!("{count} {unit}{} ago", if count == 1 { "" } else { "s" })
}

/// The time of day `at` is, in milliseconds since the Unix epoch, as "15:04", with `today` the
/// start of today in the user's calendar. Off by an hour after a clock change, the day it changes.
pub fn clock(at: f64, today: f64) -> String {
    let minutes = ((at - today) / 60_000.).floor().rem_euclid(24. * 60.) as u32;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// What the view says of a problem, by where the account is.
fn problem_text(problem: Problem) -> &'static str {
    match problem {
        Problem::WrongWords => "These words are for another account.",
        Problem::Declined => "Your other device didn't add this one.",
        Problem::Expired => "No device added this one in time.",
        Problem::Removed => "This device was removed from your account.",
        Problem::Server => "Couldn't reach the server. Try again in a while.",
    }
}

impl Account {
    pub fn new(host: Rc<dyn Host>, cx: &mut Context<Self>) -> Self {
        Account {
            host,
            focus: cx.focus_handle(),
            state: State::default(),
            devices: None,
            words: None,
            server: None,
            entering: None,
            asked: false,
            drawn: Rc::default(),
            look: Look::settings(),
            step: false,
        }
    }

    /// Shows it as the first launch's step, which says what signing in does and has its own Sign In
    /// button.
    pub fn as_step(mut self) -> Account {
        self.step = true;
        self
    }

    pub fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        if self.look != look {
            self.look = look;
            cx.notify();
        }
    }

    /// The first device's recovery words show, or are being made, which show only once.
    pub fn keeping_words(&self) -> bool {
        self.words.is_some() || self.state.status == Status::FirstDevice
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    /// The account as the core says it is now.
    pub fn set_state(&mut self, state: State, cx: &mut Context<Self>) {
        if state == self.state {
            return;
        }
        // check.sh waits for this line. The email stays out of the log.
        log!("browser: account {:?} {:?}", state.status, state.problem);
        let was = self.state.status;
        if state.status != Status::On {
            self.devices = None;
        }
        match state.status {
            Status::SignedOut | Status::SigningIn => {
                self.asked = false;
                self.entering = None;
                self.words = None;
            }
            Status::FirstDevice => self.entering = None,
            Status::On if matches!(was, Status::NeedsSecret | Status::Pairing) => {
                self.entering = None;
            }
            _ => {}
        }
        if let Some(entering) = &mut self.entering {
            entering.refused = false;
        }
        let had_server = !self.state.server.is_empty();
        let finished = state.status == Status::On && was != Status::On && self.words.is_none();
        self.state = state;
        if finished {
            cx.emit(AccountEvent::Finished);
        }
        // A build that names no server, and a user who hasn't named one, sign in to the one typed
        // here, so the field stays until the core has a server.
        let signed_out = self.state.status == Status::SignedOut;
        match self.no_server() {
            true if signed_out && self.server.is_none() => self.change_server(cx),
            false if !had_server => self.server = None,
            _ => {}
        }
        cx.notify();
    }

    /// The account has no server to sign in to yet: the build names none (branding/sync-server)
    /// and the user hasn't typed one.
    fn no_server(&self) -> bool {
        self.state.server.is_empty()
    }

    /// The account's devices, as the core listed them.
    pub fn set_devices(&mut self, devices: Vec<Device>, cx: &mut Context<Self>) {
        self.devices = Some(devices);
        cx.notify();
    }

    /// Where the button named `name` was last drawn, for a test to press.
    pub fn drawn(&self, name: &str) -> Option<Bounds<Pixels>> {
        self.drawn.borrow().get(name).copied()
    }

    /// What the account shows asks the core for what it needs, only once it shows: the first
    /// device's words, a new device's request to the others, and the devices.
    fn ask_what_shows(&mut self) {
        // The words cover the rest until Done.
        if self.words.is_some() {
            return;
        }
        match self.state.status {
            Status::FirstDevice => {
                let words = self.host.recovery_words();
                if !words.is_empty() {
                    self.show_words(words);
                }
            }
            Status::NeedsSecret if !self.asked && self.entering.is_none() => {
                if self.state.problem.is_none() {
                    self.asked = true;
                    self.host.request_pairing();
                }
            }
            Status::On if self.devices.is_none() => {
                self.devices = Some(Vec::new());
                self.host.read_devices();
            }
            _ => {}
        }
    }

    fn show_words(&mut self, words: String) {
        let words = words.split_whitespace().map(str::to_owned).collect();
        self.words = Some(Words { words, copied: None });
    }

    /// The server as the account names it: the build's own by the app's name, else its host.
    fn server_name(&self) -> SharedString {
        match self.state.default_server {
            true => env!("APP_NAME").into(),
            false => host_of(&self.state.server).to_owned().into(),
        }
    }

    /// Signs in to the server named, as its Sign In button does.
    pub fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.no_server() {
            true if !self.keep_server(window, cx) => return,
            true => {}
            false => self.server = None,
        }
        self.host.sign_in();
        cx.notify();
    }

    fn change_server(&mut self, cx: &mut Context<Self>) {
        let server = self.state.server.clone();
        let input = cx.new(|cx| {
            TextInput::new("https://", cx)
                .placeholder_color(self.look.detail)
                .caret_color(self.look.ink)
                .with_text(server)
        });
        self.server = Some(Field { input, refused: false, focus: true });
        cx.notify();
    }

    /// Keeps the address typed, unless the core won't take it, and returns whether it did.
    fn keep_server(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let no_server = self.no_server();
        let Some(field) = &mut self.server else {
            return false;
        };
        let server = field.input.read(cx).text().trim().to_owned();
        let kept = !server.is_empty()
            && (server == self.state.server.as_ref() || self.host.set_server(&server));
        field.refused = !kept;
        if kept && !no_server {
            self.server = None;
        }
        if kept {
            window.focus(&self.focus, cx);
        }
        cx.notify();
        kept
    }

    fn enter_words(&mut self, cx: &mut Context<Self>) {
        if self.state.status == Status::Pairing {
            self.host.cancel();
        }
        let input = cx.new(|cx| {
            TextInput::new("24 words", cx)
                .placeholder_color(self.look.detail)
                .caret_color(self.look.ink)
        });
        self.entering = Some(Field { input, refused: false, focus: true });
        cx.notify();
    }

    fn keep_words(&mut self, cx: &mut Context<Self>) {
        let Some(field) = &mut self.entering else {
            return;
        };
        let words = field.input.read(cx).text().to_owned();
        field.refused = !self.host.enter_recovery_words(&words);
        cx.notify();
    }

    fn leave_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // With no server, the field is the only way to name one, so it stays.
        let stays = self.no_server() && self.server.is_some();
        if !stays && self.server.take().is_none() {
            self.entering = None;
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn ask_again(&mut self, cx: &mut Context<Self>) {
        self.asked = true;
        self.host.request_pairing();
        cx.notify();
    }

    fn copy_words(&mut self, cx: &mut Context<Self>) {
        let Some(words) = &mut self.words else {
            return;
        };
        self.host.copy_secret(&words.words.join(" "), WORDS_KEPT, cx);
        words.copied = Some(cx.background_executor().now());
        // Once Copied is Copy again, and once the clipboard has let the words go.
        for after in [COPIED, WORDS_KEPT] {
            cx.spawn(async move |account, cx| {
                cx.background_executor().timer(after).await;
                account.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// Signs out, asking first if this is the account's only device, as its words are then the
    /// only way to read what it synced.
    fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let alone = self.devices.as_ref().is_some_and(|devices| devices.len() == 1);
        if !alone || self.state.status != Status::On {
            self.host.sign_out();
            return;
        }
        let host = self.host.clone();
        self.host.confirm(
            format!("Sign out of {}?", self.state.email),
            "This is the account's only device. To sync again, you'll need your recovery words."
                .into(),
            "Sign Out",
            Return::Action,
            Box::new(move |_| host.sign_out()),
            window,
            cx,
        );
    }

    /// Starts the account over, once the user confirms, for someone with neither another device
    /// nor the words: the core then makes new words, which show as on a first device.
    fn start_over(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let host = self.host.clone();
        self.host.confirm(
            "Erase your synced data and start over?".into(),
            "This erases everything synced on the server, for every device. What's on this \
             device stays, and syncs again under new recovery words. Your old words stop working."
                .into(),
            "Erase and Start Over",
            // It erases every device's synced data, which nothing brings back, so only a click
            // does.
            Return::Nothing,
            Box::new(move |_| host.start_over()),
            window,
            cx,
        );
    }

    /// Shows the words again, once the user confirms, as anyone who sees them can read what
    /// syncs.
    fn show_recovery_words(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let account = cx.entity().downgrade();
        self.host.confirm(
            "Show your recovery words?".into(),
            "Anyone who sees them can read what you sync, passwords too.".into(),
            "Show",
            Return::Action,
            Box::new(move |cx| {
                account
                    .update(cx, |account, cx| {
                        let words = account.host.recovery_words();
                        if !words.is_empty() {
                            account.show_words(words);
                            cx.notify();
                        }
                    })
                    .ok();
            }),
            window,
            cx,
        );
    }

    /// A button that records where it's drawn, by `name`.
    fn button(
        &self,
        name: impl Into<String>,
        title: impl Into<SharedString>,
        cx: &mut Context<Self>,
        clicked: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> gpui::Stateful<gpui::Div> {
        let name = name.into();
        button(&self.look, SharedString::from(name.clone()), title)
            .relative()
            .child(self.place(name))
            .on_click(cx.listener(move |account, _, window, cx| clicked(account, window, cx)))
    }

    /// A quiet link in a line of small grey text, brightening under the pointer, that records where
    /// it's drawn, by `name`.
    fn link(
        &self,
        name: &'static str,
        title: &'static str,
        cx: &mut Context<Self>,
        clicked: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> gpui::Stateful<gpui::Div> {
        let ink = self.look.ink;
        div()
            .id(name)
            .relative()
            .hover(move |link| link.text_color(ink))
            .child(title)
            .child(self.place(name.into()))
            .on_click(cx.listener(move |account, _, window, cx| clicked(account, window, cx)))
    }

    /// Records where what it's in is drawn, by `name`.
    /// Logs it as "browser: settings sync `name` at x y", its centre, each time it moves, for
    /// check.sh to click it there.
    fn place(&self, name: String) -> impl IntoElement + use<> {
        let drawn = self.drawn.clone();
        canvas(
            move |bounds, _, _| {
                if drawn.borrow_mut().insert(name.clone(), bounds) != Some(bounds) {
                    let (x, y) = (f32::from(bounds.center().x), f32::from(bounds.center().y));
                    log!("browser: settings sync {name} at {x:.0} {y:.0}");
                }
            },
            |_, _, _, _| {},
        )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
    }

    /// Signed out: what signing in does, and the server it signs in to, which Change… changes. As
    /// the first launch's step, whose title and line say what signing in does and whose button
    /// signs in, the box says only what it has to: Cancel while the sign-in waits, or why the last
    /// didn't go.
    fn signed_out(&self, cx: &mut Context<Self>) -> gpui::Div {
        let signing_in = self.state.status == Status::SigningIn;
        let control = match signing_in {
            true => self.button("cancel", "Cancel", cx, |account, _, cx| {
                account.host.cancel();
                cx.notify();
            }),
            false => {
                self.button("sign-in", "Sign In", cx, |account, window, cx| {
                    account.sign_in(window, cx)
                })
            }
        };
        let detail = match (signing_in, self.state.problem) {
            (true, _) => Some("Finish in the window that opened."),
            // The address it needs, not a server it couldn't reach, which the user can't act on.
            (false, _) if self.no_server() => Some("Enter your sync server's address to sign in."),
            (false, Some(problem)) => Some(problem_text(problem)),
            (false, None) => None,
        };
        let server = match &self.server {
            Some(field) => self.field(field, "That isn't a server address ilo can use.", cx),
            None => {
                let name = self.server_name();
                div()
                    .pl(px(PADDING))
                    .flex()
                    // Under the step's button, centred as it is.
                    .when(self.step, |line| line.pl_0().justify_center())
                    .text_size(px(text::SMALL))
                    .text_color(self.look.detail)
                    .child(format!("Server: {name} · "))
                    .when(!signing_in, |row| {
                        row.child(self.link("change-server", "Change…", cx, |account, _, cx| {
                            account.change_server(cx)
                        }))
                    })
            }
        };
        let said = match self.step {
            true => detail.map(|detail| {
                let control = signing_in.then(|| control.into_any_element());
                lines(&self.look, detail, None, control)
            }),
            false => Some(lines(
                &self.look,
                "Sync Spaces, bookmarks, passwords and open tabs across your devices.",
                detail.map(SharedString::from),
                Some(control.into_any_element()),
            )),
        };
        let boxed = said.is_some();
        div()
            .children(said.map(|said| group(&self.look, [said])))
            .child(div().when(boxed, |below| below.mt(px(BUTTONS_TOP))).child(server))
    }

    /// The field in `field`, and what it says when what was typed wasn't taken.
    fn field(&self, field: &Field, refused: &'static str, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .child(
                group(&self.look, [div()
                    .relative()
                    .px(px(PADDING))
                    .py(px(PADDING))
                    .key_context("AccountField")
                    .on_action(cx.listener(|account, _: &Keep, window, cx| {
                        match account.server.is_some() {
                            true => _ = account.keep_server(window, cx),
                            false => account.keep_words(cx),
                        }
                    }))
                    .on_action(cx.listener(|account, _: &Leave, window, cx| {
                        account.leave_field(window, cx)
                    }))
                    .child(self.place("field".into()))
                    .child(field.input.clone())]),
            )
            .when(field.refused, |below| {
                below.child(
                    div()
                        .mt(px(HEADER_GAP))
                        .pl(px(PADDING))
                        .text_size(px(text::SMALL))
                        .text_color(self.look.detail)
                        .child(refused),
                )
            })
    }

    /// The words, numbered in columns, to write down, and Copy and Done.
    fn words(&self, words: &Words, cx: &mut Context<Self>) -> gpui::Div {
        let columns = WORDS_COLUMNS as usize;
        let rows = words.words.len().div_ceil(columns);
        // Down each column, as they're written down.
        let column = |c: usize| {
            div().flex_1().flex().flex_col().gap(px(WORD_GAP)).children(
                (0..rows).filter_map(|r| {
                    let index = c * rows + r;
                    let word = words.words.get(index)?;
                    Some(
                        div()
                            .flex()
                            .child(
                                div()
                                    .w(px(WORD_NUMBER))
                                    .text_color(self.look.detail)
                                    .child(format!("{}", index + 1)),
                            )
                            .child(word.clone()),
                    )
                }),
            )
        };
        // The executor's clock, which tests move on.
        let now = cx.background_executor().now();
        let copied = words.copied.is_some_and(|at| now - at < COPIED);
        let title = if copied { "Copied" } else { "Copy" };
        let copy = self.button("copy", title, cx, |account, _, cx| account.copy_words(cx));
        // Copied says it happened, with a check, until it's Copy again.
        let check = svg().path("icons/check.svg").ml(px(HEADER_GAP)).size(px(text::BODY));
        let copy = copy.when(copied, |copy| copy.child(check.text_color(self.look.ink)));
        div()
            .child(header("Your recovery words", true))
            .child(group(&self.look, [div()
                .px(px(PADDING))
                .py(px(PADDING))
                .flex()
                .children((0..columns).map(column))]))
            .child(note(
                &self.look,
                "Save these somewhere safe. You need them only if you lose every device.",
            ))
            // Where a copy went, and that it doesn't stay, while it's there.
            .when(words.copied.is_some_and(|at| now - at < WORDS_KEPT), |words| {
                words.child(note(&self.look, COPIED_NOTE).relative().child(self.place("copied".into())))
            })
            .child(
                div()
                    .mt(px(BUTTONS_TOP))
                    .flex()
                    .justify_between()
                    .child(copy)
                    .child(self.button("done", "Done", cx, |account, _, cx| {
                        account.words = None;
                        if account.state.status == Status::On {
                            cx.emit(AccountEvent::Finished);
                        }
                        cx.notify();
                    })),
            )
    }

    /// A new device: the code, while it waits for another device to add it, or the field for the
    /// words.
    fn new_device(&self, cx: &mut Context<Self>) -> gpui::Div {
        if let Some(field) = &self.entering {
            let refused = "These aren't 24 recovery words. Check each one.";
            let wrong = self.state.problem == Some(Problem::WrongWords) && !field.refused;
            return div()
                .child(header("Enter your recovery words", true))
                .child(self.field(field, refused, cx))
                .child(note(&self.look, match wrong {
                    true => problem_text(Problem::WrongWords),
                    false => "The 24 words you saved when you first turned on sync.",
                }))
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .flex()
                        .justify_between()
                        .child(self.button("back", "Back", cx, |account, window, cx| {
                            account.leave_field(window, cx)
                        }))
                        .child(self.button("continue", "Continue", cx, |account, _, cx| {
                            account.keep_words(cx)
                        })),
                )
                .child(
                    div()
                        .mt(px(BUTTONS_TOP))
                        .pl(px(PADDING))
                        .flex()
                        .text_size(px(text::SMALL))
                        .text_color(self.look.detail)
                        .child("Lost your devices and words? ")
                        .child(self.link("start-over", "Start over…", cx, |account, window, cx| {
                            account.start_over(window, cx)
                        })),
                );
        }
        let pairing = self.state.status == Status::Pairing;
        let waiting = match pairing {
            true => {
                let code = match self.state.code.is_empty() {
                    true => div().text_color(self.look.detail).child("––– –––"),
                    false => div().child(grouped(&self.state.code)),
                };
                lines(
                    &self.look,
                    format!(
                        "On a device signed in to {}, {} asks to add this one. Add it there if it \
                         shows this code.",
                        self.state.email,
                        env!("APP_NAME"),
                    ),
                    None,
                    Some(self.button("cancel", "Cancel", cx, |account, _, cx| {
                        account.host.cancel();
                        cx.notify();
                    })
                    .into_any_element()),
                )
                .child(
                    code.mt(px(PADDING))
                        .text_size(px(text::HEADING))
                        .font_weight(FontWeight::SEMIBOLD),
                )
            }
            false => lines(
                &self.look,
                self.state.problem.map_or(
                    "Ask a device already signed in to add this one.",
                    problem_text,
                ),
                None,
                Some(self.button("ask-again", "Ask Again", cx, |account, _, cx| {
                    account.ask_again(cx)
                })
                .into_any_element()),
            ),
        };
        div()
            .child(header("Approve on another device", true))
            .child(group(&self.look, [waiting]))
            .child(
                div()
                    .mt(px(BUTTONS_TOP))
                    .flex()
                    .justify_between()
                    .child(self.button("use-words", "Use Recovery Words Instead", cx, |a, _, cx| {
                        a.enter_words(cx)
                    }))
                    .child(self.button("sign-out", "Sign Out", cx, |account, window, cx| {
                        account.sign_out(window, cx)
                    })),
            )
    }

    /// Signed in: the account, its devices, and the way to see the words again.
    fn signed_in(&self, cx: &mut Context<Self>) -> gpui::Div {
        let sign_out = self.button("sign-out", "Sign Out", cx, |account, window, cx| {
            account.sign_out(window, cx)
        });
        let account = lines(
            &self.look,
            self.state.email.clone(),
            Some(format!("Syncing through {}", self.server_name()).into()),
            Some(sign_out.into_any_element()),
        );
        let devices = self.devices.iter().flatten().map(|device| {
            let control = match device.current {
                true => div().text_color(self.look.detail).child("This device").into_any_element(),
                false => {
                    let id = device.id.clone();
                    let remove = move |account: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
                        account.host.remove_device(&id);
                        cx.notify();
                    };
                    self.button(format!("remove {}", device.id), "Remove", cx, remove)
                        .into_any_element()
                }
            };
            div()
                .h(px(PROFILE_ROW))
                .px(px(PADDING))
                .flex()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().child(device.name.clone()))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::SMALL))
                                .text_color(self.look.detail)
                                .child(device.os.clone()),
                        ),
                )
                .child(div().flex_none().ml(px(PADDING)).child(control))
        });
        let devices: Vec<gpui::Div> = devices.collect();
        div()
            .flex()
            .flex_col()
            .child(group(&self.look, [account]))
            .when(!devices.is_empty(), |section| {
                section
                    .child(header("Devices", false))
                    .child(group(&self.look, devices))
            })
            .child(div().mt(px(BUTTONS_TOP)).flex().child(self.button(
                "show-words",
                "Show Recovery Words…",
                cx,
                |account, window, cx| account.show_recovery_words(window, cx),
            )))
    }

    /// Gives a field just shown the keyboard.
    fn focus_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for field in [&mut self.server, &mut self.entering].into_iter().flatten() {
            if std::mem::take(&mut field.focus) {
                window.focus(&field.input.focus_handle(cx), cx);
            }
        }
    }
}

/// The host of a server's address, as the row names it.
fn host_of(server: &str) -> &str {
    let rest = server.split_once("://").map_or(server, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// A header over a box, as the Shortcuts tab's.
fn header(title: &'static str, first: bool) -> gpui::Div {
    div()
        .when(!first, |header| header.mt(px(SECTION_GAP)))
        .mb(px(HEADER_GAP))
        .pl(px(PADDING))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

/// A grey line under a box.
fn note(look: &Look, text: &'static str) -> gpui::Div {
    div()
        .mt(px(HEADER_GAP))
        .pl(px(PADDING))
        .text_size(px(text::SMALL))
        .text_color(look.detail)
        .child(text)
}

/// A row whose text wraps, with a grey line under it if any, and its control at the right.
fn lines(
    look: &Look,
    text: impl Into<SharedString>,
    detail: Option<SharedString>,
    control: Option<gpui::AnyElement>,
) -> gpui::Div {
    div().px(px(PADDING)).py(px(PADDING)).child(
        div()
            .flex()
            .items_center()
            .gap(px(BOX_GAP))
            .child(
                div().flex_1().min_w_0().child(text.into()).children(detail.map(|detail| {
                    div().text_size(px(text::SMALL)).text_color(look.detail).child(detail)
                })),
            )
            .children(control.map(|control| div().flex_none().child(control))),
    )
}

impl Focusable for Account {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Account {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ask_what_shows();
        self.focus_field(window, cx);
        self.drawn.borrow_mut().clear();
        let content = match (&self.words, self.state.status) {
            (Some(words), _) => self.words(words, cx),
            (None, Status::SignedOut | Status::SigningIn) => self.signed_out(cx),
            (None, Status::NeedsSecret | Status::Pairing) => self.new_device(cx),
            (None, Status::On) => self.signed_in(cx),
            // Nothing until the core has made the first device's words.
            (None, Status::FirstDevice) => div(),
        };
        div().track_focus(&self.focus).text_color(self.look.ink).child(content)
    }
}

#[cfg(test)]
mod tests;
