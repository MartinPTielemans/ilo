use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    App, Bounds, KeyDownEvent, Keystroke, Modifiers, Pixels, Point, TestAppContext,
    VisualTestContext, point, px, size,
};

use super::{
    CHECK, Closed, Closing, DETAIL_GAP, GAP, Key, Layout, Look, Outcome, PADDING, Pointer,
    QUIET_RADIUS, RADIUS, ROW_X, Request, SUBMENU_REST, Swatch, Tracking, as_held, described,
    described_held, key_of, lay_out, lay_out_over, narrowed,
};
use gpui::Focusable as _;

use crate::shell::MenuEntry;

/// Seven points a character, near enough to SF's 13 pt for where things go.
fn width_of(text: &str) -> f32 {
    7. * text.chars().count() as f32
}

fn area() -> Bounds<Pixels> {
    Bounds::new(point(px(0.), px(0.)), size(px(1280.), px(800.)))
}

fn at(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}

fn item(title: &str) -> MenuEntry {
    MenuEntry::named(title.to_string(), true)
}

fn under(parent: usize, title: &str) -> MenuEntry {
    MenuEntry {
        parent: Some(parent),
        ..item(title)
    }
}

/// A tab's menu as the sidebar has it, with a dimmed item, lines and a submenu.
fn tab_menu() -> Vec<MenuEntry> {
    vec![
        item("Copy Link"),             // 0
        MenuEntry::line(),             // 1
        item("Rename…"),               // 2
        MenuEntry::new("Mute", false), // 3
        item("Duplicate"),             // 4
        item("Move to"),               // 5
        MenuEntry {
            checked: true,
            ..under(5, "Space 1")
        }, // 6
        under(5, "reading list"),      // 7
        under(5, "New Space…"),        // 8
        MenuEntry::line(),             // 9
        item("Archive Tab"),           // 10
        item("Archive Tabs Below"),    // 11
    ]
}

fn opened(entries: Vec<MenuEntry>, x: f32, y: f32) -> Tracking {
    let layout = lay_out(&entries, at(x, y), area(), &width_of);
    Tracking::new(entries, layout, at(x, y))
}

/// The middle of the row of `entry` in the open level holding it.
fn row_of(tracking: &Tracking, entry: usize) -> Point<Pixels> {
    let level = (tracking.layout.levels.iter())
        .find(|level| level.items.contains(&entry))
        .unwrap();
    let row = level.items.iter().position(|&each| each == entry).unwrap();
    level.row_bounds(row, &tracking.entries).center()
}

fn level_of(layout: &Layout, parent: Option<usize>) -> Bounds<Pixels> {
    layout
        .levels
        .iter()
        .find(|level| level.parent == parent)
        .unwrap()
        .bounds
}

#[test]
fn the_arrows_walk_the_items_skipping_lines_and_dimmed_ones_and_stop_at_the_ends() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.key(Key::Down, now);
    assert_eq!(menu.highlighted(), ["Copy Link"]);
    menu.key(Key::Down, now);
    assert_eq!(menu.highlighted(), ["Rename…"]);
    menu.key(Key::Down, now);
    assert_eq!(menu.highlighted(), ["Duplicate"], "past the dimmed Mute");
    menu.key(Key::Last, now);
    menu.key(Key::Down, now);
    assert_eq!(
        menu.highlighted(),
        ["Archive Tabs Below"],
        "it stays at the end, as NSMenu's does"
    );
    menu.key(Key::First, now);
    menu.key(Key::Up, now);
    assert_eq!(menu.highlighted(), ["Copy Link"]);

    let mut menu = opened(tab_menu(), 100., 100.);
    menu.key(Key::Up, now);
    assert_eq!(
        menu.highlighted(),
        ["Archive Tabs Below"],
        "up from nothing starts at the bottom"
    );
}

#[test]
fn return_picks_the_highlighted_item_and_escape_closes() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    assert_eq!(
        menu.key(Key::Choose, now),
        Outcome::Stay,
        "nothing is highlighted yet"
    );
    menu.key(Key::Down, now);
    assert_eq!(menu.key(Key::Choose, now), Outcome::Pick(0));
    assert_eq!(menu.key(Key::Escape, now), Outcome::Escape);
}

#[test]
fn right_and_return_open_a_submenu_and_left_and_escape_close_only_it() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.key(Key::Char('m'), now);
    assert_eq!(menu.highlighted(), ["Move to"]);
    menu.key(Key::Right, now);
    assert_eq!(
        menu.highlighted(),
        ["Move to", "Space 1"],
        "with its first item highlighted"
    );
    menu.key(Key::Down, now);
    assert_eq!(menu.highlighted(), ["Move to", "reading list"]);
    menu.key(Key::Left, now);
    assert_eq!(menu.highlighted(), ["Move to"]);
    assert_eq!(
        menu.key(Key::Choose, now),
        Outcome::Stay,
        "Return on it opens the submenu"
    );
    assert_eq!(menu.highlighted(), ["Move to", "Space 1"]);
    assert_eq!(menu.key(Key::Escape, now), Outcome::Stay);
    assert_eq!(menu.highlighted(), ["Move to"]);
    menu.key(Key::Right, now);
    menu.key(Key::Down, now);
    assert_eq!(menu.key(Key::Choose, now), Outcome::Pick(7));
}

#[test]
fn typing_selects_by_the_titles_start_and_starts_over_after_a_second() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.key(Key::Char('A'), now);
    assert_eq!(menu.highlighted(), ["Archive Tab"], "case doesn't matter");
    menu.key(Key::Char('r'), now + Duration::from_millis(300));
    menu.key(Key::Char('c'), now + Duration::from_millis(600));
    menu.key(Key::Char('h'), now + Duration::from_millis(900));
    menu.key(Key::Char('i'), now + Duration::from_millis(1200));
    menu.key(Key::Char('v'), now + Duration::from_millis(1500));
    menu.key(Key::Char('e'), now + Duration::from_millis(1800));
    menu.key(Key::Char(' '), now + Duration::from_millis(2100));
    menu.key(Key::Char('t'), now + Duration::from_millis(2400));
    menu.key(Key::Char('a'), now + Duration::from_millis(2700));
    menu.key(Key::Char('b'), now + Duration::from_millis(3000));
    menu.key(Key::Char('s'), now + Duration::from_millis(3300));
    assert_eq!(menu.highlighted(), ["Archive Tabs Below"]);
    menu.key(Key::Char('d'), now + Duration::from_millis(4400));
    assert_eq!(
        menu.highlighted(),
        ["Duplicate"],
        "after a second, a key starts a new search"
    );
    menu.key(Key::Char('x'), now + Duration::from_millis(4500));
    assert_eq!(
        menu.highlighted(),
        ["Duplicate"],
        "what matches nothing moves nothing"
    );
}

#[test]
fn the_same_letter_again_goes_to_the_next_item_it_starts() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.key(Key::Char('a'), now);
    menu.key(Key::Char('a'), now + Duration::from_millis(200));
    assert_eq!(menu.highlighted(), ["Archive Tabs Below"]);
    menu.key(Key::Char('a'), now + Duration::from_millis(400));
    assert_eq!(
        menu.highlighted(),
        ["Archive Tab"],
        "round to the first again"
    );
}

