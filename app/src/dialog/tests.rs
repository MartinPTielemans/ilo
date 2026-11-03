//! The dialog's keys and clicks, on a host that records what the dialog asks of its window.

use std::{cell::RefCell, rc::Rc};

use gpui::{Modifiers, TestAppContext, VisualTestContext, point, px, size};

use super::*;
use crate::settling::SETTLING;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self, window: Option<u64>, over: Option<Bounds<Pixels>>) {
        let mut call = match over {
            Some(over) => format!("show over {}", f32::from(over.origin.x)),
            None => "show".into(),
        };
        if let Some(window) = window {
            call += &format!(" in {window}");
        }
        self.0.borrow_mut().push(call);
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
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
        let window = cx.update(|cx| open(host.clone(), cx)).expect("no window for the dialog");
        let deleted = host.clone();
        cx.update(|cx| {
            let delete = Box::new(move |_: &mut App| deleted.0.borrow_mut().push("delete".into()));
            let (message, detail) = ("Delete “Work”?".into(), "Its 3 tabs close too.".into());
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
        self.cx.simulate_click(point(px(x), px(y)), Modifiers::none());
        self.cx.run_until_parked();
    }

    /// What the dialog asked of the host since the last time, once it has had time to leave.
    fn calls(&mut self) -> Vec<String> {
        self.cx.executor().advance_clock(Duration::from_secs_f32(MOVE));
        self.cx.run_until_parked();
        self.host.0.take()
    }
}

#[gpui::test]
fn escape_cancels(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    dialog.keys("escape");
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

#[gpui::test]
fn an_ask_while_one_shows_is_dropped(cx: &mut TestAppContext) {
    let mut dialog = Asking::new(cx);
    dialog.cx.cx.update(|cx| {
        let clear = Box::new(|_: &mut App| panic!("the second dialog ran"));
        ask(None, "Clear the archive?".into(), String::new(), "Clear Archive", clear, cx);
    });
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show", "hide", "delete"]);
}

#[gpui::test]
fn a_window_s_question_shows_over_that_window(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    cx.update(|cx| open(host.clone(), cx)).expect("no window for the dialog");
    // A window behind another asks from its sidebar's context menu (#1432).
    cx.update(|cx| {
        let delete = Box::new(|_: &mut App| {});
        ask(Some(2), "Delete “Work”?".into(), String::new(), "Delete", delete, cx);
    });
    cx.run_until_parked();
    assert_eq!(*host.0.borrow(), ["show in 2"]);
}

/// The dialog asking to quit, with "Quit, and don't ask again" at the left. Each answer records
/// itself on the host when it runs.
fn asking_to_quit(cx: &mut TestAppContext) -> Asking {
    let host = Rc::new(Recorder::default());
    let window = cx.update(|cx| open(host.clone(), cx)).expect("no window for the dialog");
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
        pose(None, question, Box::new(move |_, _| quit.0.borrow_mut().push("quit".into())), cx);
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
            ("Read and change your data on example.org".into(), vec!["example.org".into()]),
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
        host.0.borrow_mut().push(if yes { "yes" } else { "no" }.into());
    });
    ask_chrome(question, answered, cx);
}

/// The dialog, open and asking nothing.
fn opened(cx: &mut TestAppContext) -> (VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let window = cx.update(|cx| open(host.clone(), cx)).expect("no window for the dialog");
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
            answers.0.borrow_mut().push(if quit { "quit" } else { "continue" }.into());
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
    dialog.cx.cx.update(|cx| add_extension("Add extension", &host, cx));
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show", "hide", "delete", "show"]);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["hide", "no"]);
}

#[gpui::test]
fn chromes_next_question_takes_the_place_of_the_last(cx: &mut TestAppContext) {
    let mut dialog = Asking::chrome("Add extension", cx);
    let host = dialog.host.clone();
    dialog.cx.cx.update(|cx| add_extension("Re-enable", &host, cx));
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
        host.0.borrow_mut().push(if yes { "add" } else { "not me" }.into());
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
    dialog.cx.cx.update(|cx| add_extension("Add extension", &host, cx));
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
    let over = (Bounds::new(point(px(x), px(10.)), size(px(600.), px(500.))), 10.);
    let host = host.clone();
    let answered = Box::new(move |reply: Reply, _: &mut App| {
        let answer = if reply.accept { "accept" } else { "cancel" };
        let checked = if reply.checked { " checked" } else { "" };
        host.0.borrow_mut().push(format!("{answer} {}{checked}", reply.texts.join(" ")));
    });
    ask_page((tab, id), question, over, answered, cx);
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
    Field { placeholder: "".into(), text: text.to_owned().into(), secure }
}

