//! The Site Controls' keys, clicks and hovers, on a host that records what the panel asks of its
//! window and the core.

use std::cell::{Cell, RefCell};

use gpui::{Modifiers, TestAppContext, VisualTestContext};

use super::*;

#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<String>>,
    /// The shape the panel last gave its window.
    corners: RefCell<Vec<(f32, f32, f32)>>,
    /// The tooltips asked for, apart from the calls, as each close takes one away.
    tips: RefCell<Vec<String>>,
    /// The index the next menu answers with.
    answer: Cell<Option<usize>>,
    /// The system has no share sheet, as on Linux.
    no_share_sheet: bool,
}

impl Recorder {
    fn push(&self, call: String) {
        self.calls.borrow_mut().push(call);
    }
}

impl Host for Recorder {
    fn show(&self, at: Point<Pixels>, size: Size<Pixels>, corners: &[(f32, f32, f32)]) {
        let (x, y) = (f32::from(at.x), f32::from(at.y));
        self.push(format!(
            "show {x} {y} {} {}",
            f32::from(size.width),
            f32::from(size.height)
        ));
        self.corners.replace(corners.to_vec());
    }
    fn set_height(&self, height: f32, corners: &[(f32, f32, f32)]) {
        self.push(format!("height {height}"));
        self.corners.replace(corners.to_vec());
    }
    // A frame of the motion, which tells nothing of what the user did.
    fn drew(&self, _: f32, _: f32, _: Point<Pixels>) {}
    fn release(&self) {
        self.push("release".into());
    }
    fn hide(&self) {
        self.push("hide".into());
    }
    fn read(&self, tab: u64) {
        self.push(format!("read {tab}"));
    }
    fn stop_reading(&self) {
        self.push("stop".into());
    }
    fn set_permission(&self, tab: u64, kind: i32, value: Value) {
        self.push(format!("set {tab} {kind} {value:?}"));
    }
    fn reset_permissions(&self, tab: u64) {
        self.push(format!("reset {tab}"));
    }
    fn remove_device(&self, tab: u64, index: usize) {
        self.push(format!("remove device {tab} {index}"));
    }
    fn set_system_engine(&self, tab: u64, on: bool) {
        self.push(format!("system engine {tab} {on}"));
    }
    fn allow_saving(&self, tab: u64, login: &Login) {
        self.push(format!("allow saving {tab} {}", login.site));
    }
    fn set_ad_blocking(&self, tab: u64, on: bool) {
        self.push(format!("ad blocking {tab} {on}"));
    }
    fn set_developer_mode(&self, tab: u64, on: bool) {
        self.push(format!("developer mode {tab} {on}"));
    }
    fn set_boost(&self, tab: u64, on: bool) {
        self.push(format!("boost {tab} {on}"));
    }
    fn run(&self, run: Run, tab: u64, _: &mut App) {
        let run = match run {
            Run::Command(command) => format!("{command:?}"),
            Run::Share(frame) => {
                let (x, y) = (f32::from(frame.origin.x), f32::from(frame.origin.y));
                let (width, height) = (f32::from(frame.size.width), f32::from(frame.size.height));
                format!("share {x} {y} {width} {height}")
            }
            Run::SiteSettings => "site settings".into(),
            Run::Extension(id, tile) => {
                let (x, y) = (f32::from(tile.origin.x), f32::from(tile.origin.y));
                let (width, height) = (f32::from(tile.size.width), f32::from(tile.size.height));
                format!("extension {id} {x} {y} {width} {height}")
            }
            Run::Certificate => "certificate".into(),
            Run::RemoveLogin(login) => format!("remove login {} {}", login.site, login.username),
            Run::Passwords => "passwords".into(),
            Run::ExtensionItem(id, item) => format!("extension {id} {item:?}"),
            Run::OpenInProfile(profile) => format!("open in profile {profile}"),
            Run::NewSpaceInProfile(profile) => format!("new space in profile {profile}"),
        };
        self.push(format!("run {run} {tab}"));
    }
    fn menu(
        &self,
        items: Vec<MenuEntry>,
        _: Point<Pixels>,
        chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        let items: Vec<String> = (items.iter())
            .map(|item| format!("{}{}", item.title, if item.checked { "*" } else { "" }))
            .collect();
        self.push(format!("menu {}", items.join(", ")));
        // Once the update that asked is over, as ilo's menu answers.
        let answer = self.answer.get();
        cx.defer(move |cx| chosen(answer, cx));
    }
    fn tooltip(&self, tip: Option<(SharedString, tooltip::Anchor)>, _: &mut App) {
        self.tips.borrow_mut().push(match tip {
            Some((text, anchor)) => format!("{text} at {} {}", anchor.x, anchor.bottom),
            None => "none".into(),
        });
    }
    fn has_share_sheet(&self) -> bool {
        !self.no_share_sheet
    }
}

const TAB: u64 = 7;
/// Window management, in Chrome's ContentSettingsType.
const WINDOWS: i32 = 36;

fn permission(kind: i32, name: &'static str, value: Value, choices: &[Value]) -> Permission {
    let state = if value == Value::Allow {
        "Allowed"
    } else {
        "Ask"
    };
    Permission {
        kind,
        key: "".into(),
        name: name.into(),
        value,
        state: state.into(),
        choices: choices.to_vec(),
    }
}

/// A site that asks for where the user is, may manage windows, and runs JavaScript as a policy
/// says, which the user can't change.
fn site() -> Site {
    Site {
        site: "example.com".into(),
        connection: Connection::Secure,
        connection_summary: "Connection is secure".into(),
        certificate: true,
        permissions: vec![
            permission(5, "Location", Value::Ask, &Value::ALL),
            permission(WINDOWS, "Window management", Value::Allow, &Value::ALL),
            permission(2, "JavaScript", Value::Allow, &[]),
        ],
        reset: Some("Reset permissions".into()),
        system_engine: None,
        ad_blocking: None,
        ad_blocked: SharedString::default(),
        developer_mode: None,
        logins: vec![],
        devices: Vec::new(),
        boostable: false,
        boost: None,
    }
}

fn login(username: &str, never: bool) -> Login {
    Login {
        site: "example.com".into(),
        username: username.to_string().into(),
        never,
        key: "https://example.com/".into(),
    }
}

fn extension() -> Extension {
    Extension {
        id: "abc".into(),
        title: "Notes".into(),
        icon: None,
        enabled: true,
        pinned: false,
        options: true,
        removable: true,
    }
}

fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

/// The URL field and its Site Controls button, as the shell lays them out in a 228 pt sidebar.
fn under_button() -> Place {
    Place::Button {
        field: bounds(8., 46., 212., 36.),
        button: bounds(191., 49., 30., 30.),
    }
}

fn window() -> Size<Pixels> {
    size(px(1200.), px(800.))
}

/// The panel opened from its button, once the core has read the site.
struct Panel {
    cx: VisualTestContext,
    host: Rc<Recorder>,
    /// The tab's profile and the others, which the shell gives the panel as it opens.
    profiles: Option<Profiles>,
}