#[test]
fn typing_ignores_accents_and_finds_only_what_can_be_chosen() {
    let now = Instant::now();
    let entries = vec![item("Éditer"), MenuEntry::new("Mute", false), item("Move")];
    let mut menu = opened(entries, 100., 100.);
    menu.key(Key::Char('e'), now);
    assert_eq!(menu.highlighted(), ["Éditer"]);
    menu.key(Key::Char('m'), now + Duration::from_secs(2));
    assert_eq!(menu.highlighted(), ["Move"], "past the dimmed Mute");
}

#[test]
fn typing_in_an_open_submenu_selects_there() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.key(Key::Char('m'), now);
    menu.key(Key::Right, now);
    menu.key(Key::Char('n'), now + Duration::from_secs(2));
    assert_eq!(menu.highlighted(), ["Move to", "New Space…"]);
}

#[test]
fn a_key_makes_a_character_and_the_arrows_their_moves() {
    let key = |key: &str, key_char: Option<&str>, alt: bool| {
        let keystroke = Keystroke {
            modifiers: Modifiers {
                alt,
                ..Modifiers::default()
            },
            key: key.into(),
            key_char: key_char.map(Into::into),
        };
        key_of(&KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        })
    };
    assert_eq!(key("down", None, false), Some(Key::Down));
    assert_eq!(
        key("up", None, true),
        Some(Key::First),
        "⌥↑ goes to the top, as in NSMenu"
    );
    assert_eq!(key("enter", None, false), Some(Key::Choose));
    assert_eq!(key("escape", None, false), Some(Key::Escape));
    assert_eq!(key("r", Some("r"), false), Some(Key::Char('r')));
    assert_eq!(
        key("a", Some("あ"), false),
        Some(Key::Char('あ')),
        "kana selects as itself"
    );
    assert_eq!(key("tab", None, false), None);
}

#[test]
fn a_menu_opens_at_the_pointer_and_flips_where_the_screen_ends() {
    let menu = lay_out(&tab_menu(), at(100., 100.), area(), &width_of);
    let root = level_of(&menu, None);
    assert_eq!(root.origin, at(100., 100.));
    assert_eq!(
        f32::from(root.size.width),
        200.,
        "no narrower than the least"
    );
    // Six rows and two lines, inside the padding.
    assert_eq!(
        f32::from(root.size.height),
        PADDING + 7. * 24. + 2. * 9. + PADDING
    );

    let menu = lay_out(&tab_menu(), at(1200., 780.), area(), &width_of);
    let root = level_of(&menu, None);
    assert_eq!(
        root.bottom_right(),
        at(1200., 780.),
        "up and to the left of the pointer"
    );

    // A menu taller than the room above and below sits inside the screen, 8 from its edge.
    let short = Bounds::new(point(px(0.), px(0.)), size(px(1280.), px(150.)));
    let menu = lay_out(&tab_menu(), at(100., 60.), short, &width_of);
    assert_eq!(level_of(&menu, None).origin.y, px(8.));
}

#[test]
fn the_menus_window_sits_on_whole_points_round_its_shadow() {
    for at in [at(100., 100.), at(1000., 100.), at(100.5, 700.25)] {
        let menu = lay_out(&tab_menu(), at, area(), &width_of);
        let (origin, end) = (menu.frame.origin, menu.frame.bottom_right());
        for value in [origin.x, origin.y, end.x, end.y] {
            assert_eq!(
                value,
                value.round(),
                "{:?} isn't on whole points",
                menu.frame
            );
        }
        let union = menu
            .levels
            .iter()
            .fold(menu.levels[0].bounds, |all, level| all.union(&level.bounds));
        let reach = crate::elevation::menu_reach();
        assert!(
            origin.x <= union.left() - px(reach.sides) && origin.y <= union.top() - px(reach.above)
        );
        assert!(
            end.x >= union.right() + px(reach.sides) && end.y >= union.bottom() + px(reach.below)
        );
    }
}

/// Where the window can't blend, a pixel a panel's edge partly covers would show black, and so
/// would a shadow's room: each panel goes out to whole points, and the window is just round them.
#[test]
fn where_the_window_cant_blend_the_panels_sit_on_whole_points_with_nothing_round_them() {
    for at in [at(100., 100.), at(1000., 100.), at(100.5, 700.25)] {
        let menu = lay_out(&tab_menu(), at, area(), &width_of);
        let opaque = menu.clone().opaque();
        for (level, was) in opaque.levels.iter().zip(&menu.levels) {
            let (bounds, was) = (level.bounds, was.bounds);
            for value in [bounds.left(), bounds.top(), bounds.right(), bounds.bottom()] {
                assert_eq!(value, value.round(), "{bounds:?} isn't on whole points");
            }
            let out = [
                was.left() - bounds.left(),
                was.top() - bounds.top(),
                bounds.right() - was.right(),
                bounds.bottom() - was.bottom(),
            ];
            assert!(
                out.iter().all(|&by| by >= px(0.) && by < px(1.)),
                "{bounds:?} from {was:?}"
            );
        }
        let levels = &opaque.levels;
        let union = (levels.iter()).fold(levels[0].bounds, |all, level| all.union(&level.bounds));
        assert_eq!(opaque.frame, union, "the window is just round the panels");
    }
}

#[test]
fn a_submenu_opens_beside_its_row_and_on_the_left_where_the_right_has_no_room() {
    let menu = lay_out(&tab_menu(), at(100., 100.), area(), &width_of);
    let root = level_of(&menu, None);
    let sub = level_of(&menu, Some(5));
    assert_eq!(
        sub.left(),
        root.right() - px(4.),
        "over its parent's edge by 4"
    );
    // Its first row is level with the row that opened it.
    let row_top = 100. + PADDING + 4. * 24. + 9.;
    assert_eq!(sub.top(), px(row_top - PADDING));

    let menu = lay_out(&tab_menu(), at(1000., 100.), area(), &width_of);
    let root = level_of(&menu, None);
    let sub = level_of(&menu, Some(5));
    assert_eq!(root.left(), px(1000.), "the root fits");
    assert_eq!(
        sub.right(),
        root.left() + px(4.),
        "the submenu goes on the left"
    );

    let menu = lay_out(&tab_menu(), at(100., 700.), area(), &width_of);
    assert!(
        level_of(&menu, Some(5)).bottom() <= px(792.),
        "and slides up to fit"
    );
}

#[test]
fn a_long_title_widens_the_menu_to_the_most() {
    let title = "A very long page title that goes on and on and on, as page titles do";
    let menu = lay_out(&[item(title)], at(10., 10.), area(), &width_of);
    assert_eq!(f32::from(level_of(&menu, None).size.width), 320.);
}

