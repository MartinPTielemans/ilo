use std::rc::Rc;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

use crate::shell::{
    AlwaysTranslated, Command, Folders, Shell, Translation, bind_keys, recorder::Recorder,
};
use crate::tokens::shell::permission::{BUTTON_GAP, BUTTON_HEIGHT};

fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    cx.update(bind_keys);
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

fn offered(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (bool, bool) {
    shell.read_with(cx, |shell, _| {
        (
            shell.offers(Command::TranslatePage),
            shell.offers(Command::UseOriginal),
        )
    })
}

/// A page is offered in the user's language only when it declares another they don't read: not in
/// the user's own with another region's spelling, nor one the system prefers, as Arc offers none
/// for those, nor one that declares none.
#[gpui::test]
fn only_a_page_in_another_language_is_offered(cx: &mut TestAppContext) {
    let (shell, _, cx) = window(cx);
    assert_eq!(offered(&shell, cx), (false, false));
    for (language, offer) in [
        ("en-GB", false),
        ("und", false),
        ("es", true),
        ("EN", false),
        ("nl-BE", false),
    ] {
        shell.update(cx, |shell, cx| shell.set_page_language(10, language, cx));
        assert_eq!(offered(&shell, cx).0, offer, "{language}");
    }
    // Another tab's language is its own.
    shell.update(cx, |shell, cx| shell.set_page_language(11, "es", cx));
    shell.update(cx, |shell, cx| shell.set_page_language(10, "en", cx));
    assert_eq!(offered(&shell, cx), (false, false));
}

/// Translate Page asks for the page's language into the user's, and Use Original takes it back:
/// at once while it translates, and once it's translated.
#[gpui::test]
fn translate_page_and_use_original_go_both_ways(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_page_language(10, "es", cx);
        shell.run(Command::TranslatePage, "ui", cx);
    });
    assert_eq!(host.take(), ["translate 10 es en"]);
    assert_eq!(offered(&shell, cx), (false, true));
    shell.update(cx, |shell, cx| shell.run(Command::UseOriginal, "ui", cx));
    assert_eq!(host.take(), ["use original 10"]);
    assert_eq!(offered(&shell, cx), (true, false));

    shell.update(cx, |shell, cx| {
        shell.run(Command::TranslatePage, "ui", cx);
        shell.set_translation(10, Translation::Translated, cx);
    });
    host.take();
    assert_eq!(offered(&shell, cx), (false, true));
    shell.update(cx, |shell, cx| shell.run(Command::UseOriginal, "ui", cx));
    assert_eq!(host.take(), ["use original 10"]);
}

/// A new page forgets the last one's translation, and a translation that comes back for it after
/// is dropped: it was the last page's.
#[gpui::test]
fn a_new_page_forgets_the_translation(cx: &mut TestAppContext) {
    let (shell, _, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_page_language(10, "es", cx);
        shell.run(Command::TranslatePage, "ui", cx);
        shell.set_page_language(10, "", cx);
        shell.set_translation(10, Translation::Translated, cx);
    });
    assert_eq!(offered(&shell, cx), (false, false));
    shell.update(cx, |shell, cx| shell.set_page_language(10, "es", cx));
    assert_eq!(offered(&shell, cx), (true, false));
}

/// An agent's Space offers none, so the page an agent reads is in its own words.
#[gpui::test]
fn an_agents_space_offers_no_translation(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    host.space_agents.borrow_mut().insert(1, "claude".into());
    shell.update(cx, |shell, cx| {
        shell.set_page_language(10, "es", cx);
        shell.run(Command::TranslatePage, "ui", cx);
    });
    assert_eq!(offered(&shell, cx), (false, false));
    assert_eq!(host.take(), Vec::<String>::new());
}

/// The mark's press opens the card, whose Translate translates, and a language whose model isn't
/// on the computer opens it again to say so, with the way to System Settings. Escape closes it.
#[gpui::test]
fn the_card_asks_and_says_what_went_wrong(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| shell.set_page_language(10, "es", cx));
    cx.run_until_parked();
    let mark = shell.read_with(cx, |shell, _| shell.translate_mark_drawn.get());
    let mark = mark.expect("the URL field drew no mark for a page in Spanish");
    cx.simulate_mouse_down(mark.center(), gpui::MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let card = shell.read_with(cx, |shell, _| shell.translate_card_drawn.get());
    let card = card.expect("the mark's press opened no card");
    // Translate is the right of the buttons, over Always Translate along the card's bottom.
    let bottom = card.origin.y + card.size.height;
    let answers = bottom - px(26. + BUTTON_HEIGHT + BUTTON_GAP);
    let translate = point(card.origin.x + card.size.width * 0.75, answers);
    cx.simulate_click(translate, Modifiers::none());
    cx.run_until_parked();
    // The mark's tooltip came and went with the pointer.
    let asked: Vec<_> = (host.take().into_iter())
        .filter(|call| !call.starts_with("tooltip"))
        .collect();
    assert_eq!(asked, ["translate 10 es en"]);

    shell.update(cx, |shell, cx| {
        shell.set_translation(10, Translation::Missing, cx)
    });
    cx.run_until_parked();
    let card = shell.read_with(cx, |shell, _| shell.translate_card_drawn.get());
    let card = card.expect("a missing language opened no card");
    let bottom = card.origin.y + card.size.height;
    let settings = point(card.origin.x + card.size.width * 0.25, bottom - px(26.));
    cx.simulate_click(settings, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["translation settings"]);

    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let card = shell.read_with(cx, |shell, _| shell.translate_card_drawn.get());
    assert_eq!(card, None, "escape left the card open");
}

