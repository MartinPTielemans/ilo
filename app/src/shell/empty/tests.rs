//! The card of a window with no page, on a host that records what the shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Entity, Modifiers, Pixels, Point, SharedString, TestAppContext, VisualTestContext, point, px,
};

use crate::agents::task::{Card, Dot};
use crate::command_bar::PLACEHOLDER_TEXT;
use crate::shell::{
    Folders, Page, ROW_HEIGHT, SECONDARY, Shell, Slot,
    empty::{
        FADE,
        composer::{group_top, mark_top},
    },
    recorder::Recorder,
    task_cards::tests::card,
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
    // Dimmer than a host where the window lets it be, and never under what gray text needs to read.
    let (floor, host) = shell.read_with(cx, |shell, _| {
        (shell.palette.gray_floor, shell.palette.text(SECONDARY).a)
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

/// The welcome is the mark, its one line and the button: nothing stands between the line and the
/// button, and the bar's mark isn't there with it.
#[gpui::test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "GPUI records debug selectors only in debug builds"
)]
fn the_welcome_has_no_line_under_its_title(cx: &mut TestAppContext) {
    use crate::tokens::shell::empty::BUTTON_GAP;
    let (shell, cx, _) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| shell.set_welcome(cx));
    cx.run_until_parked();
    let title = cx.debug_bounds("welcome-title").expect("no welcome");
    let button = cx.debug_bounds("welcome-button").expect("no button");
    let gap = f32::from(button.top() - title.bottom());
    assert!(
        (gap - BUTTON_GAP).abs() < 0.5,
        "the button is {gap} under the title: something stands between them"
    );
    assert_eq!(cx.debug_bounds("resting-mark"), None);
}