#[test]
fn an_items_detail_widens_the_menu_and_is_described_after_its_title() {
    // Wide enough for both, past the narrowest a menu is.
    let title = "A title of thirty characters..";
    let plain = lay_out(&[item(title)], at(10., 10.), area(), &width_of);
    let detailed = [MenuEntry {
        detail: Some("A detail".into()),
        ..item(title)
    }];
    let wider = lay_out(&detailed, at(10., 10.), area(), &width_of);
    let width = |menu| f32::from(level_of(menu, None).size.width);
    assert_eq!(
        width(&wider) - width(&plain),
        DETAIL_GAP + width_of("A detail")
    );
    assert_eq!(described(&detailed), format!("{title} [A detail]"));
}

#[test]
fn a_release_where_the_menu_opened_leaves_it_open_and_a_drag_to_an_item_picks_it() {
    let mut menu = opened(tab_menu(), 100., 100.);
    assert_eq!(
        menu.released(at(102., 101.)),
        Outcome::Stay,
        "a right-click, not a drag"
    );
    let rename = row_of(&menu, 2);
    menu.moved(rename, Instant::now());
    assert_eq!(
        menu.released(rename),
        Outcome::Stay,
        "the button is up: only a click picks now"
    );
    assert!(menu.pressed(rename));
    assert_eq!(menu.released(rename), Outcome::Pick(2));

    let mut menu = opened(tab_menu(), 100., 100.);
    let duplicate = row_of(&menu, 4);
    menu.moved(duplicate, Instant::now());
    assert_eq!(
        menu.released(duplicate),
        Outcome::Pick(4),
        "press, drag and release"
    );
}

#[test]
fn a_release_on_what_cant_be_chosen_leaves_the_menu_open() {
    let mut menu = opened(tab_menu(), 100., 100.);
    assert_eq!(
        menu.released(row_of(&menu, 3)),
        Outcome::Stay,
        "the dimmed Mute"
    );
    let mut menu = opened(tab_menu(), 100., 100.);
    assert_eq!(menu.released(row_of(&menu, 1)), Outcome::Stay, "a line");
    let mut menu = opened(tab_menu(), 100., 100.);
    let move_to = row_of(&menu, 5);
    menu.moved(move_to, Instant::now());
    assert_eq!(menu.released(move_to), Outcome::Stay, "a submenu's row");
    let space = row_of(&menu, 7);
    menu.moved(space, Instant::now());
    assert!(menu.pressed(space));
    assert_eq!(menu.released(space), Outcome::Pick(7), "in the submenu");
}

/// The log tells apart each way a menu closes, the ones its answer calls `Otherwise` too, and says
/// which menu by where it opened, naming none of its items, which can be a page's title (#2873).
#[test]
fn the_log_tells_how_a_menu_closed_and_names_none_of_its_items() {
    let all = [
        Closing::Chose(0),
        Closing::PressedOutside,
        Closing::Left,
        Closing::Escape,
        Closing::Shortcut,
        Closing::ReleasedOutside,
        Closing::Replaced,
    ];
    let lines: Vec<String> = all
        .iter()
        .map(|closing| closing.line(at(100., 210.)))
        .collect();
    let told: std::collections::HashSet<&String> = lines.iter().collect();
    assert_eq!(
        told.len(),
        all.len(),
        "two ways of closing read the same: {lines:?}"
    );
    for line in &lines {
        assert!(
            line.contains(" 100 210 "),
            "the line doesn't say where the menu opened: {line}"
        );
        let named = (tab_menu().into_iter())
            .map(|entry| entry.title)
            .find(|title| !title.is_empty() && line.contains(title.as_ref() as &str));
        assert_eq!(named, None, "the line names an item: {line}");
    }
    let answers: Vec<Closed> = all.iter().map(|closing| closing.closed()).collect();
    assert_eq!(answers[0], Closed::Chose(0));
    assert_eq!(answers[2], Closed::Left);
    for way in [1, 3, 4, 5, 6] {
        assert_eq!(answers[way], Closed::Otherwise, "{:?}", all[way]);
    }
}

#[test]
fn a_drag_released_outside_closes_the_menu_and_a_press_outside_is_not_on_it() {
    let mut menu = opened(tab_menu(), 100., 100.);
    assert_eq!(menu.released(at(60., 60.)), Outcome::ReleasedOutside);
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.released(at(100., 100.));
    assert!(!menu.pressed(at(600., 600.)));
}

#[test]
fn the_pointer_opens_a_submenu_and_its_row_keeps_it_open() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.moved(row_of(&menu, 5), now);
    assert_eq!(menu.highlights(), [Some(5), None]);
    menu.moved(row_of(&menu, 5) + point(px(20.), px(0.)), now);
    assert_eq!(menu.highlights(), [Some(5), None], "anywhere on the row");
    menu.moved(row_of(&menu, 7), now);
    assert_eq!(menu.highlights(), [Some(5), Some(7)]);
}

#[test]
fn on_its_way_to_the_submenu_the_pointer_crosses_other_rows_without_closing_it() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    let parent = row_of(&menu, 5);
    menu.moved(parent, now);
    let sub = level_of(&menu.layout, Some(5));
    // Down and right, over Archive Tab's row, towards the submenu's lower half.
    let below = row_of(&menu, 10);
    let crossing = point(sub.left() - px(10.), below.y);
    assert!(
        menu.moved(crossing, now + Duration::from_millis(30)),
        "in the safe triangle"
    );
    assert_eq!(
        menu.highlights(),
        [Some(5), None],
        "Move to stays highlighted, its submenu open"
    );
    menu.moved(crossing, now + Duration::from_millis(31));
    assert_eq!(
        menu.highlights(),
        [Some(5), None],
        "the same move again isn't a rest's end"
    );

    // Measured from the move, as a timer can fire a little early (#2158).
    let rested = now + Duration::from_millis(30) + Duration::from_secs_f32(SUBMENU_REST);
    assert_eq!(
        menu.rested(rested - Duration::from_millis(10)),
        Some(Duration::from_millis(10)),
        "10 ms more to rest"
    );
    assert_eq!(menu.highlights(), [Some(5), None], "not yet");
    assert_eq!(menu.rested(rested), None);
    assert_eq!(
        menu.highlights(),
        [Some(10)],
        "resting 300 ms gives the row under it the highlight"
    );
}

#[test]
fn moving_away_from_the_submenu_closes_it_at_once() {
    let now = Instant::now();
    let mut menu = opened(tab_menu(), 100., 100.);
    menu.moved(row_of(&menu, 5), now);
    // Straight down along the left edge, away from the submenu on the right.
    let away = point(px(110.), row_of(&menu, 10).y);
    assert!(!menu.moved(away, now + Duration::from_millis(30)));
    assert_eq!(menu.highlights(), [Some(10)]);
}

#[test]
fn the_driver_chooses_an_item_by_its_title_or_its_submenus() {
    let menu = opened(tab_menu(), 100., 100.);
    assert_eq!(menu.find("Duplicate"), Some(4));
    assert_eq!(menu.find("Move to › reading list"), Some(7));
    assert_eq!(menu.find("reading list"), Some(7));
    assert_eq!(menu.find("Mute"), None, "dimmed");
    assert_eq!(menu.find("Move to"), None, "a submenu's row");
}

