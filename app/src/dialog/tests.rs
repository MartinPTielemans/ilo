//! The dialog's keys and clicks, on a host that records what the dialog asks of its window.

use std::{cell::RefCell, rc::Rc};

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px, size};

use super::*;
use crate::settling::SETTLING;

/// What the dialog asks of its window, and apart, in order, its holds of the keys around its hide.
#[derive(Default)]
struct Recorder(RefCell<Vec<String>>, RefCell<Vec<&'static str>>);

impl Host for Recorder {
    fn show(&self, window: Option<u64>, over: Option<Bounds<Pixels>>, keyboard: bool) {
        let mut call = match over {
            Some(over) => format!("show over {}", f32::from(over.origin.x)),
            None => "show".into(),
        };
        if let Some(window) = window {
            call += &format!(" in {window}");
        }
        if !keyboard {
            call += " at rest";
        }
        self.0.borrow_mut().push(call);
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
        self.1.borrow_mut().push("hide");
    }

    fn hold_keys(&self) {
        self.1.borrow_mut().push("hold");
    }

    fn let_go(&self) {
        self.1.borrow_mut().push("let go");
    }

    fn give_held_keys(&self) {
        self.1.borrow_mut().push("give");
    }
}

/// What the dialog asks of the host about the keys as it starts leaving: it holds them, and on the
/// Mac it lets go of the clicks and the keyboard too (#3681).
fn leaving() -> &'static [&'static str] {
    match LETS_GO_AS_IT_LEAVES {
        true => &["hold", "let go"],
        false => &["hold"],
    }
}

/// The dialog, asking to delete a Space. The action records itself on the host when it runs.
struct Asking {
    cx: VisualTestContext,
    host: Rc<Recorder>,
}

impl Asking {
    fn new(cx: &mut TestAppContext) -> Self {
        let host = Rc::new(Recorder::default());
        let window = cx
            .update(|cx| open(host.clone(), cx))
            .expect("no window for the dialog");
        let deleted = host.clone();
        cx.update(|cx| {
            let delete = Box::new(move |_: &mut App| deleted.0.borrow_mut().push("delete".into()));
            let detail = "Its 3 tabs go with it. ⌘Z brings them back.".into();
            let (message, detail) = ("Delete “Work”?".into(), detail);
            ask(None, message, detail, "Delete", delete, cx);
        });
        let cx = VisualTestContext::from_window(window.into(), cx);
        cx.simulate_resize(size(px(800.), px(600.)));
        cx.run_until_parked();
        Asking { cx, host }
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    fn click(&mut self, x: f32, y: f32) {
        self.cx
            .simulate_click(point(px(x), px(y)), Modifiers::none());
        self.cx.run_until_parked();
    }

    /// What the dialog asked of the host since the last time, once it has had time to leave.
    fn calls(&mut self) -> Vec<String> {
        self.cx
            .executor()
            .advance_clock(Duration::from_secs_f32(MOVE));
        self.cx.run_until_parked();
        self.host.0.take()
    }
}

/// The keys typed as the dialog leaves, such as ⌥⌘N right after Esc, are held from the answer
/// until it has hidden, then given to the browser window, which has the keyboard back by then,
/// rather than lost to the dialog going (#1930). A click's answer holds them the same way.
#[gpui::test]
fn the_keys_typed_as_it_leaves_wait_for_the_window(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    dialog.keys("escape");
    let leaving = leaving();
    assert_eq!(
        *dialog.host.1.borrow(),
        leaving,
        "it didn't hold the keys as it left"
    );
    dialog.calls();
    assert_eq!(
        *dialog.host.1.borrow(),
        [leaving, &["hide", "give"]].concat()
    );
}

/// On the Mac a page's question taken back, as its tab closed, lets go of the clicks and the
/// keyboard as it starts to leave, and still holds the keys until it has hidden: a click on the
/// page as it leaves reaches the page, and a key typed after it goes there (#3681).
#[gpui::test]
fn a_question_taken_back_lets_go_as_it_leaves_on_the_mac(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    dialog.cx.cx.update(withdraw_page);
    let leaving = leaving();
    assert_eq!(
        *dialog.host.1.borrow(),
        leaving,
        "taken back, it didn't let go as it started to leave"
    );
    dialog.calls();
    assert_eq!(
        *dialog.host.1.borrow(),
        [leaving, &["hide", "give"]].concat()
    );
}

/// A question answered or taken back keeps the app's shortcuts waiting no longer, though it still
/// draws its leaving: on Linux the keys are the browser window's at once (#3617).
#[gpui::test]
fn the_apps_shortcuts_wait_only_while_it_awaits_its_answer(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    let awaits = |dialog: &mut Asking| dialog.cx.cx.update(|cx| (asking(cx), awaits_answer(cx)));
    assert_eq!(
        awaits(&mut dialog),
        (true, true),
        "it didn't wait for its answer"
    );
    dialog.cx.cx.update(withdraw_page);
    assert_eq!(
        awaits(&mut dialog),
        (true, false),
        "taken back, it kept the shortcuts waiting as it left"
    );
    dialog.calls();
    assert_eq!(awaits(&mut dialog), (false, false));

    let mut dialog = Asking::new(cx);
    dialog.keys("escape");
    assert_eq!(
        awaits(&mut dialog),
        (true, false),
        "answered, it kept the shortcuts waiting as it left"
    );
}

#[gpui::test]
fn escape_cancels(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["show", "hide"]);
}

#[gpui::test]
fn asked_to_close_it_cancels_and_its_window_stays(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    // As Windows asks for Alt+F4 (#1362).
    assert!(
        !dialog.cx.simulate_close(),
        "the dialog's window let itself be closed"
    );
    assert_eq!(dialog.calls(), ["show", "hide"]);
}

#[gpui::test]
fn return_deletes_once_the_dialog_has_gone(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    dialog.keys("enter");
    assert_eq!(dialog.host.0.borrow().as_slice(), ["show"]);
    // A second ↩ while it leaves does nothing more.
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show", "hide", "delete"]);
}

#[gpui::test]
fn a_click_around_the_card_cancels(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    // The card's middle, and then the dimmed window above it.
    dialog.click(400., 300.);
    assert_eq!(dialog.calls(), ["show"]);
    dialog.click(400., 40.);
    assert_eq!(dialog.calls(), ["hide"]);
}

