//! The Site Controls: the panel the URL field's Site Controls button opens, as Arc's Site
//! Control Center (docs/ux-spec.md › Extensions › Site Controls). For the site of the page shown:
//! the action tiles, the turned-on extensions, whether ads and trackers are blocked there, its
//! saved logins, its permissions as Chrome's page info lists them, each changeable, and its
//! connection. GPUI can't draw over the page's native view, so it's a window of its own that the
//! host shows over the browser window, as its child, with the keyboard, as the command bar is,
//! over a blur in its shape with a shadow. What it asks of its window and the core goes through
//! `Host`. Tab and the arrows take a ring over its tiles, rows and footer buttons, and space or
//! return does what a click on the ringed one does (#1382).

use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    App, Bounds, ClickEvent, Context, FocusHandle, Global, Image, KeyBinding, MouseButton,
    MouseDownEvent, PathBuilder, Pixels, Point, SharedString, Size, Task, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    accesskit::{HasPopup, Role, Toggled},
    actions, canvas, div, img, linear_color_stop, linear_gradient, point,
    prelude::*,
    px, size, svg,
};

use crate::{
    icon_picker,
    palette::{self, Palette, STATE_HOVERED, STATE_PRESSED},
    passwords::{self, Login},
    shell::{Command, DeviceKind, MenuEntry, cubic_bezier, scaled::scaled},
    tokens::shell::{KEYED, KEYED_RING},
    tooltip,
};

actions!(
    site_controls,
    [Close, NextStop, PreviousStop, StopDown, StopUp, Press]
);

// Arc 1.170.0 cap, in a dark Space, 1 pt a pixel, except where a comment says ours.
/// The body's width: a row of four tiles inside the padding.
pub const WIDTH: f32 = 4. * TILE_WIDTH + 3. * GAP + 2. * PADDING;
use crate::elevation::Level;
use crate::tokens::shell::extensions::ACTION_DIMMED;
use crate::tokens::site_controls::BEAK;
use crate::tokens::site_controls::BEAK_FOOT_RADIUS;
use crate::tokens::site_controls::BEAK_TIP_RADIUS;
use crate::tokens::site_controls::BELOW_FIELD;
use crate::tokens::site_controls::CIRCLE;
use crate::tokens::site_controls::EXTENSION_ICON;
use crate::tokens::site_controls::GAP;
use crate::tokens::site_controls::HEADING_SIZE;
use crate::tokens::site_controls::LINK;
use crate::tokens::site_controls::LINK_SIZE;
use crate::tokens::site_controls::NAME_SIZE;
use crate::tokens::site_controls::PADDING;
use crate::tokens::site_controls::PLATE;
use crate::tokens::site_controls::PLUS_GLYPH;
use crate::tokens::site_controls::RADIUS;
use crate::tokens::site_controls::ROW_PITCH;
use crate::tokens::site_controls::SECONDARY;
use crate::tokens::site_controls::STATE_SIZE;
use crate::tokens::site_controls::TEXT;
use crate::tokens::site_controls::TILE_BOTTOM;
use crate::tokens::site_controls::TILE_HEIGHT;
use crate::tokens::site_controls::TILE_HIGHLIGHT;
use crate::tokens::site_controls::TILE_RADIUS;
use crate::tokens::site_controls::TILE_TOP;
use crate::tokens::site_controls::TILE_WIDTH;
use crate::tokens::site_controls::WINDOW_MARGIN;
/// The text's left edge, past the circle.
const ROW_TEXT: f32 = PADDING + CIRCLE + 8.;
use crate::tokens::site_controls::OFF;
use crate::tokens::site_controls::OFF_GLYPH;
use crate::tokens::site_controls::ROW_GLYPH;
/// Ours: the reset row's glyph, as Chrome's page info's button has none.
const RESET: &str = "icons/rotate-ccw-clock.svg";
/// Ours: the row of a site the profile keeps for Safari's engine, first, as the URL field's mark
/// for it. Named for Safari, which people know, though the page plays here, in ilo's tab.
const ENGINE: &str = "icons/compass.svg";
const ENGINE_NAME: &str = "Safari's engine";

