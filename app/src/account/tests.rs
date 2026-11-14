//! Settings › Sync, driven as the user does, on a host that records what it asks of the core.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{App, Entity, Modifiers, TestAppContext, VisualTestContext, Window, size};

use crate::dialog;

use super::*;

/// 24 words, as the core gives them: lowercase, one space between them.
const WORDS: &str = "abandon ability able about above absent absorb abstract absurd abuse access \
                     accident account accuse achieve acid acoustic acquire across act action \
                     actor actress actual";

/// The core, as the view asks it, recording each call.
#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    /// What set_server and enter_recovery_words answer.
    refuse: Cell<bool>,
    /// The question asked, and what confirming it does, until a test answers.
    asked: RefCell<Option<(String, Box<dyn FnOnce(&mut App)>)>>,
    /// Where it asks instead, as Settings does on Linux, for a test that answers with keys.
    dialog: RefCell<Option<Entity<dialog::Dialog>>>,
    /// What activity answers.
    activity: RefCell<Activity>,
    /// Whether it copies a secret as Linux's host does, which can't itself: through GPUI's
    /// clipboard, with the clear after (copy_for).
    copies_through_gpui: Cell<bool>,
}

impl Recorder {
    fn record(&self, call: impl Into<String>) {
        self.calls.borrow_mut().push(call.into());
    }

    fn take(&self) -> Vec<String> {
        self.calls.take()
    }
}

impl Host for Recorder {
    fn set_server(&self, server: &str) -> bool {
        self.record(format!("server {server}"));
        !self.refuse.get()
    }

    fn sign_in(&self) {
        self.record("sign in");
    }

    fn cancel(&self) {
        self.record("cancel");
    }

    fn sign_out(&self) {
        self.record("sign out");
    }

    fn recovery_words(&self) -> String {
        self.record("recovery words");
        WORDS.into()
    }

    fn enter_recovery_words(&self, words: &str) -> bool {
        self.record(format!("words {}", words.split_whitespace().count()));
        !self.refuse.get()
    }

    fn start_over(&self) {
        self.record("start over");
    }

    fn request_pairing(&self) {
        self.record("request pairing");
    }

    fn read_devices(&self) {
        self.record("read devices");
    }

    fn activity(&self) -> Activity {
        self.record("activity");
        self.activity.borrow().clone()
    }

    fn delete_account(&self) {
        self.record("delete account");
    }

    fn remove_device(&self, id: &str) {
        self.record(format!("remove {id}"));
    }

    fn choose(&self, what: Choice, on: bool) {
        self.record(format!("choose {what:?} {on}"));
    }

    fn copy_secret(&self, text: &str, kept: Duration, cx: &mut App) {
        self.record(format!(
            "copy secret {} {}",
            text.split(' ').count(),
            kept.as_secs()
        ));
        if self.copies_through_gpui.get() {
            copy_for(text, kept, cx);
        }
    }

    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        on_return: Return,
        confirmed: Box<dyn FnOnce(&mut App)>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.record(format!("ask {message} {action}"));
        match self.dialog.borrow().clone() {
            Some(dialog) => dialog::ask_in(
                &dialog, message, detail, action, on_return, confirmed, window, cx,
            ),
            None => *self.asked.borrow_mut() = Some((message, confirmed)),
        }
    }
}

/// The Sync tab under the dialog it asks in, as Settings draws it on Linux.
struct Page {
    account: Entity<Account>,
    dialog: Entity<dialog::Dialog>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(self.account.clone())
            .children(dialog::cover(&self.dialog, cx))
    }
}

/// The dialog's window, which is the tab's own.
struct InWindow;

impl dialog::Host for InWindow {
    fn show(&self, _: Option<u64>, _: Option<Bounds<Pixels>>) {}

    fn hide(&self) {}
}

struct Shown<'a> {
    account: Entity<Account>,
    cx: &'a mut VisualTestContext,
    host: Rc<Recorder>,
}

