//! The Site Controls: the panel the URL field's Site Controls button opens, as Arc's Site
//! Control Center (docs/ux-spec.md › Extensions › Site Controls). For the site of the page shown:
//! the action tiles with the shield, whether ads and trackers are blocked there, last; the switches
//! special to the site; the turned-on extensions; its saved logins and the permissions that differ
//! for it, each changeable; and its connection (#2334). GPUI can't draw over the page's native view, so it's a window of its own that the
//! host shows over the browser window, as its child, with the keyboard, as the command bar is,
//! over a blur in its shape with a shadow. What it asks of its window and the core goes through
//! `Host`. Tab and the arrows take a ring over its tiles, switches, lines and footer buttons, in
//! their order down the panel, and space or return does what a click on the ringed one does
//! (#1382).

use std::{rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

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
use crate::tokens::site_controls::SECONDARY;
use crate::tokens::site_controls::TEXT;
use crate::tokens::site_controls::TILE_BOTTOM;
use crate::tokens::site_controls::TILE_HEIGHT;
use crate::tokens::site_controls::TILE_HIGHLIGHT;
use crate::tokens::site_controls::TILE_RADIUS;
use crate::tokens::site_controls::TILE_TOP;
use crate::tokens::site_controls::TILE_WIDTH;
use crate::tokens::site_controls::WINDOW_MARGIN;
use crate::tokens::site_controls::{
    HEADING_TO_ITEMS, ITEM_GLYPH, ITEM_GLYPH_OFF, ITEM_HEIGHT, ITEM_INSET, ITEM_NAME, ITEM_PITCH,
    ITEMS_TO_DIVIDER, STRIP_LEFT, STRIP_NAME_GAP, SWITCH_GLYPH, SWITCH_HEIGHT, SWITCH_NAME_GAP,
    TINT, TINT_HOVERED, TINT_PRESSED,
};
/// Ours: the reset line's glyph, as Chrome's page info's button has none.
const RESET: &str = "icons/rotate-ccw-clock.svg";
/// Ours: the switch of a site the profile keeps for Safari's engine, as the URL field's mark for
/// it. Named for Safari, which people know, though the page plays here, in ilo's tab.
const ENGINE: &str = "icons/compass.svg";
const ENGINE_NAME: &str = "Safari's engine";

/// Ours: the shield tile's, which Arc's panel didn't have, struck while ads are let through.
const SHIELD_ON: &str = "icons/shield.svg";
const SHIELD_OFF: &str = "icons/shield-off.svg";
/// Ours: the site's logins, in place of Chrome's key icon and its manage bubble.
const PASSWORDS: &str = "icons/key-round.svg";
/// Ours: Developer Mode's switch, as Arc's panel's isn't recorded, and its command's in the bar.
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
use crate::tokens::site_controls::{PROFILE_CHEVRON, PROFILE_CHEVRON_GAP, PROFILE_NAME_MIN};
use crate::tooltip::text_width;
// Down the body, each from the top of what's above it.
const HEADING_CAP: f32 = TILE_HEIGHT + 14.;
use crate::tokens::site_controls::HEADING_TO_TILES;
const TILES_TO_HEADING: f32 = TILE_HEIGHT + 18.;
/// With nothing to list, the line is this far under the extensions.
const GRID_TO_DIVIDER: f32 = 15.;
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
    /// "This site", over its lines.
    Site,
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
    profiles: Option<Profiles>,
    /// The grid's order as it last drew, by id.
    drawn: Vec<SharedString>,
    /// Where the shield, switches and lines were last logged.
    placed: Vec<String>,
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
    /// The permission whose row it rings once the core has read the site, by its lasting name.
    at_permission: Option<&'static str>,
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

/// What the keyboard's ring can be on, down the panel: the action tiles and the shield, the
/// switches, the extensions' tiles and +, the lines a click changes, the lock button while it shows
/// a certificate, and "…". The headings' links, there only under the pointer, are the "…" menu's
/// items too.
#[derive(Clone, PartialEq, Debug)]
enum Stop {
    Tile(&'static str),
    Extension(SharedString),
    AddExtension,
    Engine,
    /// The shield, last of the action tiles.
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
    Profile,
    More,
}

/// Where each part of the panel goes, down its window.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Layout {
    /// The body's top: under the beak, if it has one.
    body: f32,
    tiles: f32,
    /// The switch area's top, under the action tiles.
    switches: f32,
    extensions_cap: f32,
    extensions: f32,
    /// "This site"'s cap and its first line's top, if there are lines.
    site_cap: Option<f32>,
    lines: f32,
    divider: f32,
    footer: f32,
    height: f32,
}

/// The panel's layout for `site` with `extensions` tiles, under a beak or not.
fn layout(site: &Site, extensions: usize, beak: bool) -> Layout {
    let body = if beak { BEAK } else { 0. };
    let tiles = body + PADDING;
    let switches = tiles + TILE_HEIGHT + GAP;
    let area = match switch_area(switches_of(site).len()) {
        0. => 0.,
        height => GAP + height,
    };
    let extensions_cap = tiles + area + HEADING_CAP;
    let grid = extensions_cap + HEADING_TO_TILES;
    let lines = (extensions + 1).div_ceil(EXTENSIONS_PER_ROW) as f32;
    let grid_bottom = grid + lines * TILE_HEIGHT + (lines - 1.) * GAP;
    let (site_cap, first, divider) = match items(site) {
        // Ours: with nothing to list, the heading goes, and the line follows the tiles.
        0 => (None, grid_bottom, grid_bottom + GRID_TO_DIVIDER),
        items => {
            let cap = grid_bottom - TILE_HEIGHT + TILES_TO_HEADING;
            let first = cap + HEADING_TO_ITEMS;
            let last = first + (items - 1) as f32 * ITEM_PITCH;
            (Some(cap), first, last + ITEM_HEIGHT + ITEMS_TO_DIVIDER)
        }
    };
    let footer = divider + DIVIDER_TO_FOOTER;
    Layout {
        body,
        tiles,
        switches,
        extensions_cap,
        extensions: grid,
        site_cap,
        lines: first,
        divider,
        footer,
        height: footer + FOOTER + PADDING,
    }
}

/// "This site"'s lines: Passwords, the permissions, the devices, then Reset.
fn items(site: &Site) -> usize {
    let logins = passwords::state(&site.logins).is_some() as usize;
    logins + site.permissions.len() + site.devices.len() + site.reset.is_some() as usize
}

/// A switch special to the site, under the action tiles (#2334). Ad blocking is the shield in the
/// action row, always in reach, so it isn't one.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Switch {
    /// Only while on: off, it's in "…" and the command bar.
    DeveloperMode,
    /// Safari's engine where the profile keeps or offers it, on or not.
    Engine(bool),
    /// The site's Boost, where it has one, on or not.
    Boost(bool),
}