/// Ours: the ad blocking row's, which Arc's panel didn't have.
const AD_BLOCKING: &str = "icons/shield.svg";
/// Ours: the site's logins, in place of Chrome's key icon and its manage bubble.
const PASSWORDS: &str = "icons/key-round.svg";
/// Ours: Developer Mode's row, as Arc's panel's isn't recorded, and its command's in the bar.
const DEVELOPER_MODE: &str = "icons/square-code.svg";
const DEVELOPER_MODE_NAME: &str = "Developer Mode";
/// The site's Boost (#695), with the tile's glyph.
const BOOST: &str = "icons/paintbrush.svg";
const BOOST_NAME: &str = "Boost";
const PASSWORDS_NAME: &str = "Passwords";
use crate::tokens::site_controls::DISABLED_OPACITY;
use crate::tokens::site_controls::DIVIDER;
use crate::tokens::site_controls::FOOTER;
use crate::tokens::site_controls::LOCK_CAP;
use crate::tokens::site_controls::LOCK_GLYPH;
use crate::tokens::site_controls::LOCK_LEFT;
use crate::tokens::site_controls::LOCK_RADIUS;
use crate::tokens::site_controls::LOCK_RIGHT;
use crate::tokens::site_controls::LOCK_SLOT;
use crate::tokens::site_controls::LOCK_TEXT_GAP;
use crate::tokens::site_controls::MANAGED_OPACITY;
use crate::tokens::site_controls::MORE_FILL;
use crate::tokens::site_controls::MORE_GLYPH;
// Down the body, each from the top of what's above it.
const HEADING_CAP: f32 = TILE_HEIGHT + 14.;
use crate::tokens::site_controls::HEADING_TO_TILES;
const TILES_TO_HEADING: f32 = TILE_HEIGHT + 18.;
use crate::tokens::site_controls::HEADING_TO_ROWS;
const ROWS_TO_DIVIDER: f32 = CIRCLE + 15.;
use crate::tokens::site_controls::DIVIDER_TO_FOOTER;
/// Ours: the extensions' rows are as far apart as their tiles.
const EXTENSIONS_PER_ROW: usize = 4;
use crate::tokens::site_controls::ASCENT;
use crate::tokens::site_controls::CAP;
use crate::tokens::site_controls::DESCENT;
use crate::tokens::site_controls::LINE;
/// Ours, as before: it opens in 0.15 s and closes in 0.1 s, growing from the beak's tip and
/// shrinking back into it, from and to this much of its size, as it fades.
const OPEN: f32 = 0.15;
const CLOSE: f32 = 0.1;
const FROM_SCALE: f32 = 0.9;
/// The keyboard's ring is this far outside a row's circle, as round a theme's swatch.
const RING_GAP: f32 = 1.;

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
    /// Whether the site's tabs show in Developer Mode, none where that isn't the user's to change:
    /// a page that isn't http(s), or a private window.
    pub developer_mode: Option<bool>,
    /// The page can have a Boost (#695): an http(s) page in a regular window, outside an agent's
    /// Space, with Settings' Boosts on. The Boost tile and the "…" menu's Boost items show then.
    pub boostable: bool,
    /// Whether the site's Boost is on, none where it has none.
    pub boost: Option<bool>,
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

/// A heading whose link shows under the pointer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Heading {
    Extensions,
    Settings,
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
    extensions: Vec<Extension>,
    /// The grid's order as it last drew, by id.
    drawn: Vec<SharedString>,
    /// None until the core first reports, which is when it shows.
    site: Option<Site>,
    height: f32,
    hovered: Option<Heading>,
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

/// What the keyboard's ring can be on, down the panel: the tiles, the extensions' tiles and +, the
/// Settings rows a click changes, the lock button while it shows a certificate, and "…". The
/// headings' links, there only under the pointer, are the "…" menu's items too.
#[derive(Clone, PartialEq, Debug)]
enum Stop {
    Tile(&'static str),
    Extension(SharedString),
    AddExtension,
    Engine,
    AdBlocking,
    DeveloperMode,
    Boost,
    Passwords,
    /// By Chrome's ContentSettingsType.
    Permission(i32),
    /// By its place among the site's devices.
    Device(usize),
    Reset,
    Lock,
    More,
}

/// Where each part of the panel goes, down its window.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Layout {
    /// The body's top: under the beak, if it has one.
    body: f32,
    tiles: f32,
    extensions_cap: f32,
    extensions: f32,
    /// The Settings heading's cap and the first row's top, if there are rows.
    settings_cap: Option<f32>,
    rows: f32,
    divider: f32,
    footer: f32,
    height: f32,
}

/// The panel's layout for `site` with `extensions` tiles, under a beak or not.
fn layout(site: &Site, extensions: usize, beak: bool) -> Layout {
    let body = if beak { BEAK } else { 0. };
    let tiles = body + PADDING;
    let extensions_cap = tiles + HEADING_CAP;
    let grid = extensions_cap + HEADING_TO_TILES;
    let lines = (extensions + 1).div_ceil(EXTENSIONS_PER_ROW) as f32;
    let grid_bottom = grid + lines * TILE_HEIGHT + (lines - 1.) * GAP;
    let rows = rows(site);
    let (settings_cap, first, divider) = match rows {
        // Ours: with nothing to list, the heading goes, and the line follows the tiles.
        0 => (None, grid_bottom, grid_bottom + ROWS_TO_DIVIDER - CIRCLE),
        rows => {
            let cap = grid_bottom - TILE_HEIGHT + TILES_TO_HEADING;
            let first = cap + HEADING_TO_ROWS;
            (
                Some(cap),
                first,
                first + (rows - 1) as f32 * ROW_PITCH + ROWS_TO_DIVIDER,
            )
        }
    };
    let footer = divider + DIVIDER_TO_FOOTER;
    Layout {
        body,
        tiles,
        extensions_cap,
        extensions: grid,
        settings_cap,
        rows: first,
        divider,
        footer,
        height: footer + FOOTER + PADDING,
    }
}