#[test]
fn the_log_names_the_items_as_the_systems_menus_did() {
    assert_eq!(
        described(&tab_menu()),
        "Copy Link, Rename…, Mute (disabled), Duplicate, Move to › Space 1 (checked), \
         Move to › reading list, Move to › New Space…, Archive Tab, Archive Tabs Below"
    );
}

/// What the menu asked of its window, in order.
#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<&'static str>>,
    /// Each window the menu asked for, and its panels in it.
    shown: RefCell<Vec<(Bounds<Pixels>, Vec<Bounds<Pixels>>)>>,
    /// The panels the host was asked to put its material under each time, and whether light.
    glass: RefCell<Vec<(Vec<(Bounds<Pixels>, f32)>, bool)>>,
    /// Whether its window can't blend (`Host::blends`).
    opaque: bool,
}

impl super::Host for Recorder {
    fn show(&self, frame: Bounds<Pixels>, panels: &[Bounds<Pixels>]) {
        self.calls.borrow_mut().push("show");
        self.shown.borrow_mut().push((frame, panels.to_vec()));
    }

    fn glass(&self, panels: &[(Bounds<Pixels>, f32)], light: bool) {
        self.glass.borrow_mut().push((panels.to_vec(), light));
    }

    fn hide(&self) {
        self.calls.borrow_mut().push("hide");
    }

    fn blends(&self) -> bool {
        !self.opaque
    }
}

/// Shows the tab menu at (100, 100) and returns how it closed once it does. Each call is an update
/// of the app, as the host's calls are, not of the menu's window.
fn show_menu(cx: &mut VisualTestContext) -> Rc<Cell<Option<Closed>>> {
    let chosen = Rc::new(Cell::new(None));
    let answer = chosen.clone();
    cx.cx.update(|cx: &mut App| {
        let request = Request {
            items: tab_menu(),
            at: at(100., 100.),
            area: area(),
            look: Look::dark(),
            chosen: Box::new(move |closed, _| answer.set(Some(closed))),
        };
        super::show(request, cx);
    });
    cx.run_until_parked();
    chosen
}

/// The menu's window takes the pointer on its panels, the submenu's too before it opens, and lets
/// it through the room round them that their shadows fade out in, so a click just outside a panel
/// lands on the page under it.
#[gpui::test]
fn the_menu_takes_the_pointer_on_its_panels_and_not_in_their_shadows(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    show_menu(cx);
    let shown = host.shown.borrow();
    let (frame, panels) = &shown[0];
    assert_eq!(
        panels.len(),
        2,
        "the tab menu and its Move to submenu: {panels:?}"
    );
    let reach = crate::elevation::menu_reach();
    let window = Bounds::new(point(px(0.), px(0.)), frame.size);
    for panel in panels {
        assert!(
            window.contains(&panel.origin) && window.contains(&panel.bottom_right()),
            "{panel:?}"
        );
    }
    // The root's top left is where the menu opened, the shadow's room from the window's edges.
    assert_eq!(frame.origin + panels[0].origin, at(100., 100.));
    assert!(
        f32::from(panels[0].origin.x) >= reach.sides
            && f32::from(panels[0].origin.y) >= reach.above
    );
}

/// A window that can't blend, on X11 with no compositor, shows its clear pixels black (#1479), so
/// it holds the open panels alone, with no room round them, and is shown again with each submenu
/// that opens or closes.
#[gpui::test]
fn where_the_window_cant_blend_it_holds_the_open_panels_alone(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder {
        opaque: true,
        ..Recorder::default()
    });
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    show_menu(cx);
    let (frame, panels) = host.shown.borrow()[0].clone();
    assert_eq!(
        panels,
        [Bounds::new(at(0., 0.), panels[0].size)],
        "the tab menu alone, at the corner"
    );
    assert_eq!(
        frame.origin,
        at(100., 100.),
        "where the menu opened, with no room for a shadow"
    );

    cx.simulate_keystrokes("m right");
    let (again, panels) = host.shown.borrow().last().unwrap().clone();
    assert_eq!(again, frame, "the window stays");
    assert_eq!(
        panels.len(),
        2,
        "and holds Move to's submenu too: {panels:?}"
    );
    assert_eq!(
        panels[0].union(&panels[1]),
        Bounds::new(at(0., 0.), frame.size),
        "just round the two"
    );

    cx.simulate_keystrokes("escape");
    assert_eq!(
        host.shown.borrow().last().unwrap().1.len(),
        1,
        "Move to's submenu closed"
    );
    cx.simulate_keystrokes("escape");
    assert_eq!(*host.calls.borrow(), ["show", "show", "show", "hide"]);
}

/// The host's material goes under the panels that show, each with its own corner, and follows a
/// submenu as it opens and closes (#4090), so no glass shows in the shadows' room round them or
/// where a closed submenu was.
#[gpui::test]
fn the_material_goes_under_the_open_panels_alone(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    show_menu(cx);
    let panels = host.shown.borrow()[0].1.clone();
    let last = || {
        host.glass
            .borrow()
            .last()
            .cloned()
            .expect("no glass asked for")
    };
    assert_eq!(
        last(),
        (vec![(panels[0], RADIUS)], false),
        "the tab menu's panel alone, in its dark look, though the window holds Move to's too"
    );

    cx.simulate_keystrokes("m right");
    assert_eq!(
        last().0,
        [(panels[0], RADIUS), (panels[1], QUIET_RADIUS)],
        "and Move to's as it opens, with a list of choices' quieter corner"
    );

    cx.simulate_keystrokes("escape");
    assert_eq!(last().0, [(panels[0], RADIUS)], "Move to's closed");
}

/// Where the window can't blend, the panels are square, and so is what the host puts under them.
#[gpui::test]
fn where_the_window_cant_blend_the_material_is_square(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder {
        opaque: true,
        ..Recorder::default()
    });
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    show_menu(cx);
    cx.simulate_keystrokes("m right");
    let (glass, _) = host
        .glass
        .borrow()
        .last()
        .cloned()
        .expect("no glass asked for");
    let panels = host.shown.borrow().last().unwrap().1.clone();
    assert_eq!(glass, [(panels[0], 0.), (panels[1], 0.)]);
}

/// A scroll is the menu's while it shows, over it or outside it: the host hears it's taken, so
/// nothing under the pointer scrolls, and the menu stays open with the keys.
#[gpui::test]
fn a_scroll_leaves_the_menu_open_and_goes_no_further(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let chosen = show_menu(cx);
    let menu_at = host.shown.borrow().last().expect("no menu shown").0.origin;
    let over = point(px(150.), px(100. + PADDING + 12.)) - menu_at;
    let outside = point(px(60.), px(40.)) - menu_at;
    for (place, at) in [("over the menu", over), ("outside it", outside)] {
        assert!(
            cx.cx
                .update(|cx: &mut App| super::pointer(Pointer::Scrolled, at, cx)),
            "a scroll {place} went on to what's under it"
        );
        cx.run_until_parked();
        assert_eq!(chosen.get(), None, "a scroll {place} closed the menu");
    }
    assert_eq!(*host.calls.borrow(), ["show"]);
    cx.simulate_keystrokes("escape");
    assert_eq!(chosen.get(), Some(Closed::Otherwise));
}

