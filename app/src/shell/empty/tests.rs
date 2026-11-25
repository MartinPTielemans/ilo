//! The card of a window with no page, on a host that records what the shell asks of the core.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Entity, Modifiers, Pixels, Point, SharedString, TestAppContext, VisualTestContext, point, px,
};

use crate::agents::task::{Card, Dot};
use crate::agents::tier::Tier;
use crate::command_bar::{Given, PLACEHOLDER_TEXT};
use crate::shell::{
    Folders, Page, ROW_HEIGHT, SECONDARY, Shell, Slot,
    empty::{FADE, MARK_HEIGHT, composer::group_top},
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

/// ↩ in Ask with "book a table" typed, on a window with no tab.
fn composed(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.simulate_input("book a table");
    cx.simulate_keystrokes("enter");
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
    cx.simulate_keystrokes("tab");
    cx.simulate_input("book a table");
    cx.simulate_keystrokes("enter");
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

/// The level the next task starts at, as the chip on Ask's line shows it.
fn tier(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Tier {
    shell.read_with(cx, |shell, _| shell.composer_tier())
}

/// The user's pick on the level menu, as its item does.
fn pick(shell: &Entity<Shell>, tier: Tier, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| shell.pick_tier(tier, cx));
}

/// The level's menu shows names only: the levels, where the task may go, and Settings, which opens
/// Settings › Agents at Access. Only the sites you add waits for a page the user added. It hangs
/// 4 under the chip, its names under the chip's word.
#[gpui::test]
fn the_level_menu_names_the_levels_the_sites_and_settings(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let chip = shell.read_with(cx, |shell, _| {
        shell
            .resting
            .as_ref()
            .and_then(|resting| resting.tier_chip.get())
    });
    let chip = chip.expect("the level chip drawn on Ask's line");
    shell.update(cx, |shell, cx| shell.tier_menu(point(px(0.), px(0.)), cx));
    let titles = host.menus.borrow().last().cloned().expect("a menu shown");
    let titles: Vec<(String, bool)> = (titles.into_iter())
        .map(|(title, enabled)| (title.to_string(), enabled))
        .collect();
    let row = |title: &str, enabled: bool| (title.to_string(), enabled);
    assert_eq!(
        titles,
        vec![
            row("Look only", true),
            row("Work on pages", true),
            row("Full access", true),
            row("", false),
            row("Any site", true),
            row("Only the sites you add", false),
            row("", false),
            row("Settings…", true),
        ],
        "the level menu's rows"
    );
    let checked: Vec<bool> = host.entries.borrow().iter().map(|e| e.checked).collect();
    assert_eq!(
        checked,
        [false, true, false, false, true, false, false, false],
        "Work on pages and Any site checked"
    );
    let at = host.menu_at.get().expect("where the menu was shown");
    assert_eq!(
        at.y,
        chip.bottom() + px(4.),
        "the menu hangs 4 under the chip"
    );
    assert!(
        at.x < chip.left(),
        "the menu's check column sits left of the chip's word"
    );
    host.take();
    cx.update(|_, cx| host.choose("Settings…", cx));
    assert_eq!(
        host.take(),
        vec!["agent access".to_string()],
        "Settings… opens Access"
    );
    assert_eq!(
        tier(&shell, cx),
        Tier::Work,
        "Settings… left the level as it was"
    );
}

/// The assistant's chip names the model the next task starts on, Settings' (Opus here) till the
/// user picks on its menu: its default first, then the models from Settings' list. The pick starts
/// that task alone; the next starts on Settings' again, as does a trip out of Ask.
#[gpui::test]
fn the_model_chip_picks_for_one_task(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let chip = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            let resting = shell.resting.as_ref().unwrap();
            let agent = resting.bar.update(cx, |bar, cx| bar.assistant(cx)).unwrap();
            super::composer::model_words(
                &agent,
                &super::composer::next_model(resting.model.as_ref(), &agent),
            )
        })
    };
    assert_eq!(
        chip(&shell, cx),
        "Claude · Opus",
        "the chip says Settings' model"
    );
    shell.update(cx, |shell, cx| shell.model_menu(point(px(0.), px(0.)), cx));
    let rows: Vec<String> = (host.entries.borrow().iter())
        .map(|entry| {
            format!(
                "{}{}",
                entry.title,
                if entry.checked { " (checked)" } else { "" }
            )
        })
        .collect();
    assert_eq!(
        rows,
        ["Claude Code's default", "Opus (checked)", "Sonnet", "Haiku"],
        "the model menu's rows"
    );
    cx.update(|_, cx| host.choose("Haiku", cx));
    assert_eq!(chip(&shell, cx), "Claude · Haiku", "the chip says the pick");
    host.take();
    cx.simulate_input("book a table");
    cx.simulate_keystrokes("enter");
    let calls = host.take();
    assert!(
        calls.contains(
            &"start task claude book a table from 1 at Work on pages on \"haiku\"".to_string()
        ),
        "the pick didn't reach its task: {calls:?}"
    );
    assert_eq!(
        chip(&shell, cx),
        "Claude · Opus",
        "the next task starts on Settings' again"
    );

    shell.update(cx, |shell, cx| {
        shell.pick_model("claude".into(), String::new(), cx)
    });
    assert_eq!(
        chip(&shell, cx),
        "Claude Code's default",
        "the default picked"
    );
    cx.simulate_keystrokes("tab");
    cx.simulate_keystrokes("tab");
    assert_eq!(
        chip(&shell, cx),
        "Claude · Opus",
        "out of Ask forgets the pick"
    );
}

