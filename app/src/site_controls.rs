//! The Site Controls: the panel the URL field's Site Controls button opens, as Arc's Site
//! Control Center (docs/ux-spec.md › Extensions › Site Controls), in Arc's order (#4404): the
//! header is the site, its connection in quiet words, and "…", which holds the rest; the action
//! tiles; the site's plate, with whether ads and trackers are blocked there, its switches, the
//! permissions that differ and its saved logins, each changeable; the turned-on extensions; and a
//! foot only while there's a permission to reset. GPUI can't draw over the page's native view, so
//! it's a window of its own that the host shows over the browser window, as its child, with the
//! keyboard, as the command bar is, over a blur in its shape with a shadow. What it asks of its
//! window and the core goes through `Host`. Tab and the arrows take a ring over its buttons, tiles
//! and rows, in their order down the panel, and space or return does what a click on the ringed
//! one does (#1382).

use std::time::Instant;
use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    App, Bounds, ClickEvent, Context, FocusHandle, Global, Image, KeyBinding, MouseButton,
    MouseDownEvent, PathBuilder, Pixels, Point, Radians, SharedString, Size, Task, TitlebarOptions,
    Transformation, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions,
    accesskit::{HasPopup, Role, Toggled},
    actions, canvas, div, img, point,
    prelude::*,
    px, size, svg,
};
use std::f32::consts::FRAC_PI_2;

use crate::{
    dismissal::{self, By, Why},
    icon_picker,
    palette::{self, Float, Palette},
    passwords::{self, Login},
    shell::{Command, DeviceKind, MenuEntry, cubic_bezier, scaled::scaled},
    tokens::shell::{KEYED, KEYED_RING},
    tooltip,
};

actions!(
    site_controls,
    [Close, NextStop, PreviousStop, StopDown, StopUp, Press]
);

mod look;

// Arc 1.170.0 cap, in a dark Space, 1 pt a pixel, except where a comment says ours.
/// The body's width: a row of four tiles inside the padding.
pub const WIDTH: f32 = 4. * TILE_WIDTH + 3. * GAP + 2. * PADDING;
use crate::elevation::Level;
use crate::tokens::shell::extensions::ACTION_DIMMED;
use crate::tokens::site_controls::{
    BEAK, BEAK_FOOT_RADIUS, BEAK_TIP_RADIUS, BELOW_FIELD, CHEVRON, CHIP_GLYPH, CHIP_INSET,
    CONNECTION_GLYPH, CONNECTION_GLYPH_GAP, CONNECTION_LINE, CONNECTION_PAD, CONNECTION_SIZE,
    EXTENSION_ICON, FAVICON, FAVICON_GAP, FAVICON_GLYPH, FAVICON_PLATE, FAVICON_RADIUS, FOOT_ABOVE,
    FOOT_BELOW, FOOT_ROW, GAP, HEADER, HEADER_INSET, HEADER_TO_TILES, ITEM_GLYPH, ITEM_GLYPH_OFF,
    ITEM_INSET, ITEM_VALUE, LABEL_ABOVE, LABEL_BELOW, LABEL_LINE, MANAGED_OPACITY, MORE, MORE_FILL,
    MORE_GLYPH, NAME_LINE, NAME_SIZE, PADDING, PLATE, PLUS_GLYPH, PROFILE_CHEVRON,
    PROFILE_CHEVRON_GAP, PROFILE_NAME_MIN, RADIUS, ROW, SECONDARY, TEXT, TILE_HEIGHT, TILE_WIDTH,
    WINDOW_MARGIN,
};
use crate::tooltip::text_width;
/// Ours: the reset row's glyph, as Chrome's page info's button has none.
const RESET: &str = "icons/rotate-ccw-clock.svg";
/// Ours: the switch of a site the profile keeps for Safari's engine, as the URL field's mark for
/// it. Named for Safari, which people know, though the page plays here, in ilo's tab.
const ENGINE: &str = "icons/compass.svg";
const ENGINE_NAME: &str = "Safari's engine";
/// Ours: the ad blocking row's, which Arc's panel didn't have.
const SHIELD: &str = "icons/shield.svg";
/// The words table's SC3.
const AD_BLOCKING_NAME: &str = "Block ads and trackers";
/// Ours: the site's logins, in place of Chrome's key icon and its manage bubble.
const PASSWORDS: &str = "icons/key-round.svg";
/// Ours: Developer Mode's switch, as Arc's panel's isn't recorded, and its command's in the bar.
const DEVELOPER_MODE: &str = "icons/square-code.svg";
const DEVELOPER_MODE_NAME: &str = "Developer Mode";
/// The site's Boost (#695), with the tile's glyph.
const BOOST: &str = "icons/paintbrush.svg";
const BOOST_NAME: &str = "Boost";
const RECENT: &str = "icons/git-pull-request.svg";
const RECENT_NAME: &str = "Hover Card";
const ARRANGEMENT: &str = "icons/grid-2x2.svg";
const ARRANGEMENT_NAME: &str = "Arrangement";
const PASSWORDS_NAME: &str = "Passwords";
/// The words table's SC2, and the extensions' label.
const SITE_LABEL: &str = "This site";
/// The foot's row, the words table's SC4. The core still names it in Chrome's words, which its
/// logged place keeps for the checks.
const RESET_NAME: &str = "Reset Permissions";
const EXTENSIONS_LABEL: &str = "Extensions";
/// Ours: the extensions' rows are as far apart as their tiles.
const EXTENSIONS_PER_ROW: usize = 4;
/// Ours, as before: it opens in 0.15 s and closes in 0.1 s, growing from the beak's tip and
/// shrinking back into it, from and to this much of its size, as it fades.
const OPEN: f32 = 0.15;
const CLOSE: f32 = 0.1;
const FROM_SCALE: f32 = 0.9;

/// A value of a site's permission, in Chrome's page info's words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Value {
    Allow,
    Block,
    Ask,
}

impl Value {
    pub const ALL: [Value; 3] = [Value::Allow, Value::Block, Value::Ask];

    fn label(self) -> &'static str {
        match self {
            Value::Allow => "Allow",
            Value::Block => "Block",
            Value::Ask => "Ask",
        }
    }
}

/// A permission of the site, as Chrome's page info lists it.
#[derive(Clone, PartialEq, Debug)]
pub struct Permission {
    /// Chrome's ContentSettingsType for it, which the core takes back.
    pub kind: i32,
    /// Chrome's lasting name for it, such as "geolocation", which picks its glyph.
    pub key: SharedString,
    /// Chrome's name for it, such as "Location".
    pub name: SharedString,
    pub value: Value,
    /// Chrome's words for its value, such as "Allowed".
    pub state: SharedString,
    /// What the user may set it to, none for one that isn't theirs to change, as a policy's isn't.
    pub choices: Vec<Value>,
}

/// A device the site was given, as Chrome's page info lists it.
#[derive(Clone, PartialEq, Debug)]
pub struct Device {
    /// Chrome's name for it, such as "Arduino Uno".
    pub name: SharedString,
    /// Chrome's words for what it is, such as "Serial port".
    pub kind: SharedString,
    /// What it is, for its glyph.
    pub glyph: DeviceKind,
    /// Its Remove takes it away; not for one a policy gave.
    pub removable: bool,
}

/// What the connection to the page is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Connection {
    Secure,
    NotSecure,
    /// A file, or one of the browser's own pages.
    Internal,
}

/// The site of a tab's page, as the core reports it.
#[derive(Clone, PartialEq, Debug)]
pub struct Site {
    /// As the user should see it, such as "example.com".
    pub site: SharedString,
    pub connection: Connection,
    /// Chrome's words for the connection, such as "Connection is secure".
    pub connection_summary: SharedString,
    /// The page came with a certificate, which the lock button shows.
    pub certificate: bool,
    pub permissions: Vec<Permission>,
    /// Chrome's words for its button that sets the permissions back to their defaults, when it
    /// shows one.
    pub reset: Option<SharedString>,
    /// Its row for Safari's engine, which plays the protected video Chromium can't: on for a site
    /// the profile keeps for it, off for one it offers it for, where a page of the tab asked or the
    /// user closed the site's note, else none. The row turns it on or off.
    pub system_engine: Option<bool>,
    /// The logins the profile keeps for the site, and Never if it's set to it.
    pub logins: Vec<Login>,
    /// The devices the site was given, after the permissions, each with Remove (#1649).
    pub devices: Vec<Device>,
    /// Whether ads and trackers are blocked on the site, none where that isn't the user's to change
    /// there: a page that isn't http(s), one Safari's engine shows, a private window, or with
    /// blocking off in Settings.
    pub ad_blocking: Option<bool>,
    /// How many requests it blocked on the page, such as "12" or ">1k", empty while it blocked
    /// none or isn't on there.
    pub ad_blocked: SharedString,
    /// Whether the site's tabs show in Developer Mode, none where that isn't the user's to change:
    /// a page that isn't http(s), or a private window.
    pub developer_mode: Option<bool>,
    /// The page can have a Boost (#695): an http(s) page in a regular window, outside an agent's
    /// Space, with Settings' Boosts on. The Boost tile and the "…" menu's Boost items show then.
    pub boostable: bool,
    /// Whether the site's Boost is on, none where it has none.
    pub boost: Option<bool>,
    /// The Boost's arrangement (#2347): how many of its changes no longer fit, said after the
    /// Boost's state, and why the core keeps it off, which a row of its own says, whose click is
    /// Turn On. Each none where there's nothing to say.
    pub arrangement_misfits: Option<SharedString>,
    pub arrangement_off: Option<SharedString>,
    /// Whether a pinned tab's or favorite's card on the site shows what's recent there (#3074), on
    /// or not, none where it shows none: another site, or with Settings' switch off.
    pub recent: Option<bool>,
    /// Whether agents keep the site as a sensitive one in the tab's profile (#4369), which the "…"
    /// menu's mark says and turns, none where it can't be: a page that isn't http(s), or a
    /// private window.
    pub sensitive: Option<bool>,
}

/// A turned-on extension, as its action is for the tab's page.
#[derive(Clone)]
pub struct Extension {
    pub id: SharedString,
    pub title: SharedString,
    pub icon: Option<Arc<Image>>,
    /// It acts on the page. One that doesn't is dimmed, as in the toolbar.
    pub enabled: bool,
    /// Its action is pinned at the URL field's end, where the grid's first tiles come from.
    pub pinned: bool,
    /// It has an options page, which its menu opens.
    pub options: bool,
    /// The user may remove it, which the ad blocker the browser comes with isn't.
    pub removable: bool,
}

/// An item of an extension tile's menu, the pinned action's with Move to Top.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExtensionItem {
    /// Makes its action the first pinned, which pins it if it isn't, so its tile leads the grid.
    MoveToTop,
    Pin,
    Unpin,
    Options,
    /// Removes it, once the user confirms in the browser window's dialog.
    Remove,
    /// Its details on Chrome's extensions page.
    Manage,
}

impl ExtensionItem {
    /// For the logs.
    fn name(self) -> &'static str {
        match self {
            ExtensionItem::MoveToTop => "move-to-top",
            ExtensionItem::Pin => "pin",
            ExtensionItem::Unpin => "unpin",
            ExtensionItem::Options => "options",
            ExtensionItem::Remove => "remove",
            ExtensionItem::Manage => "manage",
        }
    }

    /// Whether the panel stays to show the grid it changes; the others close it for what they open.
    fn stays(self) -> bool {
        matches!(
            self,
            ExtensionItem::MoveToTop | ExtensionItem::Pin | ExtensionItem::Unpin
        )
    }
}