/// VoiceOver reads the card as an alert by its question, with its detail after, and its buttons
/// by their labels, and its press on one answers as a click does (#1381).
#[gpui::test]
fn voiceover_reads_the_question_and_its_buttons(cx: &mut TestAppContext) {
    use gpui::accesskit::{Action, Role};
    let mut dialog = Asking::new(cx);
    dialog.cx.simulate_a11y_activation();
    dialog.cx.run_until_parked();
    let tree = dialog.cx.a11y_tree().expect("GPUI built a tree");
    let find = |role: Role, label: &str| {
        let found = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == role && node.label() == Some(label));
        found
            .unwrap_or_else(|| panic!("no {role:?} called {label}"))
            .clone()
    };
    let (_, alert) = find(Role::AlertDialog, "Delete “Work”?");
    assert_eq!(
        alert.description(),
        Some("Its 3 tabs go with it. ⌘Z brings them back.")
    );
    assert_eq!(tree.focus, find(Role::AlertDialog, "Delete “Work”?").0);
    find(Role::Button, "Cancel");
    let (delete, _) = find(Role::Button, "Delete");
    dialog.cx.simulate_a11y_action(delete, Action::Click);
    assert_eq!(dialog.calls(), ["show", "hide", "delete"]);
}

#[gpui::test]
fn an_ask_while_one_shows_is_dropped(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    dialog.cx.cx.update(|cx| {
        let clear = Box::new(|_: &mut App| panic!("the second dialog ran"));
        ask(
            None,
            "Clear the archive?".into(),
            String::new(),
            "Clear Archive",
            clear,
            cx,
        );
    });
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show", "hide", "delete"]);
}

#[gpui::test]
fn a_window_s_question_shows_over_that_window(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), cx))
        .expect("no window for the dialog");
    // A window behind another asks from its sidebar's context menu (#1432).
    cx.update(|cx| {
        let delete = Box::new(|_: &mut App| {});
        ask(
            Some(2),
            "Delete “Work”?".into(),
            String::new(),
            "Delete",
            delete,
            cx,
        );
    });
    cx.run_until_parked();
    assert_eq!(*host.0.borrow(), ["show in 2"]);
}

/// The dialog asking to quit, with "Quit, and don't ask again" at the left. Each answer records
/// itself on the host when it runs.
fn asking_to_quit(cx: &mut TestAppContext) -> Asking {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| open(host.clone(), cx))
        .expect("no window for the dialog");
    let (quit, always) = (host.clone(), host.clone());
    cx.update(|cx| {
        let question = Question {
            icon: None,
            message: "Quit Browser?".into(),
            detail: "".into(),
            list: Vec::new(),
            action: "Quit".into(),
            cancel: "Cancel".into(),
            tone: Tone::Accent,
            also: Some((
                "Quit, and don’t ask again".into(),
                Box::new(move |_: &mut App| always.0.borrow_mut().push("always".into())),
            )),
            fields: Vec::new(),
            check: None,
            on_return: Return::Action,
        };
        pose(
            None,
            question,
            Box::new(move |_, _| quit.0.borrow_mut().push("quit".into())),
            cx,
        );
    });
    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.), px(600.)));
    cx.run_until_parked();
    Asking { cx, host }
}

#[gpui::test]
fn return_quits_and_escape_doesnt(cx: &mut TestAppContext) {
    let mut dialog = asking_to_quit(cx);
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show", "hide", "quit"]);
    let mut dialog = asking_to_quit(cx);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["show", "hide"]);
}

#[gpui::test]
fn the_left_button_gives_the_other_answer(cx: &mut TestAppContext) {
    let mut dialog = asking_to_quit(cx);
    // The card is centred in the 800 × 600 window, still 32 above its place as it comes down:
    // 172 tall with no detail, its buttons 107 under its top and the left one 29 in.
    let card = (800. - WIDTH) / 2.;
    let buttons = (600. - 172.) / 2. - DROP + 1. + TEXT_INSET + TITLE_LINE + BUTTONS_UNDER_TITLE;
    dialog.click(card + 1. + BUTTON_INSET + 40., buttons + BUTTON_HEIGHT / 2.);
    assert_eq!(dialog.calls(), ["show", "hide", "always"]);
}

/// Chrome's question whether to add an extension, which records the answer on `host`.
fn add_extension(action: &str, host: &Rc<Recorder>, cx: &mut App) {
    let question = Question {
        icon: None,
        message: "Add \"Check\"?".into(),
        detail: "It can:".into(),
        list: vec![
            ("Read your browsing history".into(), Vec::new()),
            (
                "Read and change your data on example.org".into(),
                vec!["example.org".into()],
            ),
        ],
        action: action.to_owned().into(),
        cancel: "Cancel".into(),
        tone: Tone::Accent,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: Return::Action,
    };
    let host = host.clone();
    let answered = Box::new(move |yes, _: &mut App| {
        host.0
            .borrow_mut()
            .push(if yes { "yes" } else { "no" }.into());
    });
    ask_chrome(question, answered, cx);
}

/// The dialog, open and asking nothing.
fn opened(cx: &mut TestAppContext) -> (VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| open(host.clone(), cx))
        .expect("no window for the dialog");
    let cx = VisualTestContext::from_window(window.into(), cx);
    cx.simulate_resize(size(px(800.), px(600.)));
    (cx, host)
}

impl Asking {
    /// The dialog, asking Chrome's question.
    fn chrome(action: &str, cx: &mut TestAppContext) -> Self {
        let (cx, host) = opened(cx);
        cx.cx.update(|cx| add_extension(action, &host, cx));
        cx.run_until_parked();
        Asking { cx, host }
    }

    /// The dialog has been on screen long enough to take a yes.
    fn settle(&mut self) {
        self.cx.executor().advance_clock(SETTLING);
        self.cx.run_until_parked();
    }
}

#[gpui::test]
fn return_adds_the_extension_once_the_dialog_has_gone(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("Add extension", cx);
    dialog.settle();
    dialog.keys("enter");
    assert_eq!(dialog.host.0.borrow().as_slice(), ["show"]);
    assert_eq!(dialog.calls(), ["show", "hide", "yes"]);
}