/// The Sync tab, drawn, as the core says it is in `state`.
fn shown(state: State, cx: &mut TestAppContext) -> Shown<'_> {
    cx.update(|cx| {
        crate::text_input::bind_keys(cx);
        bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    let core = host.clone();
    let (account, cx) = cx.add_window_view(|_, cx| Account::new(core, cx));
    account.update(cx, |account, cx| account.set_state(state, cx));
    cx.run_until_parked();
    Shown { account, cx, host }
}

fn state(status: Status) -> State {
    State {
        status,
        server: "https://sync.example.org".into(),
        default_server: true,
        email: "ada@example.org".into(),
        ..State::default()
    }
}

fn device(id: &str, current: bool) -> Device {
    Device {
        id: id.into(),
        name: format!("Device {id}").into(),
        os: "macOS".into(),
        current,
    }
}

impl Shown<'_> {
    fn set(&mut self, state: State) {
        self.account
            .update(self.cx, |account, cx| account.set_state(state, cx));
        self.cx.run_until_parked();
    }

    fn set_devices(&mut self, devices: Vec<Device>) {
        self.account
            .update(self.cx, |account, cx| account.set_devices(devices, cx));
        self.cx.run_until_parked();
    }

    fn drawn(&mut self, name: &str) -> bool {
        self.account
            .read_with(self.cx, |account, _| account.drawn(name))
            .is_some()
    }

    /// Clicks the button named `name`, which must be drawn.
    fn click(&mut self, name: &str) {
        let bounds = self
            .account
            .read_with(self.cx, |account, _| account.drawn(name));
        let bounds = bounds.unwrap_or_else(|| panic!("no {name} drawn"));
        self.cx.simulate_click(bounds.center(), Modifiers::none());
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    fn type_text(&mut self, text: &str) {
        self.cx.simulate_input(text);
        self.cx.run_until_parked();
    }

    /// Answers the question asked with its action.
    fn confirm(&mut self) {
        let (_, confirmed) = self.host.asked.take().expect("nothing was asked");
        self.cx.update(|_, cx| confirmed(cx));
        self.cx.run_until_parked();
    }

    fn showing_words(&mut self) -> bool {
        self.account
            .read_with(self.cx, |account, _| account.words.is_some())
    }
}

/// Signed out, Sign In opens the sign-in window, and while it's open Cancel closes it.
#[gpui::test]
fn sign_in_and_cancel(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::SignedOut), cx);
    tab.click("sign-in");
    assert_eq!(tab.host.take(), ["sign in"]);
    tab.set(state(Status::SigningIn));
    assert!(!tab.drawn("sign-in") && !tab.drawn("change-server"));
    tab.click("cancel");
    assert_eq!(tab.host.take(), ["cancel"]);
    tab.set(state(Status::SignedOut));
    assert!(tab.drawn("sign-in"));
}

/// Change… shows the server's address in a field, all selected: ↩ keeps what's typed over it,
/// unless the core won't take it, and escape leaves it as it was.
#[gpui::test]
fn change_takes_the_servers_address(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::SignedOut), cx);
    tab.click("change-server");
    tab.host.refuse.set(true);
    tab.type_text("ftp://example.org");
    tab.keys("enter");
    assert_eq!(tab.host.take(), ["server ftp://example.org"]);
    let refused = tab.account.read_with(tab.cx, |account, _| {
        account.server.as_ref().map(|f| f.refused)
    });
    assert_eq!(refused, Some(true), "the field stays, saying so");

    tab.keys("escape");
    assert!(
        tab.account
            .read_with(tab.cx, |account, _| account.server.is_none())
    );
    assert!(tab.host.take().is_empty());

    tab.host.refuse.set(false);
    tab.click("change-server");
    tab.type_text("https://ilo.example.net");
    tab.keys("enter");
    assert_eq!(tab.host.take(), ["server https://ilo.example.net"]);
    assert!(
        tab.account
            .read_with(tab.cx, |account, _| account.server.is_none())
    );
}

/// A build that names no server, before the user has named one, asks for the address in place of
/// a server it can't reach: the field is open, Sign In signs in to what's typed there, and the
/// field stays until the core has the server, as nothing else could name one.
#[gpui::test]
fn with_no_server_sign_in_asks_for_its_address(cx: &mut TestAppContext) {
    let none = State {
        server: "".into(),
        default_server: false,
        ..state(Status::SignedOut)
    };
    let mut tab = shown(none.clone(), cx);
    let field = |tab: &mut Shown| {
        tab.account.read_with(tab.cx, |account, _| {
            account.server.as_ref().map(|f| f.refused)
        })
    };
    assert_eq!(field(&mut tab), Some(false), "it asks for the address");
    assert!(tab.drawn("sign-in") && !tab.drawn("change-server"));

    tab.click("sign-in");
    assert!(tab.host.take().is_empty(), "nothing to sign in to");
    assert_eq!(field(&mut tab), Some(true));
    tab.keys("escape");
    assert!(field(&mut tab).is_some(), "the field stays");

    // The core's answer to a sign-in with no server says only to name one.
    tab.set(State {
        problem: Some(Problem::Server),
        ..none.clone()
    });
    assert!(field(&mut tab).is_some());
    tab.click("field");
    tab.type_text("https://sync.example.net");
    tab.click("sign-in");
    assert_eq!(
        tab.host.take(),
        ["server https://sync.example.net", "sign in"]
    );
    assert!(field(&mut tab).is_some(), "until the core has it");
    tab.set(State {
        server: "https://sync.example.net".into(),
        ..none
    });
    assert_eq!(field(&mut tab), None);
    assert!(tab.drawn("change-server"));
}

