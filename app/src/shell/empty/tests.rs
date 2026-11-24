//! The card of a window with no page, on a host that records what the shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use crate::agents::tier::Tier;
use crate::command_bar::PLACEHOLDER_TEXT;
use crate::shell::{
    Folders, Page, ROW_HEIGHT, SECONDARY, Shell, Slot,
    empty::{FADE, MARK_HEIGHT, composer::group_top},
    recorder::Recorder,
};

/// A drawn shell showing one Space with Today tabs 1 and 2, and no tab shown, as a fresh window.
fn window_with_no_tab(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Space".into(), None, 0, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![1, 2],
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// The middle of the card, where a click is as far from the sidebar as from the window's edge.
fn middle(shell: &Entity<Shell>, cx: &VisualTestContext) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0.center())
}

/// The line is there with no tab selected, from nothing to all of it over the fade, and gone with
/// the next frame once a tab is selected, without waiting for the fade to end.
#[gpui::test]
fn the_line_shows_only_while_no_tab_is_selected(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    let start = Instant::now() - Duration::from_secs(10);
    shell.update(cx, |shell, _| shell.empty_since = Some(start));
    let opacity =
        |cx: &VisualTestContext, at| shell.read_with(cx, |shell, _| shell.empty_opacity(at));
    let fade = Duration::from_secs_f32(FADE);
    assert!(opacity(cx, start) < 1e-3, "it starts from nothing");
    let halfway = opacity(cx, start + fade / 2);
    assert!(
        halfway > 0. && halfway < 1.,
        "halfway through the fade it's at {halfway}"
    );
    assert!(opacity(cx, start + fade) > 0.999);

    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.empty_since.is_none()));
    assert_eq!(
        opacity(cx, Instant::now()),
        0.,
        "a page shown has no line, not a fading one"
    );

    // The card is empty again once the page goes, and the fade starts over.
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.empty_since.is_some()));
}

/// Whether the field the empty card rests on has the keyboard.
fn field_focused(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| {
        let shell = shell.read(cx);
        let resting = shell.resting.as_ref().expect("no field on the empty card");
        resting.bar.read(cx).field_focus(cx).is_focused(window)
    })
}

/// A click anywhere on the empty card gives its field the keyboard and opens no command bar
/// (#4140): the field is where ⌘T's bar was.
#[gpui::test]
fn a_click_on_the_empty_card_gives_its_field_the_keyboard(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    let frame = shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0);
    let corner = Point::new(
        frame.right() - gpui::px(20.),
        frame.bottom() - gpui::px(20.),
    );
    for at in [middle(&shell, cx), corner] {
        cx.update(|window, cx| window.blur(cx));
        assert!(!field_focused(&shell, cx));
        cx.simulate_click(at, Modifiers::none());
        assert!(field_focused(&shell, cx), "a click at {at:?}");
        assert!(
            host.command_bars.take().is_empty(),
            "a click at {at:?} opened the bar"
        );
    }
}

/// With a tab selected the same place is the page's, so the card takes no click.
#[gpui::test]
fn a_page_shown_takes_the_card_away(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    assert!(host.command_bars.take().is_empty());
}

/// The sidebar's New Tab row and its URL field open the bar as the card does, so a click on either
/// with the bar open closes it too.
#[gpui::test]
fn the_other_targets_that_open_the_bar_only_close_it_too(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    let (new_tab, field) = shell.read_with(cx, |shell, _| {
        let row = shell.list_top_with(0) + shell.layout()[&Slot::NewTab] + ROW_HEIGHT / 2.;
        let field = shell
            .url_field_bounds()
            .expect("the URL field is drawn")
            .center();
        (point(px(100.), px(row)), field)
    });
    for at in [new_tab, field] {
        host.bar_open.set(false);
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(
            host.command_bars.take().len(),
            1,
            "a click at {at:?} opens it"
        );
        cx.simulate_click(at, Modifiers::none());
        assert!(
            host.command_bars.take().is_empty(),
            "a second click at {at:?} opened it again"
        );
    }
}