/// Whether the empty page's bar is the composer, and what its field says with nothing typed.
fn composer(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (bool, String) {
    cx.update(|_, cx| {
        let resting = shell
            .read(cx)
            .resting
            .as_ref()
            .expect("no bar on the empty page");
        let bar = resting.bar.read(cx);
        (bar.composing_thread(), bar.placeholder(cx).to_string())
    })
}

/// What the empty page's bar asked of the core, without the assistants it looked for.
fn rested(host: &Recorder) -> Vec<String> {
    (host.resting.take().into_iter())
        .filter(|call| call != "task agents" && call != "look for agents")
        .collect()
}

/// The empty card's field is made with the card's first frame and asks the core nothing for it:
/// no suggestion, no assistant, no task, until the user types or grows it with ⇥ (#4140).
#[gpui::test]
fn the_empty_card_rests_on_a_field_that_asks_nothing(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    assert!(host.take().contains(&"command bar in place".to_string()));
    assert_eq!(*host.resting.borrow(), Vec::<String>::new());
    assert_eq!(
        composer(&shell, cx),
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

/// ⇥ grows the page's bar into ⌘T's composer where it rests, and ⇥ again, ⇧⇥ or Esc take it back
/// to search with what's typed; Esc in search leaves the page as it is, the field keeping the
/// keyboard. Nothing opens at Today's top for it.
#[gpui::test]
fn tab_grows_the_bar_in_place_and_back_keeps_the_words(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_input("book a table");
    host.resting.take();
    cx.simulate_keystrokes("tab");
    assert!(composer(&shell, cx).0, "⇥ didn't grow the bar");
    for back in ["tab", "shift-tab", "escape"] {
        cx.simulate_keystrokes(back);
        let typed = cx.update(|_, cx| {
            let resting = shell.read(cx).resting.as_ref().unwrap();
            resting.bar.read(cx).typed_text(cx)
        });
        assert_eq!(
            (composer(&shell, cx).0, typed.as_str()),
            (false, "book a table"),
            "{back} back lost the words"
        );
        cx.simulate_keystrokes("tab");
    }
    cx.simulate_keystrokes("escape escape");
    assert!(!composer(&shell, cx).0);
    assert!(
        field_focused(&shell, cx),
        "Esc in search let the keyboard go"
    );
    assert!(shell.read_with(cx, |shell, _| shell.resting.is_some()));
    assert!(
        !rested(&host).iter().any(|call| call.starts_with("asking")),
        "the composer at rest opened Today's slot"
    );
}

/// ↩ in the composer at rest starts what was typed as a thread on the assistant, where the bar
/// rests, at the level the composer shows, and empties the field.
#[gpui::test]
fn return_in_the_composer_at_rest_starts_the_thread(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_input("book a table");
    host.resting.take();
    cx.simulate_keystrokes("tab");
    assert_eq!(rested(&host), Vec::<String>::new(), "⇥ asked the core");
    cx.simulate_keystrokes("enter");
    assert_eq!(
        rested(&host),
        ["run task at rest claude book a table at work"]
    );
    let typed = cx.update(|_, cx| {
        let resting = shell.read(cx).resting.as_ref().unwrap();
        resting.bar.read(cx).typed_text(cx)
    });
    assert_eq!(typed, "");
}

/// ↩ in the composer at rest with "book a table" typed, on a window with no tab, as the host
/// takes it: the start is the Space's, over the empty page.
fn composed(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_input("book a table");
    cx.simulate_keystrokes("tab enter");
    assert!(
        (host.resting.borrow().iter()).any(|call| call.starts_with("run task at rest")),
        "↩ started nothing"
    );
    shell.update(cx, |shell, _| shell.bar_starting());
    (shell, cx, host)
}

/// Tasks of Space 1, each card's own Space made as the core makes it before its card comes, as
/// the Agents do, so its view can open.
fn made(cards: &[Card], shell: &Entity<Shell>, host: &Recorder, cx: &mut VisualTestContext) {
    let space = |id: u64, name: &str, agent: &str| {
        let (name, agent) = (name.to_string().into(), agent.to_string().into());
        (id, name, None, 0, "".into(), agent, "".into())
    };
    let mut spaces = vec![space(1, "Space", "")];
    for made in cards {
        host.space_agents
            .borrow_mut()
            .insert(made.space, made.agent.clone());
        spaces.push(space(made.space, &made.title, "Claude"));
    }
    shell.update(cx, |shell, cx| {
        shell.set_spaces(spaces, cx);
        for made in cards {
            shell.set_tabs(
                made.space,
                Vec::new(),
                vec![made.space * 10],
                Vec::new(),
                Folders::default(),
                cx,
            );
        }
    });
}

/// The task whose view is open, once the cards are `cards`.
fn opened(
    shell: &Entity<Shell>,
    cards: Vec<Card>,
    cx: &mut VisualTestContext,
) -> Option<SharedString> {
    shell.update(cx, |shell, cx| shell.set_task_cards(cards, cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| shell.talk_agent())
}

/// ↩ in Ask opens the started task's view as its card comes in, as the Space has no page to stay
/// on; a card of another Space's task opens nothing.
#[gpui::test]
fn return_in_ask_opens_the_started_task(cx: &mut TestAppContext) {
    let (shell, cx, host) = composed(cx);
    let started = card(2, Dot::Working);
    made(std::slice::from_ref(&started), &shell, &host, cx);
    let elsewhere = Card {
        home: 2,
        ..card(1, Dot::Working)
    };
    let cards = vec![elsewhere.clone()];
    assert_eq!(
        opened(&shell, cards, cx),
        None,
        "another Space's task opened"
    );
    assert_eq!(
        opened(&shell, vec![started, elsewhere], cx),
        Some("task-2".into()),
        "the started task's view didn't open"
    );
}

/// ↩ in Ask from the resting field opens the started task's conversation in the page's place,
/// though its card comes before the window hears of the task's Space, as the core tells that
/// after the call that made it: the start waits for the Space, and opens as it comes.
#[gpui::test]
fn return_in_ask_opens_its_task_when_its_space_comes_after_its_card(cx: &mut TestAppContext) {
    let (shell, cx, host) = composed(cx);
    let started = card(2, Dot::Working);
    shell.update(cx, |shell, cx| {
        shell.composed_started(Some("task-2".into()));
        shell.set_task_cards(vec![started.clone()], cx);
    });
    cx.run_until_parked();
    let area = |cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            (shell.talk_agent(), shell.task_area.is_some())
        })
    };
    assert_eq!(
        area(cx),
        (None, false),
        "it opened with no Space to open in"
    );
    made(std::slice::from_ref(&started), &shell, &host, cx);
    cx.run_until_parked();
    assert_eq!(
        area(cx),
        (Some("task-2".into()), true),
        "the started task's conversation didn't take the page's place once its Space came"
    );
}

/// ↩ in ⌘T's composer over a tab opens the task it started in that tab's place, though its card
/// comes before its Space, as from the resting field; another tab shown by then stays, with the
/// task's card at the foot.
#[gpui::test]
fn return_in_the_bars_composer_opens_its_task_over_the_tab(cx: &mut TestAppContext) {
    for (another_shown, opened) in [(false, Some("task-2")), (true, None)] {
        let (shell, cx, host) = window_with_no_tab(cx);
        let started = card(2, Dot::Working);
        shell.update(cx, |shell, cx| {
            shell.set_selected(1, cx);
            shell.bar_starting();
            shell.composed_started(Some("task-2".into()));
            shell.set_task_cards(vec![started.clone()], cx);
            if another_shown {
                shell.set_selected(2, cx);
            }
        });
        made(std::slice::from_ref(&started), &shell, &host, cx);
        cx.run_until_parked();
        let shown = shell.read_with(cx, |shell, _| {
            (shell.talk_agent(), shell.task_area.is_some())
        });
        assert_eq!(
            shown,
            (opened.map(SharedString::from), opened.is_some()),
            "with another tab shown first: {another_shown}"
        );
    }
}

/// Once the Agents say which task ↩ started, that one's card alone opens a view: another of the
/// same Space that comes first opens none.
#[gpui::test]
fn return_in_ask_opens_the_task_it_started_alone(cx: &mut TestAppContext) {
    let (shell, cx, host) = composed(cx);
    let (other, started) = (card(3, Dot::Working), card(2, Dot::Working));
    made(&[other.clone(), started.clone()], &shell, &host, cx);
    shell.update(cx, |shell, _| shell.composed_started(Some("task-2".into())));
    assert_eq!(opened(&shell, vec![other.clone()], cx), None);
    assert_eq!(
        opened(&shell, vec![other, started], cx),
        Some("task-2".into())
    );
}

/// A ↩ that started no task leaves no mark: the next task of that Space, another's, opens no view
/// by itself.
#[gpui::test]
fn a_start_that_made_no_task_opens_no_later_one(cx: &mut TestAppContext) {
    let (shell, cx, host) = composed(cx);
    let later = card(2, Dot::Working);
    made(std::slice::from_ref(&later), &shell, &host, cx);
    shell.update(cx, |shell, _| shell.composed_started(None));
    assert_eq!(
        opened(&shell, vec![later], cx),
        None,
        "a stale mark opened it"
    );
}

/// The user who leaves the empty page after ↩, here for another task's conversation, leaves the
/// start's mark behind: back on the empty page, the next task of that Space opens no view.
#[gpui::test]
fn leaving_the_empty_page_forgets_the_start(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    let (seen, later) = (card(3, Dot::Working), card(2, Dot::Working));
    made(&[seen.clone(), later.clone()], &shell, &host, cx);
    assert_eq!(opened(&shell, vec![seen.clone()], cx), None);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_input("book a table");
    cx.simulate_keystrokes("tab enter");
    shell.update(cx, |shell, _| shell.bar_starting());
    shell.update(cx, |shell, cx| shell.show_task("task-3".into(), cx));
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.close_task(cx));
    cx.run_until_parked();
    assert_eq!(shell.read_with(cx, |shell, _| shell.talk_agent()), None);
    assert_eq!(
        opened(&shell, vec![seen, later], cx),
        None,
        "a mark left from before opened it"
    );
}

