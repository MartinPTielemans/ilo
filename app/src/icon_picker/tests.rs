//! The icon picker's tabs, clicks and keys, on a host that records what the picker asks of its
//! window and what it picks.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    Modifiers, TestAppContext, VisualTestContext,
    accesskit::{Node, NodeId, Role},
    point, px,
};

use super::*;

#[derive(Default)]
struct Recorder(RefCell<Vec<String>>, Cell<usize>);

impl Recorder {
    fn record(&self, call: String) {
        self.0.borrow_mut().push(call);
    }
}

impl Host for Recorder {
    fn show(&self, window: u64, left: f32, top: f32) {
        self.record(format!("show {left} {top} over {window}"));
    }

    fn hide(&self) {
        self.record("hide".into());
    }

    fn keep_recents(&self, recents: &str) {
        self.record(format!("recents {recents:?}"));
    }

    /// A system too old for 🫠 melting face, which came with Unicode 14.
    fn draws(&self, emoji: &str) -> bool {
        self.1.set(self.1.get() + 1);
        emoji != "🫠"
    }
}

/// The picker, shown for Space 7, which has `icon`, its arrow at (23, 66).
struct Picker {
    cx: VisualTestContext,
    window: WindowHandle<IconPicker>,
    host: Rc<Recorder>,
}

impl Picker {
    fn new(icon: &str, cx: &mut TestAppContext) -> Self {
        Self::with_recents(icon, "", cx)
    }

    /// The picker, as the app opens it after `recents` were saved.
    fn with_recents(icon: &str, recents: &str, cx: &mut TestAppContext) -> Self {
        Self::of(Subject::Space(7), icon, recents, cx)
    }

    /// The picker for a tab, 7, which has `icon`.
    fn for_tab(icon: &str, cx: &mut TestAppContext) -> Self {
        Self::of(Subject::Tab(7), icon, "", cx)
    }

    fn of(subject: Subject, icon: &str, recents: &str, cx: &mut TestAppContext) -> Self {
        let host = Rc::new(Recorder::default());
        let recents = Recents::parse(recents);
        // The app binds the field's keys once for every window.
        cx.update(text_input::bind_keys);
        let window = cx
            .update(|cx| open(host.clone(), recents, cx))
            .expect("no window");
        let recorded = host.clone();
        let picked: Picked = Rc::new(move |icon, _| recorded.record(format!("picked {icon:?}")));
        let icon = SharedString::from(icon.to_string());
        // From browser window 2, as a context menu in a window behind another asks (#1432).
        cx.update(|cx| show(2, subject, icon, point(23., 66.), picked, cx));
        let cx = VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();
        Picker { cx, window, host }
    }

    fn tab(&mut self) -> Option<Tab> {
        let picking = self
            .window
            .read_with(&self.cx.cx, |picker, _| picker.picking())
            .unwrap();
        picking.map(|(_, tab)| tab)
    }

    fn click(&mut self, x: f32, y: f32) {
        self.cx
            .simulate_click(point(px(x), px(y)), Modifiers::none());
        self.cx.run_until_parked();
    }

    /// A click on the glyph `index` places from the first, row by row.
    fn click_glyph(&mut self, index: usize) {
        let (row, column) = (index / COLUMNS, index % COLUMNS);
        let x = GRID_X + PITCH * (column as f32 + 0.5);
        let y = ARROW + GRID_TOP + PITCH * (row as f32 + 0.5);
        self.click(x, y);
    }

    /// A click on the emoji `column` across in the list's `row`th row in view, from the first.
    fn click_emoji(&mut self, row: usize, column: usize) {
        let x = GRID_X + PITCH * (column as f32 + 0.5);
        let y = ARROW + emoji::LIST_TOP + PITCH * (row as f32 + 0.5);
        self.click(x, y);
    }

    fn type_in_search(&mut self, text: &str) {
        self.cx.simulate_input(text);
        self.cx.run_until_parked();
    }