/// A page can bring Chrome's question up under a double-click the user started, so for its first
/// moment on screen ↩ doesn't add the extension, while escape still cancels.
#[gpui::test]
fn chromes_question_takes_no_yes_for_its_first_moment(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("Add extension", cx);
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show"]);
    dialog.settle();
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["hide", "yes"]);
}

#[gpui::test]
fn escape_tells_chrome_no(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("Add extension", cx);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["show", "hide", "no"]);
}

#[gpui::test]
fn without_an_action_only_cancel_answers(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("", cx);
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show"]);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["hide", "no"]);
}

#[gpui::test]
fn a_question_chrome_takes_back_leaves_untold(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("Add extension", cx);
    dialog.cx.cx.update(withdraw_chrome);
    assert_eq!(dialog.calls(), ["show", "hide"]);
    // ↩ while it leaves answers nothing.
    dialog.keys("enter");
    assert_eq!(dialog.calls(), Vec::<String>::new());
}

/// Chrome's question whether to quit with a download running, where ↩ keeps it, as in Chrome's.
fn asking_about_downloads(cx: &mut TestAppContext) -> Asking {
    let (cx, host) = opened(cx);
    let answers = host.clone();
    cx.cx.update(|cx| {
        let question = Question {
            icon: None,
            message: "Download is in progress".into(),
            detail: "Quit Browser anyway?".into(),
            list: Vec::new(),
            action: "Quit".into(),
            cancel: "Continue Downloading".into(),
            tone: Tone::Destructive,
            also: None,
            fields: Vec::new(),
            check: None,
            on_return: Return::Cancel,
        };
        let answered = Box::new(move |quit, _: &mut App| {
            answers
                .0
                .borrow_mut()
                .push(if quit { "quit" } else { "continue" }.into());
        });
        ask_chrome(question, answered, cx);
    });
    cx.run_until_parked();
    Asking { cx, host }
}

/// The drivers choose a button by its label, as they do an alert's on the Mac.
#[gpui::test]
fn a_driver_chooses_a_button_by_its_label(cx: &mut TestAppContext) {
    let mut dialog = asking_about_downloads(cx);
    dialog.settle();
    assert!(!dialog.cx.cx.update(|cx| choose("Exit", cx)));
    assert!(dialog.cx.cx.update(|cx| choose("Quit", cx)));
    assert_eq!(dialog.calls(), ["show", "hide", "quit"]);
    let mut dialog = asking_about_downloads(cx);
    assert!(dialog.cx.cx.update(|cx| choose("Continue Downloading", cx)));
    assert_eq!(dialog.calls(), ["show", "hide", "continue"]);
    // With nothing asked, there's nothing to choose.
    assert!(!dialog.cx.cx.update(|cx| choose("Quit", cx)));
    let mut dialog = asking_about_downloads(cx);
    dialog.settle();
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show", "hide", "continue"]);
}

#[gpui::test]
fn chromes_question_waits_for_the_dialog_that_shows(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    let host = dialog.host.clone();
    dialog
        .cx
        .cx
        .update(|cx| add_extension("Add extension", &host, cx));
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show", "hide", "delete", "show"]);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["hide", "no"]);
}

#[gpui::test]
fn chromes_next_question_takes_the_place_of_the_last(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("Add extension", cx);
    let host = dialog.host.clone();
    dialog
        .cx
        .cx
        .update(|cx| add_extension("Re-enable", &host, cx));
    // The first leaves untold, and the second comes up after it.
    assert_eq!(dialog.calls(), ["show", "hide", "show"]);
    dialog.settle();
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["hide", "yes"]);
}

#[gpui::test]
fn with_no_dialog_chrome_hears_no_at_once(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| add_extension("Add extension", &host, cx));
    assert_eq!(host.0.take(), ["no"]);
}

/// The account's question whether to add a device, which records the answer on `host`.
fn add_device(host: &Rc<Recorder>, cx: &mut App) {
    let pairing = crate::account::Pairing {
        id: "p".into(),
        name: "Pixel 9".into(),
        os: "Android".into(),
        code: "312568".into(),
        asked: "15:04".into(),
        signed_in: "2 minutes ago".into(),
    };
    let host = host.clone();
    let answered = Box::new(move |yes, _: &mut App| {
        host.0
            .borrow_mut()
            .push(if yes { "add" } else { "not me" }.into());
    });
    ask_account(crate::account::pairing_question(&pairing), answered, cx);
}

/// Another device asks to be added while Chrome asks about an extension: it waits its turn, and
/// neither takes the other's place. ↩ is Not me, even once it has settled, so a key meant for the
/// page can't add a device: Add takes a click.
#[gpui::test]
fn the_accounts_question_waits_for_chromes_and_return_is_not_me(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("Add extension", cx);
    let host = dialog.host.clone();
    dialog.cx.cx.update(|cx| add_device(&host, cx));
    assert_eq!(dialog.calls(), ["show"]);
    dialog.settle();
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["hide", "yes", "show"]);
    dialog.settle();
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["hide", "not me"]);
}

/// The core takes the account's question back, as another device took the request: it leaves
/// untold, and Chrome's waiting behind it comes up.
#[gpui::test]
fn the_account_takes_its_question_back(cx: &mut TestAppContext) {
    let (cx, host) = opened(cx);
    let mut dialog = Asking { cx, host };
    let host = dialog.host.clone();
    dialog.cx.cx.update(|cx| add_device(&host, cx));
    dialog
        .cx
        .cx
        .update(|cx| add_extension("Add extension", &host, cx));
    dialog.cx.cx.update(withdraw_account);
    assert_eq!(dialog.calls(), ["show", "hide", "show"]);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["hide", "no"]);
}

/// A page's question, over its tab's card at `x`, which records its answer, with what was typed,
/// on `host`.
fn ask_from_page(
    (tab, id): (u64, u64),
    question: Question,
    x: f32,
    host: &Rc<Recorder>,
    cx: &mut App,
) {
    ask_from_pane((tab, id), question, x, false, host, cx);
}