/// The bar rests with its input row level with ⌘T's bar's, and the group moves up only as far as
/// it needs to fit the card, never nearer its top than `TOP`.
#[test]
fn the_bar_rests_where_the_bar_opens_and_rises_only_to_fit() {
    use crate::tokens::command_bar::HEIGHT;
    use crate::tokens::shell::composer::TOP;
    let row = crate::command_bar::centered_row_middle(800.) - 40.;
    let top = group_top(row, 720., false, false);
    assert!(
        (top + HEIGHT / 2. - row).abs() < 1e-3,
        "the bar's row's middle is at {}, ⌘T's at {row}",
        top + HEIGHT / 2.
    );
    // A card 560 tall has room for the bar there, but not for the cards under it.
    let cards = group_top(row, 560., true, true);
    assert!(cards < top, "the cards didn't lift the group");
    assert!(group_top(row, 400., true, true) == TOP, "it went above TOP");
    assert_eq!(
        group_top(row, 2000., true, true),
        top,
        "a tall card moved it"
    );
}

/// The mark stands its gap over the bar where the card has that room over it. A Little Window
/// has none, and neither has a card so short that the group was lifted to its top.
#[test]
fn the_mark_stands_its_gap_over_the_bar_and_gives_way() {
    use crate::tokens::shell::composer::{MARK_GAP, TOP};
    use crate::tokens::shell::empty::MARK_SIZE;
    let row = crate::command_bar::centered_row_middle(800.) - 40.;
    let top = group_top(row, 720., false, false);
    assert_eq!(mark_top(top, false), Some(top - MARK_GAP - MARK_SIZE));
    assert_eq!(mark_top(top, true), None, "a Little Window drew the mark");
    assert_eq!(mark_top(TOP, false), None, "it stands over the card's top");
}