/// The menu in its window, driven the ways the host drives it: keys through the window, pointer
/// events and the driver's choose and dismiss. Each closes it from inside the menu's update, which
/// must leave its window alone until the update is over, or drawing it finds the menu held.
#[gpui::test]
fn the_host_drives_the_menu_and_the_menu_closes_outside_its_update(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);

    let chosen = show_menu(cx);
    cx.simulate_keystrokes("m right down enter");
    assert_eq!(
        chosen.get(),
        Some(Closed::Chose(7)),
        "Move to › reading list"
    );
    assert_eq!(*host.calls.borrow(), ["show", "hide"]);

    // Press and release on Duplicate, in the window's terms, which start left of the menu and
    // above it by the room its shadow takes.
    let chosen = show_menu(cx);
    let reach = crate::elevation::menu_reach();
    let window_at = point(px(100. - reach.sides), px(100. - reach.above));
    let duplicate = point(px(150.), px(100. + PADDING + 81. + 12.)) - window_at;
    cx.cx.update(|cx: &mut App| {
        super::pointer(Pointer::Released, point(px(100.), px(100.)) - window_at, cx);
        assert!(super::pointer(Pointer::Moved, duplicate, cx));
        assert!(super::pointer(Pointer::Pressed, duplicate, cx));
        assert!(super::pointer(Pointer::Released, duplicate, cx));
    });
    cx.run_until_parked();
    assert_eq!(chosen.get(), Some(Closed::Chose(4)));

    let chosen = show_menu(cx);
    assert!(
        cx.cx
            .update(|cx: &mut App| super::choose("Archive Tab", cx))
    );
    cx.run_until_parked();
    assert_eq!(chosen.get(), Some(Closed::Chose(10)));

    let chosen = show_menu(cx);
    cx.cx.update(|cx: &mut App| super::dismissed(cx));
    cx.run_until_parked();
    assert_eq!(chosen.get(), Some(Closed::Left));

    // A press outside it, which it takes, was spent on the menu: it answers as the menu's keys do,
    // with nothing of where the press was, so whatever had the menu goes on as it was.
    let chosen = show_menu(cx);
    let page = point(px(60.), px(40.));
    let menu_at = host.shown.borrow().last().expect("no menu shown").0.origin;
    assert!(
        cx.cx
            .update(|cx: &mut App| super::pointer(Pointer::Pressed, page - menu_at, cx))
    );
    cx.run_until_parked();
    assert_eq!(chosen.get(), Some(Closed::Otherwise));

    // A menu opened over one that shows takes its place and stays.
    show_menu(cx);
    let chosen = show_menu(cx);
    assert_eq!(host.calls.borrow().last(), Some(&"show"));
    cx.simulate_keystrokes("escape");
    assert_eq!(chosen.get(), Some(Closed::Otherwise));
    assert_eq!(
        host.calls
            .borrow()
            .iter()
            .filter(|call| **call == "hide")
            .count(),
        7
    );
}

/// An item's ⌥ alternate takes no row of its own, as AppKit's: holding ⌥ shows it in the item's
/// place, where a click picks it, and once ⌥ is let go the item is back.
#[gpui::test]
fn holding_option_shows_and_picks_an_items_alternate(cx: &mut TestAppContext) {
    let markdown = MenuEntry {
        alternate: true,
        ..item("Copy Link as Markdown")
    };
    let entries = vec![item("Copy Link"), markdown, item("Rename…")];
    let layout = lay_out(&entries, at(100., 100.), area(), &width_of);
    assert_eq!(layout.levels[0].items, [0, 2]);
    assert_eq!(described(&entries), "Copy Link, Rename…");
    assert_eq!(
        described_held(&entries, true),
        "Copy Link as Markdown, Rename…"
    );
    assert_eq!(
        (as_held(&entries, 0, true), as_held(&entries, 0, false)),
        (1, 0)
    );
    assert_eq!(as_held(&entries, 2, true), 2);

    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    let reach = crate::elevation::menu_reach();
    let window_at = point(px(100. - reach.sides), px(100. - reach.above));
    let copy = point(px(150.), px(100. + PADDING + 12.)) - window_at;
    let click = |alt: Modifiers, cx: &mut VisualTestContext| {
        let chosen = Rc::new(Cell::new(None));
        let answer = chosen.clone();
        let items = entries.clone();
        cx.cx.update(|cx: &mut App| {
            let request = Request {
                items,
                at: at(100., 100.),
                area: area(),
                look: Look::dark(),
                chosen: Box::new(move |closed: Closed, _| answer.set(Some(closed.chosen()))),
            };
            super::show(request, cx);
        });
        cx.run_until_parked();
        cx.simulate_modifiers_change(alt);
        cx.cx.update(|cx: &mut App| {
            super::pointer(Pointer::Released, point(px(100.), px(100.)) - window_at, cx);
            super::pointer(Pointer::Moved, copy, cx);
            super::pointer(Pointer::Pressed, copy, cx);
            super::pointer(Pointer::Released, copy, cx);
        });
        cx.run_until_parked();
        chosen.get()
    };
    assert_eq!(click(Modifiers::alt(), cx), Some(Some(1)));
    assert_eq!(click(Modifiers::none(), cx), Some(Some(0)));
}

/// A pop-up button's menu, such as the Space form's Profile: the current choice checked and over
/// the button's title.
fn pop_up() -> Vec<MenuEntry> {
    let checked = |title| MenuEntry {
        checked: true,
        over: true,
        ..item(title)
    };
    vec![
        item("Default"),
        checked("work"),
        MenuEntry::line(),
        item("New Profile"),
    ]
}

/// The current choice's title goes on the button's, highlighted, and a click on the button, which
/// lands on it, leaves the menu open, as a pop-up button's does.
#[test]
fn a_pop_ups_menu_opens_with_its_choice_over_the_buttons_title() {
    let entries = pop_up();
    let button = at(400., 300.);
    let layout = lay_out_over(&entries, button, area(), &width_of);
    let root = &layout.levels[0];
    let row = root.row_bounds(1, &entries);
    assert_eq!(
        row.origin.x + px(ROW_X + CHECK + GAP),
        button.x,
        "the title's left on the button's"
    );
    // A menu of choices has quiet rows, whose titles sit lower in them than a command row's.
    let lower = (super::QUIET_ROW - super::ROW) / 2.;
    assert_eq!(
        row.origin.y + px(lower),
        button.y,
        "the row's title on the button's text"
    );
    assert_eq!(
        root.bounds.origin.y,
        button.y - px(PADDING + super::QUIET_ROW + lower),
        "Default above it"
    );

    let mut menu = Tracking::new(entries.clone(), layout, button);
    assert_eq!(menu.highlighted(), ["work"]);
    let release = button + point(px(10.), px(8.));
    assert_eq!(
        menu.released(release),
        Outcome::Stay,
        "a click on the button leaves it open"
    );
    menu.moved(row_of(&menu, 3), Instant::now());
    menu.pressed(row_of(&menu, 3));
    assert_eq!(menu.released(row_of(&menu, 3)), Outcome::Pick(3));

    // Without a choice over the button, it opens at the point as any menu does.
    let plain: Vec<MenuEntry> = entries
        .into_iter()
        .map(|entry| MenuEntry {
            over: false,
            ..entry
        })
        .collect();
    assert_eq!(
        lay_out_over(&plain, button, area(), &width_of),
        lay_out(&plain, button, area(), &width_of)
    );
}