/// The `+` menu lists the Space's http(s) pages by title (Add a Page's list, a menu of its own till
/// files and folders join it), and a pick adds one, whose
/// title shows on Ask's line and whose address alone the task gets; Only the sites you add waits
/// for one. A second pick takes it off, and the picks go with the run. Files and folders aren't
/// offered yet.
#[gpui::test]
fn add_a_page_gives_the_task_its_address_alone(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    shell.update(cx, |shell, cx| {
        let page = |url: &str| Page {
            url: url.into(),
            ..Page::default()
        };
        shell.set_page(1, page("https://fieldbook.example/lakeside"), cx);
        shell.set_title(1, "The Lakeside loop".into(), cx);
        shell.set_page(2, page("file:///Users/someone/notes.txt"), cx);
    });
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let rows = |host: &Recorder| -> Vec<String> {
        (host.entries.borrow().iter())
            .map(|entry| {
                let under = if entry.parent.is_some() { "  " } else { "" };
                let on = if entry.checked { " (checked)" } else { "" };
                let off = if entry.enabled { "" } else { " (off)" };
                format!("{under}{}{on}{off}", entry.title)
            })
            .collect()
    };
    shell.update(cx, |shell, cx| shell.tier_menu(point(px(0.), px(0.)), cx));
    assert!(
        rows(&host).contains(&"Only the sites you add (off)".to_string()),
        "Only the sites you add waits for a page"
    );
    shell.update(cx, |shell, cx| shell.pages_menu(point(px(0.), px(0.)), cx));
    assert_eq!(
        rows(&host),
        ["The Lakeside loop"],
        "the + menu lists the Space's web page, and no file"
    );
    cx.update(|_, cx| host.choose("The Lakeside loop", cx));
    shell.update(cx, |shell, cx| shell.pages_menu(point(px(0.), px(0.)), cx));
    assert_eq!(rows(&host), ["The Lakeside loop (checked)"]);
    cx.update(|_, cx| host.choose("The Lakeside loop", cx));
    let added = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.resting.as_ref().unwrap().pages.len())
    };
    assert_eq!(added(&shell, cx), 0, "a second pick takes it off");

    shell.update(cx, |shell, cx| {
        shell.toggle_page(1, cx);
        shell.tier_menu(point(px(0.), px(0.)), cx);
    });
    cx.update(|_, cx| host.choose("Only the sites you add", cx));
    host.take();
    cx.simulate_input("find a place to stay");
    cx.simulate_keystrokes("enter");
    let calls = host.take();
    let started = "start task claude find a place to stay from 1 at Work on pages \
                   with [\"https://fieldbook.example/lakeside\"] only them";
    assert!(
        calls.contains(&started.to_string()),
        "the page's address didn't reach the task: {calls:?}"
    );
    assert_eq!(added(&shell, cx), 0, "the pages go with the run");
    let only = shell.read_with(cx, |shell, _| shell.resting.as_ref().unwrap().only_added);
    assert!(!only, "the next task may go to any site");
}