/// With no page the URL field says what typing there does, in the command bar's words and fading
/// in with the card's mark; a page shown puts its host there, and closing it brings the words back.
#[gpui::test]
fn the_url_field_says_what_to_type_only_with_no_page(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://www.iana.org/domains".into(),
            ..Page::default()
        };
        shell.set_page(1, page, cx);
    });
    let words = |cx: &VisualTestContext| {
        shell.read_with(cx, |shell, cx| {
            let (words, color) = shell.url_field_words(&shell.url(shell.selected), cx);
            (words.to_string(), color.a)
        })
    };
    let now = cx.executor().now();
    shell.update(cx, |shell, _| shell.empty_since = Some(now));
    let (said, alpha) = words(cx);
    assert_eq!(said, PLACEHOLDER_TEXT);
    assert!(
        alpha < 0.1,
        "it starts from nothing with the mark, at {alpha}"
    );
    shell.update(cx, |shell, _| {
        shell.empty_since = Some(now - Duration::from_secs(1))
    });
    let (_, faded_in) = words(cx);
    // Dimmer than a host where the window lets it be, and never under what text needs to read.
    let (floor, host) = shell.read_with(cx, |shell, _| {
        (shell.palette.text_floor, shell.palette.text(SECONDARY).a)
    });
    assert!(
        faded_in > floor.max(0.3) - 0.01 && faded_in <= host,
        "reads, no brighter than a host, at {faded_in}"
    );

    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    let (said, alpha) = words(cx);
    assert_eq!(said, "iana.org");
    assert!(
        alpha > faded_in,
        "the host is brighter than the placeholder"
    );

    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert_eq!(words(cx).0, PLACEHOLDER_TEXT);
}

/// The first run's welcome stays until a page shows, and an empty card after that has the Space's
/// name and line again, not the welcome.
#[gpui::test]
fn the_welcome_shows_once(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_welcome(cx));
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.welcome),
        "the empty card has the welcome"
    );

    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert!(
        !shell.read_with(cx, |shell, _| shell.welcome),
        "once a page showed, it's gone"
    );
}

/// A click on the welcome, in the card's middle, opens the command bar, as ⌘T and its button do.
#[gpui::test]
fn the_welcome_opens_a_new_tab(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_welcome(cx));
    cx.run_until_parked();
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    assert_eq!(host.command_bars.take(), [None]);
}

/// Whether the empty card's field is on Ask, and what it says with nothing typed.
fn switch(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (bool, String) {
    cx.update(|_, cx| {
        let resting = shell
            .read(cx)
            .resting
            .as_ref()
            .expect("no field on the empty card");
        let bar = resting.bar.read(cx);
        (bar.on_ask(), bar.placeholder(cx).to_string())
    })
}

/// The empty card's field is made with the card's first frame and asks the core nothing for it:
/// no suggestion, no assistant, no task, until the user types or flips to Ask (#4140).
#[gpui::test]
fn the_empty_card_rests_on_a_field_that_asks_nothing(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    assert!(host.take().contains(&"command bar in place".to_string()));
    assert_eq!(*host.resting.borrow(), Vec::<String>::new());
    assert_eq!(
        switch(&shell, cx),
        (false, "Search or enter address".into())
    );
}

/// A page shown drops the field, and no key the user presses after reaches it or the core through
/// it; with no page again the card rests on a new one.
#[gpui::test]
fn a_page_shown_drops_the_field(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.resting.is_none()));
    cx.simulate_input("ex");
    assert_eq!(
        *host.resting.borrow(),
        Vec::<String>::new(),
        "a key reached the dropped field"
    );
    host.take();
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.resting.is_some()));
    assert!(host.take().contains(&"command bar in place".to_string()));
}