/// Asks as `ask_from_page` does, at rest if `rests`, where a click that focuses its pane says
/// "focus pane" on `host`.
fn ask_from_pane(
    (tab, id): (u64, u64),
    question: Question,
    x: f32,
    rests: bool,
    host: &Rc<Recorder>,
    cx: &mut App,
) {
    let over = (
        Bounds::new(point(px(x), px(10.)), size(px(600.), px(500.))),
        10.,
    );
    let focused = host.clone();
    let host = host.clone();
    let answered = Box::new(move |reply: Reply, _: &mut App| {
        let answer = if reply.accept { "accept" } else { "cancel" };
        let checked = if reply.checked { " checked" } else { "" };
        host.0
            .borrow_mut()
            .push(format!("{answer} {}{checked}", reply.texts.join(" ")));
    });
    let focus_pane = Rc::new(move |_: &mut App| focused.0.borrow_mut().push("focus pane".into()));
    ask_page((tab, id), question, over, rests, focus_pane, answered, cx);
}

/// What a page asks: a confirm(), or with `fields` such as a prompt's, and a checkbox.
fn page_question(fields: Vec<Field>, check: Option<&'static str>) -> Question {
    Question {
        icon: None,
        message: "example.org says".into(),
        detail: "Go on?".into(),
        list: Vec::new(),
        action: "OK".into(),
        cancel: "Cancel".into(),
        tone: Tone::Accent,
        also: None,
        fields,
        check: check.map(Into::into),
        on_return: Return::Action,
    }
}

fn field(text: &str, secure: bool) -> Field {
    Field {
        placeholder: "".into(),
        text: text.to_owned().into(),
        secure,
    }
}

impl Asking {
    /// The dialog, asking a page's `question` of tab 1, as its dialog 7.
    fn page(question: Question, cx: &mut TestAppContext) -> Self {
        let (cx, host) = opened(cx);
        cx.cx.update(crate::text_input::bind_keys);
        cx.cx
            .update(|cx| ask_from_page((1, 7), question, 100., &host, cx));
        cx.run_until_parked();
        Asking { cx, host }
    }
}

/// What a page's dialog asked of the host from `shown` on, answered with `answer`: as it starts
/// leaving on Linux, whose page has the keyboard back then, and once it has hidden elsewhere.
fn left(shown: &[&str], answer: &str) -> Vec<String> {
    let mut calls: Vec<String> = shown.iter().map(|call| call.to_string()).collect();
    match PAGE_ANSWERED_AS_IT_LEAVES {
        true => calls.extend([answer.into(), "hide".into()]),
        false => calls.extend(["hide".into(), answer.into()]),
    }
    calls
}

/// On Linux the page has the keyboard back as its dialog starts leaving, and Chromium drops its keys
/// until its question is answered, so a shortcut typed right after Esc, such as ⌘L, needs the answer
/// at once (#1711, #1963).
#[gpui::test]
fn a_pages_question_is_answered_once_the_page_has_the_keyboard_back(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    dialog.keys("escape");
    let answered = dialog.host.0.borrow().iter().any(|call| call == "cancel ");
    assert_eq!(answered, PAGE_ANSWERED_AS_IT_LEAVES);
    assert_eq!(*dialog.host.1.borrow(), leaving(), "it hid before leaving");
}

impl Asking {
    /// The dialog, asking a page's confirm() of tab 1 at rest, over a split view's other pane.
    fn resting(cx: &mut TestAppContext) -> Self {
        let (cx, host) = opened(cx);
        cx.cx.update(crate::text_input::bind_keys);
        let question = page_question(Vec::new(), None);
        cx.cx
            .update(|cx| ask_from_pane((1, 7), question, 100., true, &host, cx));
        cx.run_until_parked();
        Asking { cx, host }
    }

    fn rest(&mut self, rests: bool) {
        let host = self.host.clone();
        let question = page_question(Vec::new(), None);
        self.cx
            .cx
            .update(|cx| ask_from_pane((1, 7), question, 100., rests, &host, cx));
        self.cx.run_until_parked();
    }

    /// The card's bounds in the dialog's window, as it last drew.
    fn card(&mut self) -> Bounds<Pixels> {
        self.cx.cx.update(|cx| {
            let window = cx.global::<DialogWindow>().0;
            window
                .read(cx)
                .unwrap()
                .asked
                .as_ref()
                .unwrap()
                .check_drawn
                .get()
                .card
        })
    }

    /// A point on OK, the card's last button, by its bottom right corner.
    fn on_ok(&mut self) -> (f32, f32) {
        let card = self.card();
        let x = f32::from(card.right()) - BUTTON_INSET - 12.;
        let y = f32::from(card.bottom()) - BUTTON_INSET - BUTTON_HEIGHT / 2.;
        (x, y)
    }
}

#[gpui::test]
fn a_pages_question_at_rest_leaves_the_keyboard_and_shortcuts(cx: &mut TestAppContext) {
    let mut dialog = Asking::resting(cx);
    let waits = |dialog: &mut Asking| dialog.cx.cx.update(|cx| (asking(cx), awaits_answer(cx)));
    assert_eq!(dialog.calls(), ["show over 100 at rest"]);
    // The app's shortcuts wait for no question at rest: they're the page beside's.
    assert_eq!(
        waits(&mut dialog),
        (false, false),
        "at rest it held the keys"
    );
    assert!(dialog.cx.cx.update(|cx| asks_page(cx)));
    // Its pane focused, it's live with the keyboard, and back at rest when the user leaves it.
    dialog.rest(false);
    assert_eq!(dialog.calls(), ["show over 100"]);
    assert_eq!(
        waits(&mut dialog),
        (true, true),
        "live, it didn't hold the keys"
    );
    dialog.rest(true);
    assert_eq!(dialog.calls(), ["show over 100 at rest"]);
    assert_eq!(
        waits(&mut dialog),
        (false, false),
        "back at rest it held the keys"
    );
}

#[gpui::test]
fn a_click_on_a_resting_card_focuses_its_pane_without_answering(cx: &mut TestAppContext) {
    let mut dialog = Asking::resting(cx);
    dialog.calls();
    let card = dialog.card();
    dialog.click(f32::from(card.center().x), f32::from(card.top()) + 20.);
    assert_eq!(dialog.calls(), ["focus pane"]);
    // And one on its dim beside the card, which a live one's leaves be.
    dialog.click(110., 20.);
    assert_eq!(dialog.calls(), ["focus pane"]);
}