impl Panel {
    fn new(cx: &mut TestAppContext, site: Site) -> Self {
        Panel::at(cx, under_button(), window(), site)
    }

    fn at(cx: &mut TestAppContext, place: Place, window: Size<Pixels>, site: Site) -> Self {
        Panel::on(Recorder::default(), cx, place, window, site)
    }

    fn on(
        host: Recorder,
        cx: &mut TestAppContext,
        place: Place,
        window: Size<Pixels>,
        site: Site,
    ) -> Self {
        Panel::profiled(host, cx, place, window, site, None)
    }

    fn profiled(
        host: Recorder,
        cx: &mut TestAppContext,
        place: Place,
        window: Size<Pixels>,
        site: Site,
        profiles: Option<Profiles>,
    ) -> Self {
        let host = Rc::new(host);
        let panel = cx.update(|cx| open(host.clone(), cx));
        let panel = panel.expect("no window for the Site Controls");
        let cx = VisualTestContext::from_window(panel.into(), cx);
        // As tall as the host makes it for the tallest panel here, so every part takes a click.
        cx.simulate_resize(size(px(WIDTH), px(500.)));
        let mut panel = Panel { cx, host, profiles };
        panel.show(place, window);
        assert_eq!(panel.calls(), [format!("read {TAB}")]);
        panel.changed(site);
        panel
    }

    /// What the button's press does.
    fn show(&mut self, place: Place, window: Size<Pixels>) {
        let profiles = self.profiles.clone();
        self.cx
            .cx
            .update(|cx| show(TAB, place, window, vec![extension()], profiles, cx));
        self.cx.run_until_parked();
    }