/// On a new profile, ilo hasn't looked for an assistant yet: the switch to Ask looks, as ⌘T's ask
/// does, and once the browser has found one its chip is on Ask's line and the field names it.
#[gpui::test]
fn ask_on_a_new_profile_looks_for_the_assistant_and_shows_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    host.unlearned.set(true);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    host.resting.borrow_mut().clear();
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let asked = host.resting.borrow().clone();
    assert_eq!(
        (asked.iter())
            .filter(|call| *call == "look for agents")
            .count(),
        1,
        "the switch to Ask looks for the assistants, once: {asked:?}"
    );
    let shown = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            let resting = shell.resting.as_ref().unwrap();
            let placeholder = resting.bar.read(cx).placeholder(cx);
            (resting.model_chip.get().is_some(), placeholder.to_string())
        })
    };
    assert_eq!(
        shown(&shell, cx),
        (false, "Ask an assistant to do something".into()),
        "none found yet"
    );

    // The browser found Claude, and the window is sent what it knows (send_agents).
    host.unlearned.set(false);
    shell.update(cx, |shell, cx| shell.assistants_learned(cx));
    cx.run_until_parked();
    assert_eq!(
        shown(&shell, cx),
        (true, "Ask Claude to do something".into()),
        "the assistant's chip is there, and the field names it"
    );
}

/// With no page of the Space's to add, the `+` is disabled in its place: there, and a click on it
/// shows no menu. Once a page can be added the same click lists it.
#[gpui::test]
fn the_plus_with_nothing_to_add_is_disabled(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let plus = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        let chip = shell.read_with(cx, |shell, _| {
            (shell.resting.as_ref()).and_then(|resting| resting.plus_chip.get())
        });
        chip.expect("the + drawn on Ask's line, disabled or not")
            .center()
    };
    let offers = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, cx| shell.plus_offers(cx))
    };
    assert!(!offers(&shell, cx), "the Space's tabs have no page to add");
    let at = plus(&shell, cx);
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(
        host.menus.borrow().is_empty() && host.entries.borrow().is_empty(),
        "a click on the disabled + shows nothing"
    );

    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://fieldbook.example/lakeside".into(),
            ..Page::default()
        };
        shell.set_page(1, page, cx);
        shell.set_title(1, "The Lakeside loop".into(), cx);
    });
    cx.run_until_parked();
    assert!(offers(&shell, cx));
    let at = plus(&shell, cx);
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let titles: Vec<String> = (host.entries.borrow().iter())
        .map(|entry| entry.title.to_string())
        .collect();
    assert_eq!(
        titles,
        ["The Lakeside loop"],
        "the same click lists the page"
    );
}

/// A tab's address can carry a user name and password, which the task never gets; a page whose
/// address is too long to give a task isn't offered.
#[gpui::test]
fn an_added_page_carries_no_login_and_a_long_one_isnt_offered(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    let long = format!("https://fieldbook.example/{}", "a".repeat(2048));
    shell.update(cx, |shell, cx| {
        let page = |url: &str| Page {
            url: url.into(),
            ..Page::default()
        };
        shell.set_page(
            1,
            page("https://sam:hunter2@fieldbook.example/lakeside"),
            cx,
        );
        shell.set_title(1, "The Lakeside loop".into(), cx);
        shell.set_page(2, page(&long), cx);
        shell.set_title(2, "A long one".into(), cx);
    });
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.pages_menu(point(px(0.), px(0.)), cx));
    let titles: Vec<String> = (host.entries.borrow().iter())
        .map(|entry| entry.title.to_string())
        .collect();
    assert_eq!(
        titles,
        ["The Lakeside loop"],
        "a page over the cap is offered"
    );
    cx.update(|_, cx| host.choose("The Lakeside loop", cx));
    host.take();
    cx.simulate_input("find a place to stay");
    cx.simulate_keystrokes("enter");
    let started = "start task claude find a place to stay from 1 at Work on pages \
                   with [\"https://fieldbook.example/lakeside\"]";
    let calls = host.take();
    assert!(
        calls.contains(&started.to_string()),
        "the task got more than the page's address: {calls:?}"
    );
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