/// The account's first device shows its words once, made as the tab shows them, with Copy and
/// Done, and then it's signed in, listing its devices.
#[gpui::test]
fn the_first_device_shows_its_words_once(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::FirstDevice), cx);
    assert_eq!(tab.host.take(), ["recovery words"]);
    assert!(tab.showing_words());
    // Making them turned sync on; the words stay until Done.
    tab.set(state(Status::On));
    assert!(tab.showing_words());
    tab.click("copy");
    assert_eq!(tab.host.take(), ["activity", "copy secret 24 90"]);
    tab.click("done");
    assert!(!tab.showing_words());
    assert_eq!(tab.host.take(), ["read devices"]);
    assert!(tab.drawn("sign-out") && tab.drawn("show-words"));
}

/// Copy puts the words on the clipboard as a secret, for this device alone and off it after a
/// minute and a half, and says so for as long as they're there (browser#641).
#[gpui::test]
fn copied_words_say_they_leave_the_clipboard(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::FirstDevice), cx);
    tab.host.take();
    tab.click("copy");
    assert_eq!(tab.host.take(), ["copy secret 24 90"]);
    assert!(tab.drawn("copied"));
    tab.cx.executor().advance_clock(Duration::from_secs(89));
    tab.cx.run_until_parked();
    assert!(tab.drawn("copied"), "still on the clipboard");
    tab.cx.executor().advance_clock(Duration::from_secs(2));
    tab.cx.run_until_parked();
    assert!(!tab.drawn("copied"), "the clipboard has let them go");
}

/// Where the host can't copy a secret (Linux), the words go on GPUI's clipboard with KDE's password
/// manager hint, which keeps them out of clipboard histories that honour it, and off it after the
/// minute and a half, unless something else was copied since.
#[gpui::test]
fn the_clipboard_lets_the_words_go(cx: &mut TestAppContext) {
    let clipboard = |cx: &mut TestAppContext| cx.read(|cx| cx.read_from_clipboard()?.text());
    cx.update(|cx| copy_for(WORDS, WORDS_KEPT, cx));
    assert_eq!(clipboard(cx).as_deref(), Some(WORDS));
    let hint = cx.read(|cx| cx.read_from_clipboard()?.metadata().cloned());
    assert_eq!(hint.as_deref(), Some("x-kde-passwordManagerHint"));
    cx.executor().advance_clock(Duration::from_secs(89));
    assert_eq!(clipboard(cx).as_deref(), Some(WORDS));
    cx.executor().advance_clock(Duration::from_secs(2));
    assert_eq!(clipboard(cx).unwrap_or_default(), "", "the words have gone");

    cx.update(|cx| copy_for(WORDS, WORDS_KEPT, cx));
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string("mine".into())));
    cx.executor().advance_clock(WORDS_KEPT * 2);
    assert_eq!(
        clipboard(cx).as_deref(),
        Some("mine"),
        "the user's own copy stays"
    );
}