/// A press-drag from a pop-up button onto another choice picks it, as AppKit's do: from where the
/// pointer first moved while the button was held, as the menu opens over the button's title
/// wherever the press was on it.
#[test]
fn a_drag_from_a_pop_up_button_picks_where_it_ends() {
    let entries = pop_up();
    let button = at(400., 300.);
    let layout = lay_out_over(&entries, button, area(), &width_of);
    let mut menu = Tracking::new(entries, layout, button);
    let press = button + point(px(150.), px(8.));
    menu.moved(press + point(px(1.), px(-1.)), Instant::now());
    let default = row_of(&menu, 0);
    menu.moved(default, Instant::now());
    assert_eq!(menu.released(default), Outcome::Pick(0));
}

/// The release of the click that opened a pop-up's menu picks nothing, wherever on the button it
/// was, under another choice or off the menu; then a press and release on a choice picks it.
#[test]
fn the_click_that_opens_a_pop_ups_menu_picks_nothing() {
    let entries = pop_up();
    let button = at(400., 300.);
    let opened = || {
        let layout = lay_out_over(&entries, button, area(), &width_of);
        Tracking::new(entries.clone(), layout, button)
    };
    let mut menu = opened();
    let default = row_of(&menu, 0);
    assert_eq!(
        menu.released(default),
        Outcome::Stay,
        "released over another choice"
    );
    menu.moved(default, Instant::now());
    assert!(menu.pressed(default));
    assert_eq!(menu.released(default), Outcome::Pick(0));
    let mut menu = opened();
    let chevrons = button + point(px(400.), px(8.));
    menu.moved(chevrons, Instant::now());
    assert_eq!(
        menu.released(chevrons),
        Outcome::Stay,
        "released off the menu, where it was pressed"
    );
}

/// A Space's swatch gets a ring only where it would fade into the panel (#320), by the palette's
/// rule for a theme's swatch: a dark one on the dark panel takes Paper, a pale one on the light
/// panel Ink, and one whose ends lie either side of the panel always takes one.
#[test]
fn a_swatch_is_ringed_only_where_its_edge_fades_into_the_panel() {
    use crate::palette::{INK, PAPER, Palette};
    use gpui::rgb;
    let ring = |from: u32, to: u32, fill: u32| {
        Palette::swatch_ring_of(rgb(from), rgb(to), &[rgb(fill >> 8)])
    };
    let (ink_like, vivid, pale) = (
        (0x1a1a1a, 0x303030),
        (0x4f94ff, 0xff7a3c),
        (0xfaf6ea, 0xffd9a0),
    );
    let (dark, light) = (super::DARK_FILL, super::LIGHT_FILL);
    assert_eq!(ring(vivid.0, vivid.1, dark), None);
    assert_eq!(ring(ink_like.0, ink_like.1, dark), Some(rgb(PAPER)));
    assert_eq!(ring(pale.0, pale.1, light), Some(rgb(INK)));
    assert_eq!(ring(ink_like.0, ink_like.1, light), None);
    // Black to white reads at each end on a mid grey, but passes through it.
    assert!(ring(0x000000, 0xffffff, 0x777777ff).is_some());
}

/// Swatches take room before the titles, so a submenu with them is wider by one and a gap.
#[test]
fn swatches_widen_their_submenu() {
    let mut entries = tab_menu();
    // Wider than the narrowest a panel is.
    entries[7].title = "a reading list with a long name".into();
    let plain = level_of(
        &lay_out(&entries, at(100., 100.), area(), &width_of),
        Some(5),
    );
    for entry in &mut entries[6..8] {
        entry.swatch = Some(Swatch {
            from: 0x4f94ff,
            to: 0xff7a3c,
        });
    }
    let with = level_of(
        &lay_out(&entries, at(100., 100.), area(), &width_of),
        Some(5),
    );
    assert_eq!(with.size.width - plain.size.width, px(super::SWATCH + GAP));
}

/// What VoiceOver reads of the tab menu: a menu with the keyboard, its items in order with where
/// each stands, the lines between them, the dimmed one flagged, and the submenu's row saying it
/// opens one. The highlight is what VoiceOver reads as the keys move it, into the submenu too.
#[gpui::test]
fn voiceover_reads_the_menu_and_follows_the_keys(cx: &mut TestAppContext) {
    use gpui::accesskit::{HasPopup, NodeId, Role, Toggled, TreeUpdate};
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = &mut VisualTestContext::from_window(window.into(), cx);
    cx.simulate_a11y_activation();
    show_menu(cx);
    let named = |tree: &TreeUpdate, role: Role, label: &str| -> NodeId {
        let found = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == role && node.label() == Some(label));
        found
            .unwrap_or_else(|| panic!("no {role:?} called {label}"))
            .0
    };
    let node = |tree: &TreeUpdate, id: NodeId| {
        tree.nodes
            .iter()
            .find(|(each, _)| *each == id)
            .unwrap()
            .1
            .clone()
    };
    let focused = |tree: &TreeUpdate| node(tree, tree.focus).label().map(str::to_owned);

    let tree = cx.a11y_tree().expect("GPUI built a tree");
    let menu = named(&tree, Role::Menu, "Menu");
    assert_eq!(
        tree.focus, menu,
        "the menu has the keyboard, nothing highlighted yet"
    );
    let rows: Vec<_> = (node(&tree, menu).children().iter())
        .map(|&child| node(&tree, child))
        .map(|row| (row.role(), row.label().unwrap_or("").to_owned()))
        .collect();
    let item = |title: &str| (Role::MenuItem, title.to_owned());
    let line = (Role::Splitter, String::new());
    assert_eq!(
        rows,
        [
            item("Copy Link"),
            line.clone(),
            item("Rename…"),
            item("Mute"),
            item("Duplicate"),
            item("Move to"),
            line,
            item("Archive Tab"),
            item("Archive Tabs Below"),
        ]
    );
    let archive = node(&tree, named(&tree, Role::MenuItem, "Archive Tab"));
    assert_eq!(
        (archive.position_in_set(), archive.size_of_set()),
        (Some(6), Some(7))
    );
    assert!(node(&tree, named(&tree, Role::MenuItem, "Mute")).is_disabled());
    let move_to = node(&tree, named(&tree, Role::MenuItem, "Move to"));
    assert_eq!(
        (move_to.has_popup(), move_to.is_expanded()),
        (Some(HasPopup::Menu), Some(false))
    );

    cx.simulate_keystrokes("down");
    assert_eq!(
        focused(&cx.a11y_tree().unwrap()).as_deref(),
        Some("Copy Link")
    );
    cx.simulate_keystrokes("down down");
    assert_eq!(
        focused(&cx.a11y_tree().unwrap()).as_deref(),
        Some("Duplicate"),
        "past the line and Mute"
    );

    cx.simulate_keystrokes("m right");
    let tree = cx.a11y_tree().unwrap();
    named(&tree, Role::Menu, "Move to");
    assert_eq!(
        node(&tree, named(&tree, Role::MenuItem, "Move to")).is_expanded(),
        Some(true)
    );
    let space = node(&tree, named(&tree, Role::MenuItemRadio, "Space 1"));
    assert_eq!(space.toggled(), Some(Toggled::True));
    assert_eq!(focused(&tree).as_deref(), Some("Space 1"));
}