/// Closing the last tab rests the Space on its field again, in Search and with the keyboard,
/// whatever it was on before the tab showed.
#[gpui::test]
fn closing_the_last_tab_returns_to_the_field_in_search(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    key_window(cx);
    cx.simulate_keystrokes("tab");
    assert!(switch(&shell, cx).0, "⇥ didn't go to Ask");
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.blur(cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![2], Vec::new(), Folders::default(), cx);
        shell.set_selected(0, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        switch(&shell, cx),
        (false, "Search or enter address".into())
    );
    assert!(
        field_focused(&shell, cx),
        "the field came back without the keyboard"
    );
}

/// A page added is a chip in the field, before what's typed, as ⌘T's ask has its page; nothing on
/// Ask's line moves for it. ⌫ at the field's start takes the last one off, and a run takes them
/// all.
#[gpui::test]
fn an_added_page_is_a_chip_in_the_field(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    cx.update(|_, cx| crate::text_input::bind_keys(cx));
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://fieldbook.example/lakeside".into(),
            ..Page::default()
        };
        shell.set_page(1, page, cx);
        shell.set_title(1, "The Lakeside loop".into(), cx);
    });
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let line = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| {
            let resting = shell.resting.as_ref().unwrap();
            (resting.tier_chip.get(), resting.model_chip.get())
        })
    };
    let chips = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let bar = shell.read(cx).resting.as_ref().unwrap().bar.read(cx);
            (bar.added().iter())
                .map(|(url, title)| format!("{title} {url}"))
                .collect::<Vec<_>>()
        })
    };
    let before = line(&shell, cx);
    assert!(before.0.is_some(), "the level's chip wasn't drawn");
    shell.update(cx, |shell, cx| shell.toggle_page(1, cx));
    cx.run_until_parked();
    assert_eq!(
        chips(&shell, cx),
        ["The Lakeside loop https://fieldbook.example/lakeside"],
        "the page added isn't a chip in the field"
    );
    assert_eq!(line(&shell, cx), before, "Ask's line moved for the page");

    cx.simulate_input("plan");
    cx.simulate_keystrokes("backspace");
    assert_eq!(
        chips(&shell, cx).len(),
        1,
        "⌫ after typing took the page off"
    );
    cx.simulate_keystrokes("backspace backspace backspace backspace");
    cx.run_until_parked();
    assert!(chips(&shell, cx).is_empty(), "⌫ at the start left the page");
    let pages = shell.read_with(cx, |shell, _| shell.resting.as_ref().unwrap().pages.len());
    assert_eq!(
        pages, 0,
        "the chip went but the task would still have the page"
    );

    shell.update(cx, |shell, cx| shell.toggle_page(1, cx));
    cx.simulate_input("plan a walk");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(chips(&shell, cx).is_empty(), "the run left the page's chip");
}