#[gpui::test]
fn a_click_on_a_resting_ok_answers_it(cx: &mut TestAppContext) {
    let mut dialog = Asking::resting(cx);
    dialog.calls();
    let (x, y) = dialog.on_ok();
    dialog.click(x, y);
    let calls = dialog.calls();
    assert!(calls.contains(&"accept ".to_string()), "{calls:?}");
    assert!(calls.contains(&"focus pane".to_string()), "{calls:?}");
}

/// The click on a resting OK focuses its pane, so the shell asks again, live, while the card leaves
/// with its answer, before the shell hears it: the answered question never shows again (#3231).
#[gpui::test]
fn a_resting_ok_answered_isnt_asked_again_as_its_pane_comes_live(cx: &mut TestAppContext) {
    let mut dialog = Asking::resting(cx);
    dialog.calls();
    let (x, y) = dialog.on_ok();
    dialog.click(x, y);
    dialog.rest(false);
    let calls = dialog.calls();
    assert!(calls.contains(&"accept ".to_string()), "{calls:?}");
    let hidden = calls.iter().position(|call| call == "hide");
    assert!(
        hidden.is_some(),
        "the answered card didn't leave: {calls:?}"
    );
    assert!(
        !calls.iter().any(|call| call.starts_with("show")),
        "the answered question showed again: {calls:?}"
    );
    assert!(dialog.calls().is_empty());
}

/// A page's question taken back, as its tab no longer shows, and asked again while it leaves, as
/// the tab shows again, comes back once it has gone.
#[gpui::test]
fn a_pages_question_taken_back_and_asked_again_as_it_leaves_comes_back(cx: &mut TestAppContext) {
    let mut dialog = Asking::resting(cx);
    dialog.calls();
    dialog.cx.cx.update(withdraw_page);
    dialog.rest(true);
    assert_eq!(dialog.calls(), ["hide", "show over 100 at rest"]);
}

/// A card at rest stays on screen while the user picks View › Appearance, which redraws every
/// window before the shell gives the new palette, so the card draws again once the palette changes,
/// where it kept the old tone on the dialogs run under Wayland.
#[gpui::test]
fn a_resting_card_draws_in_the_windows_new_tone(cx: &mut TestAppContext) {
    let mut dialog = Asking::resting(cx);
    let drawn = |dialog: &mut Asking| {
        dialog.cx.cx.update(|cx| {
            let window = cx.global::<DialogWindow>().0;
            window.read(cx).unwrap().asked.as_ref().unwrap().drawn_tone
        })
    };
    let other = match drawn(&mut dialog) {
        Some(palette::Tone::Light) => palette::Tone::Dark,
        _ => palette::Tone::Light,
    };
    dialog.cx.cx.update(|cx| {
        let mut changed = palette::current(cx);
        changed.tone = other;
        palette::set_current(changed, cx);
    });
    dialog.cx.run_until_parked();
    assert_eq!(drawn(&mut dialog), Some(other));
}

#[gpui::test]
fn a_card_over_a_narrow_pane_takes_its_width(cx: &mut TestAppContext) {
    let (cx, host) = opened(cx);
    let mut dialog = Asking { cx, host };
    dialog.cx.cx.update(|cx| {
        let over = (
            Bounds::new(point(px(100.), px(10.)), size(px(300.), px(500.))),
            10.,
        );
        let answered = Box::new(|_: Reply, _: &mut App| {});
        let question = page_question(Vec::new(), None);
        ask_page((1, 7), question, over, true, Rc::new(|_| {}), answered, cx);
    });
    dialog.cx.run_until_parked();
    let card = dialog.card();
    // Inside its border, which the card's width takes in.
    assert_eq!(f32::from(card.size.width) + 2., 300. - 2. * PANE_MARGIN);
}

#[gpui::test]
fn a_pages_question_shows_over_its_card_and_return_answers_it(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    dialog.keys("enter");
    assert_eq!(dialog.calls(), left(&["show over 100"], "accept "));
}

#[gpui::test]
fn a_click_on_the_cards_dim_leaves_a_pages_question(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    dialog.click(400., 40.);
    assert_eq!(dialog.calls(), ["show over 100"]);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), left(&[], "cancel "));
}

/// A prompt's field stays as the window under the card goes from dark to light and back, and draws
/// in the palette the card draws in each time, not in the one it was made in.
#[gpui::test]
fn a_prompts_field_follows_the_look(cx: &mut TestAppContext) {
    use crate::palette::{Palette, Tone as Look};
    let mut dialog = Asking::page(page_question(vec![field("", false)], None), cx);
    let window = dialog.cx.read(|cx| cx.global::<DialogWindow>().0);
    let mut colours = Vec::new();
    for look in [Look::Light, Look::Dark, Look::Light] {
        let palette = Palette::of(None, look);
        dialog.cx.update(|window, cx| {
            palette::set_current(palette, cx);
            window.refresh();
        });
        dialog.cx.run_until_parked();
        let drawn = window
            .read_with(&dialog.cx, |dialog, cx| {
                let asked = dialog.asked.as_ref().expect("the question stays");
                asked.fields[0].read(cx).colors()
            })
            .expect("the dialog's window");
        let meant = Colors {
            placeholder: palette.on_overlay(FIELD_PLACEHOLDER),
            caret: Some(palette.accent.into()),
            selection: Some(palette.selection()),
        };
        assert_eq!(drawn, meant, "{look:?}");
        colours.push(drawn);
    }
    assert_ne!(colours[0], colours[1], "light and dark drew the same field");
    assert_eq!(colours[0], colours[2]);
}

#[gpui::test]
fn a_prompt_answers_what_was_typed(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(vec![field("Ada", false)], None), cx);
    // What it starts with is selected, so typing replaces it.
    dialog.cx.simulate_input("Grace");
    dialog.keys("enter");
    assert_eq!(dialog.calls(), left(&["show over 100"], "accept Grace"));
}