/// The product's mark stands over the bar at rest: in the bar's column, its gap over the bar, at
/// the size the tokens give it. GPUI records where an element was drawn only in a debug build.
#[gpui::test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "GPUI records debug selectors only in debug builds"
)]
fn the_mark_stands_over_the_resting_bar_in_its_column(cx: &mut TestAppContext) {
    use crate::tokens::shell::composer::MARK_GAP;
    use crate::tokens::shell::empty::MARK_SIZE;
    let (_, cx, _) = window_with_no_tab(cx);
    let mark = cx
        .debug_bounds("resting-mark")
        .expect("the bar at rest has no mark over it");
    let bar = cx.debug_bounds("resting-bar").expect("no bar at rest");
    let off = f32::from(mark.center().x - bar.center().x);
    assert!(off.abs() < 0.5, "the mark is {off} off the bar's column");
    let gap = f32::from(bar.top() - mark.bottom());
    assert!(
        (gap - MARK_GAP).abs() < 0.5,
        "the mark is {gap} over the bar"
    );
    assert_eq!(mark.size, gpui::size(px(MARK_SIZE), px(MARK_SIZE)));
}

/// A Little Window is a look at one page, so its card with no page has no mark, whatever else
/// it rests on.
#[gpui::test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "GPUI records debug selectors only in debug builds"
)]
fn a_little_window_with_no_page_draws_no_mark(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::little_window(browser, 9, 1, 1, None, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            9,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.little.is_some() && shell.selected == 0),
        "it isn't a Little Window with no page"
    );
    assert_eq!(cx.debug_bounds("resting-mark"), None);
}

/// Makes the test's window the key one, as the user's window is when they're in it.
fn key_window(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
}

