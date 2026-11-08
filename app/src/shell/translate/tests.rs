use std::rc::Rc;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

use crate::shell::{Command, Folders, Shell, Translation, bind_keys, recorder::Recorder};

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

/// A page is offered in the user's language only when it declares another: not in the user's own
/// with another region's spelling, nor one that declares none.
#[gpui::test]
fn only_a_page_in_another_language_is_offered(cx: &mut TestAppContext) {
    let (shell, _, cx) = window(cx);
    assert_eq!(offered(&shell, cx), (false, false));
    for (language, offer) in [
        ("en-GB", false),
        ("und", false),
        ("es", true),
        ("EN", false),
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
    // Translate is the right of the buttons along the card's bottom.
    let bottom = card.origin.y + card.size.height;
    let translate = point(card.origin.x + card.size.width * 0.75, bottom - px(26.));
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