/// The Settings heading's rows: Safari's engine, ad blocking, Developer Mode, the Boost,
/// Passwords, the permissions, the devices, then Reset.
fn rows(site: &Site) -> usize {
    let firsts = site.system_engine.is_some() as usize
        + site.ad_blocking.is_some() as usize
        + site.developer_mode.is_some() as usize
        + site.boost.is_some() as usize;
    let logins = passwords::state(&site.logins).is_some() as usize;
    firsts + logins + site.permissions.len() + site.devices.len() + site.reset.is_some() as usize
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

/// Where a line of text `size` tall goes for its cap's top to be at `cap`, in a LINE-tall box.
fn text_top(cap: f32, size: f32) -> f32 {
    cap - ((LINE - size * (ASCENT + DESCENT)) / 2. + size * (ASCENT - CAP))
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
/// `window`'s size, with the tab's `extensions`, once the core has read the site. If it shows,
/// closes it instead, as its button's second press does.
pub fn show(
    tab: u64,
    place: Place,
    window: Size<Pixels>,
    extensions: Vec<Extension>,
    cx: &mut App,
) {
    crate::lazy_window::ready::<SiteControls>(cx);
    update(cx, |panel, _, cx| {
        if panel
            .open
            .as_ref()
            .is_some_and(|open| open.closing.is_none())
        {
            return panel.close("toggle", cx);
        }
        let (at, beak) = placed(place, window);
        log_extensions(&extensions);
        panel.open = Some(Open {
            tab,
            at,
            beak,
            window,
            extensions,
            drawn: Vec::new(),
            site: None,
            height: 0.,
            hovered: None,
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
    update(cx, |panel, _, cx| panel.close("dismissed", cx));
}

/// The tab closed, and its panel with it.
pub fn tab_closed(tab: u64, cx: &mut App) {
    update(cx, |panel, _, cx| {
        if panel.open.as_ref().is_some_and(|open| open.tab == tab) {
            panel.close("tab closed", cx);
        }
    });
}

/// The site for the log: Chrome's words for the connection, then each row and its value, in their
/// order, which check.sh clicks by.
fn describe(site: &Site) -> String {
    let ad_blocking = (site.ad_blocking.iter())
        .map(|&on| format!("Ad blocking {}", if on { "On" } else { "Off" }));
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

/// The lock button's words: Chrome's short ones for the connection, the page's own for a file or
/// one of the browser's pages, which Chrome gives none.
fn lock_label(site: &Site) -> SharedString {
    match site.connection {
        Connection::Secure => "Secure".into(),
        Connection::NotSecure => "Not secure".into(),
        Connection::Internal => site.site.clone(),
    }
}

/// The "…" menu: Arc's, its Boost items where the page can have one.
fn more_items(site: &Site) -> Vec<(MenuEntry, Option<Run>)> {
    let item = |title: &'static str, run: Run| (MenuEntry::new(title, true), Some(run));
    let mut items = vec![
        item("Clear Cache", Run::Command(Command::ClearCacheAndRefresh)),
        item(
            "Clear Cookies",
            Run::Command(Command::ClearCookiesAndRefresh),
        ),
    ];
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

/// A Settings row that turns its setting on or off, read as a switch called `name`.
fn switch_named<E: StatefulInteractiveElement>(element: E, name: &'static str, on: bool) -> E {
    let toggled = if on { Toggled::True } else { Toggled::False };
    element
        .role(Role::Switch)
        .aria_label(name)
        .aria_toggled(toggled)
}

/// A row's name and its state, as VoiceOver reads a tab's row: "Location, Ask".
fn said(name: &str, state: &str) -> String {
    format!("{name}, {state}")
}

impl SiteControls {
    /// Closes the panel back into its button, giving the browser window the keyboard and the
    /// clicks at once.
    fn close(&mut self, why: &str, cx: &mut Context<Self>) {
        let Some(open) = self.open.as_mut().filter(|open| open.closing.is_none()) else {
            return;
        };
        open.closing = Some((Instant::now(), open.progress()));
        self.host.release();
        self.host.stop_reading();
        self.host.tooltip(None, cx);
        // check.sh reads this line.
        log!("browser: site controls closing ({why})");
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
    fn run(&mut self, name: &str, run: Run, cx: &mut Context<Self>) {
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
        self.close(name, cx);
        self.host.run(run, tab, cx);
    }

    /// Runs `tile`. ⌥ on Capture Full Page captures in Portrait Mode on the background kept, as
    /// Arc's ⌥-click on its Portrait Mode skips the panel.
    fn run_tile(&mut self, tile: Tile, frame: Bounds<Pixels>, alt: bool, cx: &mut Context<Self>) {
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
        self.run(tile.name, run, cx);
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
            panel.update(cx, |panel, cx| panel.run(&name, run, cx)).ok();
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
                            panel.run("remove-login", Run::RemoveLogin(login), cx)
                        }
                        passwords::Action::Allow(login) => {
                            // check.sh reads this line.
                            log!("browser: site controls allow saving {}", login.site);
                            panel.host.allow_saving(tab, &login);
                        }
                        passwords::Action::ShowAll => panel.run("passwords", Run::Passwords, cx),
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
                        false => panel.run(&format!("extension {}", item.name()), run, cx),
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

    fn set_hovered(&mut self, heading: Heading, hovered: bool, cx: &mut Context<Self>) {
        if let Some(open) = self.open.as_mut() {
            match hovered {
                true => open.hovered = Some(heading),
                false if open.hovered == Some(heading) => open.hovered = None,
                false => return,
            }
            cx.notify();
        }
    }

    /// The stops the keyboard goes over, in their order down the panel, each with its frame in the
    /// panel's window, which a menu hangs from: the circle's for a row.
    fn stops(&self) -> Vec<(Stop, Bounds<Pixels>)> {
        let Some(open) = self.open.as_ref().filter(|open| open.closing.is_none()) else {
            return Vec::new();
        };
        let Some(site) = open.site.as_ref() else {
            return Vec::new();
        };
        let at = layout(site, open.extensions.len(), open.beak.is_some());
        let tiles = self.tiles(site).into_iter().enumerate();
        let mut stops: Vec<(Stop, Bounds<Pixels>)> = tiles
            .map(|(index, tile)| {
                let x = PADDING + index as f32 * (TILE_WIDTH + GAP);
                (Stop::Tile(tile.name), Self::tile_frame(x, at.tiles))
            })
            .collect();
        let extension = |index: usize| {
            let (column, line) = (index % EXTENSIONS_PER_ROW, index / EXTENSIONS_PER_ROW);
            let x = PADDING + column as f32 * (TILE_WIDTH + GAP);
            Self::tile_frame(x, at.extensions + line as f32 * (TILE_HEIGHT + GAP))
        };
        for (index, each) in open.extensions.iter().enumerate() {
            stops.push((Stop::Extension(each.id.clone()), extension(index)));
        }
        stops.push((Stop::AddExtension, extension(open.extensions.len())));
        // The rows in render's order, a managed permission's taking its place without a stop.
        let rows = [
            site.system_engine.map(|_| Some(Stop::Engine)),
            site.ad_blocking.map(|_| Some(Stop::AdBlocking)),
            site.developer_mode.map(|_| Some(Stop::DeveloperMode)),
            site.boost.map(|_| Some(Stop::Boost)),
            passwords::state(&site.logins).map(|_| Some(Stop::Passwords)),
        ];
        let permissions = (site.permissions.iter())
            .map(|each| Some((!each.choices.is_empty()).then_some(Stop::Permission(each.kind))));
        let devices = (site.devices.iter().enumerate())
            .map(|(index, each)| each.removable.then_some(Stop::Device(index)));
        let reset = site.reset.as_ref().map(|_| Some(Stop::Reset));
        let rows = rows
            .into_iter()
            .flatten()
            .chain(permissions.flatten())
            .chain(devices)
            .chain(reset);
        for (index, stop) in rows.enumerate() {
            let y = at.rows + index as f32 * ROW_PITCH;
            let circle = Bounds::new(point(px(PADDING), px(y)), size(px(CIRCLE), px(CIRCLE)));
            stops.extend(stop.map(|stop| (stop, circle)));
        }
        let footer =
            |x: f32| Bounds::new(point(px(x), px(at.footer)), size(px(FOOTER), px(FOOTER)));
        if site.certificate {
            stops.push((Stop::Lock, footer(PADDING)));
        }
        stops.push((Stop::More, footer(WIDTH - PADDING - FOOTER)));
        stops
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

    /// Rings `stop`, naming a tile, which shows a glyph alone, with its tooltip.
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
            Stop::Engine | Stop::Passwords | Stop::Permission(_) | Stop::Device(_) | Stop::More
        );
        match stop {
            Stop::Tile(name) => {
                if let Some(&tile) = TILES.iter().find(|tile| tile.name == name) {
                    self.run_tile(tile, frame, false, cx);
                }
            }
            Stop::Extension(id) => {
                let tile = open.in_window(frame);
                self.run("extension", Run::Extension(id, tile), cx)
            }
            Stop::AddExtension => {
                self.run("add-extension", Run::Command(Command::ManageExtensions), cx)
            }
            Stop::Engine => self.choose_engine(site.system_engine == Some(true), under, cx),
            Stop::AdBlocking => self.toggle_ad_blocking(site.ad_blocking != Some(true), cx),
            Stop::DeveloperMode => {
                self.toggle_developer_mode(site.developer_mode != Some(true), cx)
            }
            Stop::Boost => self.toggle_boost(site.boost != Some(true), cx),
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
            Stop::Lock => self.run("certificate", Run::Certificate, cx),
            Stop::More => self.more(under, cx),
        }
        if menu {
            // After the host's own show, which waits for the overlays to open for a shortcut.
            cx.defer(crate::menu::highlight_first);
        }
    }

    /// The keyboard's ring, Focus Sidebar's, if it's on `stop`: inside a plate with corners of
    /// `radius`, or with none `RING_GAP` round a row's circle, as round a theme's swatch.
    fn ring(&self, stop: &Stop, radius: Option<f32>, cx: &App) -> Option<gpui::Div> {
        let keyed = self.open.as_ref().and_then(|open| open.keyed.as_ref());
        if keyed != Some(stop) {
            return None;
        }
        let ring = div()
            .absolute()
            .border(px(KEYED_RING))
            .border_color(palette::current(cx).plate(KEYED));
        Some(match radius {
            Some(radius) => ring.inset_0().rounded(px(radius)),
            None => {
                let reach = RING_GAP + KEYED_RING;
                ring.left(px(-reach))
                    .top(px(-reach))
                    .size(px(CIRCLE + 2. * reach))
                    .rounded_full()
            }
        })
    }

    /// The tiles the panel shows for `site`, closed up from the left. Without a share sheet, as on
    /// Linux, Share would do nothing, so it goes, and Boost goes where the page can have none,
    /// leaving the row's right end empty, as it already is.
    fn tiles(&self, site: &Site) -> Vec<Tile> {
        let share = self.host.has_share_sheet();
        (TILES.iter())
            .filter(|tile| share || tile.run != TileRun::Share)
            .filter(|tile| site.boostable || tile.run != TileRun::Command(Command::NewBoost))
            .copied()
            .collect()
    }

    /// A tile `TILE_WIDTH` by `TILE_HEIGHT` at (`x`, `y`), named by its tooltip.
    fn tile_frame(x: f32, y: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(TILE_WIDTH), px(TILE_HEIGHT)))
    }

    fn tile(&self, index: usize, tile: Tile, y: f32, cx: &mut Context<Self>) -> gpui::AnyElement {
        let frame = Self::tile_frame(PADDING + index as f32 * (TILE_WIDTH + GAP), y);
        let palette = palette::current(cx);
        div()
            .id(tile.name)
            .when(self.a11y, |plate| {
                button_named(plate, tile.label, (false, true, false))
            })
            .absolute()
            .left(frame.origin.x)
            .top(frame.origin.y)
            .w(frame.size.width)
            .h(frame.size.height)
            .rounded(px(TILE_RADIUS))
            .bg(linear_gradient(
                180.,
                linear_color_stop(palette.plate(TILE_TOP), 0.),
                linear_color_stop(palette.plate(TILE_BOTTOM), 1.),
            ))
            .border_t_1()
            .border_color(palette.plate(TILE_HIGHLIGHT))
            .flex()
            .items_center()
            .justify_center()
            .on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                panel.tooltip(tile.label.into(), frame, *hovered, cx)
            }))
            .on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                panel.run_tile(tile, frame, event.modifiers().alt, cx)
            }))
            .child(
                div()
                    .id("state")
                    .absolute()
                    .size_full()
                    .rounded(px(TILE_RADIUS))
                    .hover(move |style| style.bg(palette.plate(STATE_HOVERED)))
                    .active(move |style| style.bg(palette.plate(STATE_PRESSED))),
            )
            .child(
                svg()
                    .path(tile.glyph)
                    .size(px(tile.glyph_size))
                    .text_color(palette.on_overlay_glyph(TEXT)),
            )
            .children(self.ring(&Stop::Tile(tile.name), Some(TILE_RADIUS), cx))
            .into_any_element()
    }

    /// The extensions' tiles, then the + tile, four to a row.
    fn extensions(&self, open: &Open, y: f32, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let palette = palette::current(cx);
        let plate = |id: SharedString, index: usize| {
            let (column, line) = (index % EXTENSIONS_PER_ROW, index / EXTENSIONS_PER_ROW);
            let x = PADDING + column as f32 * (TILE_WIDTH + GAP);
            let frame = Self::tile_frame(x, y + line as f32 * (TILE_HEIGHT + GAP));
            let element = div()
                .id(id)
                .absolute()
                .left(frame.origin.x)
                .top(frame.origin.y)
                .w(frame.size.width)
                .h(frame.size.height)
                .rounded(px(TILE_RADIUS))
                .bg(palette.plate(PLATE))
                .hover(move |style| style.bg(palette.state(PLATE, STATE_HOVERED)))
                .active(move |style| style.bg(palette.state(PLATE, STATE_PRESSED)))
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
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        panel.run("extension", Run::Extension(id.clone(), tile), cx)
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
                    .children(self.ring(
                        &Stop::Extension(extension.id.clone()),
                        Some(TILE_RADIUS),
                        cx,
                    ))
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
            .on_click(cx.listener(|panel, _, _, cx| {
                panel.run("add-extension", Run::Command(Command::ManageExtensions), cx)
            }))
            .child(
                svg()
                    .path("icons/plus.svg")
                    .size(px(PLUS_GLYPH))
                    .text_color(palette.on_overlay_glyph(TEXT)),
            )
            .children(self.ring(&Stop::AddExtension, Some(TILE_RADIUS), cx))
            .into_any_element(),
        );
        tiles
    }

    /// A heading with its cap's top at `cap`, and its link while the pointer is on it.
    fn heading(
        &self,
        heading: Heading,
        cap: f32,
        hovered: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let (title, link, run) = match heading {
            Heading::Extensions => (
                "Extensions",
                "Manage",
                Run::Command(Command::ManageExtensions),
            ),
            Heading::Settings => ("Settings", "More", Run::SiteSettings),
        };
        // The heading's line, from the gap above it to the gap below.
        let (top, height) = (cap - 9., HEADING_SIZE * CAP + 18.);
        let text = |cap: f32, size: f32| {
            div()
                .absolute()
                .top(px(text_top(cap, size) - top))
                .h(px(LINE))
                .line_height(px(LINE))
                .text_size(px(size))
        };
        let link_cap = cap + (HEADING_SIZE - LINK_SIZE) * CAP / 2.;
        let name = link.to_lowercase();
        let palette = palette::current(cx);
        div()
            .id(SharedString::from(format!(
                "heading-{}",
                title.to_lowercase()
            )))
            .when(self.a11y, |row| row.role(Role::Heading).aria_label(title))
            .absolute()
            .left_0()
            .top(px(top))
            .w(px(WIDTH))
            .h(px(height))
            .on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                panel.set_hovered(heading, *hovered, cx)
            }))
            .child(
                text(cap, HEADING_SIZE)
                    .left(px(PADDING))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(palette.on_overlay(TEXT))
                    .child(title),
            )
            .when(hovered, |row| {
                row.child(
                    text(link_cap, LINK_SIZE)
                        .id(SharedString::from(format!("link-{name}")))
                        .when(self.a11y, |text| {
                            button_named(text, link, (false, true, false))
                        })
                        .right(px(PADDING))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(palette.on_overlay(LINK))
                        .on_click(
                            cx.listener(move |panel, _, _, cx| panel.run(&name, run.clone(), cx)),
                        )
                        .child(link),
                )
            })
            .into_any_element()
    }

    /// A row at `y` with a circle holding `glyph`, filled if `on`, `name` and `state` under it.
    fn row(
        palette: &Palette,
        id: SharedString,
        y: f32,
        glyph: &'static str,
        on: bool,
        name: SharedString,
        state: Option<SharedString>,
    ) -> gpui::Stateful<gpui::Div> {
        let text = |cap: f32, size: f32| {
            div()
                .absolute()
                .left(px(ROW_TEXT - PADDING))
                .w(px(WIDTH - ROW_TEXT - PADDING))
                .top(px(text_top(cap, size)))
                .h(px(LINE))
                .line_height(px(LINE))
                .text_size(px(size))
                .truncate()
        };
        let middle = CIRCLE / 2.;
        // The name's cap is 8 pt over the circle's middle and the state's 8 under, or the name's
        // on it alone.
        let name_cap = match state {
            Some(_) => middle - 8. - NAME_SIZE * CAP / 2.,
            None => middle - NAME_SIZE * CAP / 2.,
        };
        div()
            .id(id)
            .absolute()
            .left(px(PADDING))
            .top(px(y))
            .w(px(WIDTH - 2. * PADDING))
            .h(px(CIRCLE))
            .child(
                div()
                    .absolute()
                    .size(px(CIRCLE))
                    .rounded_full()
                    .bg(if on {
                        palette.accent.into()
                    } else {
                        palette.plate(OFF)
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(svg().path(glyph).size(px(ROW_GLYPH)).text_color(match on {
                        true => palette.accent_label.into(),
                        false => palette.on_overlay_glyph(OFF_GLYPH),
                    })),
            )
            .child(
                text(name_cap, NAME_SIZE)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(palette.on_overlay(TEXT))
                    .child(name),
            )
            .children(state.map(|state| {
                text(middle + 8. - STATE_SIZE * CAP / 2., STATE_SIZE)
                    .text_color(palette.on_overlay(SECONDARY))
                    .child(state)
            }))
    }

    fn permission(
        &self,
        index: usize,
        permission: &Permission,
        y: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let row = Self::row(
            &palette::current(cx),
            format!("site-permission-{index}").into(),
            y,
            glyph_for(&permission.key),
            permission.value == Value::Allow,
            permission.name.clone(),
            Some(permission.state.clone()),
        );
        // A row a policy sets takes no click, so it reads as dimmed.
        let managed = permission.choices.is_empty();
        let name = said(&permission.name, &permission.state);
        let row = row.when(self.a11y, |row| {
            button_named(row, name, (!managed, !managed, false))
        });
        match managed {
            true => row.opacity(MANAGED_OPACITY).into_any_element(),
            false => {
                let row = row.children(self.ring(&Stop::Permission(permission.kind), None, cx));
                let permission = permission.clone();
                row.on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                    panel.choose_value(permission.clone(), event.position(), cx)
                }))
                .into_any_element()
            }
        }
    }

    fn device(
        &self,
        index: usize,
        device: &Device,
        y: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let row = Self::row(
            &palette::current(cx),
            format!("site-device-{index}").into(),
            y,
            device.glyph.glyph(),
            true,
            device.name.clone(),
            Some(device.kind.clone()),
        );
        match device.removable {
            false => row.opacity(MANAGED_OPACITY).into_any_element(),
            true => {
                let row = row.children(self.ring(&Stop::Device(index), None, cx));
                let device = device.clone();
                row.on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                    panel.choose_device(index, device.clone(), event.position(), cx)
                }))
                .into_any_element()
            }
        }
    }

    fn footer(&self, site: &Site, y: f32, cx: &mut Context<Self>) -> [gpui::AnyElement; 2] {
        let lock = match site.connection {
            Connection::Secure => "icons/lock.svg",
            _ => "icons/lock-open.svg",
        };
        let palette = palette::current(cx);
        let button = div()
            .id("site-lock")
            .when(self.a11y, |button| {
                button_named(button, lock_label(site), (false, site.certificate, false))
            })
            .absolute()
            .left(px(PADDING))
            .top(px(y))
            .h(px(FOOTER))
            .max_w(px(WIDTH - 2. * PADDING - FOOTER - GAP))
            .rounded(px(LOCK_RADIUS))
            .bg(palette.plate(PLATE))
            .flex()
            .items_start()
            .pl(px(LOCK_LEFT))
            .pr(px(LOCK_RIGHT))
            .child(
                div()
                    .flex_none()
                    .w(px(LOCK_SLOT))
                    .h(px(FOOTER))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(lock)
                            .size(px(LOCK_GLYPH))
                            .text_color(palette.on_overlay_glyph(TEXT)),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .ml(px(LOCK_TEXT_GAP))
                    .mt(px(text_top(LOCK_CAP, NAME_SIZE)))
                    .h(px(LINE))
                    .line_height(px(LINE))
                    .text_size(px(NAME_SIZE))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(palette.on_overlay(TEXT))
                    .truncate()
                    .child(lock_label(site)),
            )
            .map(|button| match site.certificate {
                true => button
                    .hover(move |style| style.bg(palette.state(PLATE, STATE_HOVERED)))
                    .active(move |style| style.bg(palette.state(PLATE, STATE_PRESSED)))
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.run("certificate", Run::Certificate, cx)
                    })),
                false => button.opacity(DISABLED_OPACITY),
            })
            .children(self.ring(&Stop::Lock, Some(LOCK_RADIUS), cx));
        let more = div()
            .id("site-more")
            .when(self.a11y, |more| {
                button_named(more, "More", (true, true, false))
            })
            .absolute()
            .left(px(WIDTH - PADDING - FOOTER))
            .top(px(y))
            .size(px(FOOTER))
            .rounded_full()
            .bg(palette.plate(MORE_FILL))
            .hover(move |style| style.bg(palette.state(MORE_FILL, STATE_HOVERED)))
            .active(move |style| style.bg(palette.state(MORE_FILL, STATE_PRESSED)))
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
            .children(self.ring(&Stop::More, Some(FOOTER / 2.), cx));
        [button.into_any_element(), more.into_any_element()]
    }
}