thread_local! {
    /// How often the browser's reader was asked, and the text GPUI holds the clipboard with.
    static READS: Cell<u32> = const { Cell::new(0) };
    static HELD: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Under Wayland, Chromium's reader read the app's own copy of the words as empty while the app
/// hadn't the keyboard, and the clear, which asked it, left them on the clipboard (sync-devices on
/// batch 1113-1114). The clear asks what GPUI holds the clipboard with instead and never the
/// reader: the words go after the minute and a half, and a copy made elsewhere since stays.
#[gpui::test]
fn the_words_go_by_what_the_app_holds_not_the_browsers_reader(cx: &mut TestAppContext) {
    // A reader that never answers, as the clear never heard the words back.
    crate::clipboard::read_through(|_| READS.set(READS.get() + 1));
    crate::clipboard::held_through(|| HELD.with_borrow(Clone::clone));
    let mut tab = shown(state(Status::FirstDevice), cx);
    tab.host.copies_through_gpui.set(true);
    tab.host.take();
    let clipboard = |tab: &mut Shown| tab.cx.read(|cx| cx.read_from_clipboard()?.text());

    tab.click("copy");
    assert_eq!(tab.host.take(), ["copy secret 24 90"]);
    assert_eq!(clipboard(&mut tab).as_deref(), Some(WORDS));
    HELD.set(Some(WORDS.into()));
    tab.cx.executor().advance_clock(Duration::from_secs(89));
    tab.cx.run_until_parked();
    assert_eq!(clipboard(&mut tab).as_deref(), Some(WORDS), "still there");
    tab.cx.executor().advance_clock(Duration::from_secs(2));
    tab.cx.run_until_parked();
    assert_eq!(
        clipboard(&mut tab).unwrap_or_default(),
        "",
        "the words have gone"
    );

    // Another app copied since, so GPUI's source was cancelled and it holds nothing: the clear
    // leaves the clipboard alone, whatever GPUI last wrote.
    tab.click("copy");
    HELD.set(None);
    tab.cx.executor().advance_clock(WORDS_KEPT * 2);
    tab.cx.run_until_parked();
    assert_eq!(
        clipboard(&mut tab).as_deref(),
        Some(WORDS),
        "the clear wrote nothing"
    );
    assert_eq!(READS.get(), 0, "the clear never asked the browser's reader");
}

/// A new device asks the others to add it once, by itself, and shows the code once one answers.
/// Cancel takes the request back, and it asks again only when the user does.
#[gpui::test]
fn a_new_device_asks_once_and_cancel_takes_it_back(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::NeedsSecret), cx);
    assert_eq!(tab.host.take(), ["request pairing"]);
    tab.set(state(Status::Pairing));
    tab.set(State {
        code: "312568".into(),
        ..state(Status::Pairing)
    });
    assert!(tab.drawn("cancel") && tab.drawn("use-words") && tab.drawn("sign-out"));
    tab.click("cancel");
    assert_eq!(tab.host.take(), ["cancel"]);
    tab.set(state(Status::NeedsSecret));
    assert!(tab.host.take().is_empty(), "it doesn't ask again by itself");
    tab.click("ask-again");
    assert_eq!(tab.host.take(), ["request pairing"]);

    // Declined, it says so, and waits for the user too.
    tab.set(state(Status::Pairing));
    tab.set(State {
        problem: Some(Problem::Declined),
        ..state(Status::NeedsSecret)
    });
    assert!(tab.host.take().is_empty());
    assert!(tab.drawn("ask-again"));
}

/// A device signed in after a problem, such as one removed and signing in again, still asks once.
#[gpui::test]
fn signing_in_again_asks_again(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::NeedsSecret), cx);
    assert_eq!(tab.host.take(), ["request pairing"]);
    tab.set(State {
        problem: Some(Problem::Removed),
        ..state(Status::SignedOut)
    });
    tab.set(state(Status::SigningIn));
    tab.set(state(Status::NeedsSecret));
    assert_eq!(tab.host.take(), ["request pairing"]);
}

/// Use Recovery Words Instead takes the request back and shows a field for them: words that
/// aren't 24 of the list stay, saying so, and words for another account too, until the core
/// takes them.
#[gpui::test]
fn the_recovery_words_instead(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::NeedsSecret), cx);
    tab.set(state(Status::Pairing));
    tab.host.take();
    tab.click("use-words");
    assert_eq!(tab.host.take(), ["cancel"]);
    tab.set(state(Status::NeedsSecret));
    assert!(
        tab.host.take().is_empty(),
        "it doesn't ask the other devices while they're typed"
    );

    tab.host.refuse.set(true);
    tab.type_text("abandon ability");
    tab.keys("enter");
    assert_eq!(tab.host.take(), ["words 2"]);
    let refused = tab.account.read_with(tab.cx, |account, _| {
        account.entering.as_ref().map(|f| f.refused)
    });
    assert_eq!(refused, Some(true));

    tab.host.refuse.set(false);
    tab.keys("cmd-a");
    tab.type_text(WORDS);
    tab.click("continue");
    assert_eq!(tab.host.take(), ["words 24"]);
    tab.set(State {
        problem: Some(Problem::WrongWords),
        ..state(Status::NeedsSecret)
    });
    assert!(
        tab.drawn("continue"),
        "the field stays for the words to be fixed"
    );
    tab.set(state(Status::On));
    assert!(
        tab.account
            .read_with(tab.cx, |account, _| account.entering.is_none())
    );
    assert_eq!(tab.host.take(), ["activity", "read devices"]);
}