/// The menu of the extension at `index` in the grid: Move to Top, which the first tile has done,
/// then the pinned action's items, with Pin for an unpinned one.
fn extension_items(extension: &Extension, index: usize) -> Vec<(MenuEntry, Option<ExtensionItem>)> {
    let item = |title: &'static str, run: ExtensionItem| (MenuEntry::new(title, true), Some(run));
    let mut items = vec![
        (
            MenuEntry::new("Move to Top", index > 0),
            Some(ExtensionItem::MoveToTop),
        ),
        (MenuEntry::line(), None),
        match extension.pinned {
            true => item("Unpin", ExtensionItem::Unpin),
            false => item("Pin", ExtensionItem::Pin),
        },
    ];
    if extension.options {
        items.push(item("Options", ExtensionItem::Options));
    }
    if extension.removable {
        items.push((
            MenuEntry::new("Remove", true).destructive(),
            Some(ExtensionItem::Remove),
        ));
    }
    items.push(item("Manage Extension", ExtensionItem::Manage));
    items
}

/// What the panel hangs from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    /// Its button in the URL field: the beak at the button's middle, the body under the field.
    Button {
        field: Bounds<Pixels>,
        button: Bounds<Pixels>,
    },
    /// With the sidebar hidden, the page's top left corner, without a beak.
    Corner(Point<Pixels>),
}

/// What a click in the panel asks the host to do, once the panel has let go.
#[derive(Clone, PartialEq, Debug)]
pub enum Run {
    /// A shell command on the tab shown.
    Command(Command),
    /// The system's share menu for the page, hanging from this, in the browser window.
    Share(Bounds<Pixels>),
    /// Chrome's settings for the site.
    SiteSettings,
    /// The extension's action, as a click on its toolbar button, its popup hanging from this, its
    /// tile, in the browser window.
    Extension(SharedString, Bounds<Pixels>),
    /// The certificate the page came with, in the system's viewer.
    Certificate,
    /// Removes the login, once the user confirms in the browser window's dialog.
    RemoveLogin(Login),
    /// Every login of the tab's profile, in Settings.
    Passwords,
    /// An item of the extension's tile's menu.
    ExtensionItem(SharedString, ExtensionItem),
    /// The page in a Space of the profile, as a link from another app lands (#2339).
    OpenInProfile(u64),
    /// New Space's form, its Space in the profile, for one none of the user's Spaces is in.
    NewSpaceInProfile(u64),
    /// Marks the site sensitive for agents in the tab's profile, or with false takes it off.
    Sensitive(bool),
}

/// The tab's profile and the user's others, for the footer's chip, while there's more than one
/// (#2339). A private window has none, as its session shares none of the profile's logins.
#[derive(Clone, Debug, PartialEq)]
pub struct Profiles {
    pub name: SharedString,
    /// Each other profile: its id, its name, and whether one of the user's Spaces is in it.
    pub others: Vec<(u64, SharedString, bool)>,
}

/// An action tile. Arc's row is Share, Boost, Capture a portion and Capture the full page, each a
/// line in TILES.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tile {
    /// For the logs.
    pub name: &'static str,
    /// Its tooltip.
    pub label: &'static str,
    pub glyph: &'static str,
    /// The glyph's size, for its ink's measured size.
    pub glyph_size: f32,
    pub run: TileRun,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TileRun {
    Share,
    Command(Command),
}

/// Arc's order. The captures' glyphs are Lucide's frame and page and Boost's its paintbrush,
/// *interim*: Arc's aren't recorded.
pub const TILES: &[Tile] = &[
    Tile {
        name: "share",
        label: "Share",
        glyph: "icons/share.svg",
        glyph_size: 19.5,
        run: TileRun::Share,
    },
    // New Boost or Edit Boost, as the site has one: either opens its card.
    Tile {
        name: "boost",
        label: "Boost",
        glyph: "icons/paintbrush.svg",
        glyph_size: 19.5,
        run: TileRun::Command(Command::NewBoost),
    },
    Tile {
        name: "capture",
        label: "Capture a Portion of This Page",
        glyph: "icons/maximize.svg",
        glyph_size: 19.5,
        run: TileRun::Command(Command::CapturePortion),
    },
    Tile {
        name: "capture-full-page",
        label: "Capture Full Page",
        glyph: "icons/file.svg",
        glyph_size: 19.5,
        run: TileRun::Command(Command::CaptureFullPage),
    },
];

/// The Lucide glyph for a permission, by Chrome's name for its type.
pub fn glyph_for(key: &str) -> &'static str {
    match key {
        "geolocation" => "icons/map-pin.svg",
        "notifications" => "icons/bell.svg",
        "media-stream-camera" | "camera-pan-tilt-zoom" => "icons/video.svg",
        "media-stream-mic" => "icons/mic.svg",
        "javascript" => "icons/code.svg",
        "javascript-jit" | "javascript-optimizer" => "icons/zap.svg",
        "popups" => "icons/external-link.svg",
        "images" => "icons/image.svg",
        "cookies" | "storage-access" => "icons/cookie.svg",
        "automatic-downloads" => "icons/download.svg",
        "sound" => "icons/volume-2.svg",
        "clipboard" => "icons/clipboard.svg",
        "sensors" => "icons/activity.svg",
        "midi-sysex" => "icons/piano.svg",
        "usb-guard" => "icons/usb.svg",
        "serial-guard" => "icons/cable.svg",
        "hid-guard" => "icons/gamepad-2.svg",
        "bluetooth-guard" | "bluetooth-scanning" => "icons/bluetooth.svg",
        "nfc-devices" => "icons/nfc.svg",
        "window-placement" => "icons/app-window.svg",
        "local-fonts" => "icons/type.svg",
        "hand-tracking" => "icons/hand.svg",
        "ar" | "vr" => "icons/glasses.svg",
        "auto-picture-in-picture" => "icons/picture-in-picture-2.svg",
        "automatic-fullscreen" => "icons/maximize.svg",
        "keyboard-lock" => "icons/keyboard.svg",
        "pointer-lock" => "icons/mouse-pointer-2.svg",
        "payment-handler" => "icons/credit-card.svg",
        "web-printing" => "icons/printer.svg",
        "speaker-selection" => "icons/speaker.svg",
        "local-network-access" | "local-network" | "loopback-network" | "direct-sockets" => {
            "icons/network.svg"
        }
        "subresource-filter" => "icons/megaphone.svg",
        "file-system-write-guard" => "icons/file.svg",
        "durable-storage" => "icons/database.svg",
        "idle-detection" => "icons/clock.svg",
        _ => "icons/settings.svg",
    }
}

/// Chrome's settings page for the site of `url`, an http or https page's origin.
pub fn site_settings_url(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    if host.is_empty() {
        return None;
    }
    let origin = format!("{}://{host}", scheme.to_ascii_lowercase());
    let site: String = (origin.bytes())
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect();
    Some(format!("chrome://settings/content/siteDetails?site={site}"))
}

/// What the panel asks of its window and the core.
pub trait Host {
    /// Shows the panel's window over the browser window, as it draws now, with the keyboard:
    /// `size`, its top left at `at` in the window's content, in the shape of `corners` (x, y and
    /// radius from its top left, clockwise), which its blur and shadow take.
    fn show(&self, at: Point<Pixels>, size: Size<Pixels>, corners: &[(f32, f32, f32)]);
    /// Makes it `height` tall, keeping its top edge, in its new shape.
    fn set_height(&self, height: f32, corners: &[(f32, f32, f32)]);
    /// It drew a frame of its motion, at `scale` about `origin` in its window and `opacity`,
    /// which its blur and shadow follow.
    fn drew(&self, scale: f32, opacity: f32, origin: Point<Pixels>);
    /// It stops taking the keyboard and clicks, which the browser window gets back, as it leaves.
    fn release(&self);
    /// Hides its window, once it has left.
    fn hide(&self);
    /// Reads the site of `tab`'s page, which comes in `changed` and again as it changes, until
    /// `stop_reading`.
    fn read(&self, tab: u64);
    fn stop_reading(&self);
    /// Gives the site the permission of `kind` at `value`. The user's: agents get none.
    fn set_permission(&self, tab: u64, kind: i32, value: Value);
    /// Sets the site's permissions back to their defaults. The user's, as above.
    fn reset_permissions(&self, tab: u64);
    /// Takes the site's device at `index` away from it.
    fn remove_device(&self, tab: u64, index: usize);
    /// Has the profile keep the site for Safari's engine or not, and the tab show its page there or
    /// in Chromium. The user's, as above.
    fn set_system_engine(&self, tab: u64, on: bool);
    /// Takes the site, set to Never in the tab's profile, off Never. The user's, as above.
    fn allow_saving(&self, tab: u64, login: &Login);
    /// Blocks ads and trackers on the site or lets them load, and reloads the page. The user's, as
    /// above.
    fn set_ad_blocking(&self, tab: u64, on: bool);
    /// Turns Developer Mode on or off for the site, in the tab's profile. The user's view: agents
    /// get none.
    fn set_developer_mode(&self, tab: u64, on: bool);
    /// Turns the site's Boost on or off there, in the tab's profile. The user's alone, as a Boost
    /// is.
    fn set_boost(&self, tab: u64, on: bool);
    /// Turn On, for the site's arrangement the core keeps off as it changed outside ilo: the
    /// user keeps it as it is. The user's alone, as a Boost is.
    fn turn_on_arrangement(&self, tab: u64);
    /// Whether the cards of the site of `tab`'s page show what's recent there (#3074).
    fn set_recent(&self, tab: u64, on: bool, cx: &mut App);
    fn run(&self, run: Run, tab: u64, cx: &mut App);
    /// Shows ilo's menu of `items` at `position` in the panel's window, and calls `chosen` once it
    /// closes, with the index of the one chosen, if any.
    fn menu(
        &self,
        items: Vec<MenuEntry>,
        position: Point<Pixels>,
        chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    );
    /// Shows the app's tooltip, hanging from a tile in the browser window, or takes it away.
    fn tooltip(&self, tip: Option<(SharedString, tooltip::Anchor)>, cx: &mut App);
    /// Whether the system has a share sheet for the Share tile to open, as the tab menu's Share…
    /// asks.
    fn has_share_sheet(&self) -> bool;
}

/// The panel for one tab, from the press that opens it until it has left.
struct Open {
    tab: u64,
    /// Its window's top left in the browser window.
    at: Point<Pixels>,
    /// The beak's tip across its top, none in the corner.
    beak: Option<f32>,
    /// The browser window's size, which tooltips stay inside.
    window: Size<Pixels>,
    /// The tab's icon, which the header shows on its plate.
    favicon: Option<Arc<Image>>,
    extensions: Vec<Extension>,
    profiles: Option<Profiles>,
    /// Where the header's parts went as it last drew, which its text's widths decide.
    head: Head,
    /// The grid's order as it last drew, by id.
    drawn: Vec<SharedString>,
    /// Where the tiles, rows and buttons were last logged.
    placed: Vec<String>,
    /// None until the core first reports, which is when it shows.
    site: Option<Site>,
    height: f32,
    /// The tile whose tooltip shows, as the pointer enters the next before it leaves the last.
    tip: Option<SharedString>,
    /// When its window was first on screen, where the motion starts.
    shown: Option<Instant>,
    /// What the keyboard's ring is on, from the first Tab or arrow until a press.
    keyed: Option<Stop>,
    settled: bool,
    /// Once it's closing: when, and how far it had opened.
    closing: Option<(Instant, f32)>,
    _leave: Option<Task<()>>,
}

impl Open {
    /// How far it has opened, from 0 to 1.
    fn progress(&self) -> f32 {
        match self.closing {
            Some((at, from)) => {
                let t = (at.elapsed().as_secs_f32() / CLOSE).min(1.);
                from * (1. - cubic_bezier(0.42, 0., 1., 1., t))
            }
            None => {
                let opened = self.shown.map_or(0., |shown| shown.elapsed().as_secs_f32());
                cubic_bezier(0., 0., 0.58, 1., (opened / OPEN).min(1.))
            }
        }
    }