impl Render for SiteControls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.a11y = window.is_a11y_active();
        let a11y = self.a11y;
        let root = div()
            .key_context("SiteControls")
            .track_focus(&self.focus)
            .on_action(cx.listener(|panel, _: &Close, _, cx| panel.close("escape", cx)))
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
        let (overlay, rim) = (palette.overlay, Level::Popover.rim_in(&palette));
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
        let tiles: Vec<_> = (self.tiles(&site).into_iter().enumerate())
            .map(|(index, tile)| self.tile(index, tile, at.tiles, cx))
            .collect();
        let extensions = self.extensions(open, at.extensions, cx);
        let extensions_heading = self.heading(
            Heading::Extensions,
            at.extensions_cap,
            open.hovered == Some(Heading::Extensions),
            cx,
        );
        let settings = at.settings_cap.map(|cap| {
            let hovered = open.hovered == Some(Heading::Settings);
            let mut rows = vec![self.heading(Heading::Settings, cap, hovered, cx)];
            let mut y = at.rows;
            if let Some(on) = site.system_engine {
                let state = if on { "On" } else { "Off" };
                let row = Self::row(
                    &palette,
                    "site-engine".into(),
                    y,
                    ENGINE,
                    on,
                    ENGINE_NAME.into(),
                    Some(state.into()),
                );
                let name = said(ENGINE_NAME, state);
                let row = row.when(a11y, |row| button_named(row, name, (true, true, false)));
                let row = row.on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                    panel.choose_engine(on, event.position(), cx)
                }));
                let row = row.children(self.ring(&Stop::Engine, None, cx));
                rows.push(row.into_any_element());
                y += ROW_PITCH;
            }
            // Before the permissions, as the row a page that doesn't work needs.
            if let Some(on) = site.ad_blocking {
                let state = if on { "On" } else { "Off for this site" };
                const NAME: &str = "Block ads and trackers";
                let state = Some(state.into());
                let id = "site-ad-blocking".into();
                let row = Self::row(&palette, id, y, AD_BLOCKING, on, NAME.into(), state);
                let row = row.when(a11y, |row| switch_named(row, NAME, on));
                let row = row.on_click(
                    cx.listener(move |panel, _, _, cx| panel.toggle_ad_blocking(!on, cx)),
                );
                let row = row.children(self.ring(&Stop::AdBlocking, None, cx));
                rows.push(row.into_any_element());
                y += ROW_PITCH;
            }
            if let Some(on) = site.developer_mode {
                let state = Some(if on { "On" } else { "Off" }.into());
                let (id, name) = ("site-developer-mode".into(), DEVELOPER_MODE_NAME.into());
                let row = Self::row(&palette, id, y, DEVELOPER_MODE, on, name, state);
                let row = row.when(a11y, |row| switch_named(row, DEVELOPER_MODE_NAME, on));
                let row = row.on_click(
                    cx.listener(move |panel, _, _, cx| panel.toggle_developer_mode(!on, cx)),
                );
                let row = row.children(self.ring(&Stop::DeveloperMode, None, cx));
                rows.push(row.into_any_element());
                y += ROW_PITCH;
            }
            if let Some(on) = site.boost {
                let state = Some(if on { "On" } else { "Off for this site" }.into());
                let (id, name) = ("site-boost".into(), BOOST_NAME.into());
                let row = Self::row(&palette, id, y, BOOST, on, name, state);
                let row = row.when(a11y, |row| switch_named(row, BOOST_NAME, on));
                let row =
                    row.on_click(cx.listener(move |panel, _, _, cx| panel.toggle_boost(!on, cx)));
                let row = row.children(self.ring(&Stop::Boost, None, cx));
                rows.push(row.into_any_element());
                y += ROW_PITCH;
            }
            let logins = passwords::state(&site.logins);
            if let Some((state, on)) = logins {
                let name = PASSWORDS_NAME.into();
                let said = said(PASSWORDS_NAME, &state);
                let row = Self::row(
                    &palette,
                    "site-passwords".into(),
                    y,
                    PASSWORDS,
                    on,
                    name,
                    Some(state),
                );
                let row = row.when(a11y, |row| button_named(row, said, (true, true, false)));
                let logins = site.logins.clone();
                let row = row.on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                    panel.choose_login(logins.clone(), event.position(), cx)
                }));
                let row = row.children(self.ring(&Stop::Passwords, None, cx));
                rows.push(row.into_any_element());
                y += ROW_PITCH;
            }
            for (index, permission) in site.permissions.iter().enumerate() {
                rows.push(self.permission(index, permission, y, cx));
                y += ROW_PITCH;
            }
            for (index, device) in site.devices.iter().enumerate() {
                rows.push(self.device(index, device, y, cx));
                y += ROW_PITCH;
            }
            if let Some(label) = site.reset.clone() {
                let id = "site-permissions-reset".into();
                let row = Self::row(&palette, id, y, RESET, false, label.clone(), None);
                let row = row.when(a11y, |row| button_named(row, label, (false, true, false)));
                let row = row.on_click(cx.listener(|panel, _, _, cx| panel.reset(cx)));
                let row = row.children(self.ring(&Stop::Reset, None, cx));
                rows.push(row.into_any_element());
            }
            rows
        });
        let divider = div()
            .absolute()
            .left(px(PADDING))
            .top(px(at.divider))
            .w(px(WIDTH - 2. * PADDING))
            .h(px(1.))
            .bg(palette.plate(DIVIDER));
        let card = div()
            .id("site-controls")
            .relative()
            .size_full()
            .opacity(progress)
            .child(card)
            .children(tiles)
            .child(extensions_heading)
            .children(extensions)
            .children(settings.into_iter().flatten())
            .child(divider)
            .children(self.footer(&site, at.footer, cx));
        root.child(scaled(card, scale, origin))
    }
}

#[cfg(test)]
mod tests;