/// The mark names itself under the pointer, as the URL field's other buttons do, and says so once
/// the page is translated, as its press then opens the card that takes it back (#2112).
#[gpui::test]
fn the_mark_names_itself_under_the_pointer(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| shell.set_page_language(10, "es", cx));
    cx.run_until_parked();
    let mark = shell.read_with(cx, |shell, _| shell.translate_mark_drawn.get());
    let mark = mark.expect("the URL field drew no mark for a page in Spanish");
    let mut tips = |at, cx: &mut VisualTestContext| {
        host.take();
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.run_until_parked();
        host.take()
            .into_iter()
            .filter(|call| call.starts_with("tooltip"))
            .collect::<Vec<_>>()
    };
    assert_eq!(tips(mark.center(), cx), ["tooltip Translate Page "]);
    let away = point(mark.center().x, mark.center().y + px(200.));
    assert_eq!(tips(away, cx), ["tooltip none"]);

    shell.update(cx, |shell, cx| {
        shell.run(Command::TranslatePage, "ui", cx);
        shell.set_translation(10, Translation::Translated, cx);
    });
    cx.run_until_parked();
    assert_eq!(tips(mark.center(), cx), ["tooltip Translated "]);
}

/// A row along the bottom of the card the mark opens, whatever it says.
fn click_card_bottom(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let card = shell.read_with(cx, |shell, _| shell.translate_card_drawn.get());
    let card = card.expect("no translate card");
    let bottom = card.origin.y + card.size.height;
    cx.simulate_click(
        point(card.origin.x + card.size.width * 0.5, bottom - px(26.)),
        Modifiers::none(),
    );
    cx.run_until_parked();
}

/// What the host was asked, without what the pointer on its way showed: the mark's tooltip and a
/// sidebar row's hover card.
fn asked(host: &Recorder) -> Vec<String> {
    (host.take().into_iter())
        .filter(|call| !call.starts_with("tooltip") && !call.starts_with("card left"))
        .collect()
}

fn always_translated(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        (cx.try_global::<AlwaysTranslated>())
            .map(|always| always.0.iter().map(ToString::to_string).collect())
            .unwrap_or_default()
    })
}

/// The card's Always Translate translates the page and keeps its language, so the next page in it
/// translates as it opens, in any region's spelling; Stop Always Translating on a translated page
/// leaves it translated and the next asks again.
#[gpui::test]
fn always_translate_keeps_the_language_until_stopped(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_page_language(10, "es", cx);
        shell.translate_card = Some(10);
        cx.notify();
    });
    cx.run_until_parked();
    click_card_bottom(&shell, cx);
    assert_eq!(asked(&host), ["always translated es", "translate 10 es en"]);
    assert_eq!(always_translated(cx), ["es"]);

    // The next page, and another tab's, translate as they come.
    shell.update(cx, |shell, cx| {
        shell.set_translation(10, Translation::Translated, cx);
        shell.set_page_language(10, "", cx);
        shell.set_page_language(10, "es-MX", cx);
        shell.set_page_language(11, "es", cx);
    });
    assert_eq!(host.take(), ["translate 10 es-MX en", "translate 11 es en"]);
    // One the user reads doesn't, nor an agent's Space's.
    host.space_agents.borrow_mut().insert(1, "claude".into());
    shell.update(cx, |shell, cx| {
        shell.set_page_language(11, "", cx);
        shell.set_page_language(11, "es", cx);
    });
    host.space_agents.borrow_mut().clear();
    shell.update(cx, |shell, cx| shell.set_page_language(11, "en", cx));
    assert_eq!(host.take(), Vec::<String>::new());

    // A translated page's card says why, and stops it.
    shell.update(cx, |shell, cx| {
        shell.set_translation(10, Translation::Translated, cx);
        shell.translate_card = Some(10);
        cx.notify();
    });
    cx.run_until_parked();
    click_card_bottom(&shell, cx);
    assert_eq!(asked(&host), ["always translated "]);
    assert_eq!(always_translated(cx), Vec::<String>::new());
    assert_eq!(
        offered(&shell, cx),
        (false, true),
        "the page stays translated"
    );
    shell.update(cx, |shell, cx| {
        shell.set_page_language(10, "", cx);
        shell.set_page_language(10, "es", cx);
    });
    assert_eq!(host.take(), Vec::<String>::new());
    assert_eq!(offered(&shell, cx), (true, false));
}