    fn moving(&self) -> bool {
        self.closing.is_some()
            || self
                .shown
                .is_none_or(|shown| shown.elapsed().as_secs_f32() < OPEN)
    }

    /// Where it grows from in its window: the beak's tip, else its top left.
    fn origin(&self) -> Point<Pixels> {
        point(px(self.beak.unwrap_or(0.)), px(0.))
    }

    /// `frame`, in the panel's window, in the browser window.
    fn in_window(&self, frame: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::new(self.at + frame.origin, frame.size)
    }
}

/// What the keyboard's ring can be on, down the panel (#1382): the connection's words while they
/// show a certificate, the profile's chip, "…", the tiles, the site's rows a click changes, the
/// extensions' tiles and +, and Reset permissions.
#[derive(Clone, PartialEq, Debug)]
enum Stop {
    Tile(&'static str),
    Extension(SharedString),
    AddExtension,
    Engine,
    /// The site's first row, Block ads and trackers.
    AdBlocking,
    DeveloperMode,
    Boost,
    Recent,
    Arrangement,
    Passwords,
    /// By Chrome's ContentSettingsType.
    Permission(i32),
    /// By its place among the site's devices.
    Device(usize),
    Reset,
    Lock,
    Profile,
    More,
}

/// Where each part of the panel goes, down its window.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Layout {
    /// The body's top: under the beak, if it has one.
    body: f32,
    header: f32,
    tiles: f32,
    /// "This site"'s top and its plate's, if it has rows.
    site_label: Option<f32>,
    rows: f32,
    extensions_label: f32,
    extensions: f32,
    /// The foot's line, while Reset permissions has something to reset.
    foot: Option<f32>,
    height: f32,
}

/// The panel's layout for `site` with `extensions` tiles, under a beak or not (#4404): the header,
/// the tiles, the site's plate, the extensions' plates, and the foot only while there's something
/// to reset.
fn layout(site: &Site, extensions: usize, beak: bool) -> Layout {
    let body = if beak { BEAK } else { 0. };
    let header = body + PADDING;
    let tiles = header + HEADER + HEADER_TO_TILES;
    let below_tiles = tiles + TILE_HEIGHT;
    let labelled = |above: f32| {
        (
            above + LABEL_ABOVE,
            above + LABEL_ABOVE + LABEL_LINE + LABEL_BELOW,
        )
    };
    let (site_label, rows, below_rows) = match rows(site).len() {
        0 => (None, below_tiles, below_tiles),
        count => {
            let (label, rows) = labelled(below_tiles);
            (Some(label), rows, rows + count as f32 * ROW)
        }
    };
    let (extensions_label, grid) = labelled(below_rows);
    let lines = (extensions + 1).div_ceil(EXTENSIONS_PER_ROW) as f32;
    let grid_bottom = grid + lines * TILE_HEIGHT + (lines - 1.) * GAP;
    let (foot, height) = match site.reset {
        Some(_) => {
            let line = grid_bottom + FOOT_ABOVE;
            (Some(line), line + FOOT_BELOW + FOOT_ROW + PADDING)
        }
        None => (None, grid_bottom + PADDING),
    };
    Layout {
        body,
        header,
        tiles,
        site_label,
        rows,
        extensions_label,
        extensions: grid,
        foot,
        height,
    }
}

/// A row of the site's plate.
#[derive(Clone, PartialEq, Debug)]
enum Row {
    /// Block ads and trackers, on or not.
    AdBlocking(bool),
    Switch(Switch),
    /// Why the core keeps the Boost's arrangement off, whose click is Turn On (#2347).
    Arrangement(SharedString),
    /// By its place among the site's permissions.
    Permission(usize),
    Device(usize),
    Passwords,
}

/// The site's plate, in its order: Block ads and trackers where it's the user's to change, the
/// site's switches, the arrangement while the core keeps it off, the permissions that differ, the
/// devices it was given, then Passwords.
fn rows(site: &Site) -> Vec<Row> {
    let ad_blocking = site.ad_blocking.map(Row::AdBlocking);
    let switches = switches_of(site).into_iter().map(Row::Switch);
    let arrangement = site.arrangement_off.clone().map(Row::Arrangement);
    let permissions = (0..site.permissions.len()).map(Row::Permission);
    let devices = (0..site.devices.len()).map(Row::Device);
    let passwords = passwords::state(&site.logins).map(|_| Row::Passwords);
    (ad_blocking.into_iter())
        .chain(switches)
        .chain(arrangement)
        .chain(permissions)
        .chain(devices)
        .chain(passwords)
        .collect()
}

impl Row {
    /// Its stop, none for one a policy sets, which takes no click.
    fn stop(&self, site: &Site) -> Option<Stop> {
        match self {
            Row::AdBlocking(_) => Some(Stop::AdBlocking),
            Row::Switch(switch) => Some(switch.stop()),
            Row::Arrangement(_) => Some(Stop::Arrangement),
            Row::Permission(index) => (site.permissions.get(*index))
                .filter(|each| !each.choices.is_empty())
                .map(|each| Stop::Permission(each.kind)),
            Row::Device(index) => (site.devices.get(*index))
                .filter(|each| each.removable)
                .map(|_| Stop::Device(*index)),
            Row::Passwords => Some(Stop::Passwords),
        }
    }

    /// Its name in the log, which check.sh clicks by: the ad blocking row is "shield", as the
    /// tile it was.
    fn place(&self, site: &Site) -> String {
        match self {
            Row::AdBlocking(_) => "shield".into(),
            Row::Switch(switch) => format!("switch {}", switch.name()),
            Row::Arrangement(_) => format!("line {ARRANGEMENT_NAME}"),
            Row::Permission(index) => format!("line {}", site.permissions[*index].name),
            Row::Device(index) => format!("line {}", site.devices[*index].name),
            Row::Passwords => format!("line {PASSWORDS_NAME}"),
        }
    }
}

/// A switch special to the site, a row of its plate (#2334).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Switch {
    /// Only while on: off, it's in "…" and the command bar.
    DeveloperMode,
    /// Safari's engine where the profile keeps or offers it, on or not, whose click is a menu.
    Engine(bool),
    /// The site's Boost, where it has one, on or not.
    Boost(bool),
    /// What's recent in its pinned tabs' and favorites' cards, where it has some, on or not.
    Recent(bool),
}

impl Switch {
    fn stop(self) -> Stop {
        match self {
            Switch::DeveloperMode => Stop::DeveloperMode,
            Switch::Engine(_) => Stop::Engine,
            Switch::Boost(_) => Stop::Boost,
            Switch::Recent(_) => Stop::Recent,
        }
    }

    fn on(self) -> bool {
        match self {
            Switch::DeveloperMode => true,
            Switch::Engine(on) | Switch::Boost(on) | Switch::Recent(on) => on,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Switch::DeveloperMode => DEVELOPER_MODE,
            Switch::Engine(_) => ENGINE,
            Switch::Boost(_) => BOOST,
            Switch::Recent(_) => RECENT,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Switch::DeveloperMode => DEVELOPER_MODE_NAME,
            Switch::Engine(_) => ENGINE_NAME,
            Switch::Boost(_) => BOOST_NAME,
            Switch::Recent(_) => RECENT_NAME,
        }
    }
}

/// The site's switches, in their order: Developer Mode while on, Safari's engine, the Boost, what's
/// recent in its cards.
fn switches_of(site: &Site) -> Vec<Switch> {
    let developer_mode = (site.developer_mode == Some(true)).then_some(Switch::DeveloperMode);
    let engine = site.system_engine.map(Switch::Engine);
    let boost = site.boost.map(Switch::Boost);
    let recent = site.recent.map(Switch::Recent);
    [developer_mode, engine, boost, recent]
        .into_iter()
        .flatten()
        .collect()
}

/// The `index`th row's frame, of the plate from `top`.
fn row_frame(top: f32, index: usize) -> Bounds<Pixels> {
    Bounds::new(
        point(px(PADDING), px(top + index as f32 * ROW)),
        size(px(WIDTH - 2. * PADDING), px(ROW)),
    )
}

/// The panel's corners in its window, clockwise from the beak's tip, or its top left without one.
fn corners(beak: Option<f32>, height: f32) -> Vec<(f32, f32, f32)> {
    let Some(tip) = beak else {
        let corner = |x: f32, y: f32| (x, y, RADIUS);
        return vec![
            corner(0., 0.),
            corner(WIDTH, 0.),
            corner(WIDTH, height),
            corner(0., height),
        ];
    };
    vec![
        (tip, 0., BEAK_TIP_RADIUS),
        (tip + BEAK, BEAK, BEAK_FOOT_RADIUS),
        (WIDTH, BEAK, RADIUS),
        (WIDTH, height, RADIUS),
        (0., height, RADIUS),
        (0., BEAK, RADIUS),
        (tip - BEAK, BEAK, BEAK_FOOT_RADIUS),
    ]
}

/// The header's parts, which the widths of its text place (#4404).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Head {
    /// The site's name and its connection's top, from the panel's left.
    text: Point<Pixels>,
    /// How wide they may go, before the chip or "…".
    room: f32,
    /// The connection's words, which take a click while there's a certificate, none for a file
    /// or one of the browser's pages.
    lock: Option<Bounds<Pixels>>,
    /// The profile's chip, and how wide its name shows, 0 where it gave way to the glyph.
    chip: Option<(Bounds<Pixels>, f32)>,
    /// The chip's name is cut or gone, so its tooltip says it.
    chip_cut: bool,
    more: Bounds<Pixels>,
}

/// The connection's words under the site's name, the words table's SC1, none for a file or one of
/// the browser's pages, which Chrome gives none.
fn connection_words(site: &Site) -> Option<&'static str> {
    match site.connection {
        Connection::Secure => Some("Secure"),
        Connection::NotSecure => Some("Not secure"),
        Connection::Internal => None,
    }
}

/// The chip's glyph, chevron and insets, without its name.
const CHIP_BARE: f32 = 2. * CHIP_INSET + CHIP_GLYPH + PROFILE_CHEVRON_GAP + PROFILE_CHEVRON;

/// Where the header's parts go from `top`, for `site` and the `profiles`. The site's name keeps the
/// room it needs, so the chip's name cuts first, and with too little room goes.
fn head(
    site: &Site,
    profiles: Option<&Profiles>,
    top: f32,
    system: &gpui::WindowTextSystem,
) -> Head {
    let more = Bounds::new(
        point(px(WIDTH - PADDING - MORE), px(top + (HEADER - MORE) / 2.)),
        size(px(MORE), px(MORE)),
    );
    let left = PADDING + HEADER_INSET + FAVICON_PLATE + FAVICON_GAP;
    let end = WIDTH - PADDING - MORE - GAP / 2.;
    let words = connection_words(site);
    let lines = match words {
        Some(_) => NAME_LINE + CONNECTION_LINE,
        None => NAME_LINE,
    };
    let text = point(px(left), px(top + (HEADER - lines) / 2.));
    let semibold = gpui::FontWeight::SEMIBOLD;
    let name = text_width(&site.site, NAME_SIZE, semibold, system).ceil();
    let connection = words.map(|words| {
        let words = text_width(
            &words.into(),
            CONNECTION_SIZE,
            gpui::FontWeight::NORMAL,
            system,
        );
        CONNECTION_GLYPH + CONNECTION_GLYPH_GAP + words.ceil()
    });
    let wanted = name.max(connection.unwrap_or(0.));
    let (room, chip, chip_cut) = match profiles {
        None => (end - left, None, false),
        Some(profiles) => {
            let full =
                text_width(&profiles.name, NAME_SIZE, gpui::FontWeight::MEDIUM, system).ceil();
            let room = wanted.min(end - left - GAP - CHIP_BARE);
            let spare = end - left - room - GAP - CHIP_BARE - PROFILE_CHEVRON_GAP;
            let shown = match spare >= full.min(PROFILE_NAME_MIN) {
                true => full.min(spare),
                false => 0.,
            };
            let width = CHIP_BARE
                + if shown > 0. {
                    PROFILE_CHEVRON_GAP + shown
                } else {
                    0.
                };
            let frame = Bounds::new(
                point(px(end - width), px(top + (HEADER - MORE) / 2.)),
                size(px(width), px(MORE)),
            );
            (end - width - GAP - left, Some((frame, shown)), shown < full)
        }
    };
    let lock = connection.map(|width| {
        let pad = CONNECTION_PAD;
        Bounds::new(
            point(px(left - pad), text.y + px(NAME_LINE - 1.)),
            size(px(width.min(room) + 2. * pad), px(CONNECTION_LINE + 2.)),
        )
    });
    Head {
        text,
        room,
        lock,
        chip,
        chip_cut,
        more,
    }
}