/// Someone with neither another device nor the words starts over from where the words are typed,
/// once they confirm, and the account's first device again shows its new words.
#[gpui::test]
fn start_over_asks_then_shows_new_words(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::NeedsSecret), cx);
    tab.click("use-words");
    tab.host.take();
    tab.click("start-over");
    let asked = "ask Erase your synced data and start over? Erase and Start Over";
    assert_eq!(tab.host.take(), [asked], "nothing goes until confirmed");
    tab.confirm();
    assert_eq!(tab.host.take(), ["start over"]);
    tab.set(state(Status::FirstDevice));
    assert!(tab.showing_words());
    assert!(
        tab.account
            .read_with(tab.cx, |account, _| account.entering.is_none())
    );
}

/// ↩ in Start over's question does nothing, as erasing every device's synced data takes a click:
/// the core is never asked to start over, and the account stays as it was until escape cancels.
#[gpui::test]
fn return_never_starts_over(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::text_input::bind_keys(cx);
        bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    let core = host.clone();
    let (page, cx) = cx.add_window_view(|_, cx| {
        let dialog = dialog::in_window(Rc::new(InWindow), cx);
        *core.dialog.borrow_mut() = Some(dialog.clone());
        Page {
            account: cx.new(|cx| Account::new(core, cx)),
            dialog,
        }
    });
    cx.simulate_resize(size(px(684.), px(650.)));
    let account = page.read_with(cx, |page, _| page.account.clone());
    let mut tab = Shown { account, cx, host };
    tab.set(state(Status::NeedsSecret));
    tab.click("use-words");
    tab.host.take();
    tab.click("start-over");
    let asked = "ask Erase your synced data and start over? Erase and Start Over";
    assert_eq!(tab.host.take(), [asked]);
    let asking = |tab: &mut Shown| {
        let page = page.clone();
        tab.cx
            .update(|_, cx| dialog::cover(&page.read(cx).dialog, cx).is_some())
    };
    assert!(asking(&mut tab));

    tab.keys("enter");
    tab.cx.executor().advance_clock(Duration::from_secs(1));
    tab.cx.run_until_parked();
    assert!(tab.host.take().is_empty(), "↩ never starts over");
    assert!(asking(&mut tab), "the question stays for a click");
    let now = tab
        .account
        .read_with(tab.cx, |account, _| account.state().clone());
    assert_eq!(now, state(Status::NeedsSecret));

    tab.keys("escape");
    tab.cx.executor().advance_clock(Duration::from_secs(1));
    tab.cx.run_until_parked();
    assert!(!asking(&mut tab));
    assert!(tab.host.take().is_empty());
}

/// Back from the field goes back to waiting for another device, which the user asks for again.
#[gpui::test]
fn back_from_the_words(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::NeedsSecret), cx);
    tab.click("use-words");
    tab.click("back");
    assert!(tab.drawn("ask-again"));
    assert_eq!(tab.host.take(), ["request pairing"]);
}

/// Signed in, the devices are listed, this one marked and the others with Remove.
#[gpui::test]
fn the_devices_and_remove(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::On), cx);
    assert_eq!(tab.host.take(), ["activity", "read devices"]);
    tab.set_devices(vec![device("this", true), device("phone", false)]);
    assert!(!tab.drawn("remove this"));
    tab.click("remove phone");
    assert_eq!(tab.host.take(), ["remove phone"]);
}

/// Signed in, each of what else can sync has its switch, which asks the core for the other way,
/// and shows what the core last said: a choice it didn't take goes back.
#[gpui::test]
fn the_choices_switch_both_ways(cx: &mut TestAppContext) {
    let on = State {
        extensions: true,
        ..state(Status::On)
    };
    let mut tab = shown(on.clone(), cx);
    tab.host.take();
    tab.click("switch settings");
    tab.click("switch extensions");
    assert_eq!(
        tab.host.take(),
        ["choose Settings true", "choose Extensions false"]
    );
    tab.set(on);
    tab.host.take();
    tab.click("switch settings");
    assert_eq!(tab.host.take(), ["choose Settings true"]);
}

/// Show Recovery Words… asks first, as anyone who sees them can read what syncs.
#[gpui::test]
fn show_recovery_words_asks_first(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::On), cx);
    tab.host.take();
    tab.click("show-words");
    assert_eq!(tab.host.take(), ["ask Show your recovery words? Show"]);
    assert!(!tab.showing_words());
    tab.confirm();
    assert_eq!(tab.host.take(), ["recovery words"]);
    assert!(tab.showing_words());
    tab.click("done");
    assert!(tab.drawn("show-words"));
}

