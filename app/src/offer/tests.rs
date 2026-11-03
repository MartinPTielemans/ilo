//! The offer's keys, clicks and tabs, on a host that records what the offer asks of its window and
//! the core.

use std::cell::RefCell;

use gpui::{Modifiers, TestAppContext, VisualTestContext, hsla};

use super::*;
use crate::settling::SETTLING;
use crate::text_input::{self, Chosen, FieldMenu};

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>);

impl Host for Recorder {
    fn show(&self) {
        self.0.borrow_mut().push("show".into());
    }

    fn hide(&self) {
        self.0.borrow_mut().push("hide".into());
    }

    fn answer(&self, tab: u64, answer: Answer, username: &str) {
        self.0
            .borrow_mut()
            .push(format!("{answer:?} {tab} {username}"));
    }
}

fn offer(kind: Kind, usernames: &[&str]) -> Offer {
    Offer {
        kind,
        title: "Save password?".into(),
        site: "127.0.0.1".into(),
        username: usernames[0].to_string().into(),
        usernames: usernames
            .iter()
            .map(|username| username.to_string().into())
            .collect(),
        password_length: 13,
        accept: "Save".into(),
        decline: "No Thanks".into(),
        never: (kind == Kind::Save).then(|| "Never".into()),
    }
}

/// The offer's window over browser window 1, showing tab 1.
struct Prompt {
    cx: VisualTestContext,
    host: Rc<Recorder>,
}

/// Opens the offer's window over browser window `browser`, showing tab `tab`.
fn open_over(
    browser: u64,
    tab: u64,
    cx: &mut TestAppContext,
) -> (WindowHandle<OfferPrompt>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| {
            open(
                browser,
                host.clone(),
                |_| (hsla(0., 0., 0.2, 1.), Default::default()),
                cx,
            )
        })
        .expect("no window for the offer");
    cx.update(|cx| tab_shown(browser, tab, cx));
    (window, host)
}

impl Prompt {
    fn new(cx: &mut TestAppContext) -> Self {
        cx.update(text_input::bind_keys);
        let (window, host) = open_over(1, 1, cx);
        let cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();
        Prompt { cx, host }
    }

    /// The core reports `tab`'s offer, and the user waits for it to settle.
    fn offered(&mut self, tab: u64, offer: Option<Offer>) {
        self.cx.cx.update(|cx| changed(tab, offer, cx));
        self.settle();
    }

    fn shown(&mut self, tab: u64) {
        self.cx.cx.update(|cx| tab_shown(1, tab, cx));
        self.settle();
    }