/// An input method composes "ぴ" in the prompt's field, or with `commit` ends its composition as it
/// does when it takes the key itself: committing `commit` in its place, or with it empty dropping
/// it. Returns what the field shows, and whether it's marked.
fn input_method(dialog: &mut Asking, commit: Option<&str>) -> (String, bool) {
    use gpui::EntityInputHandler as _;
    // The test's window is the dialog's.
    let shown = dialog.cx.update(|window, cx| {
        let dialog = window
            .root::<Dialog>()
            .flatten()
            .expect("the dialog's view");
        let field = dialog.read(cx).asked.as_ref().unwrap().fields[0].clone();
        field.update(cx, |field, cx| {
            match commit {
                None => field.replace_and_mark_text_in_range(None, "ぴ", None, window, cx),
                Some(text) => field.replace_text_in_range(None, text, window, cx),
            }
            (field.text().to_string(), field.composing())
        })
    });
    dialog.cx.run_until_parked();
    shown
}

/// What the prompt's field shows, and whether it's marked.
fn marked(dialog: &mut Asking) -> (String, bool) {
    dialog.cx.update(|window, cx| {
        let dialog = window
            .root::<Dialog>()
            .flatten()
            .expect("the dialog's view");
        let field = dialog.read(cx).asked.as_ref().unwrap().fields[0].read(cx);
        (field.text().to_string(), field.composing())
    })
}

/// ↩ while an input method composes in a prompt's field is the composition's: it answers nothing
/// and leaves the marked text for the method. Once the method has committed it, the next ↩
/// answers with it.
#[gpui::test]
fn return_while_composing_answers_nothing(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(vec![field("", false)], None), cx);
    dialog.cx.simulate_input("a");
    input_method(&mut dialog, None);
    dialog.keys("enter");
    assert_eq!(
        dialog.calls(),
        ["show over 100"],
        "↩ answered while composing"
    );
    assert_eq!(
        marked(&mut dialog),
        ("aぴ".into(), true),
        "↩ ended the composition"
    );
    input_method(&mut dialog, Some("ぴ"));
    dialog.keys("enter");
    assert_eq!(dialog.calls(), left(&[], "accept aぴ"));
}

/// Esc while an input method composes in a prompt's field is the composition's: it cancels
/// nothing and leaves the marked text for the method. Once the method has dropped it, the next Esc
/// cancels.
#[gpui::test]
fn escape_while_composing_cancels_nothing(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(vec![field("", false)], None), cx);
    dialog.cx.simulate_input("a");
    input_method(&mut dialog, None);
    dialog.keys("escape");
    assert_eq!(
        dialog.calls(),
        ["show over 100"],
        "esc cancelled while composing"
    );
    assert_eq!(
        marked(&mut dialog),
        ("aぴ".into(), true),
        "esc ended the composition"
    );
    input_method(&mut dialog, Some(""));
    dialog.keys("escape");
    assert_eq!(dialog.calls(), left(&[], "cancel a"));
}

#[gpui::test]
fn tab_moves_to_the_next_field(cx: &mut TestAppContext) {
    let fields = vec![field("", false), field("", true)];
    let mut dialog = Asking::page(page_question(fields, Some("Remember")), cx);
    dialog.cx.simulate_input("ada");
    dialog.keys("tab");
    dialog.cx.simulate_input("secret");
    dialog.keys("enter");
    assert_eq!(
        dialog.calls(),
        left(&["show over 100"], "accept ada secret")
    );
}

#[gpui::test]
fn a_reply_never_shows_what_was_typed(cx: &mut TestAppContext) {
    let reply = Reply {
        accept: true,
        texts: vec!["ada".into(), "secret".into()],
        checked: false,
    };
    let shown = format!("{reply:?}");
    assert!(
        !shown.contains("secret") && !shown.contains("ada"),
        "{shown}"
    );
    // Nor does the field show it, though it answers it.
    let mut dialog = Asking::page(page_question(vec![field("", true)], None), cx);
    dialog.cx.simulate_input("secret");
    let shown = dialog.cx.cx.update(|cx| {
        let window = cx.global::<DialogWindow>().0;
        window.read(cx).unwrap().asked.as_ref().unwrap().fields[0]
            .read(cx)
            .shown()
            .to_string()
    });
    assert_eq!(shown, "••••••");
    dialog.keys("enter");
    assert_eq!(dialog.calls(), left(&["show over 100"], "accept secret"));
}

#[gpui::test]
fn asked_again_a_pages_question_only_moves(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(vec![field("", false)], None), cx);
    dialog.cx.simulate_input("typed");
    let host = dialog.host.clone();
    dialog.cx.cx.update(|cx| {
        ask_from_page(
            (1, 7),
            page_question(vec![field("", false)], None),
            240.,
            &host,
            cx,
        )
    });
    dialog.keys("enter");
    assert_eq!(
        dialog.calls(),
        left(&["show over 100", "show over 240"], "accept typed")
    );
}

#[gpui::test]
fn a_pages_question_taken_back_leaves_untold(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    dialog.cx.cx.update(withdraw_page);
    assert_eq!(dialog.calls(), ["show over 100", "hide"]);
    // Another tab's comes up in its place.
    let host = dialog.host.clone();
    dialog
        .cx
        .cx
        .update(|cx| ask_from_page((2, 8), page_question(Vec::new(), None), 100., &host, cx));
    dialog.keys("escape");
    assert_eq!(dialog.calls(), left(&["show over 100"], "cancel "));
}

#[gpui::test]
fn the_checkbox_answers_with_a_click(cx: &mut TestAppContext) {
    let mut question = page_question(Vec::new(), Some("Always allow"));
    question.on_return = Return::Cancel;
    let mut dialog = Asking::page(question, cx);
    dialog.cx.cx.update(|cx| {
        let window = cx.global::<DialogWindow>().0;
        window
            .update(cx, |dialog, _, cx| dialog.toggle_check(cx))
            .unwrap();
    });
    // ↩ cancels, so a page can't time a key to open an app.
    dialog.keys("enter");
    assert_eq!(dialog.calls(), left(&["show over 100"], "cancel  checked"));
}

/// A window with a button all over it, and its own dialog, as Settings has on Linux.
struct Covered {
    dialog: Entity<Dialog>,
    host: Rc<Recorder>,
    /// Whether what's under hears a bare release as well as a click.
    hears_releases: bool,
}