/// Sign Out asks first only on the account's only device, whose words are then the only way back
/// to what it synced.
#[gpui::test]
fn sign_out_asks_only_on_the_only_device(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::On), cx);
    tab.set_devices(vec![device("this", true), device("phone", false)]);
    tab.host.take();
    tab.click("sign-out");
    assert_eq!(tab.host.take(), ["sign out"]);

    tab.set_devices(vec![device("this", true)]);
    tab.click("sign-out");
    assert_eq!(
        tab.host.take(),
        ["ask Sign out of ada@example.org? Sign Out"]
    );
    tab.confirm();
    assert_eq!(tab.host.take(), ["sign out"]);
}

/// Every state has its way out, drawn: Sign In, Cancel, Sign Out, Done.
#[gpui::test]
fn every_state_has_its_way_out(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::SignedOut), cx);
    let ways = [
        (state(Status::SignedOut), "sign-in"),
        (state(Status::SigningIn), "cancel"),
        (state(Status::NeedsSecret), "sign-out"),
        (state(Status::Pairing), "cancel"),
        (state(Status::On), "sign-out"),
        (state(Status::FirstDevice), "done"),
    ];
    for (state, way) in ways {
        let status = state.status;
        tab.set(state);
        assert!(tab.drawn(way), "{status:?} has no {way}");
    }
}

/// The dialog on the other device goes by what the server says, when the request came and when
/// the device signed in, gives the device's name and system only as its own word, and asks the
/// user to match the code on the device in front of them, in two threes (browser#642).
#[test]
fn the_other_device_asks_with_the_code() {
    let pairing = Pairing {
        id: "p".into(),
        name: "Pixel 9".into(),
        os: "Android".into(),
        code: "312568".into(),
        asked: "15:04".into(),
        signed_in: "2 minutes ago".into(),
    };
    let question = pairing_question(&pairing);
    assert_eq!(
        question.message.as_ref(),
        "Add the device that asked at 15:04?"
    );
    assert_eq!(
        question.detail.as_ref(),
        "It signed in to your account 2 minutes ago. It calls itself “Pixel 9” on Android, which \
         nothing checks. Add it only if the device in front of you shows 312\u{a0}568."
    );
    assert_eq!(
        (question.action.as_ref(), question.cancel.as_ref()),
        ("Add", "Not me")
    );
    assert_eq!(question.on_return, Return::Cancel);
    // A server that doesn't say when it signed in leaves that out.
    let unnamed = Pairing {
        name: "".into(),
        os: "".into(),
        signed_in: "".into(),
        ..pairing.clone()
    };
    assert_eq!(
        pairing_question(&unnamed).detail.as_ref(),
        "Add it only if the device in front of you shows 312\u{a0}568."
    );
    assert_eq!(grouped("007"), "007");
    assert_eq!(grouped("312568"), "312\u{a0}568", "on one line");

    // A name can't pass for the dialog's own words, with lines of its own or at length.
    let forged = Pairing {
        name: "MacBook\nIt's yours. Add it".into(),
        os: "".into(),
        signed_in: "".into(),
        ..pairing
    };
    assert_eq!(
        pairing_question(&forged).detail.as_ref(),
        "It calls itself “MacBook It's yours. Add it”, which nothing checks. Add it only if the \
         device in front of you shows 312\u{a0}568."
    );
    assert_eq!(given(&"a".repeat(60)).chars().count(), 40);
}

/// How long ago a device signed in, in the one unit that reads at a glance.
#[test]
fn since_reads_in_one_unit() {
    let minute = 60_000.;
    assert_eq!(since(0.), "just now");
    assert_eq!(since(59_000.), "just now");
    assert_eq!(since(-minute), "just now", "a server clock a little ahead");
    assert_eq!(since(minute), "1 minute ago");
    assert_eq!(since(59. * minute), "59 minutes ago");
    assert_eq!(since(60. * minute), "1 hour ago");
    assert_eq!(since(47. * 60. * minute), "47 hours ago");
    assert_eq!(since(48. * 60. * minute), "2 days ago");
}

/// The time of day by the user's calendar, whichever day it is.
#[test]
fn the_clock_reads_the_users_day() {
    let today = 1_700_000_000_000.;
    assert_eq!(
        clock(today + (15. * 60. + 4.) * 60_000. + 59_000., today),
        "15:04"
    );
    assert_eq!(clock(today, today), "00:00");
    assert_eq!(clock(today - 60_000., today), "23:59");
}

/// A server of the user's is named by its host, the build's own by the app's name.
#[test]
fn the_server_is_named_by_its_host() {
    assert_eq!(host_of("https://sync.example.org/"), "sync.example.org");
    assert_eq!(host_of("http://127.0.0.1:8080/x?y"), "127.0.0.1:8080");
    assert_eq!(host_of("sync.example.org"), "sync.example.org");
}