impl Asking {
    /// The dialog, asking a page's `question` of tab 1, as its dialog 7.
    fn page(question: Question, cx: &mut TestAppContext) -> Self {
        let (cx, host) = opened(cx);
        cx.cx.update(crate::text_input::bind_keys);
        cx.cx.update(|cx| ask_from_page((1, 7), question, 100., &host, cx));
        cx.run_until_parked();
        Asking { cx, host }
    }
}

#[gpui::test]
fn a_pages_question_shows_over_its_card_and_return_answers_it(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show over 100", "hide", "accept "]);
}

#[gpui::test]
fn a_click_on_the_cards_dim_leaves_a_pages_question(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(Vec::new(), None), cx);
    dialog.click(400., 40.);
    assert_eq!(dialog.calls(), ["show over 100"]);
    dialog.keys("escape");
    assert_eq!(dialog.calls(), ["hide", "cancel "]);
}

#[gpui::test]
fn a_prompt_answers_what_was_typed(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(vec![field("Ada", false)], None), cx);
    // What it starts with is selected, so typing replaces it.
    dialog.cx.simulate_input("Grace");
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show over 100", "hide", "accept Grace"]);
}

#[gpui::test]
fn tab_moves_to_the_next_field(cx: &mut TestAppContext) {
    let fields = vec![field("", false), field("", true)];
    let mut dialog = Asking::page(page_question(fields, Some("Remember")), cx);
    dialog.cx.simulate_input("ada");
    dialog.keys("tab");
    dialog.cx.simulate_input("secret");
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show over 100", "hide", "accept ada secret"]);
}

#[gpui::test]
fn a_reply_never_shows_what_was_typed(cx: &mut TestAppContext) {
    let reply = Reply { accept: true, texts: vec!["ada".into(), "secret".into()], checked: false };
    let shown = format!("{reply:?}");
    assert!(!shown.contains("secret") && !shown.contains("ada"), "{shown}");
    // Nor does the field show it, though it answers it.
    let mut dialog = Asking::page(page_question(vec![field("", true)], None), cx);
    dialog.cx.simulate_input("secret");
    let shown = dialog.cx.cx.update(|cx| {
        let window = cx.global::<DialogWindow>().0;
        window.read(cx).unwrap().asked.as_ref().unwrap().fields[0].read(cx).shown().to_string()
    });
    assert_eq!(shown, "••••••");
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show over 100", "hide", "accept secret"]);
}

#[gpui::test]
fn asked_again_a_pages_question_only_moves(cx: &mut TestAppContext) {
    let mut dialog = Asking::page(page_question(vec![field("", false)], None), cx);
    dialog.cx.simulate_input("typed");
    let host = dialog.host.clone();
    dialog.cx.cx.update(|cx| {
        ask_from_page((1, 7), page_question(vec![field("", false)], None), 240., &host, cx)
    });
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show over 100", "show over 240", "hide", "accept typed"]);
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
    assert_eq!(dialog.calls(), ["show over 100", "hide", "cancel "]);
}

#[gpui::test]
fn the_checkbox_answers_with_a_click(cx: &mut TestAppContext) {
    let mut question = page_question(Vec::new(), Some("Always allow"));
    question.on_return = Return::Cancel;
    let mut dialog = Asking::page(question, cx);
    dialog.cx.cx.update(|cx| {
        let window = cx.global::<DialogWindow>().0;
        window.update(cx, |dialog, _, cx| dialog.toggle_check(cx)).unwrap();
    });
    // ↩ cancels, so a page can't time a key to open an app.
    dialog.keys("enter");
    assert_eq!(dialog.calls(), ["show over 100", "hide", "cancel  checked"]);
}

/// A window with a button all over it, and its own dialog, as Settings has on Linux.
struct Covered {
    dialog: Entity<Dialog>,
    host: Rc<Recorder>,
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
    });
    cx.simulate_resize(size(px(684.), px(650.)));
    let ask = |cx: &mut VisualTestContext| {
        let deleted = host.clone();
        let delete = Box::new(move |_: &mut App| deleted.0.borrow_mut().push("delete".into()));
        cx.update(|window, cx| {
            let dialog = view.read(cx).dialog.clone();
            let delete_work = ("Delete “Work”?".into(), String::new(), "Delete");
            let (message, detail, action) = delete_work;
            ask_in(&dialog, message, detail, action, on_return.get(), delete, window, cx);
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
    cx.update(|_, cx| assert!(cover(&view.read(cx).dialog, cx).is_some(), "it's still asking"));
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