/// The Space shows no page: its field has the keyboard once the window is key, and only then. It
/// takes it once, so whatever the user gives the keyboard to after keeps it.
#[gpui::test]
fn an_empty_space_gives_its_field_the_keyboard_once_the_window_is_key(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    assert!(
        !field_focused(&shell, cx),
        "the field took the keyboard of a window that isn't key"
    );
    key_window(cx);
    assert!(
        field_focused(&shell, cx),
        "the key window's field has no keyboard"
    );
    assert!(host.command_bars.take().is_empty());

    cx.update(|window, cx| window.blur(cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        !field_focused(&shell, cx),
        "the field took the keyboard back from where the user put it"
    );
}

/// A fresh window on an empty Space, with no click: the field the caret is drawn in has the
/// keyboard, in GPUI and in the window, so ⇥ grows it into the composer. GPUI's focus alone draws the caret,
/// while the Mac sends a key to its window's first responder, which only the host's hold gives
/// the UI; a click gives it by itself, so ⇥ reached the composer only after one.
#[gpui::test]
fn a_fresh_windows_field_takes_tab_without_a_click(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    host.take();
    key_window(cx);
    assert!(
        field_focused(&shell, cx),
        "the key window's field has no keyboard"
    );
    assert_eq!(
        host.take(),
        ["hold keyboard for page"],
        "the field took GPUI's focus, but the window's keys weren't given to the UI"
    );
    cx.simulate_input("book");
    cx.simulate_keystrokes("tab");
    assert!(
        composer(&shell, cx).0,
        "⇥ with no click didn't reach the field"
    );
}

/// At launch the core can show the window's page before the shell hears of it. The field doesn't
/// take the window's keys then: they're that page's, which comes in a moment.
#[gpui::test]
fn the_field_takes_no_window_keys_from_a_page_the_core_already_shows(cx: &mut TestAppContext) {
    let (_, cx, host) = window_with_no_tab(cx);
    host.shown_now.set(1);
    host.take();
    key_window(cx);
    assert!(
        !host.take().contains(&"hold keyboard for page".to_string()),
        "the field took the window's keys from the page the core shows"
    );
}

/// ⌘T and ⌘L give the field the keyboard and open no bar over it. The sidebar's New Tab and URL
/// field and Start Again's ask still open the bar, as they did.
#[gpui::test]
fn cmd_t_and_cmd_l_give_the_field_the_keyboard_and_open_no_bar(cx: &mut TestAppContext) {
    use crate::shell::Command;
    let (shell, cx, host) = window_with_no_tab(cx);
    key_window(cx);
    for command in [Command::NewTab, Command::EditUrl] {
        for from in ["ui", "page"] {
            cx.update(|window, cx| window.blur(cx));
            shell.update(cx, |shell, cx| shell.run(command, from, cx));
            cx.run_until_parked();
            assert!(
                field_focused(&shell, cx),
                "{command:?} via {from} left the field without the keyboard"
            );
            assert!(
                host.command_bars.take().is_empty(),
                "{command:?} via {from} opened the bar over the field"
            );
        }
        cx.update(|window, cx| window.blur(cx));
        shell.update(cx, |shell, cx| shell.run(command, "sidebar", cx));
        cx.run_until_parked();
        assert_eq!(
            host.command_bars.take().len(),
            1,
            "{command:?} from the sidebar opened no bar"
        );
        assert!(!field_focused(&shell, cx));
        host.bar_open.set(false);
        shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
    }
    shell.update(cx, |shell, cx| {
        shell.ask_again("Find two tickets".into(), cx)
    });
    assert_eq!(
        host.command_bars.take().len(),
        1,
        "Start Again opened no bar"
    );
}

/// ⌘T pressed before the shell has heard of the page the core shows, as one pressed as the window
/// comes on screen with a page to open, opens the bar over that page: the field it would have gone
/// to goes as the page is heard of, and took the key with it (#4268).
#[gpui::test]
fn cmd_t_before_the_shown_page_is_heard_of_opens_the_bar(cx: &mut TestAppContext) {
    use crate::shell::Command;
    let (shell, cx, host) = window_with_no_tab(cx);
    key_window(cx);
    host.shown_now.set(1);
    for from in ["ui", "page"] {
        shell.update(cx, |shell, cx| shell.run(Command::NewTab, from, cx));
        cx.run_until_parked();
        assert_eq!(
            host.command_bars.take().len(),
            1,
            "⌘T via {from}, with the core showing a page the shell hadn't heard of, opened no bar"
        );
        host.bar_open.set(false);
        shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
    }
}

/// An open overlay keeps the keyboard: ⌘T's field waits for it to close.
#[gpui::test]
fn the_field_takes_no_keyboard_from_an_overlay(cx: &mut TestAppContext) {
    use crate::shell::Command;
    let (shell, cx, host) = window_with_no_tab(cx);
    key_window(cx);
    cx.update(|window, cx| window.blur(cx));
    host.bar_open.set(true);
    shell.update(cx, |shell, cx| shell.run(Command::NewTab, "ui", cx));
    cx.run_until_parked();
    assert!(
        !field_focused(&shell, cx),
        "the field took the keyboard from the open bar"
    );
    host.bar_open.set(false);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        field_focused(&shell, cx),
        "the field didn't take it once the bar closed"
    );
}