    /// The offer has been on screen long enough to take a yes.
    fn settle(&mut self) {
        self.cx.executor().advance_clock(SETTLING);
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

    fn click(&mut self, x: f32, y: f32) {
        self.cx
            .simulate_click(point(px(x), px(y)), Modifiers::none());
        self.cx.run_until_parked();
    }

    /// A click on the left of the buttons' row, where Never is.
    fn click_never(&mut self) {
        self.click(INSET + 8., HEIGHT - INSET - BUTTON_HEIGHT / 2.);
    }

    /// What the offer asked of the host since the last time.
    fn calls(&self) -> Vec<String> {
        self.host.0.take()
    }
}

#[gpui::test]
fn return_saves_the_shown_tabs_offer(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    assert_eq!(prompt.calls(), ["hide"]);
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    assert_eq!(prompt.calls(), ["show"]);
    prompt.keys("enter");
    assert_eq!(prompt.calls(), ["Accept 1 check@example.test", "hide"]);
    // Answered, it takes no more keys.
    prompt.keys("enter");
    assert!(prompt.calls().is_empty());
}

/// A page can bring the offer up under a double-click the user started on its login, so for its
/// first moment on screen neither ↩ nor Never answers, while escape still declines.
#[gpui::test]
fn the_offer_takes_no_answer_for_good_for_its_first_moment(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.calls();
    prompt
        .cx
        .cx
        .update(|cx| changed(1, Some(offer(Kind::Save, &["check@example.test"])), cx));
    prompt.cx.run_until_parked();
    assert_eq!(prompt.calls(), ["show"]);
    prompt.keys("enter");
    prompt.click_never();
    assert!(prompt.calls().is_empty());
    prompt.settle();
    prompt.click_never();
    assert_eq!(prompt.calls(), ["Never 1 check@example.test", "hide"]);
    // Shown again, as with its tab, it settles again, and escape needn't wait.
    prompt
        .cx
        .cx
        .update(|cx| changed(1, Some(offer(Kind::Save, &["check@example.test"])), cx));
    prompt.cx.run_until_parked();
    prompt.calls();
    prompt.keys("enter");
    assert!(prompt.calls().is_empty());
    prompt.keys("escape");
    assert_eq!(prompt.calls(), ["NotNow 1 check@example.test", "hide"]);
}

#[gpui::test]
fn escape_is_not_now(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    prompt.calls();
    prompt.keys("escape");
    assert_eq!(prompt.calls(), ["NotNow 1 check@example.test", "hide"]);
}

#[gpui::test]
fn never_is_for_a_save_only(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.offered(1, Some(offer(Kind::Update, &["check@example.test"])));
    prompt.calls();
    prompt.click_never();
    assert!(prompt.calls().is_empty());
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    prompt.calls();
    prompt.click_never();
    assert_eq!(prompt.calls(), ["Never 1 check@example.test", "hide"]);
}

#[gpui::test]
fn the_answer_has_the_username_as_the_user_left_it(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    prompt.calls();
    prompt.keys("cmd-a");
    prompt.type_text("other@example.test");
    prompt.keys("enter");
    assert_eq!(prompt.calls(), ["Accept 1 other@example.test", "hide"]);
}

#[gpui::test]
fn the_picker_puts_the_username_chosen_in_the_field(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    let shown: Rc<RefCell<Option<(Vec<String>, Chosen)>>> = Rc::default();
    let menu = shown.clone();
    prompt.cx.cx.update(|cx| {
        cx.set_global(FieldMenu(Rc::new(move |items, _, _, _, chosen, _| {
            let titles = items.iter().map(|(title, _)| title.to_string()).collect();
            *menu.borrow_mut() = Some((titles, chosen));
        })))
    });
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test", "check"])));
    prompt.calls();
    // The chevron, at the username row's right end.
    let row_middle = INSET + TITLE_LINE + SITE_LINE + HEADER_GAP + ROW / 2.;
    prompt.click(WIDTH - INSET - ROW / 2., row_middle);
    let (titles, chosen) = shown.take().expect("the chevron showed no menu");
    assert_eq!(titles, ["check@example.test", "check"]);
    prompt.cx.update(|window, cx| chosen(1, window, cx));
    prompt.keys("enter");
    assert_eq!(prompt.calls(), ["Accept 1 check", "hide"]);
}

#[gpui::test]
fn another_tab_closes_the_offer_for_good(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    prompt.calls();
    prompt.shown(2);
    assert_eq!(prompt.calls(), ["hide"]);
    prompt.shown(1);
    assert_eq!(prompt.calls(), ["hide"]);
    prompt.keys("enter");
    assert!(prompt.calls().is_empty());
}

#[gpui::test]
fn each_tab_has_its_own_offer(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.calls();
    // Reported for a tab no longer shown, as the core's reports come from a task of their own.
    prompt.offered(2, Some(offer(Kind::Save, &["two@example.test"])));
    assert!(prompt.calls().is_empty());
    prompt.shown(2);
    assert_eq!(prompt.calls(), ["show"]);
    prompt.keys("enter");
    assert_eq!(prompt.calls(), ["Accept 2 two@example.test", "hide"]);
}

#[gpui::test]
fn the_core_taking_it_back_hides_it(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    prompt.calls();
    prompt.offered(1, None);
    assert_eq!(prompt.calls(), ["hide"]);
    prompt.keys("escape");
    assert!(prompt.calls().is_empty());
}

#[gpui::test]
fn each_window_shows_its_own_tabs_offer(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    let (_, other) = open_over(2, 2, &mut prompt.cx.cx);
    prompt.calls();
    other.0.take();
    prompt.offered(2, Some(offer(Kind::Save, &["two@example.test"])));
    assert!(prompt.calls().is_empty());
    assert_eq!(other.0.take(), ["show"]);
    // A tab shown in one window doesn't close the offer of the other's.
    prompt.shown(3);
    assert_eq!(prompt.calls(), ["hide"]);
    prompt.cx.cx.update(|cx| changed(2, None, cx));
    assert_eq!(other.0.take(), ["hide"]);
}

#[gpui::test]
fn the_window_showing_the_page_shows_its_offer(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    let (_, other) = open_over(2, 3, &mut prompt.cx.cx);
    prompt.calls();
    other.0.take();
    // Window 2 shows tab 1's page, and window 1 a picture of it.
    prompt.cx.cx.update(|cx| tab_shown(2, 1, cx));
    assert_eq!(other.0.take(), ["show"]);
    prompt.cx.cx.update(|cx| page_elsewhere(1, true, cx));
    assert_eq!(prompt.calls(), ["hide"]);
    // Window 1 selecting another tab leaves the offer to window 2.
    prompt.shown(4);
    prompt.cx.cx.update(|cx| page_elsewhere(1, false, cx));
    assert_eq!(prompt.calls(), ["hide", "hide"]);
    prompt.cx.cx.update(|cx| changed(1, None, cx));
    assert_eq!(other.0.take(), ["hide"]);
}

#[gpui::test]
fn the_page_coming_back_brings_its_offer(cx: &mut TestAppContext) {
    let mut prompt = Prompt::new(cx);
    prompt.offered(1, Some(offer(Kind::Save, &["check@example.test"])));
    let (_, other) = open_over(2, 1, &mut prompt.cx.cx);
    prompt.cx.cx.update(|cx| page_elsewhere(1, true, cx));
    prompt.calls();
    other.0.take();
    // Window 2 selects another tab, and tab 1's page goes back to window 1.
    prompt.cx.cx.update(|cx| tab_shown(2, 3, cx));
    assert_eq!(other.0.take(), ["hide"]);
    prompt.cx.cx.update(|cx| page_elsewhere(1, false, cx));
    assert_eq!(prompt.calls(), ["show"]);
    prompt.settle();
    prompt.keys("enter");
    assert_eq!(prompt.calls(), ["Accept 1 check@example.test", "hide"]);
}