impl Switch {
    fn stop(self) -> Stop {
        match self {
            Switch::DeveloperMode => Stop::DeveloperMode,
            Switch::Engine(_) => Stop::Engine,
            Switch::Boost(_) => Stop::Boost,
        }
    }

    fn on(self) -> bool {
        match self {
            Switch::DeveloperMode => true,
            Switch::Engine(on) | Switch::Boost(on) => on,
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Switch::DeveloperMode => DEVELOPER_MODE,
            Switch::Engine(_) => ENGINE,
            Switch::Boost(_) => BOOST,
        }
    }

    /// Its name, under its glyph on a tile.
    fn name(self) -> &'static str {
        match self {
            Switch::DeveloperMode => DEVELOPER_MODE_NAME,
            Switch::Engine(_) => ENGINE_NAME,
            Switch::Boost(_) => BOOST_NAME,
        }
    }

    /// Its words on a strip, which say its state, as a lone tint can't.
    fn words(self) -> &'static str {
        match self {
            Switch::DeveloperMode => "Developer Mode on",
            Switch::Engine(true) => "In Safari's engine",
            Switch::Engine(false) => "Safari's engine off",
            Switch::Boost(true) => "Boost on",
            Switch::Boost(false) => "Boost off",
        }
    }
}

/// The site's switches, in their order: Developer Mode while on, Safari's engine, the Boost.
fn switches_of(site: &Site) -> Vec<Switch> {
    let developer_mode = (site.developer_mode == Some(true)).then_some(Switch::DeveloperMode);
    let engine = site.system_engine.map(Switch::Engine);
    let boost = site.boost.map(Switch::Boost);
    [developer_mode, engine, boost]
        .into_iter()
        .flatten()
        .collect()
}

/// Where the `index`th of `count` switches goes, from the area's top: one alone is a strip at the
/// full width; more are tiles two to a line, a last odd one keeping its size.
fn switch_frame(index: usize, count: usize, top: f32) -> Bounds<Pixels> {
    let full = WIDTH - 2. * PADDING;
    if count == 1 {
        return Bounds::new(point(px(PADDING), px(top)), size(px(full), px(TILE_HEIGHT)));
    }
    let width = (full - GAP) / 2.;
    let (column, line) = ((index % 2) as f32, (index / 2) as f32);
    Bounds::new(
        point(
            px(PADDING + column * (width + GAP)),
            px(top + line * (SWITCH_HEIGHT + GAP)),
        ),
        size(px(width), px(SWITCH_HEIGHT)),
    )
}