impl Render for Covered {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let host = self.host.clone();
        div()
            .size_full()
            .child(
                div()
                    .id("under")
                    .size_full()
                    .when(self.hears_releases, |under| {
                        let host = host.clone();
                        under.on_mouse_up(gpui::MouseButton::Left, move |_, _, _| {
                            host.0.borrow_mut().push("under up".into())
                        })
                    })
                    .on_click(move |_, _, _| host.0.borrow_mut().push("under".into())),
            )
            .children(cover(&self.dialog, cx))
    }
}

#[gpui::test]
fn a_windows_own_dialog_covers_it_until_it_has_gone(cx: &mut TestAppContext) {
    let on_return = std::cell::Cell::new(Return::Action);
    let host = Rc::new(Recorder::default());
    let (view, cx) = cx.add_window_view(|_, cx| Covered {
        dialog: in_window(host.clone(), cx),
        host: host.clone(),
        hears_releases: false,
    });
    cx.simulate_resize(size(px(684.), px(650.)));
    let ask = |cx: &mut VisualTestContext| {
        let deleted = host.clone();
        let delete = Box::new(move |_: &mut App| deleted.0.borrow_mut().push("delete".into()));
        cx.update(|window, cx| {
            let dialog = view.read(cx).dialog.clone();
            let delete_work = ("Delete “Work”?".into(), String::new(), "Delete");
            let (message, detail, action) = delete_work;
            ask_in(
                &dialog,
                message,
                detail,
                action,
                on_return.get(),
                delete,
                window,
                cx,
            );
        });
        cx.run_until_parked();
    };
    let click = |x: f32, y: f32, cx: &mut VisualTestContext| {
        cx.simulate_click(point(px(x), px(y)), Modifiers::none());
        cx.executor().advance_clock(Duration::from_secs_f32(MOVE));
        cx.run_until_parked();
    };
    ask(cx);
    // A click around the card cancels, and doesn't reach the window under it.
    click(342., 30., cx);
    assert_eq!(host.0.take(), ["show", "hide"]);
    click(342., 30., cx);
    assert_eq!(host.0.take(), ["under"]);
    ask(cx);
    cx.simulate_keystrokes("enter");
    cx.executor().advance_clock(Duration::from_secs_f32(MOVE));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["show", "hide", "delete"]);

    // One that only a click answers: ↩ does nothing, and escape still cancels.
    on_return.set(Return::Nothing);
    ask(cx);
    cx.simulate_keystrokes("enter");
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["show"]);
    cx.update(|_, cx| {
        assert!(
            cover(&view.read(cx).dialog, cx).is_some(),
            "it's still asking"
        )
    });
    cx.simulate_keystrokes("escape");
    cx.executor().advance_clock(Duration::from_secs_f32(MOVE));
    cx.run_until_parked();
    assert_eq!(host.0.take(), ["hide"]);
}

/// Under Reduce Motion the dialog doesn't drop into place: it fades in where it rests (#1380).
#[gpui::test]
fn under_reduce_motion_the_dialog_fades_in_where_it_rests(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    let window = dialog.cx.cx.update(|cx| cx.global::<DialogWindow>().0);
    let arriving = |dialog: &mut Asking| {
        dialog.cx.cx.update(|cx| {
            window
                .update(cx, |dialog, _, cx| {
                    let asked = dialog.asked.as_mut().expect("nothing asked");
                    asked.shown = Some(Instant::now());
                    let pose = asked.drawn(cx);
                    (pose.raised, pose.opacity)
                })
                .unwrap()
        })
    };
    let (raised, opacity) = arriving(&mut dialog);
    assert!(raised > DROP * 0.9 && opacity < 0.1, "{raised} {opacity}");
    dialog.cx.cx.update(|cx| cx.set_reduce_motion(true));
    let (raised, opacity) = arriving(&mut dialog);
    assert!(raised == 0. && opacity < 0.1, "{raised} {opacity}");
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["show", "hide"]);
}

/// The open-app dialog's "Always allow" line wraps inside the card, as its detail does, however
/// long the site's name, and its box sits beside the label's first line (#1920).
#[gpui::test]
fn the_checkbox_label_wraps_inside_the_card(cx: &mut TestAppContext) {
    let label = "Always allow https://sign-in.a-very-long-identity-provider.example.com:8443 to \
                 open links of this type in the associated app";
    let mut question = page_question(Vec::new(), Some(label));
    question.on_return = Return::Cancel;
    let dialog = Asking::page(question, cx);
    let mut dialog = dialog;
    dialog
        .cx
        .executor()
        .advance_clock(Duration::from_secs_f32(MOVE));
    dialog.cx.run_until_parked();
    let drawn = dialog.cx.cx.update(|cx| {
        let window = cx.global::<DialogWindow>().0;
        window
            .update(cx, |dialog, _, _| {
                dialog.asked.as_ref().map(|asked| asked.check_drawn.get())
            })
            .unwrap()
    });
    let drawn = drawn.expect("the dialog asks nothing");
    let (card, check, label) = (drawn.card, drawn.check, drawn.label);
    assert!(card.size.width > px(0.), "the card wasn't drawn: {drawn:?}");
    // Inside the card's text inset, as the detail's lines are.
    let right = card.right() - px(TEXT_INSET);
    assert!(
        label.right() <= right + px(0.5),
        "the label runs to {:?}, past the card's text at {right:?}",
        label.right()
    );
    assert!(
        label.size.height >= px(2. * DETAIL_LINE),
        "the long label didn't wrap: {:?}",
        label.size.height
    );
    // The box's middle is the first line's.
    let first_line = label.top() + px(DETAIL_LINE / 2.);
    let middle = check.top() + check.size.height / 2.;
    assert!(
        (middle - first_line).abs() < px(0.5),
        "the box's middle is at {middle:?}, the first line's at {first_line:?}"
    );
}