/// ⇥ flips the field to Ask, naming the assistant, and back; Esc in Ask goes back to Search, and
/// in Search does nothing, leaving the card as it is.
#[gpui::test]
fn tab_flips_the_field_and_escape_goes_back_to_search(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    assert_eq!(
        switch(&shell, cx),
        (true, "Ask Claude to do something".into())
    );
    cx.simulate_keystrokes("tab");
    assert_eq!(switch(&shell, cx).0, false);
    cx.simulate_keystrokes("tab escape");
    assert_eq!(
        switch(&shell, cx),
        (false, "Search or enter address".into())
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(switch(&shell, cx).0, false);
    assert!(
        field_focused(&shell, cx),
        "Esc in Search let the keyboard go"
    );
    assert!(shell.read_with(cx, |shell, _| shell.resting.is_some()));
}

/// ↩ in Ask starts what was typed as a task on the assistant, from the card's Space, and empties
/// the field; with nothing typed it starts nothing.
#[gpui::test]
fn return_in_ask_starts_the_task(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab enter");
    host.take();
    cx.simulate_input("book a table");
    cx.simulate_keystrokes("enter");
    assert!(
        host.take()
            .contains(&"start task claude book a table from 1 at Work on pages".to_string()),
        "no task started"
    );
    let typed = cx.update(|_, cx| {
        let resting = shell.read(cx).resting.as_ref().unwrap();
        resting.bar.read(cx).typed_text(cx)
    });
    assert_eq!(typed, "");
}

/// The level the next task starts at, as the chip on Ask's line shows it.
fn tier(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Tier {
    shell.read_with(cx, |shell, _| shell.composer_tier())
}

/// The user's pick on the level menu, as its item does.
fn pick(shell: &Entity<Shell>, tier: Tier, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| shell.pick_tier(tier, cx));
}

/// A task starts at the level picked for it, and the next at Work on pages: the pick holds for one
/// run. The chip shows the level each ↩ starts at.
#[gpui::test]
fn a_run_after_a_run_starts_at_work_on_pages(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    assert_eq!(tier(&shell, cx), Tier::Work);
    pick(&shell, Tier::Full, cx);
    assert_eq!(tier(&shell, cx), Tier::Full);
    host.take();
    cx.simulate_input("book a table");
    cx.simulate_keystrokes("enter");
    assert!(
        host.take()
            .contains(&"start task claude book a table from 1 at Full access".to_string()),
        "the task didn't start at the level picked"
    );
    assert_eq!(tier(&shell, cx), Tier::Work, "the pick outlived its run");
    cx.simulate_input("and a taxi");
    cx.simulate_keystrokes("enter");
    assert!(
        host.take()
            .contains(&"start task claude and a taxi from 1 at Work on pages".to_string()),
        "the next task didn't start at Work on pages"
    );
}

/// Out of Ask the pick goes: Ask → Search → Ask reads Work on pages, and in Search there's no
/// level to pick.
#[gpui::test]
fn ask_then_search_then_ask_reads_work_on_pages(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    pick(&shell, Tier::Look, cx);
    cx.simulate_keystrokes("tab tab");
    assert_eq!(tier(&shell, cx), Tier::Work, "⇥ to Search kept the pick");
    pick(&shell, Tier::Full, cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(tier(&shell, cx), Tier::Work, "Esc to Search kept the pick");
    pick(&shell, Tier::Full, cx);
    assert_eq!(tier(&shell, cx), Tier::Work, "a pick in Search took");
}

/// A pick doesn't survive the user leaving the page: a page shown, or another Space.
#[gpui::test]
fn a_pick_doesnt_survive_leaving_and_coming_back(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    pick(&shell, Tier::Full, cx);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    assert_eq!(tier(&shell, cx), Tier::Work, "a page shown kept the pick");

    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Other".into(), None, 0, "".into(), "".into(), "".into()),
            ],
            cx,
        )
    });
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    pick(&shell, Tier::Full, cx);
    for space in [2, 1] {
        shell.update(cx, |shell, cx| {
            shell.set_space(space, cx);
            shell.set_tabs(
                space,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Folders::default(),
                cx,
            );
        });
        cx.run_until_parked();
    }
    assert_eq!(tier(&shell, cx), Tier::Work, "another Space kept the pick");
}

/// The pick lives in the page's memory alone: nothing the shell asks of the core while it's
/// picked, saving included, carries it, and a run's start is the only call that does.
#[gpui::test]
fn nothing_of_a_pick_is_saved(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    host.take();
    pick(&shell, Tier::Full, cx);
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    let calls = host.take();
    assert!(
        !calls.iter().any(|call| call.contains(Tier::Full.label())),
        "a call carried the pick: {calls:?}"
    );
}

/// The field's row rests level with ⌘T's bar's, and the group moves up only as far as it needs to
/// fit the card, never nearer its top than `TOP`.
#[test]
fn the_field_rests_where_the_bar_opens_and_rises_only_to_fit() {
    use crate::tokens::shell::composer::{FIELD_HEIGHT, MARK_GAP, TOP};
    let row = crate::command_bar::centered_row_middle(800.) - 40.;
    let top = group_top(row, 720., false, false);
    let field = top + MARK_HEIGHT + MARK_GAP;
    assert!(
        (field + FIELD_HEIGHT / 2. - row).abs() < 1e-3,
        "the field's middle is at {}, the bar's row's at {row}",
        field + FIELD_HEIGHT / 2.
    );
    // A card 560 tall has room for the field there, but not for the cards under it.
    let cards = group_top(row, 560., true, true);
    assert!(cards < top, "the cards didn't lift the group");
    assert!(group_top(row, 400., true, true) == TOP, "it went above TOP");
    assert_eq!(
        group_top(row, 2000., true, true),
        top,
        "a tall card moved it"
    );
}

/// An ended task's Start Again asks again at Work on pages, whatever the composer had picked.
#[gpui::test]
fn start_again_asks_at_work_on_pages(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    pick(&shell, Tier::Full, cx);
    host.take();
    shell.update(cx, |shell, cx| {
        shell.ask_again("Find two tickets".into(), cx)
    });
    assert!(
        host.take()
            .contains(&"ask again Find two tickets".to_string())
    );
    assert_eq!(tier(&shell, cx), Tier::Work);
}