/// Where files and folders are available, the `+` menu is Add Page, Add Files… and Add Folder…;
/// the files attached and the folder granted are chips in the field after the pages, ⌫ takes the
/// last off, and the start gets them, which the record counts and never names. A folder ilo never
/// gives is said under Ask's line, and nothing is granted.
#[gpui::test]
fn files_and_a_folder_are_chips_in_the_field_and_go_with_the_start(cx: &mut TestAppContext) {
    let (shell, cx, host) = window_with_no_tab(cx);
    cx.update(|_, cx| crate::text_input::bind_keys(cx));
    host.files.set(true);
    // Not the system's temporary folder, which is under /private on the Mac, where ilo gives none.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("target/ilo-composer-files-{}", std::process::id()));
    let work = dir.join("Lakeside walk");
    std::fs::create_dir_all(&work).unwrap();
    let (map, notes) = (work.join("map.pdf"), work.join("notes.txt"));
    std::fs::write(&map, "map").unwrap();
    std::fs::write(&notes, "notes").unwrap();

    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, cx| shell.plus_offers(cx)),
        "with no page to add, the + still offers files and folders"
    );
    shell.update(cx, |shell, cx| {
        shell.composer_plus_menu(point(px(0.), px(0.)), cx)
    });
    let rows: Vec<(String, bool)> = (host.menus.borrow().last().unwrap().iter())
        .map(|(title, enabled)| (title.to_string(), *enabled))
        .collect();
    assert_eq!(
        rows,
        [
            ("Add Page".to_string(), false),
            ("Add Files…".to_string(), true),
            ("Add Folder…".to_string(), true),
        ],
        "the + menu's rows, Add Page off with no page to add"
    );

    let given = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let bar = shell.read(cx).resting.as_ref().unwrap().bar.read(cx);
            (bar.given().iter())
                .map(|given| match given {
                    Given::Files(count) => format!("Files {count}"),
                    Given::Folder(name) => format!("Folder {name}"),
                })
                .collect::<Vec<_>>()
        })
    };
    shell.update(cx, |shell, cx| {
        shell.attach_files(vec![map.clone(), notes.clone(), map.clone()], cx);
        shell.grant_folder(work.clone(), cx);
    });
    cx.run_until_parked();
    assert_eq!(
        given(&shell, cx),
        ["Files 2 files", "Folder Lakeside walk"],
        "each file once, by count, then the folder by its name"
    );

    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(
        given(&shell, cx),
        ["Files 2 files"],
        "⌫ at the start took the folder off first"
    );
    cx.simulate_keystrokes("backspace");
    cx.run_until_parked();
    assert_eq!(given(&shell, cx), ["Files 1 file"], "then a file");

    shell.update(cx, |shell, cx| shell.grant_folder("/".into(), cx));
    let (folder, refused) = shell.read_with(cx, |shell, _| {
        let resting = shell.resting.as_ref().unwrap();
        (resting.folder.is_some(), resting.refused.clone())
    });
    assert!(!folder, "a folder ilo never gives was granted");
    assert!(
        refused.is_some_and(|line| line.starts_with("ilo doesn't give")),
        "the refusal isn't said under the line"
    );

    shell.update(cx, |shell, cx| shell.grant_folder(work.clone(), cx));
    host.take();
    cx.simulate_input("plan a walk");
    cx.simulate_keystrokes("enter");
    let calls = host.take();
    let started = "start task claude plan a walk from 1 at Work on pages and 1 files and a folder";
    assert!(
        calls.contains(&started.to_string()),
        "the files and the folder didn't reach the task: {calls:?}"
    );
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("Lakeside") || call.contains("map.pdf")),
        "a name reached a record: {calls:?}"
    );
    assert!(given(&shell, cx).is_empty(), "the run left the chips");
    let refused = shell.read_with(cx, |shell, _| {
        shell.resting.as_ref().unwrap().refused.clone()
    });
    assert!(refused.is_none(), "the run left the refusal");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A private window starts no task, so its empty page offers none: Search alone, with no switch,
/// no ⇥ key and no Ask with its line. A window of the profile's offers them.
#[gpui::test]
fn a_private_window_composes_no_task(cx: &mut TestAppContext) {
    let (shell, cx, _) = window_with_no_tab(cx);
    cx.simulate_click(middle(&shell, cx), Modifiers::none());
    cx.run_until_parked();
    let shown = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let shell = shell.read(cx);
            let resting = shell.resting.as_ref().unwrap();
            (shell.offers_tasks(), resting.bar.read(cx).on_ask())
        })
    };
    assert_eq!(shown(&shell, cx), (true, false), "a profile's window");

    shell.update(cx, |shell, _| shell.private = true);
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert_eq!(
        shown(&shell, cx),
        (false, false),
        "⇥ went to Ask in a private window"
    );
    shell.update_in(cx, |shell, window, cx| shell.set_on_ask(true, window, cx));
    cx.run_until_parked();
    let drawn = shell.read_with(cx, |shell, _| {
        let resting = shell.resting.as_ref().unwrap();
        resting.tier_chip.get().is_some() || resting.plus_chip.get().is_some()
    });
    assert_eq!(
        shown(&shell, cx),
        (false, false),
        "Ask's side opened in a private window"
    );
    assert!(!drawn, "Ask's line was drawn in a private window");

    shell.update(cx, |shell, _| shell.private = false);
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    assert_eq!(
        shown(&shell, cx),
        (true, true),
        "⇥ didn't go to Ask in a profile's window"
    );
}

/// A chip of what the next task is given says its kind alone wherever it's logged: never the
/// folder's name, nor the count of files (#3656).
#[test]
fn a_given_chip_is_logged_by_its_kind_alone() {
    let folder = Given::Folder("Lakeside walk".into());
    let files = Given::Files("2 files".into());
    assert_eq!(format!("{folder:?} {files:?}"), "Folder Files");
}