/// Another empty Space shown slides in: its field takes the keyboard when the slide has settled,
/// not while the pages move.
#[gpui::test]
fn another_empty_space_takes_the_keyboard_when_its_page_settles(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Other".into(), None, 0, "".into(), "".into(), "".into()),
            ],
            cx,
        );
        shell.set_tabs(2, Vec::new(), vec![3], Vec::new(), Folders::default(), cx);
    });
    key_window(cx);
    assert!(field_focused(&shell, cx));
    cx.update(|window, cx| window.blur(cx));
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.settle.is_some()),
        "the page didn't slide in"
    );
    assert!(
        !field_focused(&shell, cx),
        "the field took the keyboard while the pages moved"
    );
    shell.update(cx, |shell, _| {
        if let Some(settle) = &mut shell.settle {
            settle.start -= Duration::from_secs(5);
        }
    });
    while shell.read_with(cx, |shell, _| shell.settle.is_some()) {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    assert!(
        field_focused(&shell, cx),
        "the settled Space's field has no keyboard"
    );
}

/// Closing the last tab rests the Space on its bar again, in search and with the keyboard,
/// though it was the composer before the tab showed.
#[gpui::test]
fn closing_the_last_tab_returns_to_the_field_in_search(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    key_window(cx);
    cx.simulate_input("book");
    cx.simulate_keystrokes("tab");
    assert!(composer(&shell, cx).0, "⇥ didn't grow the bar");
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.blur(cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![2], Vec::new(), Folders::default(), cx);
        shell.set_selected(0, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        composer(&shell, cx),
        (false, "Search or enter address".into())
    );
    assert!(
        field_focused(&shell, cx),
        "the field came back without the keyboard"
    );
}

/// A private window starts no thread, so its empty page's bar doesn't grow into the composer: ⇥
/// there is search's. A window of the profile's grows it.
#[gpui::test]
fn a_private_windows_bar_composes_no_thread(cx: &mut TestAppContext) {
    for private in [true, false] {
        let (shell, cx, host) = window_with_no_tab(cx);
        host.private_bar.set(private);
        cx.simulate_click(middle(&shell, cx), Modifiers::none());
        cx.simulate_input("book a table");
        cx.simulate_keystrokes("tab enter");
        assert_eq!(
            composer(&shell, cx).0 || rested(&host).iter().any(|c| c.starts_with("run task")),
            !private,
            "private: {private}"
        );
    }
}