/// The panel's window's top left in the browser window and its beak's tip, from `place`.
pub fn placed(place: Place, window: Size<Pixels>) -> (Point<Pixels>, Option<f32>) {
    match place {
        Place::Button { field, button } => {
            let middle = f32::from(button.center().x);
            let right = f32::from(window.width) - WINDOW_MARGIN - WIDTH;
            let left = (middle - WIDTH / 2.).min(right).max(WINDOW_MARGIN);
            // The beak stays clear of the body's rounded corners.
            let clear = RADIUS + BEAK + BEAK_FOOT_RADIUS;
            let tip = (middle - left).clamp(clear, WIDTH - clear);
            let top = f32::from(field.bottom()) + BELOW_FIELD - BEAK;
            (point(px(left), px(top)), Some(tip))
        }
        Place::Corner(corner) => (corner, None),
    }
}

pub struct SiteControls {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    open: Option<Open>,
    /// An assistive app asks for the tree, so the panel names what it draws.
    a11y: bool,
}

struct SiteControlsWindow(WindowHandle<SiteControls>);

impl Global for SiteControlsWindow {}

/// Opens the panel's window, hidden, for `host` to show over the browser window, ahead of time as
/// the command bar's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<SiteControls>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(WIDTH)));
    open_at(host, bounds, false, cx)
}

/// Opens the panel's window at `bounds`, shown if `show`, which the preview does.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<SiteControls>> {
    let context = Some("SiteControls");
    cx.bind_keys([
        KeyBinding::new("escape", Close, context),
        KeyBinding::new("tab", NextStop, context),
        KeyBinding::new("shift-tab", PreviousStop, context),
        KeyBinding::new("down", StopDown, context),
        KeyBinding::new("up", StopUp, context),
        KeyBinding::new("space", Press, context),
        KeyBinding::new("enter", Press, context),
    ]);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The panel's shape is drawn, over the host's blur in it.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| SiteControls {
            host,
            focus: cx.focus_handle(),
            open: None,
            a11y: false,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(SiteControlsWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the Site Controls' window: {error}");
            None
        }
    }
}

fn update(
    cx: &mut App,
    change: impl FnOnce(&mut SiteControls, &mut Window, &mut Context<SiteControls>),
) {
    if let Some(window) = cx.try_global::<SiteControlsWindow>().map(|panel| panel.0) {
        window.update(cx, change).ok();
    }
}

/// Opens the panel for the site of `tab`'s page, hanging from `place` in a browser window of
/// `window`'s size, with the tab's `favicon` and `extensions`, once the core has read the site. If
/// it shows, closes it instead, as its button's second press does.
pub fn show(
    tab: u64,
    place: Place,
    window: Size<Pixels>,
    favicon: Option<Arc<Image>>,
    extensions: Vec<Extension>,
    profiles: Option<Profiles>,
    cx: &mut App,
) {
    crate::lazy_window::ready::<SiteControls>(cx);
    update(cx, |panel, _, cx| {
        if panel
            .open
            .as_ref()
            .is_some_and(|open| open.closing.is_none())
        {
            // Its button's second press.
            return panel.close(dismissal::Close::new(Why::Toggled, By::Pointer), cx);
        }
        let (at, beak) = placed(place, window);
        log_extensions(&extensions);
        panel.open = Some(Open {
            tab,
            at,
            beak,
            window,
            favicon,
            extensions,
            profiles,
            head: Head::default(),
            drawn: Vec::new(),
            placed: Vec::new(),
            site: None,
            height: 0.,
            tip: None,
            shown: None,
            keyed: None,
            settled: false,
            closing: None,
            _leave: None,
        });
        panel.host.read(tab);
        cx.notify();
    });
}

/// The core read the site of `tab`'s page, or it changed.
pub fn changed(tab: u64, site: Site, cx: &mut App) {
    update(cx, |panel, window, cx| {
        let open = panel.open.as_mut();
        let Some(open) = open.filter(|open| open.tab == tab && open.closing.is_none()) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: site controls {}", describe(&site));
        let height = layout(&site, open.extensions.len(), open.beak.is_some()).height;
        let first = open.site.is_none();
        open.site = Some(site);
        let shape = corners(open.beak, height);
        if first {
            open.height = height;
            window.focus(&panel.focus, cx);
            panel
                .host
                .show(open.at, size(px(WIDTH), px(height)), &shape);
        } else if height != open.height {
            open.height = height;
            panel.host.set_height(height, &shape);
        }
        cx.notify();
    });
}

/// The extensions the panel for `tab` lists changed, or the core first reported the unpinned ones,
/// which it does once the panel opens.
pub fn set_extensions(tab: u64, extensions: Vec<Extension>, cx: &mut App) {
    update(cx, |panel, _, cx| {
        let open = panel.open.as_mut();
        let Some(open) = open.filter(|open| open.tab == tab && open.closing.is_none()) else {
            return;
        };
        let ids = |extensions: &[Extension]| -> Vec<SharedString> {
            extensions
                .iter()
                .map(|extension| extension.id.clone())
                .collect()
        };
        if ids(&extensions) != ids(&open.extensions) {
            log_extensions(&extensions);
        }
        open.extensions = extensions;
        if let Some(site) = &open.site {
            let height = layout(site, open.extensions.len(), open.beak.is_some()).height;
            if height != open.height {
                open.height = height;
                panel.host.set_height(height, &corners(open.beak, height));
            }
        }
        cx.notify();
    });
}

/// The extensions' grid for the log, by id.
fn log_extensions(extensions: &[Extension]) {
    let ids: Vec<&str> = extensions
        .iter()
        .map(|extension| extension.id.as_ref())
        .collect();
    // check.sh reads this line.
    log!("browser: site controls extensions {}", ids.join(" "));
}

/// The user left the panel: a click outside it, another window or another app. The host has
/// released it already.
pub fn dismissed(cx: &mut App) {
    let close = dismissal::Close::left(cx.active_window().is_none());
    update(cx, |panel, _, cx| panel.close(close, cx));
}

/// The tab closed, and its panel with it.
pub fn tab_closed(tab: u64, cx: &mut App) {
    update(cx, |panel, _, cx| {
        if panel.open.as_ref().is_some_and(|open| open.tab == tab) {
            panel.close(dismissal::Close::new(Why::FocusLeft, By::Code), cx);
        }
    });
}

/// The site for the log: Chrome's words for the connection, then each row and its value, in their
/// order, which check.sh clicks by.
fn describe(site: &Site) -> String {
    let ad_blocking = (site.ad_blocking.iter()).map(|&on| match on {
        true if site.ad_blocked.is_empty() => "Ad blocking On".to_string(),
        true => format!("Ad blocking On ({} blocked)", site.ad_blocked),
        false => "Ad blocking Off".to_string(),
    });
    let developer_mode = (site.developer_mode.iter())
        .map(|&on| format!("{DEVELOPER_MODE_NAME} {}", if on { "On" } else { "Off" }));
    let permissions: Vec<String> = ad_blocking
        .chain(developer_mode)
        .chain(
            (site.permissions.iter())
                .map(|permission| format!("{} {}", permission.name, permission.value.label())),
        )
        .collect();
    format!(
        "{}: {}; {}{}{}{}",
        site.site,
        site.connection_summary,
        if permissions.is_empty() {
            "no permissions".into()
        } else {
            permissions.join(", ")
        },
        match site.system_engine {
            Some(on) => format!("; {ENGINE_NAME} {}", if on { "on" } else { "off" }),
            None => String::new(),
        },
        match site.logins.is_empty() {
            true => String::new(),
            false => format!("; passwords {}", passwords::describe(&site.logins)),
        },
        match site.devices.is_empty() {
            true => String::new(),
            false => {
                let devices: Vec<String> = (site.devices.iter())
                    .map(|device| format!("{} ({})", device.name, device.kind))
                    .collect();
                format!("; devices {}", devices.join(", "))
            }
        }
    )
}

/// The "…" menu: Arc's, its Boost items where the page can have one, ours to turn Developer
/// Mode on, whose switch shows only while it's on (#2334), and the site's mark as sensitive for
/// agents, checked while it is (#4369).
fn more_items(site: &Site) -> Vec<(MenuEntry, Option<Run>)> {
    let item = |title: &'static str, run: Run| (MenuEntry::new(title, true), Some(run));
    let mut items = vec![
        item("Clear Cache", Run::Command(Command::ClearCacheAndRefresh)),
        item(
            "Clear Cookies",
            Run::Command(Command::ClearCookiesAndRefresh),
        ),
    ];
    if site.developer_mode == Some(false) {
        items.push(item(
            "Turn On Developer Mode",
            Run::Command(Command::ToggleDeveloperMode),
        ));
    }
    if let Some(on) = site.sensitive {
        // The words table's SS3.
        let mark = MenuEntry {
            checked: on,
            ..MenuEntry::new("Sensitive Site", true)
        };
        items.push((mark, Some(Run::Sensitive(!on))));
    }
    if site.boostable {
        let boost = match site.boost {
            Some(_) => item("Edit Boost…", Run::Command(Command::EditBoost)),
            None => item("New Boost…", Run::Command(Command::NewBoost)),
        };
        items.extend([
            (MenuEntry::line(), None),
            boost,
            item("View Boosts…", Run::Command(Command::ViewBoosts)),
        ]);
    }
    items.extend([
        (MenuEntry::line(), None),
        item(
            "Manage Extensions…",
            Run::Command(Command::ManageExtensions),
        ),
        // The browser has no add path of its own: Chrome's extensions page links its store.
        item("Add Extension…", Run::Command(Command::ManageExtensions)),
        (MenuEntry::line(), None),
        item("All Site Settings…", Run::SiteSettings),
    ]);
    items
}

/// What an assistive app reads of a control: a button called `name`, which says it opens a menu
/// with `menu` and is dimmed without `enabled`, and whose right-click menu it lists with
/// `context_menu`.
fn button_named<E: StatefulInteractiveElement>(
    element: E,
    name: impl Into<SharedString>,
    (menu, enabled, context_menu): (bool, bool, bool),
) -> E {
    element
        .role(Role::Button)
        .aria_label(name)
        .a11y_synthetic_children(move |builder| {
            let node = builder.parent_node();
            if menu {
                node.set_has_popup(HasPopup::Menu);
            }
            if !enabled {
                node.set_disabled();
            }
            if context_menu {
                node.add_action(gpui::accesskit::Action::ShowContextMenu);
            }
        })
}

/// A switch that turns its setting on or off, read as one called `name`.
fn switch_named<E: StatefulInteractiveElement>(element: E, name: &'static str, on: bool) -> E {
    let toggled = if on { Toggled::True } else { Toggled::False };
    element
        .role(Role::Switch)
        .aria_label(name)
        .aria_toggled(toggled)
}