/// A window's own dialog takes every press while it asks, and none once it's answered and
/// leaving: a press then reaches what's under it, as a layer animating out is inert (#4108). The
/// press that cancelled it stays spent until it's let go, so not even its release lands.
#[gpui::test]
fn a_press_as_a_windows_dialog_leaves_reaches_what_is_under_it(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let (view, cx) = cx.add_window_view(|_, cx| Covered {
        dialog: in_window(host.clone(), cx),
        host: host.clone(),
        hears_releases: true,
    });
    cx.simulate_resize(size(px(684.), px(650.)));
    cx.update(|window, cx| {
        let dialog = view.read(cx).dialog.clone();
        let (message, detail) = ("Remove “example.com”?".into(), String::new());
        let removed = Box::new(|_: &mut App| {});
        ask_in(
            &dialog,
            message,
            detail,
            "Remove",
            Return::Action,
            removed,
            window,
            cx,
        );
    });
    cx.run_until_parked();
    let leaving = |cx: &mut VisualTestContext| cx.update(|_, cx| asks(&view.read(cx).dialog, cx));
    // A press around the card cancels; held and let go, neither half reaches the window.
    let around = point(px(342.), px(30.));
    cx.simulate_mouse_down(around, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(leaving(cx), "the dialog had gone before it was let go");
    cx.simulate_mouse_up(around, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        host.0.take(),
        ["show"],
        "the cancelling press reached the window"
    );
    // A press as it leaves, where the card still is, goes to the window.
    assert!(leaving(cx), "the dialog had gone before the press");
    cx.simulate_click(point(px(342.), px(325.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        host.0.take(),
        ["under", "under up"],
        "a press as it left was lost to it"
    );
}

/// A window with a control that opens its own dialog, and a root, which Settings is on Linux.
struct Opened {
    dialog: Entity<Dialog>,
    root: FocusHandle,
    opener: Option<FocusHandle>,
}

impl Render for Opened {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.root)
            .children((self.opener.as_ref()).map(|opener| div().size_full().track_focus(opener)))
            .children(cover(&self.dialog, cx))
    }
}

/// A window's own dialog gives the keyboard back to what opened it once it has gone, before its
/// answer runs, so an answer that hands the keyboard on, as a login's Remove does to the row that
/// takes its place, starts from the opener (#4108). If the opener has gone, the window has it.
#[gpui::test]
fn a_windows_own_dialog_gives_the_keyboard_back_before_its_answer(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let (view, cx) = cx.add_window_view(|window, cx| {
        let dialog = in_window(host.clone(), cx);
        // As Settings' observer on Linux does.
        let mut asked = false;
        cx.observe_in(
            &dialog,
            window,
            move |opened: &mut Opened, dialog, window, cx| {
                let asks = asks(&dialog, cx);
                if asked && !asks {
                    give_back(&dialog, &opened.root, window, cx);
                }
                asked = asks;
            },
        )
        .detach();
        Opened {
            dialog,
            root: cx.focus_handle(),
            opener: Some(cx.focus_handle()),
        }
    });
    cx.simulate_resize(size(px(684.), px(650.)));
    let focused = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            let opened = view.read(cx);
            match () {
                _ if (opened.opener.as_ref()).is_some_and(|opener| opener.is_focused(window)) => {
                    "opener"
                }
                _ if opened.root.is_focused(window) => "root",
                _ => "elsewhere",
            }
        })
    };
    let ask = |cx: &mut VisualTestContext| {
        let answered = host.clone();
        let window = cx.window_handle();
        // Read as it answers, so the test holds no opener that went.
        let opened = view.clone();
        let remove = Box::new(move |cx: &mut App| {
            let had = (window.update(cx, |_, window, cx| {
                let opener = &opened.read(cx).opener;
                (opener.as_ref()).is_some_and(|opener| opener.is_focused(window))
            }))
            .unwrap();
            let at = if had {
                "remove at the opener"
            } else {
                "remove"
            };
            answered.0.borrow_mut().push(at.into());
        });
        cx.update(|window, cx| {
            let dialog = view.read(cx).dialog.clone();
            let (message, detail) = ("Remove “example.com”?".into(), String::new());
            ask_in(
                &dialog,
                message,
                detail,
                "Remove",
                Return::Action,
                remove,
                window,
                cx,
            );
        });
        cx.run_until_parked();
    };
    let leave = |cx: &mut VisualTestContext| {
        cx.executor().advance_clock(Duration::from_secs_f32(MOVE));
        cx.run_until_parked();
    };
    cx.update(|window, cx| {
        let opener = view.read(cx).opener.clone().unwrap();
        window.focus(&opener, cx);
    });
    ask(cx);
    assert_eq!(
        focused(cx),
        "elsewhere",
        "the dialog didn't take the keyboard"
    );
    cx.simulate_keystrokes("enter");
    leave(cx);
    assert_eq!(host.0.take(), ["show", "hide", "remove at the opener"]);
    assert_eq!(focused(cx), "opener");
    // Cancelled, the same.
    ask(cx);
    cx.simulate_keystrokes("escape");
    leave(cx);
    assert_eq!(host.0.take(), ["show", "hide"]);
    assert_eq!(focused(cx), "opener");
    // The opener went as it asked: the window takes the keyboard.
    ask(cx);
    view.update(cx, |opened, cx| {
        opened.opener = None;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    leave(cx);
    assert_eq!(host.0.take(), ["show", "hide"]);
    assert_eq!(focused(cx), "root");
}

/// Escape goes to the topmost layer (#4108): the field's menu over a page's prompt, closed by its
/// keys (as its Escape closes it), closes alone, and the next Escape cancels the prompt.
#[gpui::test]
fn escape_in_a_fields_menu_closes_the_menu_and_the_next_the_dialog(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(vec![field("Ada", false)], None), cx);
    let shown: Rc<RefCell<Option<crate::text_input::Chosen>>> = Rc::default();
    let menu = shown.clone();
    dialog.cx.update(|_, cx| {
        cx.set_global(crate::text_input::FieldMenu(Rc::new(
            move |_, _, _, _, chosen, _| *menu.borrow_mut() = Some(chosen),
        )))
    });
    dialog.keys("shift-f10");
    let chosen = (shown.take()).expect("⇧F10 showed no field's menu");
    dialog
        .cx
        .update(|window, cx| chosen(crate::menu::Closed::Otherwise, window, cx));
    assert_eq!(
        dialog.calls(),
        ["show over 100"],
        "the prompt closed with its field's menu"
    );
    dialog.keys("escape");
    assert_eq!(dialog.calls(), left(&[], "cancel Ada"));
}