/// 0xRRGGBBAA `top` over the opaque 0xRRGGBBAA `under`, as 0xRRGGBB.
fn over(top: u32, under: u32) -> u32 {
    let alpha = (top & 0xff) as f32 / 255.;
    let channel = |shift: u32| {
        let (t, u) = (
            ((top >> shift) & 0xff) as f32,
            ((under >> shift) & 0xff) as f32,
        );
        ((t * alpha + u * (1. - alpha)).round() as u32) << (shift - 8)
    };
    channel(24) | channel(16) | channel(8)
}

/// WCAG's contrast ratio between two 0xRRGGBB colours.
fn contrast(a: u32, b: u32) -> f32 {
    crate::palette::contrast(gpui::rgb(a), gpui::rgb(b))
}

/// Every label a menu draws reads at 4.5:1 against what it's on, in both looks, which are all a
/// theme picks of the menu, as the panel is opaque over it: anywhere down its glass's light, and on
/// the plate under the pointer, pressed or not (#4090). Capture's markup reads on the highlight of
/// any Space's accent. Dimmed items are exempt, as WCAG's rule exempts what can't be used.
#[test]
fn every_label_reads_at_4_5_to_1_in_both_looks() {
    use super::{
        DARK_TOP_LIGHT, DARK_TOP_LIGHT_END, DESTRUCTIVE_FILL, DESTRUCTIVE_LABEL, Highlight,
        LIGHT_TOP_LIGHT, LIGHT_TOP_LIGHT_END, PRESSED,
    };
    use crate::{
        palette::{Palette, Tone},
        theme::{Appearance, Theme},
    };
    for look in [
        Look {
            light: false,
            ..Look::dark()
        },
        Look {
            light: true,
            ..Look::dark()
        },
    ] {
        let palette = look.palette();
        let lights = match look.light {
            true => [LIGHT_TOP_LIGHT, LIGHT_TOP_LIGHT_END],
            false => [DARK_TOP_LIGHT, DARK_TOP_LIGHT_END],
        };
        let opaque = |ground: u32| (ground << 8) | 0xff;
        let fill = palette.fill >> 8;
        for panel in [
            fill,
            over(lights[0], palette.fill),
            over(lights[1], palette.fill),
        ] {
            for (what, color) in [
                ("label", palette.label),
                ("shortcut", palette.secondary),
                ("destructive", palette.destructive),
            ] {
                let ratio = contrast(over(color, opaque(panel)), panel);
                assert!(ratio >= 4.5, "{:?}'s {what} is {ratio:.2}:1", look.light);
            }
            let plate = Highlight::plate(look.light);
            let label = crate::theme::packed(plate.label);
            for (what, fill) in [("plate", plate.fill), ("pressed plate", plate.pressed)] {
                let fill = crate::theme::packed(fill) << 8 | (fill.a * 255.).round() as u32;
                let ratio = contrast(label, over(fill, opaque(panel)));
                assert!(
                    ratio >= 4.5,
                    "{:?}'s {what} label is {ratio:.2}:1",
                    look.light
                );
            }
        }
        let ratio = contrast(
            over(DESTRUCTIVE_LABEL, DESTRUCTIVE_FILL),
            over(DESTRUCTIVE_FILL, palette.fill),
        );
        assert!(
            ratio >= 4.5,
            "{:?}'s delete label is {ratio:.2}:1",
            look.light
        );
        let pressed = over(PRESSED, DESTRUCTIVE_FILL);
        let ratio = contrast(over(DESTRUCTIVE_LABEL, DESTRUCTIVE_FILL), pressed);
        assert!(
            ratio >= 4.5,
            "{:?}'s pressed delete label is {ratio:.2}:1",
            look.light
        );
    }
    let mut palettes = vec![Palette::default()];
    for appearance in Appearance::ALL {
        for step in 0..24 {
            let angle = step as f32 * std::f32::consts::TAU / 24.;
            for distance in [0., 0.35, 0.7, 1.] {
                let theme = Theme {
                    appearance,
                    ..Theme::at(distance * angle.cos(), distance * angle.sin())
                };
                palettes
                    .extend([Tone::Dark, Tone::Light].map(|tone| Palette::of(Some(&theme), tone)));
            }
        }
    }
    for palette in &palettes {
        let highlight = Highlight::of(palette);
        for (what, fill) in [
            ("highlight", highlight.fill),
            ("pressed", highlight.pressed),
        ] {
            let ratio = crate::palette::contrast(highlight.label, fill);
            assert!(
                ratio >= 4.5,
                "the {what} label on {:?} is {ratio:.2}:1",
                highlight.fill
            );
        }
    }
}

/// A model menu as the composer's: its search, each assistant's models under its name, one not
/// signed in with Sign In, and Settings.
fn models() -> Vec<MenuEntry> {
    let heading = |title: &str| MenuEntry {
        header: true,
        ..MenuEntry::named(title.to_string(), false)
    };
    vec![
        MenuEntry::search("Search models"), // 0
        heading("Claude"),                  // 1
        item("Default"),                    // 2
        item("Haiku"),                      // 3
        item("Sonnet"),                     // 4
        heading("ChatGPT"),                 // 5
        item("GPT-5"),                      // 6
        item("o4-mini"),                    // 7
        heading("Gemini"),                  // 8
        item("Sign In").kept(),             // 9
        MenuEntry::line(),                  // 10
        item("Settings").kept(),            // 11
    ]
}

/// A search leaves the rows whose titles hold what's typed, in any case, an assistant's name
/// while one of its models is left, and Sign In and Settings whatever is typed: with no match,
/// those alone, and no words of the search's own.
#[test]
fn a_search_narrows_the_rows_to_what_holds_it() {
    let menu = models();
    assert_eq!(narrowed(&menu, ""), (0..menu.len()).collect::<Vec<_>>());
    assert_eq!(narrowed(&menu, "  "), (0..menu.len()).collect::<Vec<_>>());
    assert_eq!(narrowed(&menu, "o"), [0, 1, 4, 5, 7, 8, 9, 10, 11]);
    assert_eq!(narrowed(&menu, "HAI"), [0, 1, 3, 8, 9, 10, 11]);
    assert_eq!(narrowed(&menu, "zzz"), [0, 8, 9, 10, 11]);
    // A heading's own name matches nothing: it stays for its rows.
    assert_eq!(narrowed(&menu, "claude"), [0, 8, 9, 10, 11]);
    // Without a row a search keeps, no line is left hanging at either end.
    let plain = vec![
        MenuEntry::search("Search"),
        item("Copy"),
        MenuEntry::line(),
        item("Paste"),
        MenuEntry::line(),
        item("Delete"),
    ];
    assert_eq!(narrowed(&plain, "p"), [0, 1, 2, 3]);
    assert_eq!(narrowed(&plain, "paste"), [0, 3]);
    assert_eq!(narrowed(&plain, "zzz"), [0]);
    assert_eq!(
        described(&plain),
        "Copy, Paste, Delete",
        "the field isn't a row"
    );
}