/// A line's name and its value, as VoiceOver reads a tab's row: "Location, Ask".
fn said(name: &str, state: &str) -> String {
    format!("{name}, {state}")
}

/// The words the log gives `close`, which check.sh reads.
fn words(close: dismissal::Close) -> &'static str {
    match close.why {
        Why::Toggled => "toggle",
        Why::Escape => "escape",
        Why::PressOutside | Why::WindowLeft => "dismissed",
        Why::FocusLeft => "tab closed",
        Why::Chosen => "chosen",
        Why::CloseCommand | Why::PointerLeft => "left",
    }
}

impl SiteControls {
    /// Closes the panel back into its button, giving the browser window the keyboard and the
    /// clicks at once.
    fn close(&mut self, close: dismissal::Close, cx: &mut Context<Self>) {
        let Some(open) = self.open.as_mut().filter(|open| open.closing.is_none()) else {
            return;
        };
        open.closing = Some((Instant::now(), open.progress()));
        self.host.release();
        self.host.stop_reading();
        self.host.tooltip(None, cx);
        // check.sh reads this line.
        log!("browser: site controls closing ({})", words(close));
        open._leave = Some(cx.spawn(async move |panel, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(CLOSE))
                .await;
            panel
                .update(cx, |panel, cx| {
                    // Unless it opened again meanwhile.
                    if panel
                        .open
                        .as_ref()
                        .is_some_and(|open| open.closing.is_some())
                    {
                        panel.open = None;
                        panel.host.hide();
                        cx.notify();
                    }
                })
                .ok();
        }));
        cx.notify();
    }

    /// Closes the panel, then has the host do `run`, so what it opens gets the keyboard from the
    /// browser window.
    fn run(&mut self, name: &str, run: Run, by: By, cx: &mut Context<Self>) {
        let Some(tab) = self
            .open
            .as_ref()
            .filter(|open| open.closing.is_none())
            .map(|o| o.tab)
        else {
            return;
        };
        // check.sh reads this line.
        log!("browser: site controls run {name}");
        self.close(dismissal::Close::new(Why::Chosen, by), cx);
        self.host.run(run, tab, cx);
    }

    /// Runs `tile`. ⌥ on Capture Full Page captures in Portrait Mode on the background kept, as
    /// Arc's ⌥-click on its Portrait Mode skips the panel.
    fn run_tile(
        &mut self,
        tile: Tile,
        frame: Bounds<Pixels>,
        alt: bool,
        by: By,
        cx: &mut Context<Self>,
    ) {
        let Some(open) = self.open.as_ref() else {
            return;
        };
        let boosted = open.site.as_ref().is_some_and(|site| site.boost.is_some());
        let run = match tile.run {
            TileRun::Share => Run::Share(open.in_window(frame)),
            TileRun::Command(Command::NewBoost) if boosted => Run::Command(Command::EditBoost),
            TileRun::Command(Command::CaptureFullPage) if alt => {
                Run::Command(Command::CapturePortraitNow)
            }
            TileRun::Command(command) => Run::Command(command),
        };
        self.run(tile.name, run, by, cx);
    }

    /// Shows the permission's values in a menu at `position`, and sets the one chosen.
    fn choose_value(
        &mut self,
        permission: Permission,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.open.as_ref().map(|open| open.tab) else {
            return;
        };
        let items: Vec<MenuEntry> = (permission.choices.iter())
            .map(|&value| MenuEntry {
                checked: value == permission.value,
                ..MenuEntry::new(value.label(), true)
            })
            .collect();
        let panel = cx.weak_entity();
        let chosen = Box::new(move |chosen: Option<usize>, cx: &mut App| {
            let Some(&value) = chosen.and_then(|index| permission.choices.get(index)) else {
                return;
            };
            panel
                .update(cx, |panel, _| {
                    // Unless the panel closed or went to another tab while the menu showed.
                    let open = panel.open.as_ref();
                    let still = open.is_some_and(|open| open.tab == tab && open.closing.is_none());
                    if still && value != permission.value {
                        // check.sh reads this line.
                        log!(
                            "browser: site controls set {} {}",
                            permission.name,
                            value.label()
                        );
                        panel.host.set_permission(tab, permission.kind, value);
                    }
                })
                .ok();
        });
        self.host.menu(items, position, chosen, cx);
    }

    /// Shows the device's menu at `position`: Remove, which takes it away from the site.
    fn choose_device(
        &mut self,
        index: usize,
        device: Device,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.open.as_ref().map(|open| open.tab) else {
            return;
        };
        let panel = cx.weak_entity();
        let chosen = Box::new(move |chosen: Option<usize>, cx: &mut App| {
            if chosen != Some(0) {
                return;
            }
            panel
                .update(cx, |panel, _| {
                    let open = panel.open.as_ref();
                    let still = open.is_some_and(|open| {
                        open.tab == tab
                            && open.closing.is_none()
                            && (open.site.as_ref())
                                .is_some_and(|site| site.devices.get(index) == Some(&device))
                    });
                    if still {
                        // check.sh reads this line.
                        log!("browser: site controls remove device {}", device.name);
                        panel.host.remove_device(tab, index);
                    }
                })
                .ok();
        });
        self.host
            .menu(vec![MenuEntry::new("Remove", true)], position, chosen, cx);
    }

    /// Shows the profile chip's menu at `position`: this page in each other profile, or New Space
    /// in one none of the user's Spaces is in, and runs the item chosen.
    fn profiles(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(profiles) = self.open.as_ref().and_then(|open| open.profiles.clone()) else {
            return;
        };
        let (items, runs): (Vec<MenuEntry>, Vec<Run>) = (profiles.others.iter())
            .map(|(id, name, has_space)| match has_space {
                true => (
                    MenuEntry::named(format!("Open in {name}"), true),
                    Run::OpenInProfile(*id),
                ),
                false => (
                    MenuEntry::named(format!("New Space in {name}…"), true),
                    Run::NewSpaceInProfile(*id),
                ),
            })
            .unzip();
        let titles: Vec<SharedString> = items.iter().map(|item| item.title.clone()).collect();
        let panel = cx.weak_entity();
        let chosen = Box::new(move |index: Option<usize>, cx: &mut App| {
            let Some((run, name)) =
                index.and_then(|index| Some((runs.get(index)?.clone(), titles[index].to_string())))
            else {
                return;
            };
            // The menu doesn't say who chose.
            panel
                .update(cx, |panel, cx| panel.run(&name, run, By::Pointer, cx))
                .ok();
        });
        self.host.menu(items, position, chosen, cx);
    }

    /// Shows the "…" menu at `position`, and runs the item chosen.
    fn more(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(site) = self.open.as_ref().and_then(|open| open.site.as_ref()) else {
            return;
        };
        let (items, runs): (Vec<MenuEntry>, Vec<Option<Run>>) =
            more_items(site).into_iter().unzip();
        let titles: Vec<SharedString> = items.iter().map(|item| item.title.clone()).collect();
        let panel = cx.weak_entity();
        let chosen = Box::new(move |index: Option<usize>, cx: &mut App| {
            let Some(index) = index else {
                return;
            };
            let Some(Some(run)) = runs.get(index).cloned() else {
                return;
            };
            let name = titles[index].to_string();
            // The menu doesn't say who chose.
            panel
                .update(cx, |panel, cx| panel.run(&name, run, By::Pointer, cx))
                .ok();
        });
        self.host.menu(items, position, chosen, cx);
    }

    /// Shows the engine row's menu at `position`: On and Off, the one it is checked, and the other
    /// turns it so.
    fn choose_engine(&mut self, on: bool, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(tab) = self.open.as_ref().map(|open| open.tab) else {
            return;
        };
        let items = [
            MenuEntry {
                checked: on,
                ..MenuEntry::new("On", true)
            },
            MenuEntry {
                checked: !on,
                ..MenuEntry::new("Off", true)
            },
        ];
        let panel = cx.weak_entity();
        let chosen = Box::new(move |chosen: Option<usize>, cx: &mut App| {
            let Some(chosen) = chosen else {
                return;
            };
            let turn_on = chosen == 0;
            if turn_on == on {
                return;
            }
            panel
                .update(cx, |panel, _| {
                    let open = panel.open.as_ref();
                    if open.is_some_and(|open| open.tab == tab && open.closing.is_none()) {
                        let title = if turn_on { "On" } else { "Off" };
                        // check.sh reads this line.
                        log!("browser: site controls set {ENGINE_NAME} {title}");
                        panel.host.set_system_engine(tab, turn_on);
                    }
                })
                .ok();
        });
        self.host.menu(items.to_vec(), position, chosen, cx);
    }

    /// Shows the Passwords row's menu at `position`: Remove for each login, which closes the panel
    /// for the browser window's dialog to ask, Never's way back, and every login in Settings.
    fn choose_login(
        &mut self,
        logins: Vec<Login>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.open.as_ref().map(|open| open.tab) else {
            return;
        };
        let (items, actions): (Vec<MenuEntry>, Vec<Option<passwords::Action>>) =
            passwords::menu(&logins).into_iter().unzip();
        let panel = cx.weak_entity();
        let chosen = Box::new(move |chosen: Option<usize>, cx: &mut App| {
            let Some(Some(action)) = chosen.and_then(|index| actions.get(index).cloned()) else {
                return;
            };
            panel
                .update(cx, |panel, cx| {
                    let open = panel.open.as_ref();
                    if !open.is_some_and(|open| open.tab == tab && open.closing.is_none()) {
                        return;
                    }
                    match action {
                        passwords::Action::Remove(login) => {
                            panel.run("remove-login", Run::RemoveLogin(login), By::Pointer, cx)
                        }
                        passwords::Action::Allow(login) => {
                            // check.sh reads this line.
                            log!("browser: site controls allow saving {}", login.site);
                            panel.host.allow_saving(tab, &login);
                        }
                        passwords::Action::ShowAll => {
                            panel.run("passwords", Run::Passwords, By::Pointer, cx)
                        }
                    }
                })
                .ok();
        });
        self.host.menu(items, position, chosen, cx);
    }

    /// Shows the menu of the extension at `index` in the grid at `position`. Pinning and Move to
    /// Top keep the panel, whose grid the core redraws; the others close it for what they open.
    fn extension_menu(&mut self, index: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(open) = self.open.as_ref().filter(|open| open.closing.is_none()) else {
            return;
        };
        let Some(extension) = open.extensions.get(index) else {
            return;
        };
        let (tab, id) = (open.tab, extension.id.clone());
        let (items, runs): (Vec<MenuEntry>, Vec<Option<ExtensionItem>>) =
            extension_items(extension, index).into_iter().unzip();
        let panel = cx.weak_entity();
        let chosen = Box::new(move |chosen: Option<usize>, cx: &mut App| {
            let Some(Some(item)) = chosen.and_then(|index| runs.get(index).copied()) else {
                return;
            };
            panel
                .update(cx, |panel, cx| {
                    let open = panel.open.as_ref();
                    if !open.is_some_and(|open| open.tab == tab && open.closing.is_none()) {
                        return;
                    }
                    let run = Run::ExtensionItem(id.clone(), item);
                    match item.stays() {
                        true => {
                            // check.sh reads this line.
                            log!("browser: site controls extension {} {id}", item.name());
                            panel.host.run(run, tab, cx);
                        }
                        false => {
                            let name = format!("extension {}", item.name());
                            panel.run(&name, run, By::Pointer, cx)
                        }
                    }
                })
                .ok();
        });
        self.host.menu(items, position, chosen, cx);
    }

    /// Turns blocking on the site the other way. The core reloads the page and reports the site
    /// again, which redraws the row.
    fn toggle_ad_blocking(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(open) = self.open.as_ref().filter(|open| open.closing.is_none()) {
            // check.sh reads this line.
            log!(
                "browser: site controls set ad blocking {}",
                if on { "on" } else { "off" }
            );
            self.host.set_ad_blocking(open.tab, on);
            cx.notify();
        }
    }

    /// Turns Developer Mode for the site the other way. The core reports the site again, which
    /// redraws the row, and each of its tabs.
    fn toggle_developer_mode(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(open) = self.open.as_ref().filter(|open| open.closing.is_none()) {
            // check.sh reads this line.
            log!(
                "browser: site controls set developer mode {}",
                if on { "on" } else { "off" }
            );
            self.host.set_developer_mode(open.tab, on);
            cx.notify();
        }
    }

    /// Turns the site's Boost the other way. The app keeps it, not the core, so the row shows it at
    /// once.
    fn toggle_boost(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some(open) = self.open.as_mut().filter(|open| open.closing.is_none()) else {
            return;
        };
        let Some(site) = open.site.as_mut().filter(|site| site.boost.is_some()) else {
            return;
        };
        log!(
            "browser: site controls set boost {}",
            if on { "on" } else { "off" }
        );
        site.boost = Some(on);
        self.host.set_boost(open.tab, on);
        cx.notify();
    }

    /// What's recent in the site's cards, on this device (#3074).
    fn toggle_recent(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some(open) = self.open.as_mut().filter(|open| open.closing.is_none()) else {
            return;
        };
        let Some(site) = open.site.as_mut().filter(|site| site.recent.is_some()) else {
            return;
        };
        log!(
            "browser: site controls set recent {}",
            if on { "on" } else { "off" }
        );
        site.recent = Some(on);
        self.host.set_recent(open.tab, on, cx);
        cx.notify();
    }

    /// The arrangement's row: Turn On. The app keeps it, so the row goes at once.
    fn turn_on_arrangement(&mut self, cx: &mut Context<Self>) {
        let Some(open) = self.open.as_mut().filter(|open| open.closing.is_none()) else {
            return;
        };
        let Some(site) = open
            .site
            .as_mut()
            .filter(|site| site.arrangement_off.is_some())
        else {
            return;
        };
        log!("browser: site controls turned on the arrangement");
        site.arrangement_off = None;
        self.host.turn_on_arrangement(open.tab);
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        if let Some(open) = self.open.as_ref().filter(|open| open.closing.is_none()) {
            log!("browser: site controls reset");
            self.host.reset_permissions(open.tab);
            cx.notify();
        }
    }

    /// Hangs the tooltip saying `text` from `frame` in the panel's window, or takes it away.
    fn tooltip(&mut self, text: SharedString, frame: Bounds<Pixels>, hovered: bool, cx: &mut App) {
        let Some(open) = self.open.as_mut().filter(|open| open.closing.is_none()) else {
            return;
        };
        if hovered {
            let anchor = tooltip::Anchor::under(open.in_window(frame), open.window);
            open.tip = Some(text.clone());
            self.host.tooltip(Some((text, anchor)), cx);
        } else if open.tip.as_ref() == Some(&text) {
            open.tip = None;
            self.host.tooltip(None, cx);
        }
    }

    /// The stops the keyboard goes over, in their order down the panel, each with its frame in the
    /// panel's window, which a menu hangs from.
    fn stops(&self) -> Vec<(Stop, Bounds<Pixels>)> {
        let Some(open) = self.open.as_ref().filter(|open| open.closing.is_none()) else {
            return Vec::new();
        };
        let Some(site) = open.site.as_ref() else {
            return Vec::new();
        };
        let at = layout(site, open.extensions.len(), open.beak.is_some());
        let mut stops = Vec::new();
        if let Some(lock) = open.head.lock.filter(|_| site.certificate) {
            stops.push((Stop::Lock, lock));
        }
        if let Some((chip, _)) = open.head.chip {
            stops.push((Stop::Profile, chip));
        }
        stops.push((Stop::More, open.head.more));
        for (index, tile) in self.tiles(site).iter().enumerate() {
            stops.push((Stop::Tile(tile.name), self.action_frame(index, at.tiles)));
        }
        for (index, row) in rows(site).iter().enumerate() {
            stops.extend(row.stop(site).map(|stop| (stop, row_frame(at.rows, index))));
        }
        for (index, each) in open.extensions.iter().enumerate() {
            stops.push((
                Stop::Extension(each.id.clone()),
                extension_frame(at.extensions, index),
            ));
        }
        let add = extension_frame(at.extensions, open.extensions.len());
        stops.push((Stop::AddExtension, add));
        if let Some(foot) = at.foot {
            stops.push((Stop::Reset, foot_frame(foot)));
        }
        stops
    }

    /// Where each tile, row and button is, by name, as the middle of its frame in the browser
    /// window, such as "line Location at 207 252".
    fn places(&self) -> Vec<String> {
        let Some(open) = self.open.as_ref() else {
            return Vec::new();
        };
        let Some(site) = open.site.as_ref() else {
            return Vec::new();
        };
        let at = layout(site, open.extensions.len(), open.beak.is_some());
        let middle = |frame: Bounds<Pixels>| {
            let middle = open.in_window(frame).center();
            format!("at {:.0} {:.0}", f32::from(middle.x), f32::from(middle.y))
        };
        let mut places = Vec::new();
        if let Some(lock) = open.head.lock {
            places.push(format!("lock {}", middle(lock)));
        }
        places.push(format!("more {}", middle(open.head.more)));
        for (index, tile) in self.tiles(site).iter().enumerate() {
            let frame = self.action_frame(index, at.tiles);
            places.push(format!("tile {} {}", tile.name, middle(frame)));
        }
        for (index, row) in rows(site).iter().enumerate() {
            places.push(format!(
                "{} {}",
                row.place(site),
                middle(row_frame(at.rows, index))
            ));
        }
        if let (Some(foot), Some(reset)) = (at.foot, &site.reset) {
            places.push(format!("line {reset} {}", middle(foot_frame(foot))));
        }
        places
    }

    /// Moves the ring to the next stop, or the one before, round from the last to the first if
    /// `round`, else stopping at the ends. From none it goes to the first, or back from the last.
    fn key_step(&mut self, forward: bool, round: bool, cx: &mut Context<Self>) {
        let stops = self.stops();
        let Some(open) = self.open.as_ref().filter(|_| !stops.is_empty()) else {
            return;
        };
        let at = (open.keyed.as_ref())
            .and_then(|keyed| stops.iter().position(|(stop, _)| stop == keyed));
        let last = stops.len() - 1;
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => last,
            (Some(at), true) if at == last => {
                if round {
                    0
                } else {
                    last
                }
            }
            (Some(at), true) => at + 1,
            (Some(0), false) => {
                if round {
                    last
                } else {
                    0
                }
            }
            (Some(at), false) => at - 1,
        };
        let (stop, frame) = stops[next].clone();
        self.key_to(stop, frame, cx);
    }

    /// Rings `stop`, naming a tile that shows a glyph alone with its tooltip.
    fn key_to(&mut self, stop: Stop, frame: Bounds<Pixels>, cx: &mut Context<Self>) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        // check.sh reads this line.
        log!("browser: site controls keyed {stop:?}");
        let tip = match &stop {
            Stop::Tile(name) => TILES
                .iter()
                .find(|tile| tile.name == *name)
                .map(|tile| tile.label.into()),
            Stop::Extension(id) => open
                .extensions
                .iter()
                .find(|each| &each.id == id)
                .map(|each| each.title.clone()),
            Stop::AddExtension => Some("Add Extension".into()),
            // A chip whose name gave way names it in its tooltip.
            Stop::Profile => (open.profiles.as_ref())
                .filter(|_| open.head.chip_cut)
                .map(|profiles| profiles.name.clone()),
            _ => None,
        };
        open.keyed = Some(stop);
        if open.tip.take().is_some() {
            self.host.tooltip(None, cx);
        }
        if let Some(tip) = tip {
            self.tooltip(tip, frame, true, cx);
        }
        cx.notify();
    }

    /// Space or return: what a click on the ringed stop does, a menu opening under it with its
    /// first item highlighted, as a menu from the keyboard does.
    fn key_press(&mut self, cx: &mut Context<Self>) {
        let stops = self.stops();
        let Some(open) = self.open.as_ref() else {
            return;
        };
        let Some((stop, frame)) = (open.keyed.as_ref())
            .and_then(|keyed| stops.into_iter().find(|(stop, _)| stop == keyed))
        else {
            return;
        };
        let Some(site) = open.site.clone() else {
            return;
        };
        let under = point(frame.left(), frame.bottom());
        let menu = matches!(
            stop,
            Stop::Engine
                | Stop::Passwords
                | Stop::Permission(_)
                | Stop::Device(_)
                | Stop::Profile
                | Stop::More
        );
        match stop {
            Stop::Tile(name) => {
                if let Some(&tile) = TILES.iter().find(|tile| tile.name == name) {
                    self.run_tile(tile, frame, false, By::Keyboard, cx);
                }
            }
            Stop::Extension(id) => {
                let tile = open.in_window(frame);
                self.run("extension", Run::Extension(id, tile), By::Keyboard, cx)
            }
            Stop::AddExtension => {
                let run = Run::Command(Command::ManageExtensions);
                self.run("add-extension", run, By::Keyboard, cx)
            }
            Stop::Engine => self.choose_engine(site.system_engine == Some(true), under, cx),
            Stop::AdBlocking => self.toggle_ad_blocking(site.ad_blocking != Some(true), cx),
            Stop::DeveloperMode => {
                self.toggle_developer_mode(site.developer_mode != Some(true), cx)
            }
            Stop::Boost => self.toggle_boost(site.boost != Some(true), cx),
            Stop::Recent => self.toggle_recent(site.recent != Some(true), cx),
            Stop::Arrangement => self.turn_on_arrangement(cx),
            Stop::Passwords => self.choose_login(site.logins.clone(), under, cx),
            Stop::Permission(kind) => {
                if let Some(permission) = site.permissions.iter().find(|each| each.kind == kind) {
                    self.choose_value(permission.clone(), under, cx);
                }
            }
            Stop::Device(index) => {
                if let Some(device) = site.devices.get(index) {
                    self.choose_device(index, device.clone(), under, cx);
                }
            }
            Stop::Reset => self.reset(cx),
            Stop::Lock => self.run("certificate", Run::Certificate, By::Keyboard, cx),
            Stop::Profile => self.profiles(under, cx),
            Stop::More => self.more(under, cx),
        }
        if menu {
            // After the host's own show, which waits for the overlays to open for a shortcut.
            cx.defer(crate::menu::highlight_first);
        }
    }

    /// The keyboard's ring, Focus Sidebar's, if it's on `stop`: inside its plate.
    fn ring(&self, stop: &Stop, cx: &App) -> Option<gpui::Div> {
        let keyed = self.open.as_ref().and_then(|open| open.keyed.as_ref());
        if keyed != Some(stop) {
            return None;
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .rounded(px(look::RADIUS))
                .border(px(KEYED_RING))
                .border_color(palette::current(cx).plate(KEYED)),
        )
    }

    /// The tiles the panel shows for `site`, closed up from the left. Without a share sheet, as on
    /// Linux, Share would do nothing, so it goes, and Boost goes where the page can have none,
    /// leaving the row's right end empty.
    fn tiles(&self, site: &Site) -> Vec<Tile> {
        let share = self.host.has_share_sheet();
        (TILES.iter())
            .filter(|tile| share || tile.run != TileRun::Share)
            .filter(|tile| site.boostable || tile.run != TileRun::Command(Command::NewBoost))
            .copied()
            .collect()
    }

    /// The `index`th tile's frame at `y`. The platform's tiles share the row's width equally, on
    /// whole points: four of 48 on the Mac, three of 66.7 without a share sheet (#4404).
    fn action_frame(&self, index: usize, y: f32) -> Bounds<Pixels> {
        let share = self.host.has_share_sheet();
        let slots = (TILES.iter())
            .filter(|tile| share || tile.run != TileRun::Share)
            .count() as f32;
        let width = (WIDTH - 2. * PADDING - (slots - 1.) * GAP) / slots;
        let left = PADDING + index as f32 * (width + GAP);
        let (left, right) = (left.round(), (left + width).round());
        Bounds::new(
            point(px(left), px(y)),
            size(px(right - left), px(TILE_HEIGHT)),
        )
    }

    fn tile(&self, index: usize, tile: Tile, y: f32, cx: &mut Context<Self>) -> gpui::AnyElement {
        let frame = self.action_frame(index, y);
        let palette = palette::current(cx);
        look::tile(&palette, tile.name, frame)
            .when(self.a11y, |plate| {
                button_named(plate, tile.label, (false, true, false))
            })
            .flex()
            .items_center()
            .justify_center()
            .on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                panel.tooltip(tile.label.into(), frame, *hovered, cx)
            }))
            .on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                panel.run_tile(tile, frame, event.modifiers().alt, By::of(event), cx)
            }))
            .child(
                svg()
                    .path(tile.glyph)
                    .size(px(tile.glyph_size))
                    .text_color(palette.on_overlay_glyph(TEXT)),
            )
            .children(self.ring(&Stop::Tile(tile.name), cx))
            .into_any_element()
    }

    /// The extensions' tiles, then the + tile, four to a row.
    fn extensions(&self, open: &Open, y: f32, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let palette = palette::current(cx);
        let plate = |id: SharedString, index: usize| {
            let frame = extension_frame(y, index);
            let element = look::tile(&palette, id, frame)
                .flex()
                .items_center()
                .justify_center();
            (element, frame)
        };
        let mut tiles: Vec<gpui::AnyElement> = (open.extensions.iter().enumerate())
            .map(|(index, extension)| {
                let (element, frame) = plate(format!("extension-{}", extension.id).into(), index);
                let (id, title) = (extension.id.clone(), extension.title.clone());
                let tile = open.in_window(frame);
                element
                    .when(self.a11y, |plate| {
                        button_named(plate, title.clone(), (false, true, true))
                    })
                    .on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                        panel.tooltip(title.clone(), frame, *hovered, cx)
                    }))
                    .on_click(cx.listener(move |panel, event, _, cx| {
                        let run = Run::Extension(id.clone(), tile);
                        panel.run("extension", run, By::of(event), cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |panel, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            panel.extension_menu(index, event.position, cx)
                        }),
                    )
                    .children((extension.icon.clone()).map(|icon| {
                        let icon = img(icon).size(px(EXTENSION_ICON));
                        icon.when(!extension.enabled, |icon| icon.opacity(ACTION_DIMMED))
                    }))
                    .children(self.ring(&Stop::Extension(extension.id.clone()), cx))
                    .into_any_element()
            })
            .collect();
        let (plus, frame) = plate("add-extension".into(), open.extensions.len());
        tiles.push(
            plus.when(self.a11y, |plus| {
                button_named(plus, "Add Extension", (false, true, false))
            })
            .on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                panel.tooltip("Add Extension".into(), frame, *hovered, cx)
            }))
            .on_click(cx.listener(|panel, event, _, cx| {
                let run = Run::Command(Command::ManageExtensions);
                panel.run("add-extension", run, By::of(event), cx)
            }))
            .child(
                svg()
                    .path("icons/plus.svg")
                    .size(px(PLUS_GLYPH))
                    .text_color(palette.on_overlay_glyph(TEXT)),
            )
            .children(self.ring(&Stop::AddExtension, cx))
            .into_any_element(),
        );
        tiles
    }

    /// The header (#4404): the site's icon on its plate, its name, the connection in quiet words
    /// under it, which open the certificate while there's one, the profile's chip, and "…".
    fn header(
        &self,
        open: &Open,
        site: &Site,
        top: f32,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let palette = palette::current(cx);
        let head = open.head;
        let mut parts = Vec::new();
        let plate = div()
            .absolute()
            .left(px(PADDING + HEADER_INSET))
            .top(px(top + (HEADER - FAVICON_PLATE) / 2.))
            .size(px(FAVICON_PLATE))
            .rounded(px(FAVICON_RADIUS))
            .bg(palette.on_float(Float::SiteControls, palette.plate(PLATE)))
            .flex()
            .items_center()
            .justify_center()
            .child(match open.favicon.clone() {
                Some(icon) => img(icon).size(px(FAVICON)).into_any_element(),
                None => svg()
                    .path("icons/globe.svg")
                    .size(px(FAVICON_GLYPH))
                    .text_color(palette.on_overlay_glyph(ITEM_GLYPH_OFF))
                    .into_any_element(),
            });
        parts.push(plate.into_any_element());
        let name = div()
            .absolute()
            .left(head.text.x)
            .top(head.text.y)
            .w(px(head.room))
            .h(px(NAME_LINE))
            .line_height(px(NAME_LINE))
            .text_size(px(NAME_SIZE))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(palette.on_overlay(TEXT))
            .truncate()
            .child(site.site.clone());
        parts.push(name.into_any_element());
        if let (Some(frame), Some(words)) = (head.lock, connection_words(site)) {
            let glyph = match site.connection {
                Connection::Secure => "icons/lock.svg",
                _ => "icons/lock-open.svg",
            };
            let lock = look::pressable(&palette, "site-lock", frame, MORE_FILL)
                .when(self.a11y, |lock| {
                    button_named(lock, words, (false, site.certificate, false))
                })
                .when(!site.certificate, |lock| {
                    // Nothing to show: it takes no pointer's plate and no click.
                    lock.hover(|style| style.bg(gpui::transparent_black()))
                        .active(|style| style.bg(gpui::transparent_black()))
                })
                .when(site.certificate, |lock| {
                    lock.on_click(cx.listener(|panel, event, _, cx| {
                        panel.run("certificate", Run::Certificate, By::of(event), cx)
                    }))
                })
                .flex()
                .items_center()
                .gap(px(CONNECTION_GLYPH_GAP))
                .px(px(CONNECTION_PAD))
                .child(
                    svg()
                        .path(glyph)
                        .flex_none()
                        .size(px(CONNECTION_GLYPH))
                        .text_color(palette.on_overlay_glyph(SECONDARY)),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .line_height(px(CONNECTION_LINE))
                        .text_size(px(CONNECTION_SIZE))
                        .text_color(palette.on_overlay(SECONDARY))
                        .child(words),
                )
                .children(self.ring(&Stop::Lock, cx));
            parts.push(lock.into_any_element());
        }
        if let (Some((frame, shown)), Some(profiles)) = (head.chip, open.profiles.clone()) {
            let name = profiles.name.clone();
            let chip = look::pressable(&palette, "site-profile", frame, MORE_FILL)
                .when(self.a11y, |chip| {
                    button_named(chip, said("Profile", &name), (true, true, false))
                })
                .flex()
                .items_center()
                .px(px(CHIP_INSET))
                .gap(px(PROFILE_CHEVRON_GAP))
                .child(
                    svg()
                        .path("icons/users.svg")
                        .flex_none()
                        .size(px(CHIP_GLYPH))
                        .text_color(palette.on_overlay_glyph(TEXT)),
                )
                .when(shown > 0., |chip| {
                    chip.child(
                        div()
                            .flex_none()
                            .w(px(shown))
                            .truncate()
                            .text_size(px(NAME_SIZE))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(palette.on_overlay(TEXT))
                            .child(name.clone()),
                    )
                })
                .child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .flex_none()
                        .size(px(PROFILE_CHEVRON))
                        .text_color(palette.on_overlay(SECONDARY)),
                )
                .when(head.chip_cut, |chip| {
                    chip.on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                        panel.tooltip(name.clone(), frame, *hovered, cx)
                    }))
                })
                .on_click(cx.listener(|panel, event: &ClickEvent, _, cx| {
                    panel.profiles(event.position(), cx)
                }))
                .children(self.ring(&Stop::Profile, cx));
            parts.push(chip.into_any_element());
        }
        let more = look::pressable(&palette, "site-more", head.more, MORE_FILL)
            .when(self.a11y, |more| {
                button_named(more, "More", (true, true, false))
            })
            .flex()
            .items_center()
            .justify_center()
            .on_click(
                cx.listener(|panel, event: &ClickEvent, _, cx| panel.more(event.position(), cx)),
            )
            .child(
                svg()
                    .path("icons/ellipsis.svg")
                    .size(px(MORE_GLYPH))
                    .text_color(palette.on_overlay_glyph(TEXT)),
            )
            .children(self.ring(&Stop::More, cx));
        parts.push(more.into_any_element());
        parts
    }

    /// A row of the site's plate at `frame`: its glyph, its name, then at the right its value in
    /// the secondary ink, which gives way first, with a chevron where a click opens a menu, or a
    /// switch.
    #[allow(clippy::too_many_arguments)]
    fn line(
        palette: &Palette,
        id: SharedString,
        frame: Bounds<Pixels>,
        ends: (bool, bool),
        glyph: &'static str,
        name: SharedString,
        value: Option<SharedString>,
        end: End,
    ) -> gpui::Stateful<gpui::Div> {
        look::row(palette, id, frame, ends)
            .flex()
            .items_center()
            .gap(px(ITEM_INSET))
            .px(px(ITEM_INSET))
            .text_size(px(NAME_SIZE))
            .child(
                svg()
                    .path(glyph)
                    .flex_none()
                    .size(px(ITEM_GLYPH))
                    .text_color(palette.on_overlay_glyph(ITEM_GLYPH_OFF)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(palette.on_overlay(TEXT))
                    .child(name),
            )
            // No empty cell where there's no value, whose gap would cut the name.
            .children(value.map(|value| {
                div()
                    .flex_none()
                    .max_w(px(ITEM_VALUE))
                    .truncate()
                    .text_right()
                    .text_color(palette.on_overlay(SECONDARY))
                    .child(value)
            }))
            .map(|row| match end {
                End::Nothing => row,
                End::Chevron => row.child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .flex_none()
                        .size(px(CHEVRON))
                        .with_transformation(Transformation::rotate(Radians(-FRAC_PI_2)))
                        .text_color(palette.on_overlay(SECONDARY)),
                ),
                End::Switch(on) => row.child(look::switch(palette, on)),
            })
    }

    /// The site's plate from `top`: its rows, with hairlines between them from the names' edge. One
    /// a policy sets takes no click, so it reads as dimmed.
    fn rows(
        &self,
        site: &Site,
        top: f32,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let palette = palette::current(cx);
        let a11y = self.a11y;
        let rows = rows(site);
        let count = rows.len();
        let plate = Bounds::new(
            point(px(PADDING), px(top)),
            size(px(WIDTH - 2. * PADDING), px(count as f32 * ROW)),
        );
        let mut elements = vec![look::group(&palette, plate).into_any_element()];
        for (index, row) in rows.into_iter().enumerate() {
            let frame = row_frame(top, index);
            let ends = (index == 0, index + 1 == count);
            let line = |id: String, glyph, name: SharedString, value, end| {
                Self::line(&palette, id.into(), frame, ends, glyph, name, value, end)
            };
            let element = match row {
                Row::AdBlocking(on) => line(
                    "site-ad-blocking".into(),
                    SHIELD,
                    AD_BLOCKING_NAME.into(),
                    None,
                    End::Switch(on),
                )
                .when(a11y, |row| switch_named(row, AD_BLOCKING_NAME, on))
                .on_click(cx.listener(move |panel, _, _, cx| panel.toggle_ad_blocking(!on, cx)))
                .children(self.ring(&Stop::AdBlocking, cx)),
                Row::Switch(switch) => {
                    let id = format!(
                        "site-switch-{}",
                        switch.name().to_lowercase().replace(' ', "-")
                    );
                    let (value, end) = match switch {
                        // Its click is a menu of On and Off.
                        Switch::Engine(on) => {
                            (Some(if on { "On" } else { "Off" }.into()), End::Chevron)
                        }
                        Switch::Boost(true) => {
                            (site.arrangement_misfits.clone(), End::Switch(true))
                        }
                        switch => (None, End::Switch(switch.on())),
                    };
                    line(id, switch.glyph(), switch.name().into(), value, end)
                        .when(a11y, |row| match switch {
                            Switch::Engine(on) => {
                                let state = if on { "On" } else { "Off" };
                                button_named(row, said(ENGINE_NAME, state), (true, true, false))
                            }
                            switch => switch_named(row, switch.name(), switch.on()),
                        })
                        .on_click(cx.listener(
                            move |panel, event: &ClickEvent, _, cx| match switch {
                                Switch::DeveloperMode => panel.toggle_developer_mode(false, cx),
                                Switch::Engine(on) => panel.choose_engine(on, event.position(), cx),
                                Switch::Boost(on) => panel.toggle_boost(!on, cx),
                                Switch::Recent(on) => panel.toggle_recent(!on, cx),
                            },
                        ))
                        .children(self.ring(&switch.stop(), cx))
                }
                // An arrangement that changed outside ilo, kept off, and why: a click is Turn On
                // (#2347).
                Row::Arrangement(off) => line(
                    "site-arrangement".into(),
                    ARRANGEMENT,
                    ARRANGEMENT_NAME.into(),
                    Some(off),
                    End::Nothing,
                )
                .when(a11y, |row| switch_named(row, ARRANGEMENT_NAME, false))
                .on_click(cx.listener(|panel, _, _, cx| panel.turn_on_arrangement(cx)))
                .children(self.ring(&Stop::Arrangement, cx)),
                Row::Permission(index) => {
                    let permission = site.permissions[index].clone();
                    let managed = permission.choices.is_empty();
                    let end = if managed { End::Nothing } else { End::Chevron };
                    let said = said(&permission.name, &permission.state);
                    let row = line(
                        format!("site-permission-{index}"),
                        glyph_for(&permission.key),
                        permission.name.clone(),
                        Some(permission.state.clone()),
                        end,
                    )
                    .when(a11y, |row| {
                        button_named(row, said, (!managed, !managed, false))
                    });
                    match managed {
                        true => row.opacity(MANAGED_OPACITY),
                        false => row
                            .children(self.ring(&Stop::Permission(permission.kind), cx))
                            .on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                                panel.choose_value(permission.clone(), event.position(), cx)
                            })),
                    }
                }
                Row::Device(index) => {
                    let device = site.devices[index].clone();
                    let removable = device.removable;
                    let end = if removable {
                        End::Chevron
                    } else {
                        End::Nothing
                    };
                    let said = said(&device.name, &device.kind);
                    let row = line(
                        format!("site-device-{index}"),
                        device.glyph.glyph(),
                        device.name.clone(),
                        Some(device.kind.clone()),
                        end,
                    )
                    .when(a11y, |row| {
                        button_named(row, said, (removable, removable, false))
                    });
                    match removable {
                        false => row.opacity(MANAGED_OPACITY),
                        true => {
                            row.children(self.ring(&Stop::Device(index), cx))
                                .on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                                    panel.choose_device(index, device.clone(), event.position(), cx)
                                }))
                        }
                    }
                }
                Row::Passwords => {
                    let Some((state, _)) = passwords::state(&site.logins) else {
                        continue;
                    };
                    let said = said(PASSWORDS_NAME, &state);
                    let logins = site.logins.clone();
                    line(
                        "site-passwords".into(),
                        PASSWORDS,
                        PASSWORDS_NAME.into(),
                        Some(state),
                        End::Chevron,
                    )
                    .when(a11y, |row| button_named(row, said, (true, true, false)))
                    .on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                        panel.choose_login(logins.clone(), event.position(), cx)
                    }))
                    .children(self.ring(&Stop::Passwords, cx))
                }
            };
            if index > 0 {
                let from = ITEM_INSET + ITEM_GLYPH + ITEM_INSET;
                let line = Bounds::new(
                    point(frame.origin.x + px(from), frame.origin.y),
                    size(frame.size.width - px(from), px(0.)),
                );
                elements.push(look::hairline(&palette, line, scale).into_any_element());
            }
            elements.push(element.into_any_element());
        }
        elements
    }

    /// The foot, while Reset permissions has something to reset: a line under the extensions, and
    /// its row.
    fn foot(
        &self,
        site: &Site,
        line: f32,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        if site.reset.is_none() {
            return Vec::new();
        }
        let palette = palette::current(cx);
        let rule = Bounds::new(
            point(px(PADDING), px(line)),
            size(px(WIDTH - 2. * PADDING), px(0.)),
        );
        let row = look::pressable(
            &palette,
            "site-permissions-reset",
            foot_frame(line),
            MORE_FILL,
        )
        .when(self.a11y, |row| {
            button_named(row, RESET_NAME, (false, true, false))
        })
        .flex()
        .items_center()
        .gap(px(ITEM_INSET))
        .px(px(ITEM_INSET))
        .text_size(px(NAME_SIZE))
        .child(
            svg()
                .path(RESET)
                .flex_none()
                .size(px(ITEM_GLYPH))
                .text_color(palette.on_overlay_glyph(ITEM_GLYPH_OFF)),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_color(palette.on_overlay(TEXT))
                .child(RESET_NAME),
        )
        .on_click(cx.listener(|panel, _, _, cx| panel.reset(cx)))
        .children(self.ring(&Stop::Reset, cx));
        vec![
            look::hairline(&palette, rule, scale).into_any_element(),
            row.into_any_element(),
        ]
    }
}