    /// The emoji the list shows from the first, and the one on the plate.
    fn listed(&mut self) -> (Vec<&'static str>, Option<&'static str>) {
        self.window
            .read_with(&self.cx.cx, |picker, _| {
                let picking = picker.picking.as_ref().expect("not showing");
                let items = picking
                    .grid
                    .items
                    .iter()
                    .map(|emoji| emoji.as_str())
                    .collect();
                (
                    items,
                    picking
                        .selected
                        .map(|item| picking.grid.items[item].as_str()),
                )
            })
            .unwrap()
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    /// What the picker asked of the host since it showed.
    fn calls(&mut self) -> Vec<String> {
        self.host.0.borrow().iter().skip(1).cloned().collect()
    }
}

#[test]
fn it_opens_on_the_emoji_tab_for_an_emoji_and_the_icon_tab_otherwise() {
    assert_eq!(tab_for("🦊"), Tab::Emoji);
    assert_eq!(tab_for("glyph:star"), Tab::Icon);
    assert_eq!(tab_for(""), Tab::Icon);
    assert_eq!(tab_for("glyph:unicorn"), Tab::Icon);
}

#[gpui::test]
fn it_hangs_its_card_under_the_icon_with_the_arrow_at_it(cx: &mut TestAppContext) {
    let mut picker = Picker::new("🦊", cx);
    assert_eq!(picker.tab(), Some(Tab::Emoji));
    // The arrow's tip is 13 in from the window's left, and the card 25 under the icon's middle,
    // over the window that asked.
    assert_eq!(picker.host.0.borrow()[0], "show 10 78 over 2");
}

#[gpui::test]
fn a_glyph_click_gives_the_space_that_glyph_and_closes(cx: &mut TestAppContext) {
    let mut picker = Picker::new("", cx);
    assert_eq!(picker.tab(), Some(Tab::Icon));
    picker.click_glyph(9);
    assert_eq!(picker.calls(), ["hide", "picked \"glyph:shapes\""]);
    assert_eq!(picker.tab(), None);
}

#[gpui::test]
fn the_glyph_the_space_has_changes_nothing(cx: &mut TestAppContext) {
    let mut picker = Picker::new("glyph:star", cx);
    picker.click_glyph(0);
    assert_eq!(picker.calls(), ["hide"]);
}

#[gpui::test]
fn the_trash_takes_the_icon_away(cx: &mut TestAppContext) {
    let mut picker = Picker::new("glyph:moon", cx);
    // The trash is the 24 pt button 7 after Emoji | Icon, the two centred under the arrow.
    let left = (WIDTH - (2. * SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT)) / 2.;
    let x = left + 2. * SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT / 2.;
    picker.click(x, ARROW + SEGMENTS_TOP + SEGMENTS_HEIGHT / 2.);
    assert_eq!(picker.calls(), ["hide", "picked \"\""]);
}

/// With no icon there's nothing to remove: Emoji | Icon is centred alone, with no trash after it.
#[gpui::test]
fn a_space_with_no_icon_is_offered_no_trash(cx: &mut TestAppContext) {
    let mut picker = Picker::new("", cx);
    let y = ARROW + SEGMENTS_TOP + SEGMENTS_HEIGHT / 2.;
    // Where the trash would be after the two.
    let left = (WIDTH - (2. * SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT)) / 2.;
    picker.click(
        left + 2. * SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT / 2.,
        y,
    );
    assert_eq!(picker.calls(), Vec::<String>::new());
    assert_eq!(picker.tab(), Some(Tab::Icon));
    // Emoji's right end, centred alone, is past where it ends beside the trash.
    let left = (WIDTH - 2. * SEGMENT_WIDTH) / 2.;
    picker.click(left + SEGMENT_WIDTH - 2., y);
    assert_eq!(picker.tab(), Some(Tab::Emoji));

    // A tab with no emoji: Emoji alone.
    let mut picker = Picker::for_tab("", cx);
    let left = (WIDTH - (SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT)) / 2.;
    picker.click(left + SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT / 2., y);
    assert_eq!(picker.calls(), Vec::<String>::new());
    assert_eq!(picker.tab(), Some(Tab::Emoji));
}

#[gpui::test]
fn escape_closes_it_with_no_change(cx: &mut TestAppContext) {
    let mut picker = Picker::new("glyph:moon", cx);
    picker.keys("escape");
    assert_eq!(picker.calls(), ["hide"]);
    assert_eq!(picker.tab(), None);

    // The search on the Emoji tab has the keyboard, and escape is the picker's.
    let mut picker = Picker::new("🦊", cx);
    picker.keys("escape");
    assert_eq!(picker.calls(), ["hide"]);
}

#[gpui::test]
fn the_spaces_emoji_is_on_the_plate_when_it_opens(cx: &mut TestAppContext) {
    let mut picker = Picker::new("🌱", cx);
    let (items, plate) = picker.listed();
    assert_eq!(plate, Some("🌱"));
    // The whole list, not a search's.
    assert!(items.len() > 1_500);
    // A Space with no emoji has nothing on the plate.
    let mut picker = Picker::new("", cx);
    assert_eq!(picker.tab(), Some(Tab::Icon));
}

#[gpui::test]
fn the_list_leaves_out_what_the_system_cant_draw_and_asks_once(cx: &mut TestAppContext) {
    // Not asked until the Emoji tab shows.
    let mut picker = Picker::new("glyph:star", cx);
    assert_eq!(picker.host.1.get(), 0);
    // The Emoji segment is the first of the two, centred under the arrow with the trash.
    let left = (WIDTH - (2. * SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT)) / 2.;
    picker.click(
        left + SEGMENT_WIDTH / 2.,
        ARROW + SEGMENTS_TOP + SEGMENTS_HEIGHT / 2.,
    );
    let (items, _) = picker.listed();
    assert!(items.contains(&"🦊") && !items.contains(&"🫠"));
    let asked = picker.host.1.get();
    assert!(asked > 1_500, "{asked}");
    // A search, and the tab shown again, ask nothing more.
    picker.type_in_search("face");
    assert!(!picker.listed().0.contains(&"🫠"));
    assert_eq!(picker.host.1.get(), asked);

    // A Space whose emoji the system can't draw opens on the Emoji tab with nothing on the plate.
    let mut picker = Picker::new("🫠", cx);
    assert_eq!(picker.tab(), Some(Tab::Emoji));
    assert_eq!(picker.listed().1, None);
}

#[test]
fn a_tab_is_offered_the_emoji_tab_only() {
    assert_eq!(Subject::Tab(1).tabs(), [Tab::Emoji]);
    assert_eq!(Subject::Space(1).tabs(), [Tab::Emoji, Tab::Icon]);
    assert_eq!(Subject::Tab(12).to_string(), "tab 12");
    assert_eq!(Subject::Space(12).to_string(), "12");
}

#[gpui::test]
fn a_tabs_picker_opens_on_the_emoji_tab_and_picks_an_emoji(cx: &mut TestAppContext) {
    // A tab with no icon, or a character no glyph is, still opens on the grid.
    let mut picker = Picker::for_tab("", cx);
    let showing = picker
        .window
        .read_with(&picker.cx.cx, |p, _| p.picking())
        .unwrap();
    assert_eq!(showing, Some((Subject::Tab(7), Tab::Emoji)));
    assert_eq!(picker.listed().1, None);
    picker.type_in_search("fox");
    picker.keys("enter");
    assert_eq!(
        picker.calls(),
        ["recents \"🦊\\n\"", "hide", "picked \"🦊\""]
    );

    // The tab's emoji is on the plate, and the trash gives the favicon back.
    let mut picker = Picker::for_tab("🌱", cx);
    assert_eq!(picker.listed().1, Some("🌱"));
    // The one segment and the trash after it, centred under the arrow.
    let left = (WIDTH - (SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT)) / 2.;
    let x = left + SEGMENT_WIDTH + TRASH_GAP + SEGMENTS_HEIGHT / 2.;
    picker.click(x, ARROW + SEGMENTS_TOP + SEGMENTS_HEIGHT / 2.);
    assert_eq!(picker.calls(), ["hide", "picked \"\""]);
}

#[gpui::test]
fn typing_narrows_the_list_and_the_first_match_takes_the_plate(cx: &mut TestAppContext) {
    let mut picker = Picker::new("🌱", cx);
    picker.type_in_search("fox");
    let (items, plate) = picker.listed();
    assert_eq!(items[0], "🦊");
    assert_eq!(plate, Some("🦊"));
    assert!(items.len() < 10, "{items:?}");
    // The shortcode names it too.
    picker.type_in_search("x");
    picker.type_in_search("zzz");
    assert_eq!(picker.listed(), (vec![], None));
    // Emptied, the whole list is back with the Space's emoji on the plate.
    for _ in 0..8 {
        picker.keys("backspace");
    }
    let (items, plate) = picker.listed();
    assert!(items.len() > 1_500);
    assert_eq!(plate, Some("🌱"));
}

#[gpui::test]
fn a_click_on_an_emoji_gives_the_space_that_emoji_and_closes(cx: &mut TestAppContext) {
    let mut picker = Picker::new("🌱", cx);
    picker.type_in_search("fox");
    picker.click_emoji(0, 0);
    assert_eq!(
        picker.calls(),
        ["recents \"🦊\\n\"", "hide", "picked \"🦊\""]
    );
    assert_eq!(picker.tab(), None);
}

#[gpui::test]
fn the_emoji_the_space_has_changes_nothing(cx: &mut TestAppContext) {
    let mut picker = Picker::new("🦊", cx);
    picker.type_in_search("fox");
    picker.click_emoji(0, 0);
    assert_eq!(picker.calls(), ["recents \"🦊\\n\"", "hide"]);
}

#[gpui::test]
fn arrows_move_the_plate_and_return_picks(cx: &mut TestAppContext) {
    let mut picker = Picker::new("🌱", cx);
    let (items, _) = picker.listed();
    let start = items.iter().position(|emoji| *emoji == "🌱").unwrap();
    // → one on, ↓ a row down.
    picker.keys("right");
    assert_eq!(picker.listed().1, Some(items[start + 1]));
    picker.keys("down");
    assert_eq!(picker.listed().1, Some(items[start + 1 + COLUMNS]));
    picker.keys("left up");
    assert_eq!(picker.listed().1, Some(items[start]));
    picker.keys("right");
    picker.keys("enter");
    let picked = items[start + 1];
    assert_eq!(
        picker.calls(),
        [
            format!("recents {:?}", format!("{picked}\n")),
            "hide".into(),
            format!("picked {picked:?}")
        ]
    );
}

#[gpui::test]
fn left_and_right_move_the_caret_once_something_is_typed(cx: &mut TestAppContext) {
    let mut picker = Picker::new("🌱", cx);
    picker.type_in_search("face");
    let (items, plate) = picker.listed();
    assert_eq!(plate, Some(items[0]));
    picker.keys("right");
    picker.keys("left");
    assert_eq!(picker.listed().1, Some(items[0]));
    // Down still moves the plate.
    picker.keys("down");
    assert_eq!(picker.listed().1, Some(items[COLUMNS]));
    // The caret moved left twice, so a letter typed lands after "fa".
    picker.keys("left");
    picker.type_in_search("s");
    let text = picker
        .window
        .read_with(&picker.cx.cx, |p, cx| p.search.read(cx).text().to_string());
    assert_eq!(text.unwrap(), "fasce");
}

#[gpui::test]
fn return_with_nothing_on_the_plate_does_nothing(cx: &mut TestAppContext) {
    let mut picker = Picker::new("glyph:star", cx);
    picker.keys("enter");
    assert_eq!(picker.calls(), Vec::<String>::new());
    assert_eq!(picker.tab(), Some(Tab::Icon));
}

#[gpui::test]
fn the_last_pick_is_first_in_the_list_and_kept(cx: &mut TestAppContext) {
    let mut picker = Picker::with_recents("🌱", "🌱\n🐙\n", cx);
    let (items, _) = picker.listed();
    assert_eq!(items[..2], ["🌱", "🐙"]);
    picker.type_in_search("fox");
    picker.click_emoji(0, 0);
    // The pick goes before the ones before it.
    assert_eq!(picker.calls()[0], format!("recents {:?}", "🦊\n🌱\n🐙\n"));

    // Opened again with what was kept, the list starts with it.
    let mut picker = Picker::with_recents("🌱", "🦊\n🌱\n🐙\n", cx);
    let (items, plate) = picker.listed();
    assert_eq!(items[..3], ["🦊", "🌱", "🐙"]);
    // The Space's emoji is on the plate where the recents have it.
    assert_eq!(plate, Some("🌱"));
    assert_eq!(
        picker
            .window
            .read_with(&picker.cx.cx, |p, _| p.picking.as_ref().unwrap().selected)
            .unwrap(),
        Some(1)
    );
}

#[test]
fn icons_come_in_from_the_right_and_emoji_from_the_left() {
    let near = |(old, new): (f32, f32), (x, y): (f32, f32)| {
        assert!(
            (old - x).abs() < 1e-3 && (new - y).abs() < 1e-3,
            "{old} {new}, not {x} {y}"
        );
    };
    near(slid(Tab::Emoji, Tab::Icon, 0.), (0., WIDTH));
    let (old, new) = slid(Tab::Emoji, Tab::Icon, 0.5);
    assert!(
        old < 0. && new > 0. && (new - old - WIDTH).abs() < 1e-3,
        "{old} {new}"
    );
    near(slid(Tab::Emoji, Tab::Icon, 1.), (-WIDTH, 0.));
    near(slid(Tab::Icon, Tab::Emoji, 0.), (0., -WIDTH));
    near(slid(Tab::Icon, Tab::Emoji, 1.), (WIDTH, 0.));
}

#[gpui::test]
fn a_folders_icon_is_an_emoji_as_a_tabs_is(cx: &mut TestAppContext) {
    assert_eq!(Subject::Folder(3).tabs(), [Tab::Emoji]);
    assert_eq!(Subject::Folder(3).to_string(), "folder 3");
    let mut picker = Picker::of(Subject::Folder(7), "", "", cx);
    assert_eq!(picker.tab(), Some(Tab::Emoji));
}

/// The node an assistive app reads with `role` and `label`.
fn has(picker: &mut Picker, role: Role, label: &str) -> (NodeId, Node) {
    let tree = picker.cx.a11y_tree().expect("GPUI built a tree");
    let found = tree
        .nodes
        .into_iter()
        .find(|(_, node)| node.role() == role && node.label() == Some(label));
    found.unwrap_or_else(|| panic!("no {role:?} called {label}"))
}

/// VoiceOver reads the picker as what it picks for (#1381): Emoji | Icon as tabs with the shown
/// one selected, the trash as Remove Icon, and the glyphs as a list of options by name, the
/// Space's selected. Its press on a glyph picks it.
#[gpui::test]
fn voiceover_reads_the_icon_tab(cx: &mut TestAppContext) {
    use gpui::accesskit::Action;
    let mut picker = Picker::new("glyph:star", cx);
    picker.cx.simulate_a11y_activation();
    picker.cx.run_until_parked();
    has(&mut picker, Role::Dialog, "Space Icon");
    has(&mut picker, Role::TabList, "Kind of Icon");
    assert_eq!(
        has(&mut picker, Role::Tab, "Emoji").1.is_selected(),
        Some(false)
    );
    assert_eq!(
        has(&mut picker, Role::Tab, "Icon").1.is_selected(),
        Some(true)
    );
    has(&mut picker, Role::Button, "Remove Icon");
    has(&mut picker, Role::ListBox, "Icons");
    assert_eq!(
        has(&mut picker, Role::ListBoxOption, "star")
            .1
            .is_selected(),
        Some(true)
    );
    let (moon, node) = has(&mut picker, Role::ListBoxOption, "moon");
    assert_eq!(node.is_selected(), Some(false));
    picker.cx.simulate_a11y_action(moon, Action::Click);
    picker.cx.run_until_parked();
    assert_eq!(picker.calls(), ["hide", "picked \"glyph:moon\""]);
}

/// On the Emoji tab VoiceOver lists the emoji as the system names them, the one ↩ picks
/// selected, and the tone button says the tone. A tab's picker reads as that.
#[gpui::test]
fn voiceover_reads_the_emoji_tab(cx: &mut TestAppContext) {
    let mut picker = Picker::for_tab("🦊", cx);
    picker.cx.simulate_a11y_activation();
    picker.cx.run_until_parked();
    has(&mut picker, Role::Dialog, "Tab Icon");
    assert_eq!(
        has(&mut picker, Role::Tab, "Emoji").1.is_selected(),
        Some(true)
    );
    has(&mut picker, Role::ListBox, "Emoji");
    assert_eq!(picker.listed().1, Some("🦊"));
    assert_eq!(
        has(&mut picker, Role::ListBoxOption, "🦊").1.is_selected(),
        Some(true)
    );
    has(&mut picker, Role::Button, "Skin Tone, Default");
}