/// The switch area's height for `count` switches, without the gap over it.
fn switch_area(count: usize) -> f32 {
    match count {
        0 => 0.,
        1 => TILE_HEIGHT,
        count => {
            let lines = count.div_ceil(2) as f32;
            lines * SWITCH_HEIGHT + (lines - 1.) * GAP
        }
    }
}

/// The shield's words, its tooltip and what an assistive app reads, with how many requests it
/// blocked on the page while it blocks any, as uBlock Origin's badge counts them.
fn shield_label(site: &Site) -> Option<SharedString> {
    Some(match site.ad_blocking? {
        true if site.ad_blocked.is_empty() => "Ads and trackers blocked".into(),
        true => format!("Ads and trackers blocked, {} on this page", site.ad_blocked).into(),
        false => "Ads and trackers allowed".into(),
    })
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
    /// The permission, by Chrome's lasting name, whose row the next opening rings, as a held
    /// request's bell opens it at Notifications (#2323).
    at_permission: Option<&'static str>,
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
            at_permission: None,
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
            panel.at_permission = None;
            return panel.close("toggle", cx);
        }
        let (at, beak) = placed(place, window);
        log_extensions(&extensions);
        let at_permission = panel.at_permission.take();
        panel.open = Some(Open {
            tab,
            at,
            beak,
            window,
            extensions,
            profiles,
            drawn: Vec::new(),
            placed: Vec::new(),
            site: None,
            height: 0.,
            hovered: None,
            tip: None,
            shown: None,
            keyed: None,
            at_permission,
            settled: false,
            closing: None,
            _leave: None,
        });
        panel.host.read(tab);
        cx.notify();
    });
}