/// Shows `items` at (100, 100) and returns how it closed once it does.
fn show_items(items: Vec<MenuEntry>, cx: &mut VisualTestContext) -> Rc<Cell<Option<Closed>>> {
    let chosen = Rc::new(Cell::new(None));
    let answer = chosen.clone();
    cx.cx.update(|cx: &mut App| {
        let request = Request {
            items,
            at: at(100., 100.),
            area: area(),
            look: Look::dark(),
            chosen: Box::new(move |closed, _| answer.set(Some(closed))),
        };
        super::show(request, cx);
    });
    cx.run_until_parked();
    chosen
}

/// The menu's window, for the tests to look into.
fn search_menu(
    cx: &mut TestAppContext,
) -> (
    Rc<Recorder>,
    gpui::WindowHandle<super::ContextMenu>,
    VisualTestContext,
) {
    let host = Rc::new(Recorder::default());
    let window = cx
        .update(|cx| super::open(host.clone(), cx))
        .expect("no window for the menu");
    let cx = VisualTestContext::from_window(window.into(), cx);
    (host, window, cx)
}

/// The search's field and the titles of the rows shown, the field's left out.
fn searched(
    window: gpui::WindowHandle<super::ContextMenu>,
    cx: &mut VisualTestContext,
) -> Option<(gpui::Entity<crate::text_input::TextInput>, Vec<String>)> {
    window
        .update(&mut cx.cx, |menu, _, _| {
            let shown = menu.shown.as_ref()?;
            let field = shown.search.as_ref()?.field.clone();
            let rows = (shown.tracking.entries.iter())
                .filter(|entry| !entry.field)
                .map(|entry| entry.title.to_string())
                .collect();
            Some((field, rows))
        })
        .unwrap()
}

/// The cursor is in the field as the menu opens; what's typed there narrows the rows and selects
/// nothing by its letters, space included; the arrows and ↩ walk and pick the rows left, and the
/// pick is the row of the list the menu was asked to show, not of the rows left.
#[gpui::test]
fn the_arrows_and_return_pick_while_the_cursor_is_in_the_search(cx: &mut TestAppContext) {
    let (host, window, mut cx) = search_menu(cx);
    let cx = &mut cx;
    let chosen = show_items(models(), cx);
    let (field, _) = searched(window, cx).expect("the menu has no search");
    let focused = cx.update(|window, cx| field.read(cx).focus_handle(cx).is_focused(window));
    assert!(focused, "the cursor isn't in the field as the menu opens");

    cx.simulate_keystrokes("o");
    let (field, rows) = searched(window, cx).unwrap();
    assert_eq!(
        field.read_with(cx, |field, _| field.text().to_string()),
        "o"
    );
    assert_eq!(
        rows,
        [
            "Claude", "Sonnet", "ChatGPT", "o4-mini", "Gemini", "Sign In", "", "Settings"
        ]
    );
    cx.simulate_keystrokes("space");
    assert_eq!(
        field.read_with(cx, |field, _| field.text().to_string()),
        "o "
    );
    assert_eq!(chosen.get(), None, "space picked a row in a search");
    // The field's own keys, such as ⌫, are bindings these tests don't make: the text is put back
    // through the input method's way in.
    cx.update(|window, cx| {
        use gpui::EntityInputHandler as _;
        field.update(cx, |field, cx| {
            field.replace_text_in_range(Some(0..2), "o", window, cx)
        })
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("down enter");
    assert_eq!(
        chosen.get(),
        Some(Closed::Chose(7)),
        "o4-mini in the list asked for"
    );
    assert_eq!(host.calls.borrow().last(), Some(&"hide"));

    // Up from nothing lit goes to the last row, which a search that matches nothing leaves.
    let chosen = show_items(models(), cx);
    cx.simulate_keystrokes("z z z");
    let (_, rows) = searched(window, cx).unwrap();
    assert_eq!(rows, ["Gemini", "Sign In", "", "Settings"]);
    cx.simulate_keystrokes("enter");
    assert_eq!(
        chosen.get(),
        None,
        "a search that matches nothing lights nothing"
    );
    cx.simulate_keystrokes("up enter");
    assert_eq!(chosen.get(), Some(Closed::Chose(11)), "Settings");
}

/// ↩ and esc while an input method composes in the search are the composition's: ↩ picks
/// nothing and closes nothing, esc leaves the menu open. Once the method commits, the rows narrow
/// to it, and esc closes the menu, the search going with it, so the next menu's field is empty.
#[gpui::test]
fn return_and_escape_while_composing_in_the_search_are_the_compositions(cx: &mut TestAppContext) {
    use gpui::EntityInputHandler as _;
    let (host, window, mut cx) = search_menu(cx);
    let cx = &mut cx;
    let chosen = show_items(models(), cx);
    cx.simulate_keystrokes("s");
    let (field, rows) = searched(window, cx).unwrap();
    assert_eq!(
        rows,
        ["Claude", "Sonnet", "Gemini", "Sign In", "", "Settings"]
    );
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_and_mark_text_in_range(None, "ソ", None, window, cx)
        })
    });
    cx.run_until_parked();
    assert!(field.read_with(cx, |field, _| field.composing()));
    let (_, rows) = searched(window, cx).unwrap();
    assert_eq!(
        rows,
        ["Claude", "Sonnet", "Gemini", "Sign In", "", "Settings"],
        "the rows narrowed to a composition"
    );
    cx.simulate_keystrokes("enter");
    assert_eq!(chosen.get(), None, "↩ that ends a composition picked a row");
    cx.simulate_keystrokes("escape");
    assert_eq!(chosen.get(), None, "esc while composing closed the menu");
    assert!(
        searched(window, cx).is_some(),
        "the menu closed while composing"
    );
    assert_eq!(
        field.read_with(cx, |field, _| (field.text().to_string(), field.composing())),
        ("sソ".to_string(), true)
    );

    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_text_in_range(None, "ソ", window, cx)
        })
    });
    cx.run_until_parked();
    let (_, rows) = searched(window, cx).unwrap();
    assert_eq!(rows, ["Gemini", "Sign In", "", "Settings"]);
    cx.simulate_keystrokes("escape");
    assert_eq!(chosen.get(), Some(Closed::Otherwise));
    assert_eq!(
        host.calls.borrow().last(),
        Some(&"hide"),
        "the keyboard went nowhere back"
    );
    assert!(searched(window, cx).is_none());

    show_items(models(), cx);
    let (field, rows) = searched(window, cx).unwrap();
    assert_eq!(field.read_with(cx, |field, _| field.text().to_string()), "");
    assert_eq!(rows.len(), models().len() - 1, "the last search was kept");
}