impl Shown<'_> {
    fn status(&mut self) -> (Option<String>, String) {
        self.account
            .read_with(self.cx, |account, _| account.status_words())
    }

    /// The core's answer to how sync is keeping up, as the view reads it when Settings shows.
    fn shows_with(&mut self, synced: &str, trouble: Option<Trouble>) {
        *self.host.activity.borrow_mut() = Activity {
            synced: synced.to_string().into(),
            trouble,
        };
        self.account
            .update(self.cx, |account, cx| account.read_activity(cx));
        self.cx.run_until_parked();
    }
}

/// Signed in, the line under the email says when it last synced, through which server, and once
/// something stops it, what and what happens next, read from the core only as Settings shows.
#[gpui::test]
fn the_status_line_says_when_it_synced_and_what_stops_it(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::On), cx);
    let app = env!("APP_NAME");
    assert_eq!(tab.status(), (None, format!("Syncing through {app}")));
    assert!(tab.host.take().contains(&"activity".to_string()));

    tab.shows_with("at 15:04", None);
    assert_eq!(
        tab.status(),
        (None, format!("Synced at 15:04, through {app}"))
    );
    tab.shows_with("at 15:04", Some(Trouble::Unreachable));
    assert_eq!(
        tab.status(),
        (
            Some(format!("Can't reach {app}'s server.")),
            format!("Last synced at 15:04; {app} keeps trying.")
        )
    );
    tab.shows_with("", Some(Trouble::Throttled));
    assert_eq!(
        tab.status(),
        (
            Some("The server asked to wait.".into()),
            format!("{app} syncs again soon.")
        )
    );
    tab.shows_with("at 15:04", Some(Trouble::Stopped));
    assert_eq!(
        tab.status().0.as_deref(),
        Some("Sync stopped after an error.")
    );
    // A newer version on another device wrote the Spaces: this one has to update to sync them.
    tab.shows_with("at 15:04", Some(Trouble::Update));
    assert_eq!(
        tab.status(),
        (
            Some(format!("Update {app} to keep syncing Spaces.")),
            format!("A newer {app} on another device changed them. The rest still syncs.")
        )
    );

    // A server of the user's own is named by its host.
    tab.set(State {
        default_server: false,
        ..state(Status::On)
    });
    tab.shows_with("yesterday at 09:30", Some(Trouble::Unreachable));
    assert_eq!(
        tab.status().0.as_deref(),
        Some("Can't reach sync.example.org.")
    );

    // Nothing reads it again until Settings shows again, however long it stays open.
    tab.host.take();
    tab.cx.run_until_parked();
    assert!(tab.host.take().is_empty());

    // Signed out, the core isn't asked.
    tab.set(state(Status::SignedOut));
    assert!(!tab.host.take().contains(&"activity".to_string()));
}

/// Delete Account asks first, saying what goes and what stays, only a click deletes, and the core
/// signs the browser out after.
#[gpui::test]
fn delete_account_asks_first(cx: &mut TestAppContext) {
    let mut tab = shown(state(Status::On), cx);
    tab.set_devices(vec![device("this", true), device("phone", false)]);
    tab.host.take();
    tab.click("delete-account");
    assert_eq!(
        tab.host.take(),
        ["ask Delete the account ada@example.org? Delete Account"]
    );
    tab.confirm();
    assert_eq!(tab.host.take(), ["delete account"]);

    // A device without the secret deletes it too, for someone who lost the words and devices.
    tab.set(state(Status::NeedsSecret));
    tab.host.take();
    tab.click("delete-account");
    tab.confirm();
    assert_eq!(
        tab.host.take(),
        [
            "ask Delete the account ada@example.org? Delete Account",
            "delete account"
        ]
    );
}

/// ↩ in Delete Account's question doesn't delete, as nothing brings the account back: only a click
/// does.
#[gpui::test]
fn return_never_deletes_the_account(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::text_input::bind_keys(cx);
        bind_keys(cx);
    });
    let host = Rc::new(Recorder::default());
    let core = host.clone();
    let (page, cx) = cx.add_window_view(|_, cx| {
        let dialog = dialog::in_window(Rc::new(InWindow), cx);
        *core.dialog.borrow_mut() = Some(dialog.clone());
        Page {
            account: cx.new(|cx| Account::new(core, cx)),
            dialog,
        }
    });
    cx.simulate_resize(size(px(684.), px(650.)));
    let account = page.read_with(cx, |page, _| page.account.clone());
    let mut tab = Shown { account, cx, host };
    tab.set(state(Status::On));
    tab.host.take();
    tab.click("delete-account");
    let asked = "ask Delete the account ada@example.org? Delete Account";
    assert_eq!(tab.host.take(), [asked]);
    tab.keys("enter");
    tab.cx.executor().advance_clock(Duration::from_secs(1));
    tab.cx.run_until_parked();
    assert!(tab.host.take().is_empty(), "↩ never deletes the account");
    let asking = tab
        .cx
        .update(|_, cx| dialog::cover(&page.read(cx).dialog, cx).is_some());
    assert!(asking, "the question stays for a click");
}