/// Has the panel's next opening ring the row of the permission Chrome calls `key`, such as
/// "notifications", once the core has read the site, if the site lists it. The panel's window is
/// made first if it's still to be made, or the first opening would lose the ring.
pub fn open_at_permission(key: &'static str, cx: &mut App) {
    crate::lazy_window::ready::<SiteControls>(cx);
    update(cx, |panel, _, _| panel.at_permission = Some(key));
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
        // Opened at a permission, its row is ringed, as the keyboard's Tab would ring it.
        let at = (open.at_permission.take())
            .and_then(|key| site.permissions.iter().find(|each| each.key == key))
            .map(|permission| Stop::Permission(permission.kind));
        if let Some(stop) = at {
            // check.sh reads this line.
            log!("browser: site controls keyed {stop:?}");
            open.keyed = Some(stop);
        }
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

/// The profile chip's right edge, a gap before "…" (#2339), where it stays whatever the lock says.
const PROFILE_RIGHT: f32 = WIDTH - PADDING - FOOTER - GAP;
/// The chip as its glyph and chevron alone.
const PROFILE_ALONE: f32 =
    LOCK_LEFT + LOCK_SLOT + PROFILE_CHEVRON_GAP + PROFILE_CHEVRON + LOCK_RIGHT;

/// Whether the lock is its glyph alone, a square like "…": a secure connection's beside the
/// profile's chip, so the name has the room. A page that isn't secure keeps its words, as that
/// warning matters more than the profile, and the chip gives way to its glyph instead.
fn lock_alone(site: &Site, profiles: Option<&Profiles>) -> bool {
    profiles.is_some() && matches!(site.connection, Connection::Secure)
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

/// The "…" menu: Arc's, its Boost items where the page can have one, and ours to turn Developer
/// Mode on, whose switch shows only while it's on (#2334).
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
            panel.update(cx, |panel, cx| panel.run(&name, run, cx)).ok();
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
    /// panel's window, which a menu hangs from.
    fn stops(&self) -> Vec<(Stop, Bounds<Pixels>)> {
        let Some(open) = self.open.as_ref().filter(|open| open.closing.is_none()) else {
            return Vec::new();
        };
        let Some(site) = open.site.as_ref() else {
            return Vec::new();
        };
        let at = layout(site, open.extensions.len(), open.beak.is_some());
        let tiles = self.tiles(site);
        let mut stops: Vec<(Stop, Bounds<Pixels>)> = (tiles.iter().enumerate())
            .map(|(index, tile)| (Stop::Tile(tile.name), self.action_frame(index, at.tiles)))
            .collect();
        if site.ad_blocking.is_some() {
            stops.push((Stop::AdBlocking, self.action_frame(tiles.len(), at.tiles)));
        }
        let switches = switches_of(site);
        for (index, switch) in switches.iter().enumerate() {
            let frame = switch_frame(index, switches.len(), at.switches);
            stops.push((switch.stop(), frame));
        }
        let extension = |index: usize| {
            let (column, line) = (index % EXTENSIONS_PER_ROW, index / EXTENSIONS_PER_ROW);
            let x = PADDING + column as f32 * (TILE_WIDTH + GAP);
            Self::tile_frame(x, at.extensions + line as f32 * (TILE_HEIGHT + GAP))
        };
        for (index, each) in open.extensions.iter().enumerate() {
            stops.push((Stop::Extension(each.id.clone()), extension(index)));
        }
        stops.push((Stop::AddExtension, extension(open.extensions.len())));
        // The lines in render's order, a managed one taking its place without a stop.
        let passwords = passwords::state(&site.logins).map(|_| Some(Stop::Passwords));
        let permissions = (site.permissions.iter())
            .map(|each| (!each.choices.is_empty()).then_some(Stop::Permission(each.kind)));
        let devices = (site.devices.iter().enumerate())
            .map(|(index, each)| each.removable.then_some(Stop::Device(index)));
        let reset = site.reset.as_ref().map(|_| Some(Stop::Reset));
        let lines = (passwords.into_iter())
            .chain(permissions)
            .chain(devices)
            .chain(reset);
        for (index, stop) in lines.enumerate() {
            let y = at.lines + index as f32 * ITEM_PITCH;
            let line = Bounds::new(
                point(px(PADDING), px(y)),
                size(px(WIDTH - 2. * PADDING), px(ITEM_HEIGHT)),
            );
            stops.extend(stop.map(|stop| (stop, line)));
        }
        let footer =
            |x: f32| Bounds::new(point(px(x), px(at.footer)), size(px(FOOTER), px(FOOTER)));
        if site.certificate {
            stops.push((Stop::Lock, footer(PADDING)));
        }
        if open.profiles.is_some() {
            stops.push((Stop::Profile, footer(PROFILE_RIGHT - FOOTER)));
        }
        stops.push((Stop::More, footer(WIDTH - PADDING - FOOTER)));
        stops
    }

    /// Where each tile, the shield, each switch, each line, the lock and "…" are, by name, as the middle of
    /// its frame in the browser window, such as "line Location at 207 252".
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
        for (index, tile) in self.tiles(site).iter().enumerate() {
            let frame = self.action_frame(index, at.tiles);
            places.push(format!("tile {} {}", tile.name, middle(frame)));
        }
        if site.ad_blocking.is_some() {
            let frame = self.action_frame(self.tiles(site).len(), at.tiles);
            places.push(format!("shield {}", middle(frame)));
        }
        let switches = switches_of(site);
        for (index, switch) in switches.iter().enumerate() {
            let frame = switch_frame(index, switches.len(), at.switches);
            places.push(format!("switch {} {}", switch.name(), middle(frame)));
        }
        let passwords = passwords::state(&site.logins).map(|_| SharedString::from(PASSWORDS_NAME));
        let names = (passwords.into_iter())
            .chain(site.permissions.iter().map(|each| each.name.clone()))
            .chain(site.devices.iter().map(|each| each.name.clone()))
            .chain(site.reset.clone());
        for (index, name) in names.enumerate() {
            let y = at.lines + index as f32 * ITEM_PITCH;
            let frame = Bounds::new(
                point(px(PADDING), px(y)),
                size(px(WIDTH - 2. * PADDING), px(ITEM_HEIGHT)),
            );
            places.push(format!("line {name} {}", middle(frame)));
        }
        let footer =
            |x: f32| Bounds::new(point(px(x), px(at.footer)), size(px(FOOTER), px(FOOTER)));
        places.push(format!("lock {}", middle(footer(PADDING))));
        places.push(format!("more {}", middle(footer(WIDTH - PADDING - FOOTER))));
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
            // A lock or a chip that's a glyph alone names itself in its tooltip.
            Stop::Lock => (open.site.as_ref())
                .filter(|site| lock_alone(site, open.profiles.as_ref()))
                .map(lock_label),
            Stop::Profile => (open.profiles.as_ref())
                .filter(|_| {
                    !open
                        .site
                        .as_ref()
                        .is_some_and(|site| lock_alone(site, open.profiles.as_ref()))
                })
                .map(|profiles| profiles.name.clone()),
            Stop::AdBlocking => (open.site.as_ref()).and_then(shield_label),
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
            Stop::Profile => self.profiles(under, cx),
            Stop::More => self.more(under, cx),
        }
        if menu {
            // After the host's own show, which waits for the overlays to open for a shortcut.
            cx.defer(crate::menu::highlight_first);
        }
    }

    /// The keyboard's ring, Focus Sidebar's, if it's on `stop`: inside its plate, whose corners
    /// are of `radius`.
    fn ring(&self, stop: &Stop, radius: f32, cx: &App) -> Option<gpui::Div> {
        let keyed = self.open.as_ref().and_then(|open| open.keyed.as_ref());
        if keyed != Some(stop) {
            return None;
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .rounded(px(radius))
                .border(px(KEYED_RING))
                .border_color(palette::current(cx).plate(KEYED)),
        )
    }

    /// The tiles the panel shows for `site`, closed up from the left, before the shield. Without a
    /// share sheet, as on Linux, Share would do nothing, so it goes, and Boost goes where the page
    /// can have none, leaving the row's right end empty.
    fn tiles(&self, site: &Site) -> Vec<Tile> {
        let share = self.host.has_share_sheet();
        (TILES.iter())
            .filter(|tile| share || tile.run != TileRun::Share)
            .filter(|tile| site.boostable || tile.run != TileRun::Command(Command::NewBoost))
            .copied()
            .collect()
    }

    /// The `index`th action tile's frame at `y`. The platform's tiles and the shield share the
    /// row's width equally, on whole points (#2334): five of 36.8 on the Mac, four of 48 on Linux.
    fn action_frame(&self, index: usize, y: f32) -> Bounds<Pixels> {
        let share = self.host.has_share_sheet();
        let tiles = (TILES.iter())
            .filter(|tile| share || tile.run != TileRun::Share)
            .count();
        let slots = (tiles + 1) as f32;
        let width = (WIDTH - 2. * PADDING - (slots - 1.) * GAP) / slots;
        let left = PADDING + index as f32 * (width + GAP);
        let (left, right) = (left.round(), (left + width).round());
        Bounds::new(
            point(px(left), px(y)),
            size(px(right - left), px(TILE_HEIGHT)),
        )
    }

    /// A tile `TILE_WIDTH` by `TILE_HEIGHT` at (`x`, `y`), named by its tooltip.
    fn tile_frame(x: f32, y: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(TILE_WIDTH), px(TILE_HEIGHT)))
    }

    fn tile(&self, index: usize, tile: Tile, y: f32, cx: &mut Context<Self>) -> gpui::AnyElement {
        let frame = self.action_frame(index, y);
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
            .children(self.ring(&Stop::Tile(tile.name), TILE_RADIUS, cx))
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
                    .children(self.ring(&Stop::Extension(extension.id.clone()), TILE_RADIUS, cx))
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
            .children(self.ring(&Stop::AddExtension, TILE_RADIUS, cx))
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
            Heading::Site => ("This site", "More", Run::SiteSettings),
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

    /// A plate as the action tiles', at `frame`, whose layer under the pointer and pressed is the
    /// accent's tint stepped while `tinted`, else the plate's states.
    fn plate(
        palette: &Palette,
        id: SharedString,
        frame: Bounds<Pixels>,
        tinted: bool,
    ) -> gpui::Stateful<gpui::Div> {
        let palette = *palette;
        let tint = move |alpha: f32| -> gpui::Hsla {
            let mut tint: gpui::Hsla = palette.accent.into();
            tint.a = alpha;
            tint
        };
        let state = div()
            .id("state")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(TILE_RADIUS));
        let state = match tinted {
            true => state
                .bg(tint(TINT))
                .hover(move |style| style.bg(tint(TINT_HOVERED)))
                .active(move |style| style.bg(tint(TINT_PRESSED))),
            false => state
                .hover(move |style| style.bg(palette.plate(STATE_HOVERED)))
                .active(move |style| style.bg(palette.plate(STATE_PRESSED))),
        };
        div()
            .id(id)
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
            .child(state)
    }

    /// The shield, last in the action row (#2334): tinted with its glyph in the accent while it
    /// blocks, the plain plate and a struck shield while ads are let through. A click turns it the
    /// other way.
    fn shield(&self, site: &Site, y: f32, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let on = site.ad_blocking?;
        let frame = self.action_frame(self.tiles(site).len(), y);
        let palette = palette::current(cx);
        let label = shield_label(site)?;
        let (glyph, ink): (_, gpui::Hsla) = match on {
            true => (SHIELD_ON, palette.accent.into()),
            false => (SHIELD_OFF, palette.on_overlay_glyph(TEXT)),
        };
        let tile = Self::plate(&palette, "site-shield".into(), frame, on)
            .when(self.a11y, |tile| {
                button_named(tile, label.clone(), (false, true, false))
            })
            .flex()
            .items_center()
            .justify_center()
            .on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                panel.tooltip(label.clone(), frame, *hovered, cx)
            }))
            .on_click(cx.listener(move |panel, _, _, cx| panel.toggle_ad_blocking(!on, cx)))
            .child(svg().path(glyph).size(px(19.5)).text_color(ink))
            .children(self.ring(&Stop::AdBlocking, TILE_RADIUS, cx));
        Some(tile.into_any_element())
    }

    /// The switches from `y`: a strip with its state in words when one is alone, else tiles with a
    /// glyph over its name. On is the plate tinted with the accent, its glyph in the accent.
    fn switches(&self, site: &Site, y: f32, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let palette = palette::current(cx);
        let switches = switches_of(site);
        let count = switches.len();
        let mut elements = Vec::new();
        for (index, switch) in switches.into_iter().enumerate() {
            let frame = switch_frame(index, count, y);
            let on = switch.on();
            let id = format!(
                "site-switch-{}",
                switch.name().to_lowercase().replace(' ', "-")
            );
            let ink: gpui::Hsla = match on {
                true => palette.accent.into(),
                false => palette.on_overlay_glyph(TEXT),
            };
            let glyph = svg()
                .path(switch.glyph())
                .flex_none()
                .size(px(SWITCH_GLYPH))
                .text_color(ink);
            let words = |size: f32, words: &'static str| {
                div()
                    .text_size(px(size))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(palette.on_overlay(TEXT))
                    .child(words)
            };
            let tile = Self::plate(&palette, id.into(), frame, on)
                .flex()
                .items_center();
            let tile = match count {
                1 => tile
                    .gap(px(STRIP_NAME_GAP))
                    .pl(px(STRIP_LEFT))
                    .child(glyph)
                    .child(words(NAME_SIZE, switch.words())),
                _ => tile
                    .flex_col()
                    .justify_center()
                    .gap(px(SWITCH_NAME_GAP))
                    .child(glyph)
                    .child(words(crate::tokens::text::SMALL, switch.name())),
            };
            let tile = tile.when(self.a11y, |tile| match switch {
                Switch::Engine(on) => {
                    let state = if on { "On" } else { "Off" };
                    button_named(tile, said(ENGINE_NAME, state), (true, true, false))
                }
                switch => switch_named(tile, switch.name(), on),
            });
            let tile =
                tile.on_click(
                    cx.listener(move |panel, event: &ClickEvent, _, cx| match switch {
                        Switch::DeveloperMode => panel.toggle_developer_mode(false, cx),
                        Switch::Engine(on) => panel.choose_engine(on, event.position(), cx),
                        Switch::Boost(on) => panel.toggle_boost(!on, cx),
                    }),
                );
            let tile = tile.children(self.ring(&switch.stop(), TILE_RADIUS, cx));
            elements.push(tile.into_any_element());
        }
        elements
    }

    /// A line at `y`: `glyph`, in the accent if `on`, `name`, and `value` at the right in the
    /// secondary ink, which gives way first.
    fn item(
        palette: &Palette,
        id: SharedString,
        y: f32,
        glyph: &'static str,
        on: bool,
        name: SharedString,
        value: Option<SharedString>,
    ) -> gpui::Stateful<gpui::Div> {
        let (hovered, pressed) = (palette.plate(STATE_HOVERED), palette.plate(STATE_PRESSED));
        let ink: gpui::Hsla = match on {
            true => palette.accent.into(),
            false => palette.on_overlay_glyph(ITEM_GLYPH_OFF),
        };
        div()
            .id(id)
            .absolute()
            .left(px(PADDING))
            .top(px(y))
            .w(px(WIDTH - 2. * PADDING))
            .h(px(ITEM_HEIGHT))
            .rounded(px(LOCK_RADIUS))
            .hover(move |style| style.bg(hovered))
            .active(move |style| style.bg(pressed))
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
                    .text_color(ink),
            )
            .child(
                div()
                    .flex_none()
                    .max_w(px(ITEM_NAME))
                    .truncate()
                    .text_color(palette.on_overlay(TEXT))
                    .child(name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_right()
                    .text_color(palette.on_overlay(SECONDARY))
                    .children(value),
            )
    }

    /// "This site"'s lines from `y`: Passwords, the permissions that differ for the site, the
    /// devices it was given, then Reset while a line it resets shows. One a policy sets takes no
    /// click, so it reads as dimmed.
    fn lines(&self, site: &Site, y: f32, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let palette = palette::current(cx);
        let a11y = self.a11y;
        let mut lines = Vec::new();
        let mut y = y;
        let mut push = |line: gpui::Stateful<gpui::Div>, y: &mut f32| {
            lines.push(line.into_any_element());
            *y += ITEM_PITCH;
        };
        if let Some((state, on)) = passwords::state(&site.logins) {
            let said = said(PASSWORDS_NAME, &state);
            let id = "site-passwords".into();
            let line = Self::item(
                &palette,
                id,
                y,
                PASSWORDS,
                on,
                PASSWORDS_NAME.into(),
                Some(state),
            );
            let line = line.when(a11y, |line| button_named(line, said, (true, true, false)));
            let logins = site.logins.clone();
            let line = line.on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                panel.choose_login(logins.clone(), event.position(), cx)
            }));
            push(
                line.children(self.ring(&Stop::Passwords, LOCK_RADIUS, cx)),
                &mut y,
            );
        }
        for (index, permission) in site.permissions.iter().enumerate() {
            let line = Self::item(
                &palette,
                format!("site-permission-{index}").into(),
                y,
                glyph_for(&permission.key),
                permission.value == Value::Allow,
                permission.name.clone(),
                Some(permission.state.clone()),
            );
            let managed = permission.choices.is_empty();
            let name = said(&permission.name, &permission.state);
            let line = line.when(a11y, |line| {
                button_named(line, name, (!managed, !managed, false))
            });
            let line = match managed {
                true => line.opacity(MANAGED_OPACITY),
                false => {
                    let stop = Stop::Permission(permission.kind);
                    let line = line.children(self.ring(&stop, LOCK_RADIUS, cx));
                    let permission = permission.clone();
                    line.on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                        panel.choose_value(permission.clone(), event.position(), cx)
                    }))
                }
            };
            push(line, &mut y);
        }
        for (index, device) in site.devices.iter().enumerate() {
            let line = Self::item(
                &palette,
                format!("site-device-{index}").into(),
                y,
                device.glyph.glyph(),
                true,
                device.name.clone(),
                Some(device.kind.clone()),
            );
            let removable = device.removable;
            let name = said(&device.name, &device.kind);
            let line = line.when(a11y, |line| {
                button_named(line, name, (removable, removable, false))
            });
            let line = match removable {
                false => line.opacity(MANAGED_OPACITY),
                true => {
                    let line = line.children(self.ring(&Stop::Device(index), LOCK_RADIUS, cx));
                    let device = device.clone();
                    line.on_click(cx.listener(move |panel, event: &ClickEvent, _, cx| {
                        panel.choose_device(index, device.clone(), event.position(), cx)
                    }))
                }
            };
            push(line, &mut y);
        }
        if let Some(label) = site.reset.clone() {
            let id = "site-permissions-reset".into();
            let line = Self::item(&palette, id, y, RESET, false, label.clone(), None);
            let line = line.when(a11y, |line| button_named(line, label, (false, true, false)));
            let line = line.on_click(cx.listener(|panel, _, _, cx| panel.reset(cx)));
            push(
                line.children(self.ring(&Stop::Reset, LOCK_RADIUS, cx)),
                &mut y,
            );
        }
        lines
    }

    fn footer(
        &self,
        site: &Site,
        y: f32,
        system: &gpui::WindowTextSystem,
        cx: &mut Context<Self>,
    ) -> [gpui::AnyElement; 3] {
        let lock = match site.connection {
            Connection::Secure => "icons/lock.svg",
            _ => "icons/lock-open.svg",
        };
        let palette = palette::current(cx);
        let profiles = self.open.as_ref().and_then(|open| open.profiles.clone());
        let alone = lock_alone(site, profiles.as_ref());
        let room = match &profiles {
            Some(_) => PROFILE_RIGHT - PROFILE_ALONE - GAP - PADDING,
            None => WIDTH - 2. * PADDING - FOOTER - GAP,
        };
        let square = Bounds::new(point(px(PADDING), px(y)), size(px(FOOTER), px(FOOTER)));
        let label = lock_label(site);
        let medium = gpui::FontWeight::MEDIUM;
        // Where the lock ends: its square alone, or its words, which a chip never cuts.
        let lock_width = match alone {
            true => FOOTER,
            false => (LOCK_LEFT
                + LOCK_SLOT
                + LOCK_TEXT_GAP
                + text_width(&label, NAME_SIZE, medium, system).ceil()
                + LOCK_RIGHT)
                .min(room),
        };
        let button = div()
            .id("site-lock")
            .when(self.a11y, |button| {
                button_named(button, label.clone(), (false, site.certificate, false))
            })
            .absolute()
            .left(px(PADDING))
            .top(px(y))
            .h(px(FOOTER))
            .max_w(px(room))
            .rounded(px(LOCK_RADIUS))
            .bg(palette.plate(PLATE))
            .flex()
            .items_start()
            .when(!alone, |button| button.pl(px(LOCK_LEFT)).pr(px(LOCK_RIGHT)))
            .when(alone, |button| {
                button.w(px(FOOTER)).justify_center().on_hover(cx.listener(
                    move |panel, hovered: &bool, _, cx| {
                        panel.tooltip(label.clone(), square, *hovered, cx)
                    },
                ))
            })
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
            .when(!alone, |button| {
                button.child(
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
            })
            .map(|button| match site.certificate {
                true => button
                    .hover(move |style| style.bg(palette.state(PLATE, STATE_HOVERED)))
                    .active(move |style| style.bg(palette.state(PLATE, STATE_PRESSED)))
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.run("certificate", Run::Certificate, cx)
                    })),
                false => button.opacity(DISABLED_OPACITY),
            })
            .children(self.ring(&Stop::Lock, LOCK_RADIUS, cx));
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
            .children(self.ring(&Stop::More, FOOTER / 2., cx));
        let chip = profiles.map(|profiles| {
            let name = text_width(&profiles.name, NAME_SIZE, medium, system).ceil();
            self.profile_chip(&profiles, PADDING + lock_width + GAP, name, y, cx)
        });
        let chip = chip.map_or_else(|| div().into_any_element(), |chip| chip.into_any_element());
        [button.into_any_element(), chip, more.into_any_element()]
    }

    /// The profile's chip, against "…" (#2339), in the room from `left`: the Profiles glyph, the
    /// name `name` wide and a chevron. The name gives way first, cut at its end, and with too
    /// little room it goes, the name then in its tooltip, as an ellipsis alone would read as a
    /// second "…". Its menu opens the page in another profile.
    fn profile_chip(
        &self,
        profiles: &Profiles,
        left: f32,
        name: f32,
        y: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let palette = palette::current(cx);
        let room = PROFILE_RIGHT - left - PROFILE_ALONE - LOCK_TEXT_GAP;
        let alone = room < name.min(PROFILE_NAME_MIN);
        let cut = room < name;
        let square = Bounds::new(
            point(px(PROFILE_RIGHT - FOOTER), px(y)),
            size(px(FOOTER), px(FOOTER)),
        );
        let name = profiles.name.clone();
        let chip =
            div()
                .id("site-profile")
                .when(self.a11y, |chip| {
                    button_named(chip, said("Profile", &name), (true, true, false))
                })
                .h(px(FOOTER))
                .min_w_0()
                .rounded(px(LOCK_RADIUS))
                .bg(palette.plate(PLATE))
                .hover(move |style| style.bg(palette.state(PLATE, STATE_HOVERED)))
                .active(move |style| style.bg(palette.state(PLATE, STATE_PRESSED)))
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
                                .path("icons/users.svg")
                                .size(px(LOCK_GLYPH))
                                .text_color(palette.on_overlay_glyph(TEXT)),
                        ),
                )
                .when(!alone, |chip| {
                    chip.child(
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
                            .child(name.clone()),
                    )
                })
                .when(cut, |chip| {
                    chip.on_hover(cx.listener(move |panel, hovered: &bool, _, cx| {
                        panel.tooltip(name.clone(), square, *hovered, cx)
                    }))
                })
                .child(
                    div()
                        .flex_none()
                        .ml(px(PROFILE_CHEVRON_GAP))
                        .h(px(FOOTER))
                        .flex()
                        .items_center()
                        .child(
                            svg()
                                .path("icons/chevron-down.svg")
                                .size(px(PROFILE_CHEVRON))
                                .text_color(palette.on_overlay(SECONDARY)),
                        ),
                )
                .on_click(cx.listener(|panel, event: &ClickEvent, _, cx| {
                    panel.profiles(event.position(), cx)
                }))
                .children(self.ring(&Stop::Profile, LOCK_RADIUS, cx));
        // Against "…", the name giving way first.
        div()
            .absolute()
            .left(px(left))
            .top(px(y))
            .w(px(PROFILE_RIGHT - left))
            .flex()
            .justify_end()
            .child(chip)
    }
}

impl Render for SiteControls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.a11y = window.is_a11y_active();
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
        let mut tiles: Vec<_> = (self.tiles(&site).into_iter().enumerate())
            .map(|(index, tile)| self.tile(index, tile, at.tiles, cx))
            .collect();
        tiles.extend(self.shield(&site, at.tiles, cx));
        let extensions = self.extensions(open, at.extensions, cx);
        let extensions_heading = self.heading(
            Heading::Extensions,
            at.extensions_cap,
            open.hovered == Some(Heading::Extensions),
            cx,
        );
        let switches = self.switches(&site, at.switches, cx);
        let lines = at.site_cap.map(|cap| {
            let hovered = open.hovered == Some(Heading::Site);
            let mut lines = vec![self.heading(Heading::Site, cap, hovered, cx)];
            lines.extend(self.lines(&site, at.lines, cx));
            lines
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
            .children(switches)
            .child(extensions_heading)
            .children(extensions)
            .children(lines.into_iter().flatten())
            .child(divider)
            .children(self.footer(&site, at.footer, &window.text_system().clone(), cx));
        root.child(scaled(card, scale, origin))
    }
}

#[cfg(test)]
mod tests;