    fn changed(&mut self, site: Site) {
        self.cx.cx.update(|cx| changed(TAB, site, cx));
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    fn click(&mut self, (x, y): (f32, f32)) {
        self.cx
            .simulate_click(point(px(x), px(y)), Modifiers::none());
        self.cx.run_until_parked();
    }

    fn right_click(&mut self, (x, y): (f32, f32)) {
        let position = point(px(x), px(y));
        self.cx
            .simulate_mouse_down(position, MouseButton::Right, Modifiers::none());
        self.cx
            .simulate_mouse_up(position, MouseButton::Right, Modifiers::none());
        self.cx.run_until_parked();
    }

    fn hover(&mut self, (x, y): (f32, f32)) {
        self.cx
            .simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
        self.cx.run_until_parked();
    }

    /// Where the panel says it drew `part`, as the Mac checks read it from the log, back in the
    /// panel's own window, which shows at 88 78.5.
    fn placed(&mut self, part: &str) -> (f32, f32) {
        let places = self.cx.cx.update(|cx| {
            let window = cx.global::<SiteControlsWindow>().0;
            window.update(cx, |panel, _, _| panel.places()).unwrap()
        });
        let place = (places.iter())
            .find_map(|each| each.strip_prefix(&format!("{part} at ")))
            .unwrap_or_else(|| panic!("no place for {part} in {places:?}"));
        let (x, y) = place.split_once(' ').unwrap();
        (
            x.parse::<f32>().unwrap() - 88.,
            y.parse::<f32>().unwrap() - 78.5,
        )
    }

    /// What the panel asked since the last time, once it has had time to leave.
    fn calls(&mut self) -> Vec<String> {
        self.cx
            .executor()
            .advance_clock(Duration::from_secs_f32(CLOSE));
        self.cx.run_until_parked();
        self.host.calls.take()
    }

    /// What a click closing the panel to run `run` asks.
    fn ran(&mut self, run: &str) -> Vec<String> {
        let calls = self.calls();
        assert_eq!(
            calls,
            ["release", "stop", &format!("run {run} {TAB}"), "hide"]
        );
        calls
    }
}

// The panel's places in its window, which is 236 pt wide under a beak 12.5 pt tall: the action
// row's five places 10 pt in at y 22.5, 37, 37, 36, 37 and 37 pt wide with Share, the tiles closed
// up from the left and the shield after them; the extensions' tiles from y 91.5, the "This site"
// heading's cap at y 141.5, the lines 30 pt apart from y 158.5, and the footer at y 296.5. A
// switch alone is a strip from y 62.5 that takes 40 pt, and pushes what's under it down.
const SHARE: (f32, f32) = (28.5, 38.5);
const CAPTURE: (f32, f32) = (73.5, 38.5);
const CAPTURE_FULL_PAGE: (f32, f32) = (118., 38.5);
/// The shield, after the three tiles of a site that can't have a Boost.
const SHIELD: (f32, f32) = (162.5, 38.5);
const STRIP: (f32, f32) = (118., 78.5);
const STRIP_HEIGHT: f32 = 40.;
const EXTENSION: (f32, f32) = (34., 107.5);
const ADD: (f32, f32) = (90., 107.5);
const EXTENSIONS_HEADING: (f32, f32) = (39., 73.);
const MANAGE: (f32, f32) = (213., 73.);
const SITE_HEADING: (f32, f32) = (39., 146.);
const MORE_LINK: (f32, f32) = (218., 146.);
fn line(index: usize) -> (f32, f32) {
    (99., 158.5 + index as f32 * ITEM_PITCH + 14.)
}
/// A line under a strip.
fn line_under_strip(index: usize) -> (f32, f32) {
    let (x, y) = line(index);
    (x, y + STRIP_HEIGHT)
}
const LOCK: (f32, f32) = (29., 312.5);
const MORE: (f32, f32) = (209., 312.5);

/// Centred under its button at x 206, its top 9 pt under the URL field's bottom, which its beak
/// reaches over.
#[gpui::test]
fn it_hangs_from_its_button_once_the_site_is_read(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    assert_eq!(panel.calls(), ["show 88 78.5 236 338.5"]);
    let corners = panel.host.corners.take();
    assert_eq!(corners.first(), Some(&(118., 0., BEAK_TIP_RADIUS)));
    assert_eq!(corners.len(), 7);
}

/// It stays inside the window, and its beak at its button as far as the body's corner allows.
#[gpui::test]
fn it_keeps_inside_the_window(cx: &mut TestAppContext) {
    let field = bounds(8., 46., 284., 36.);
    let place = Place::Button {
        field,
        button: bounds(265., 49., 30., 30.),
    };
    let mut panel = Panel::at(cx, place, size(px(300.), px(800.)), site());
    assert_eq!(panel.calls(), ["show 56 78.5 236 338.5"]);
    assert_eq!(
        panel.host.corners.take()[0].0,
        WIDTH - (RADIUS + BEAK + BEAK_FOOT_RADIUS)
    );
}

/// With the sidebar hidden it opens at the page's corner, without a beak.
#[gpui::test]
fn in_the_corner_it_has_no_beak(cx: &mut TestAppContext) {
    let place = Place::Corner(point(px(18.), px(8.)));
    let mut panel = Panel::at(cx, place, window(), site());
    assert_eq!(panel.calls(), ["show 18 8 236 326"]);
    assert_eq!(panel.host.corners.take().len(), 4);
}

/// Arc's, measured, is 299 pt tall with two rows, under its beak. Ours keeps its tiles, headings
/// and footer where Arc's are, with its tiles' padding 10 for corners concentric with the body's,
/// and its two lines take 33 pt less than Arc's rows (#2334).
#[gpui::test]
fn two_lines_take_less_than_arcs_two_rows(_: &mut TestAppContext) {
    let two = Site {
        permissions: site().permissions[..2].to_vec(),
        reset: None,
        ..site()
    };
    let at = layout(&two, 0, false);
    let found = (
        at.tiles,
        at.extensions_cap,
        at.extensions,
        at.site_cap,
        at.lines,
    );
    assert_eq!(found, (10., 56., 79., Some(129.), 146.));
    assert_eq!((at.divider, at.footer, at.height), (213., 224., 266.));
}

#[gpui::test]
fn escape_closes_it(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.keys("escape");
    // The browser window gets the keyboard back at once, and the window goes once it has left.
    assert_eq!(panel.host.calls.borrow().as_slice(), ["release", "stop"]);
    assert_eq!(panel.calls(), ["release", "stop", "hide"]);
}

#[gpui::test]
fn a_click_outside_closes_it(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.cx.cx.update(|cx| dismissed(cx));
    assert_eq!(panel.calls(), ["release", "stop", "hide"]);
}

/// The button's second press closes the panel, and a third opens it again.
#[gpui::test]
fn its_button_toggles_it(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.show(under_button(), window());
    assert_eq!(panel.calls(), ["release", "stop", "hide"]);
    panel.show(under_button(), window());
    panel.changed(site());
    assert_eq!(
        panel.calls(),
        [format!("read {TAB}"), "show 88 78.5 236 338.5".into()]
    );
}

/// Share closes the panel first, then the share menu hangs from it, in the browser window.
#[gpui::test]
fn share_hangs_its_menu_from_its_tile(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(SHARE);
    panel.ran("share 98 101 37 32");
}

/// Without a share sheet, as on Linux, there's no Share tile to do nothing: the row is four places
/// of 48, the captures closing up from the left and the shield after them.
#[gpui::test]
fn share_is_left_out_where_the_system_cant_share(cx: &mut TestAppContext) {
    let host = Recorder {
        no_share_sheet: true,
        ..Recorder::default()
    };
    let blocking = Site {
        ad_blocking: Some(true),
        ..site()
    };
    let mut panel = Panel::on(host, cx, under_button(), window(), blocking);
    panel.calls();
    for x in [34., 90., 146., 202.] {
        panel.hover((x, 38.5));
    }
    assert_eq!(
        panel.host.tips.take(),
        [
            "Capture a Portion of This Page at 122 133",
            "Capture Full Page at 178 133",
            "Ads and trackers blocked at 234 133",
            "none"
        ]
    );
    panel.click((90., 38.5));
    panel.ran("CaptureFullPage");
}

/// The capture tiles close the panel first, so the picture has none of it, then ask for what
/// Capture's commands do.
#[gpui::test]
fn the_capture_tile_closes_the_panel_and_starts_a_capture(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(CAPTURE);
    panel.ran("CapturePortion");
}

#[gpui::test]
fn the_full_page_tile_closes_the_panel_and_captures_the_page(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(CAPTURE_FULL_PAGE);
    panel.ran("CaptureFullPage");
}

#[gpui::test]
fn a_tile_names_itself_in_a_tooltip(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.hover(SHARE);
    panel.hover(EXTENSION);
    panel.hover((120., 60.));
    // Under each tile's middle in the browser window, and Share's leave, after Notes came, leaves
    // Notes'.
    let tips = panel.host.tips.take();
    assert_eq!(tips, ["Share at 116.5 133", "Notes at 122 202", "none"]);
}

/// Its popup hangs from the tile, in the browser window.
#[gpui::test]
fn an_extension_runs_as_its_toolbar_button(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(EXTENSION);
    panel.ran("extension abc 98 170 48 32");
}

/// A right-click on a tile gives its menu: Move to Top, which the first tile has done, then the
/// pinned action's items. Pinning keeps the panel, whose grid the core redraws.
#[gpui::test]
fn a_right_click_on_a_tile_gives_its_menu(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    let each = |id: &str, pinned: bool| Extension {
        id: id.to_string().into(),
        pinned,
        ..extension()
    };
    let blocker = Extension {
        options: false,
        removable: false,
        ..each("def", false)
    };
    panel
        .cx
        .cx
        .update(|cx| set_extensions(TAB, vec![each("abc", true), blocker], cx));
    panel.cx.run_until_parked();
    panel.calls();
    panel.host.answer.set(Some(2));
    panel.right_click(EXTENSION);
    assert_eq!(
        panel.calls(),
        [
            "menu Move to Top, , Unpin, Options, Remove, Manage Extension",
            "run extension abc Unpin 7"
        ]
    );
    panel.host.answer.set(Some(0));
    panel.right_click(ADD);
    assert_eq!(
        panel.calls(),
        [
            "menu Move to Top, , Pin, Manage Extension",
            "run extension def MoveToTop 7"
        ]
    );
}

/// Remove closes the panel for the browser window's dialog, which asks first.
#[gpui::test]
fn a_tiles_remove_closes_the_panel_to_ask(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.host.answer.set(Some(4));
    panel.right_click(EXTENSION);
    let calls = panel.calls();
    assert_eq!(
        calls[0],
        "menu Move to Top, , Pin, Options, Remove, Manage Extension"
    );
    assert_eq!(
        calls[1..],
        ["release", "stop", "run extension abc Remove 7", "hide"]
    );
}

/// The core reports the unpinned extensions once the panel opens, and the grid takes them, four to
/// a row with the + tile last, the panel growing by a row for the fifth tile.
#[gpui::test]
fn the_grid_takes_the_extensions_the_core_reports_once_open(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    let shown = panel
        .calls()
        .into_iter()
        .find(|call| call.starts_with("show "))
        .unwrap();
    let height: f32 = shown.rsplit(' ').next().unwrap().parse().unwrap();
    let each = |id: &str| Extension {
        id: id.to_string().into(),
        ..extension()
    };
    let four = vec![each("abc"), each("def"), each("ghi"), each("jkl")];
    panel.cx.cx.update(|cx| set_extensions(TAB, four, cx));
    panel.cx.run_until_parked();
    assert_eq!(
        panel.calls(),
        [format!("height {}", height + TILE_HEIGHT + GAP)]
    );
    // The second's tile is where + was, and runs it.
    panel.click(ADD);
    panel.ran("extension def 154 170 48 32");
}

/// Another tab's extensions don't reach the panel.
#[gpui::test]
fn the_grid_takes_only_its_tabs_extensions(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    let other = Extension {
        id: "def".into(),
        ..extension()
    };
    panel
        .cx
        .cx
        .update(|cx| set_extensions(TAB + 1, vec![other.clone(), other], cx));
    panel.cx.run_until_parked();
    assert_eq!(panel.calls(), Vec::<String>::new());
    panel.click(ADD);
    panel.ran("ManageExtensions");
}

/// The browser has no add path of its own, so + opens Chrome's extensions page, which links the
/// store.
#[gpui::test]
fn the_add_tile_opens_the_extensions_page(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(ADD);
    panel.ran("ManageExtensions");
}

/// "Manage" shows on the Extensions heading under the pointer, and opens the extensions page.
#[gpui::test]
fn manage_shows_on_the_extensions_heading(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(MANAGE);
    assert!(panel.calls().is_empty());
    panel.hover(EXTENSIONS_HEADING);
    panel.click(MANAGE);
    panel.ran("ManageExtensions");
}

/// "More" shows on the "This site" heading under the pointer, and opens Chrome's settings for the
/// site.
#[gpui::test]
fn more_shows_on_the_site_heading(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(MORE_LINK);
    assert!(panel.calls().is_empty());
    panel.hover(SITE_HEADING);
    // Not the other heading's.
    panel.click(MANAGE);
    assert!(panel.calls().is_empty());
    panel.hover(SITE_HEADING);
    panel.click(MORE_LINK);
    panel.ran("site settings");
}

#[gpui::test]
fn a_value_chosen_sets_the_permission(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.host.answer.set(Some(1));
    panel.click(line(1));
    let set = format!("set {TAB} {WINDOWS} Block");
    assert_eq!(panel.calls(), ["menu Allow*, Block, Ask", &set]);
    // The value it has sets nothing, nor does a menu closed without a choice.
    panel.host.answer.set(Some(0));
    panel.click(line(1));
    panel.host.answer.set(None);
    panel.click(line(1));
    assert_eq!(
        panel.calls(),
        ["menu Allow*, Block, Ask", "menu Allow*, Block, Ask"]
    );
}

fn device(name: &str, removable: bool) -> Device {
    Device {
        name: name.to_owned().into(),
        kind: "Serial port".into(),
        glyph: DeviceKind::Serial,
        removable,
    }
}

/// A device the site was given follows its permissions, and its menu's Remove takes it away; a
/// menu closed without a choice keeps it.
#[gpui::test]
fn a_devices_remove_takes_it_away(cx: &mut TestAppContext) {
    let mut site = site();
    site.devices = vec![device("Arduino Uno", true)];
    let mut panel = Panel::new(cx, site);
    panel.calls();
    panel.host.answer.set(None);
    panel.click(line(3));
    assert_eq!(panel.calls(), ["menu Remove"]);
    panel.host.answer.set(Some(0));
    panel.click(line(3));
    assert_eq!(
        panel.calls(),
        ["menu Remove", &format!("remove device {TAB} 0")]
    );
}

/// One a policy gave isn't the user's to take away, so it has no menu, and no stop for the keys.
#[gpui::test]
fn a_policys_device_stays(cx: &mut TestAppContext) {
    let mut site = site();
    site.devices = vec![device("Arduino Uno", false), device("Pico", true)];
    let mut panel = Panel::new(cx, site);
    panel.calls();
    panel.host.answer.set(Some(0));
    panel.click(line(3));
    assert_eq!(panel.calls(), Vec::<String>::new());
    panel.click(line(4));
    assert_eq!(
        panel.calls(),
        ["menu Remove", &format!("remove device {TAB} 1")]
    );
}

/// A site kept for Safari's engine has its switch, a strip alone, and its menu's Off turns it off;
/// On, which it is, does nothing.
#[gpui::test]
fn the_engine_switch_turns_safaris_engine_off(cx: &mut TestAppContext) {
    let mut panel = Panel::new(
        cx,
        Site {
            system_engine: Some(true),
            ..site()
        },
    );
    panel.calls();
    panel.host.answer.set(Some(0));
    panel.click(STRIP);
    assert_eq!(panel.calls(), ["menu On*, Off"]);
    panel.host.answer.set(Some(1));
    panel.click(STRIP);
    assert_eq!(
        panel.calls(),
        ["menu On*, Off", &format!("system engine {TAB} false")]
    );
    // The lines follow, under the strip.
    panel.host.answer.set(Some(1));
    panel.click(line_under_strip(1));
    assert_eq!(
        panel.calls(),
        [
            "menu Allow*, Block, Ask",
            &format!("set {TAB} {WINDOWS} Block")
        ]
    );
}

/// A site the core offers Safari's engine for, as a page asked for what it plays, or the user closed
/// the site's note or turned it off, lists it off, and its menu's On turns it on, as the note's button does; Off,
/// which it is, does nothing. The core then reports the site kept, and the switch says On.
#[gpui::test]
fn the_engine_switch_offers_safaris_engine(cx: &mut TestAppContext) {
    let mut panel = Panel::new(
        cx,
        Site {
            system_engine: Some(false),
            ..site()
        },
    );
    panel.calls();
    panel.host.answer.set(Some(1));
    panel.click(STRIP);
    assert_eq!(panel.calls(), ["menu On, Off*"]);
    panel.host.answer.set(Some(0));
    panel.click(STRIP);
    assert_eq!(
        panel.calls(),
        ["menu On, Off*", &format!("system engine {TAB} true")]
    );
    panel.changed(Site {
        system_engine: Some(true),
        ..site()
    });
    panel.calls();
    panel.host.answer.set(None);
    panel.click(STRIP);
    assert_eq!(panel.calls(), ["menu On*, Off"]);
}

/// A site turned off stays offered, as the core reports it after Off, so On brings it back.
#[gpui::test]
fn after_off_the_engine_switch_offers_on(cx: &mut TestAppContext) {
    let mut panel = Panel::new(
        cx,
        Site {
            system_engine: Some(true),
            ..site()
        },
    );
    panel.calls();
    panel.host.answer.set(Some(1));
    panel.click(STRIP);
    assert_eq!(
        panel.calls(),
        ["menu On*, Off", &format!("system engine {TAB} false")]
    );
    panel.changed(Site {
        system_engine: Some(false),
        ..site()
    });
    panel.calls();
    panel.host.answer.set(Some(0));
    panel.click(STRIP);
    assert_eq!(
        panel.calls(),
        ["menu On, Off*", &format!("system engine {TAB} true")]
    );
}

/// A site the core neither keeps nor offers Safari's engine for has no switch for it: its first line
/// is the first permission's.
#[gpui::test]
fn without_the_offer_there_is_no_engine_switch(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.host.answer.set(None);
    panel.click(line(0));
    assert_eq!(panel.calls(), ["menu Allow, Block, Ask*"]);
}

/// A site with logins has a line for them over the permissions, under the engine's strip; Remove closes the
/// panel for the browser window to ask, Never's way back acts at once, and the last item opens them
/// all in Settings.
#[gpui::test]
fn the_passwords_line_removes_a_login_or_takes_back_never(cx: &mut TestAppContext) {
    let logins = vec![login("me", false), login("", true)];
    let mut panel = Panel::new(
        cx,
        Site {
            system_engine: Some(true),
            logins,
            ..site()
        },
    );
    panel.calls();
    let menu = "menu Remove “me”…, Offer to Save Passwords, , All Passwords…";
    panel.host.answer.set(Some(1));
    panel.click(line_under_strip(0));
    assert_eq!(
        panel.calls(),
        [menu, &format!("allow saving {TAB} example.com")]
    );
    panel.host.answer.set(Some(0));
    panel.click(line_under_strip(0));
    let remove = format!("run remove login example.com me {TAB}");
    assert_eq!(panel.calls(), [menu, "release", "stop", &remove, "hide"]);
    panel.show(under_button(), window());
    panel.changed(Site {
        logins: vec![login("me", false)],
        devices: Vec::new(),
        ..site()
    });
    panel.calls();
    panel.host.answer.set(Some(2));
    panel.click(line(0));
    let menu = "menu Remove “me”…, , All Passwords…";
    assert_eq!(
        panel.calls(),
        [
            menu,
            "release",
            "stop",
            &format!("run passwords {TAB}"),
            "hide"
        ]
    );
}

/// The line says what the site keeps and pushes the permissions a line down; with none it isn't
/// there.
#[gpui::test]
fn the_passwords_line_shows_only_for_a_site_with_logins(cx: &mut TestAppContext) {
    let plain = Panel::new(cx, site()).calls();
    let mut panel = Panel::new(
        cx,
        Site {
            logins: vec![login("me", false)],
            devices: Vec::new(),
            ..site()
        },
    );
    let with = panel.calls();
    let height =
        |calls: &[String]| -> f32 { calls[0].rsplit(' ').next().unwrap().parse().unwrap() };
    assert_eq!(height(&with) - height(&plain), ITEM_PITCH);
    panel.host.answer.set(Some(1));
    panel.click(line(2));
    assert_eq!(
        panel.calls(),
        [
            "menu Allow*, Block, Ask",
            &format!("set {TAB} {WINDOWS} Block")
        ]
    );
}

/// With every part: the shield last in the action row, Safari's engine's strip, then the lines,
/// Passwords, the permissions, then Reset, the panel a strip and a line taller than without.
#[gpui::test]
fn the_shield_the_switch_then_the_lines(cx: &mut TestAppContext) {
    let logins = vec![login("me", false)];
    let site = Site {
        system_engine: Some(true),
        ad_blocking: Some(true),
        logins,
        ..site()
    };
    let mut panel = Panel::new(cx, site);
    assert_eq!(panel.calls(), ["show 88 78.5 236 408.5"]);
    panel.host.answer.set(None);
    panel.click(SHIELD);
    assert_eq!(panel.calls(), [format!("ad blocking {TAB} false")]);
    panel.click(STRIP);
    assert_eq!(panel.calls(), ["menu On*, Off"]);
    panel.click(line_under_strip(0));
    assert_eq!(panel.calls(), ["menu Remove “me”…, , All Passwords…"]);
    panel.click(line_under_strip(1));
    assert_eq!(panel.calls(), ["menu Allow, Block, Ask*"]);
    panel.click(line_under_strip(2));
    assert_eq!(panel.calls(), ["menu Allow*, Block, Ask"]);
}

/// Two switches or more are tiles two to a line, a glyph over its name, the third keeping its
/// tile's size on a line of its own; each turns its setting as a strip does.
#[gpui::test]
fn two_switches_or_more_are_tiles(cx: &mut TestAppContext) {
    let site = Site {
        developer_mode: Some(true),
        system_engine: Some(false),
        boostable: true,
        boost: Some(true),
        ..site()
    };
    let at = layout(&site, 1, true);
    // Two lines of 52 under the action row, 8 apart, and the gap over them.
    assert_eq!(
        at.extensions_cap - layout(&super::tests::site(), 1, true).extensions_cap,
        120.
    );
    let mut panel = Panel::new(cx, site);
    panel.calls();
    panel.click((62., 88.5));
    assert_eq!(panel.calls(), [format!("developer mode {TAB} false")]);
    panel.host.answer.set(None);
    panel.click((174., 88.5));
    assert_eq!(panel.calls(), ["menu On, Off*"]);
    panel.click((62., 148.5));
    assert_eq!(panel.calls(), [format!("boost {TAB} false")]);
    // The third's line has no second tile.
    panel.click((174., 148.5));
    assert!(panel.calls().is_empty());
}

/// A value a policy set shows, but isn't the user's to change.
#[gpui::test]
fn a_managed_value_has_no_menu(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.host.answer.set(Some(1));
    panel.click(line(2));
    assert!(panel.calls().is_empty());
}

#[gpui::test]
fn reset_sets_the_permissions_back(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(line(3));
    assert_eq!(panel.calls(), [format!("reset {TAB}")]);
}

/// Arc's "…" menu without Boost's items, each the browser's own command.
#[gpui::test]
fn the_more_menu_runs_what_it_lists(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    let menu = "menu Clear Cache, Clear Cookies, , Manage Extensions…, Add Extension…, , \
                All Site Settings…";
    for (index, run) in [
        (0, "ClearCacheAndRefresh"),
        (1, "ClearCookiesAndRefresh"),
        (3, "ManageExtensions"),
        (4, "ManageExtensions"),
        (6, "site settings"),
    ] {
        panel.host.answer.set(Some(index));
        panel.click(MORE);
        assert_eq!(
            panel.calls(),
            [menu, "release", "stop", &format!("run {run} {TAB}"), "hide"]
        );
        panel.show(under_button(), window());
        panel.changed(site());
        panel.calls();
    }
    // Closed without a choice, it leaves the panel be.
    panel.host.answer.set(None);
    panel.click(MORE);
    assert_eq!(panel.calls(), [menu]);
}

/// The tab's profile while the user has two or more, and the others: Work has a Space, Side
/// none (#2339).
fn profiles() -> Profiles {
    Profiles {
        name: "Personal".into(),
        others: vec![(2, "Work".into(), true), (3, "Side".into(), false)],
    }
}

/// The profile's chip, against "…": the middle of its last 32 pt, the chevron's end.
const PROFILE: (f32, f32) = (170., LOCK.1);

fn profiled(cx: &mut TestAppContext, site: Site) -> Panel {
    let host = Recorder::default();
    let mut panel = Panel::profiled(host, cx, under_button(), window(), site, Some(profiles()));
    panel.calls();
    panel
}

/// The chip's menu opens the page in another profile's Space, or New Space in a profile with
/// none, and closes the panel as the shell does either.
#[gpui::test]
fn the_profile_chip_opens_the_page_in_another_profile(cx: &mut TestAppContext) {
    let mut panel = profiled(cx, site());
    panel.host.answer.set(Some(0));
    panel.click(PROFILE);
    let calls = panel.calls();
    assert_eq!(calls[0], "menu Open in Work, New Space in Side…");
    assert_eq!(
        calls[1..],
        [
            "release",
            "stop",
            &format!("run open in profile 2 {TAB}"),
            "hide"
        ]
    );
    let mut panel = profiled(cx, site());
    panel.host.answer.set(Some(1));
    panel.click(PROFILE);
    let calls = panel.calls();
    assert_eq!(
        calls[1..],
        [
            "release",
            "stop",
            &format!("run new space in profile 3 {TAB}"),
            "hide"
        ]
    );
}

/// Beside the chip only a secure page's lock loses its words, to its glyph and a tooltip. Any
/// other lock keeps them, and the chip gives way instead: its name cuts at its end, and with
/// too little room goes, named in its tooltip whenever it's cut.
#[gpui::test]
fn beside_the_chip_only_a_secure_lock_loses_its_words(cx: &mut TestAppContext) {
    let tip = |panel: &mut Panel, at| {
        panel.hover(at);
        let tips = panel.host.tips.take();
        tips.into_iter().find(|tip| !tip.starts_with("none"))
    };
    let mut panel = profiled(cx, site());
    assert!(tip(&mut panel, LOCK).is_some_and(|tip| tip.starts_with("Secure at ")));
    assert_eq!(tip(&mut panel, PROFILE), None, "the whole name shows");
    let not_secure = Site {
        connection: Connection::NotSecure,
        connection_summary: "Not secure".into(),
        certificate: false,
        ..site()
    };
    // Left of the chip's glyph and chevron, where only its name is.
    let name = (PROFILE.0 - 60., PROFILE.1);
    let mut panel = profiled(cx, not_secure);
    assert_eq!(tip(&mut panel, LOCK), None, "the lock keeps its words");
    assert!(tip(&mut panel, PROFILE).is_some_and(|tip| tip.starts_with("Personal at ")));
    assert_eq!(
        tip(&mut panel, name),
        None,
        "too little room: the name goes"
    );
    // The browser's own page names itself in fewer words, which leaves the name some room: in
    // the tests' face, not all of it.
    let own = Site {
        site: "ilo".into(),
        connection: Connection::Internal,
        certificate: false,
        ..site()
    };
    let mut panel = profiled(cx, own);
    assert_eq!(tip(&mut panel, LOCK), None);
    assert!(tip(&mut panel, name).is_some_and(|tip| tip.starts_with("Personal at ")));
}

/// The keyboard reaches the chip after the lock, before "…", and space opens its menu.
#[gpui::test]
fn the_keyboard_reaches_the_profile_chip_before_more(cx: &mut TestAppContext) {
    let mut panel = profiled(cx, site());
    panel.keys("shift-tab shift-tab");
    panel.host.tips.take();
    panel.keys("space");
    assert_eq!(panel.calls()[0], "menu Open in Work, New Space in Side…");
}

/// With one profile, or in a window with a Space of its own, the shell gives none, and the
/// footer is the lock in words and "…" as before.
#[gpui::test]
fn with_one_profile_there_is_no_chip(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(PROFILE);
    assert_eq!(panel.calls(), Vec::<String>::new());
    // "Secure" stays in words, with no tooltip standing in for them.
    panel.hover(LOCK);
    let tips = panel.host.tips.take();
    assert!(tips.iter().all(|tip| tip.starts_with("none")), "{tips:?}");
}

#[gpui::test]
fn the_lock_shows_the_certificate(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(LOCK);
    panel.ran("certificate");
}

/// A page with no certificate, as an http one, has nothing to show.
#[gpui::test]
fn without_a_certificate_the_lock_does_nothing(cx: &mut TestAppContext) {
    let plain = Site {
        connection: Connection::NotSecure,
        certificate: false,
        ..site()
    };
    let mut panel = Panel::new(cx, plain.clone());
    panel.calls();
    panel.click(LOCK);
    assert!(panel.calls().is_empty());
    assert_eq!(lock_label(&plain), "Not secure");
}

/// A change the core reports, such as a reset that leaves nothing to reset, resizes the panel.
#[gpui::test]
fn a_change_resizes_it(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.changed(Site {
        reset: None,
        ..site()
    });
    assert_eq!(panel.calls(), ["height 308.5"]);
    assert_eq!(panel.host.corners.take()[3], (WIDTH, 308.5, RADIUS));
    // A report for another tab isn't this panel's.
    panel.cx.cx.update(|cx| changed(TAB + 1, site(), cx));
    assert!(panel.calls().is_empty());
}

/// With nothing to list, the "This site" heading goes, and the rule follows the extensions.
#[gpui::test]
fn without_lines_there_is_no_site_heading(cx: &mut TestAppContext) {
    let bare = Site {
        permissions: Vec::new(),
        reset: None,
        ..site()
    };
    let mut panel = Panel::new(cx, bare);
    assert_eq!(panel.calls(), ["show 88 78.5 236 191.5"]);
}

/// The shield, last in the action row, lets ads and trackers load on the site at a click, and
/// blocks them again at the next; the core reloads the page and reports the site again. A plain
/// site's panel has no switch, so the lines start under the extensions.
#[gpui::test]
fn the_shield_turns_ad_blocking_off_and_on_for_the_site(cx: &mut TestAppContext) {
    let mut panel = Panel::new(
        cx,
        Site {
            ad_blocking: Some(true),
            ..site()
        },
    );
    assert_eq!(panel.calls(), ["show 88 78.5 236 338.5"]);
    panel.hover(SHIELD);
    assert_eq!(
        panel.host.tips.take(),
        ["Ads and trackers blocked at 250.5 133"]
    );
    panel.click(SHIELD);
    assert_eq!(panel.calls(), [format!("ad blocking {TAB} false")]);
    panel.changed(Site {
        ad_blocking: Some(false),
        ..site()
    });
    panel.click(SHIELD);
    assert_eq!(panel.calls(), [format!("ad blocking {TAB} true")]);
    panel.host.answer.set(Some(1));
    panel.click(line(1));
    assert_eq!(
        panel.calls(),
        [
            "menu Allow*, Block, Ask",
            &format!("set {TAB} {WINDOWS} Block")
        ]
    );
}

/// The places the panel logs for the Mac checks take their clicks: the tiles, the shield, each
/// switch, each line, the lock and "…".
#[gpui::test]
fn a_click_at_each_logged_place_lands_on_its_part(cx: &mut TestAppContext) {
    let busy = Site {
        ad_blocking: Some(true),
        developer_mode: Some(true),
        ..site()
    };
    let mut panel = Panel::new(cx, busy.clone());
    panel.calls();
    let shield = panel.placed("shield");
    panel.click(shield);
    assert_eq!(panel.calls(), [format!("ad blocking {TAB} false")]);
    let switch = panel.placed("switch Developer Mode");
    panel.click(switch);
    assert_eq!(panel.calls(), [format!("developer mode {TAB} false")]);
    panel.host.answer.set(Some(1));
    let line = panel.placed("line Window management");
    panel.click(line);
    assert_eq!(
        panel.calls(),
        [
            "menu Allow*, Block, Ask",
            &format!("set {TAB} {WINDOWS} Block")
        ]
    );
    let reset = panel.placed("line Reset permissions");
    panel.click(reset);
    assert_eq!(panel.calls(), [format!("reset {TAB}")]);
    panel.host.answer.set(None);
    let more = panel.placed("more");
    panel.click(more);
    assert!(panel.calls()[0].starts_with("menu Clear Cache, "));
    let lock = panel.placed("lock");
    panel.click(lock);
    panel.ran("certificate");
    panel.show(under_button(), window());
    assert_eq!(panel.calls(), [format!("read {TAB}")]);
    panel.changed(busy);
    panel.calls();
    let share = panel.placed("tile share");
    panel.click(share);
    assert_eq!(panel.calls()[..2], ["release", "stop"]);
}

/// Developer Mode has its switch only while it's on, which turns it off; off, "…" turns it on, as
/// the command bar does (#2334).
#[gpui::test]
fn developer_mode_s_switch_shows_while_it_s_on(cx: &mut TestAppContext) {
    let site = Site {
        ad_blocking: Some(true),
        developer_mode: Some(true),
        ..site()
    };
    let mut panel = Panel::new(cx, site.clone());
    assert_eq!(panel.calls(), ["show 88 78.5 236 378.5"]);
    panel.click(STRIP);
    assert_eq!(panel.calls(), [format!("developer mode {TAB} false")]);
    let off = Site {
        developer_mode: Some(false),
        ..site
    };
    panel.changed(off.clone());
    assert_eq!(panel.calls(), ["height 338.5"]);
    let menu = "menu Clear Cache, Clear Cookies, Turn On Developer Mode, , Manage Extensions…, \
                Add Extension…, , All Site Settings…";
    panel.host.answer.set(Some(2));
    panel.click(MORE);
    assert_eq!(
        panel.calls(),
        [
            menu,
            "release",
            "stop",
            &format!("run ToggleDeveloperMode {TAB}"),
            "hide"
        ]
    );
}

/// Where it isn't the user's to change, such as a private window, there's no shield: its place in
/// the row is empty.
#[gpui::test]
fn without_a_say_there_is_no_shield(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.click(SHIELD);
    assert!(panel.calls().is_empty());
}

#[gpui::test]
fn the_log_names_each_permission_and_its_value(_: &mut TestAppContext) {
    let described = describe(&site());
    let permissions = "Location Ask, Window management Allow, JavaScript Allow";
    assert_eq!(
        described,
        format!("example.com: Connection is secure; {permissions}")
    );
    let bare = Site {
        permissions: Vec::new(),
        reset: None,
        ..site()
    };
    assert_eq!(
        describe(&bare),
        "example.com: Connection is secure; no permissions"
    );
    // Ad blocking first, which check.sh reads, and Developer Mode next.
    let blocking = Site {
        ad_blocking: Some(false),
        ..bare.clone()
    };
    assert_eq!(
        describe(&blocking),
        "example.com: Connection is secure; Ad blocking Off"
    );
    let developer = Site {
        developer_mode: Some(true),
        ..blocking
    };
    let described = "example.com: Connection is secure; Ad blocking Off, Developer Mode On";
    assert_eq!(describe(&developer), described);
}

#[gpui::test]
fn site_settings_are_for_the_pages_origin(_: &mut TestAppContext) {
    let settings = "chrome://settings/content/siteDetails?site=";
    let url = site_settings_url("https://me:secret@example.com:8443/path?q=1#top");
    assert_eq!(
        url,
        Some(format!("{settings}https%3A%2F%2Fexample.com%3A8443"))
    );
    let url = site_settings_url("http://localhost/");
    assert_eq!(url, Some(format!("{settings}http%3A%2F%2Flocalhost")));
    assert_eq!(site_settings_url("chrome://settings"), None);
    assert_eq!(site_settings_url("file:///Users/me/a.html"), None);
}

#[gpui::test]
fn each_permission_has_its_glyph(_: &mut TestAppContext) {
    assert_eq!(glyph_for("geolocation"), "icons/map-pin.svg");
    assert_eq!(glyph_for("media-stream-camera"), "icons/video.svg");
    assert_eq!(glyph_for("window-placement"), "icons/app-window.svg");
    // A type Chrome adds later.
    assert_eq!(glyph_for("something-new"), "icons/settings.svg");
}

impl Panel {
    /// What the keyboard's ring is on, and the tooltip last asked for.
    fn keyed(&mut self) -> (Option<Stop>, Option<String>) {
        let keyed = self.cx.cx.update(|cx| {
            let window = cx.global::<SiteControlsWindow>().0;
            window
                .update(cx, |panel, _, _| panel.open.as_ref()?.keyed.clone())
                .ok()
                .flatten()
        });
        (keyed, self.host.tips.borrow().last().cloned())
    }
}

/// Tab goes down the tiles, the extensions' tiles and +, the lines a click changes, past one a
/// policy sets, and the footer's buttons, round from the last to the first and back with ⇧Tab;
/// the arrows go the same way and stop at the ends. A tile, which is a glyph alone, is named by
/// its tooltip (#1382).
#[gpui::test]
fn tab_and_the_arrows_go_down_the_panel(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    assert_eq!(panel.keyed().0, None, "a ring before a key");
    panel.keys("tab");
    let (keyed, tip) = panel.keyed();
    assert_eq!(keyed, Some(Stop::Tile("share")));
    assert!(tip.is_some_and(|tip| tip.starts_with("Share at ")));
    panel.keys("tab tab tab");
    assert_eq!(
        panel.keyed(),
        (
            Some(Stop::Extension("abc".into())),
            Some("Notes at 122 202".into())
        )
    );
    panel.keys("tab");
    assert_eq!(panel.keyed().0, Some(Stop::AddExtension));
    panel.keys("tab");
    // The lines take it without a tooltip, which goes.
    assert_eq!(
        panel.keyed(),
        (Some(Stop::Permission(5)), Some("none".into()))
    );
    panel.keys("tab");
    assert_eq!(panel.keyed().0, Some(Stop::Permission(WINDOWS)));
    panel.keys("tab");
    assert_eq!(
        panel.keyed().0,
        Some(Stop::Reset),
        "the ring stopped on a line a policy sets"
    );
    panel.keys("tab tab");
    assert_eq!(panel.keyed().0, Some(Stop::More));
    panel.keys("down");
    assert_eq!(panel.keyed().0, Some(Stop::More), "↓ went past the last");
    panel.keys("tab");
    assert_eq!(
        panel.keyed().0,
        Some(Stop::Tile("share")),
        "Tab didn't go round"
    );
    panel.keys("up");
    assert_eq!(
        panel.keyed().0,
        Some(Stop::Tile("share")),
        "↑ went past the first"
    );
    panel.keys("shift-tab");
    assert_eq!(panel.keyed().0, Some(Stop::More), "⇧Tab didn't go round");
    panel.keys("up");
    assert_eq!(panel.keyed().0, Some(Stop::Lock));
    assert!(panel.calls().is_empty(), "moving the ring did something");
}

/// Space or return does what a click on the ringed stop does: a line's menu under it, a toggle,
/// Reset, or a tile's or the lock's run, which closes the panel. The shield comes after the action
/// tiles, named by its tooltip, and the switches after it.
#[gpui::test]
fn space_and_return_act_on_the_ringed_stop(cx: &mut TestAppContext) {
    let site = Site {
        ad_blocking: Some(true),
        developer_mode: Some(true),
        ..site()
    };
    let mut panel = Panel::new(cx, site);
    panel.calls();
    panel.host.answer.set(Some(1));
    panel.keys("tab tab tab tab");
    assert_eq!(
        panel.keyed(),
        (
            Some(Stop::AdBlocking),
            Some("Ads and trackers blocked at 250.5 133".into())
        )
    );
    panel.keys("space");
    assert_eq!(panel.calls(), [format!("ad blocking {TAB} false")]);
    panel.keys("down");
    assert_eq!(panel.keyed().0, Some(Stop::DeveloperMode));
    panel.keys("enter");
    assert_eq!(panel.calls(), [format!("developer mode {TAB} false")]);
    // Past the extension's tile and +, Location's line.
    panel.keys("down down down enter");
    assert_eq!(
        panel.calls(),
        [
            "menu Allow, Block, Ask*".to_owned(),
            format!("set {TAB} 5 Block")
        ]
    );
    panel.keys("down down space");
    assert_eq!(panel.calls(), [format!("reset {TAB}")]);
    panel.keys("down enter");
    panel.ran("certificate");
}

/// The tiles run from the keyboard as from a click, and a click takes the ring away.
#[gpui::test]
fn a_tile_runs_from_the_keyboard_and_a_click_takes_the_ring_away(cx: &mut TestAppContext) {
    let mut panel = Panel::new(cx, site());
    panel.calls();
    panel.keys("tab");
    panel.click(SITE_HEADING);
    assert_eq!(panel.keyed().0, None, "a click left the ring");
    panel.keys("tab tab space");
    panel.ran("CapturePortion");
}

/// VoiceOver reads each part of the panel by what it does (#1381): the tiles, the shield and the
/// footer as buttons by their tooltips' names, the headings as headings, a switch as a switch, a
/// line with a menu as a button that says its value and opens one, and a line a policy sets as
/// dimmed. Its press on a line does what a click does, and its menu key on an extension gives the
/// tile's menu.
#[gpui::test]
fn voiceover_reads_the_panel(cx: &mut TestAppContext) {
    use gpui::accesskit::{Action, HasPopup, Node, NodeId, Role, Toggled};
    let site = Site {
        ad_blocking: Some(true),
        boostable: true,
        boost: Some(false),
        ..site()
    };
    let mut panel = Panel::new(cx, site);
    panel.cx.simulate_a11y_activation();
    panel.cx.run_until_parked();
    let tree = panel.cx.a11y_tree().expect("GPUI built a tree");
    let find = |role: Role, label: &str| -> (NodeId, Node) {
        let found = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == role && node.label() == Some(label));
        found
            .unwrap_or_else(|| panic!("no {role:?} called {label}"))
            .clone()
    };
    for tile in [
        "Share",
        "Capture a Portion of This Page",
        "Capture Full Page",
        "Add Extension",
    ] {
        find(Role::Button, tile);
    }
    let (notes, _) = find(Role::Button, "Notes");
    find(Role::Heading, "Extensions");
    find(Role::Heading, "This site");
    let (blocking, _) = find(Role::Button, "Ads and trackers blocked");
    let (_, boost) = find(Role::Switch, "Boost");
    assert_eq!(boost.toggled(), Some(Toggled::False));
    let (location, node) = find(Role::Button, "Location, Ask");
    assert_eq!(node.has_popup(), Some(HasPopup::Menu));
    assert!(!node.is_disabled());
    let (_, javascript) = find(Role::Button, "JavaScript, Allowed");
    assert!(javascript.is_disabled());
    find(Role::Button, "Reset permissions");
    let (_, lock) = find(Role::Button, "Secure");
    assert!(!lock.is_disabled());
    let (_, more) = find(Role::Button, "More");
    assert_eq!(more.has_popup(), Some(HasPopup::Menu));