/// What a row of the site's plate ends with.
#[derive(Clone, Copy)]
enum End {
    Nothing,
    /// Its click opens a menu.
    Chevron,
    Switch(bool),
}

/// The `index`th extension's tile, or the + after them, in the grid from `top`.
fn extension_frame(top: f32, index: usize) -> Bounds<Pixels> {
    let (column, line) = (index % EXTENSIONS_PER_ROW, index / EXTENSIONS_PER_ROW);
    let x = PADDING + column as f32 * (TILE_WIDTH + GAP);
    Bounds::new(
        point(px(x), px(top + line as f32 * (TILE_HEIGHT + GAP))),
        size(px(TILE_WIDTH), px(TILE_HEIGHT)),
    )
}

/// Reset permissions' row, under the foot's line at `line`.
fn foot_frame(line: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(px(PADDING), px(line + FOOT_BELOW)),
        size(px(WIDTH - 2. * PADDING), px(FOOT_ROW)),
    )
}

impl Render for SiteControls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.a11y = window.is_a11y_active();
        let root = div()
            .key_context("SiteControls")
            .track_focus(&self.focus)
            .on_action(cx.listener(|panel, _: &Close, _, cx| {
                panel.close(dismissal::Close::new(Why::Escape, By::Keyboard), cx)
            }))
            .on_action(cx.listener(|panel, _: &NextStop, _, cx| panel.key_step(true, true, cx)))
            .on_action(
                cx.listener(|panel, _: &PreviousStop, _, cx| panel.key_step(false, true, cx)),
            )
            .on_action(cx.listener(|panel, _: &StopDown, _, cx| panel.key_step(true, false, cx)))
            .on_action(cx.listener(|panel, _: &StopUp, _, cx| panel.key_step(false, false, cx)))
            .on_action(cx.listener(|panel, _: &Press, _, cx| panel.key_press(cx)))
            // A press takes the ring away, as in the theme picker.
            .on_any_mouse_down(cx.listener(|panel, _, _, cx| {
                if let Some(open) = panel.open.as_mut().filter(|open| open.keyed.is_some()) {
                    open.keyed = None;
                    cx.notify();
                }
            }))
            // A tooltip shows once the pointer rests (tooltip.rs).
            .on_mouse_move(|_, _, cx| crate::tooltip::moved(cx))
            .size_full()
            .font_family(crate::face::UI);
        // The header's parts first, which its text's widths place and the log names.
        let system = window.text_system().clone();
        if let Some(open) = self.open.as_mut() {
            if let Some(site) = open.site.as_ref() {
                let top = layout(site, open.extensions.len(), open.beak.is_some()).header;
                open.head = head(site, open.profiles.as_ref(), top, &system);
            }
        }
        let places = self.places();
        let Some(open) = self.open.as_mut() else {
            return root;
        };
        let Some(site) = open.site.clone() else {
            return root;
        };
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, so the motion starts there rather than skip its start.
        if open.shown.is_none() && window.is_visible() {
            open.shown = Some(Instant::now());
        }
        let progress = open.progress();
        let scale = crate::motion::scale(FROM_SCALE + (1. - FROM_SCALE) * progress, cx);
        let origin = open.origin();
        let moving = open.moving();
        let settling = !moving && !open.settled;
        open.settled |= settling;
        if moving {
            window.request_animation_frame();
        }
        if moving || settling {
            self.host.drew(scale, progress, origin);
        }
        if open.settled && places != open.placed {
            // check.sh reads these lines, before "settled", to click by name.
            for place in &places {
                log!("browser: site controls {place}");
            }
            open.placed = places;
        }
        if settling {
            // check.sh reads this line.
            log!("browser: site controls settled");
        }
        let ids: Vec<SharedString> = (open.extensions.iter())
            .map(|each| each.id.clone())
            .collect();
        if ids != open.drawn {
            // check.sh reads this line.
            log!("browser: site controls drew extensions {}", ids.join(" "));
            open.drawn = ids;
        }
        let Some(open) = self.open.as_ref() else {
            return root;
        };
        let at = layout(&site, open.extensions.len(), open.beak.is_some());
        let shape = corners(open.beak, at.height);
        let palette = palette::current(cx);
        let (overlay, rim) = (
            Float::SiteControls.fill(palette.tone),
            Level::Popover.rim_in(&palette),
        );
        let card = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let mut fill = PathBuilder::fill();
                icon_picker::outline(&mut fill, bounds.origin, &shape, 0.);
                if let Ok(path) = fill.build() {
                    window.paint_path(path, overlay);
                }
                // The popover level's rim in the palette's ink, 1 pt inside the edge; its outline
                // and shadow are the host's, round the window.
                let mut edge = PathBuilder::stroke(px(1.));
                icon_picker::outline(&mut edge, bounds.origin, &shape, 0.5);
                if let Ok(path) = edge.build() {
                    window.paint_path(path, rim);
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let pixels = window.scale_factor();
        let header = self.header(open, &site, at.header, cx);
        let tiles: Vec<_> = (self.tiles(&site).into_iter().enumerate())
            .map(|(index, tile)| self.tile(index, tile, at.tiles, cx))
            .collect();
        let label_x = PADDING + HEADER_INSET;
        let site_plate = at.site_label.map(|label| {
            let mut parts = vec![
                look::label(&palette, SITE_LABEL, (label_x, label), self.a11y).into_any_element(),
            ];
            parts.extend(self.rows(&site, at.rows, pixels, cx));
            parts
        });
        let extensions_label = look::label(
            &palette,
            EXTENSIONS_LABEL,
            (label_x, at.extensions_label),
            self.a11y,
        );
        let extensions = self.extensions(open, at.extensions, cx);
        let foot = at.foot.map(|line| self.foot(&site, line, pixels, cx));
        let card = div()
            .id("site-controls")
            .relative()
            .size_full()
            .opacity(progress)
            .child(card)
            .children(header)
            .children(tiles)
            .children(site_plate.into_iter().flatten())
            .child(extensions_label)
            .children(extensions)
            .children(foot.into_iter().flatten());
        root.child(scaled(card, scale, origin))
    }
}

#[cfg(test)]
mod tests;