/// The last sync reads as a clock time today and yesterday, then in days, and not at all before
/// the first.
#[test]
fn the_last_sync_reads_off_the_users_days() {
    let hour = 3_600_000.;
    let today = 1_000. * 24. * hour;
    let days = [
        today,
        today - 24. * hour,
        today - 48. * hour,
        today - 72. * hour,
    ];
    assert_eq!(
        synced_when(today + 15. * hour + 4. * 60_000., days),
        "at 15:04"
    );
    assert_eq!(synced_when(today - 14.5 * hour, days), "yesterday at 09:30");
    assert_eq!(synced_when(today - 30. * hour, days), "2 days ago");
    assert_eq!(synced_when(today - 80. * hour, days), "4 days ago");
    assert_eq!(synced_when(0., days), "");
}

/// What an assistive app finds in the tab as it's drawn now: each control it can press and each
/// place ⇥ stops, all with a role and a name. Returns the names it can press.
fn spoken_names(tab: &mut Shown) -> Vec<String> {
    use crate::settings_rows::spoken::{pressable, stops, unnamed};
    let pressable = pressable(tab.cx);
    assert_eq!(
        unnamed(&pressable),
        [],
        "a control without a name: {pressable:?}"
    );
    let stops = stops(tab.cx);
    assert_eq!(unnamed(&stops), [], "a stop without a name: {stops:?}");
    assert!(!stops.is_empty(), "⇥ reaches none of it");
    pressable.into_iter().map(|(_, name, _)| name).collect()
}

/// Every control in each of the Sync tab's views is one an assistive app can name, and ⇥ stops
/// only at those (#1911): a button says what it does, a device's Remove which device it takes,
/// and the fields what goes in them.
#[gpui::test]
fn every_control_in_sync_has_a_name(cx: &mut TestAppContext) {
    use gpui::accesskit::Role;
    let mut tab = shown(state(Status::SignedOut), cx);
    let names = spoken_names(&mut tab);
    assert!(names.contains(&"Sign In".into()), "{names:?}");
    assert!(names.contains(&"Change Server…".into()), "{names:?}");

    tab.click("change-server");
    tab.cx.run_until_parked();
    spoken_names(&mut tab);
    let tree = tab.cx.a11y_tree().expect("a tree");
    let field = (tree.nodes.iter()).find(|(_, node)| node.role() == Role::TextInput);
    assert_eq!(
        field.and_then(|(_, node)| node.label()),
        Some("Sync Server Address"),
        "the server's field is called by its placeholder"
    );
    tab.keys("escape");

    tab.set(state(Status::SigningIn));
    assert!(spoken_names(&mut tab).contains(&"Cancel Sign In".into()));

    tab.set(state(Status::NeedsSecret));
    let names = spoken_names(&mut tab);
    for name in ["Ask Again", "Use Recovery Words Instead", "Sign Out"] {
        assert!(names.contains(&name.into()), "{name} in {names:?}");
    }
    tab.set(state(Status::Pairing));
    assert!(spoken_names(&mut tab).contains(&"Cancel Request".into()));
    tab.click("use-words");
    tab.set(state(Status::NeedsSecret));
    let names = spoken_names(&mut tab);
    for name in ["Back", "Continue", "Start Over…"] {
        assert!(names.contains(&name.into()), "{name} in {names:?}");
    }
    tab.click("back");

    tab.set(state(Status::FirstDevice));
    let names = spoken_names(&mut tab);
    for name in ["Copy Recovery Words", "Done"] {
        assert!(names.contains(&name.into()), "{name} in {names:?}");
    }
    tab.click("copy");
    assert!(spoken_names(&mut tab).contains(&"Recovery Words Copied".into()));
    tab.set(state(Status::On));
    tab.click("done");
    tab.set_devices(vec![device("a", true), device("b", false)]);
    let names = spoken_names(&mut tab);
    for name in ["Sign Out", "Remove Device b", "Show Recovery Words…"] {
        assert!(names.contains(&name.into()), "{name} in {names:?}");
    }
}