    panel.cx.simulate_a11y_action(blocking, Action::Click);
    panel.cx.run_until_parked();
    assert!(panel.calls().contains(&format!("ad blocking {TAB} false")));
    let menu = |panel: &mut Panel| {
        let calls = panel.host.calls.take();
        calls
            .into_iter()
            .find(|call| call.starts_with("menu "))
            .unwrap_or_default()
    };
    panel.cx.simulate_a11y_action(location, Action::Click);
    panel.cx.run_until_parked();
    assert!(menu(&mut panel).contains("Allow"), "Location's menu");
    panel
        .cx
        .simulate_a11y_action(notes, Action::ShowContextMenu);
    panel.cx.run_until_parked();
    assert!(!menu(&mut panel).is_empty(), "the extension's menu");
}

/// Opened at a permission, as the URL field's bell and the command bar's Site Controls open it for a
/// held request to show notifications, its row is ringed once the core has read the site, the
/// first time the panel's window is made too; the next opening has no ring (#2323).
#[gpui::test]
fn opened_at_a_permission_its_row_is_ringed(cx: &mut TestAppContext) {
    const NOTIFICATIONS: i32 = 6;
    let host = Rc::new(Recorder::default());
    let opener = host.clone();
    let panel_window = cx.update(|cx| {
        crate::lazy_window::when_asked::<SiteControls>(
            move |cx| _ = open(opener, cx).expect("no window for the Site Controls"),
            cx,
        );
        open_at_permission("notifications", cx);
        cx.global::<SiteControlsWindow>().0
    });
    let cx = VisualTestContext::from_window(panel_window.into(), cx);
    cx.simulate_resize(size(px(WIDTH), px(400.)));
    let mut panel = Panel {
        cx,
        host,
        profiles: None,
    };
    let mut site = site();
    site.permissions.push(Permission {
        key: "notifications".into(),
        ..permission(NOTIFICATIONS, "Notifications", Value::Ask, &Value::ALL)
    });
    panel.show(under_button(), window());
    panel.changed(site.clone());
    assert_eq!(panel.keyed().0, Some(Stop::Permission(NOTIFICATIONS)));
    panel.show(under_button(), window());
    panel.show(under_button(), window());
    panel.changed(site);
    assert_eq!(panel.keyed().0, None, "the ring outlived its opening");
}
