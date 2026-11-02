//! The browser window's UI, under the page: the sidebar and the card the page sits in, at Arc's
//! measured sizes (docs/ux-spec.md › Window and › Sidebar). The shell shows what the browser tells
//! it and asks the browser for changes, so the engine stays the one place tabs live.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, DispatchPhase, Entity, FocusHandle, Focusable,
    FontWeight, HitboxBehavior, Hsla, Image, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent,
    MouseUpEvent, PathBuilder, Pixels, Point, Rgba, Role, ScrollHandle, SharedString, Size,
    Subscription, Task, Window, WindowTextSystem, canvas, div, img, point, prelude::*, px, rgb,
    rgba, size, svg,
};

use crate::{
    icon_picker::Subject,
    passwords::Key,
    space_icon::{GLYPHS, SpaceIcon},
    text_input::{self, TextInput},
    palette::{Palette, Tone},
    theme::{Appearance, Theme},
};

#[cfg(test)]
mod agent_spaces;
mod beside;
#[cfg(test)]
mod link_previews;
#[cfg(test)]
mod close_tab;
#[cfg(test)]
mod dock_dot;
#[cfg(test)]
mod favorites_scope;
#[cfg(test)]
mod full_screen;
mod access;
#[cfg(test)]
mod undo_tests;
#[cfg(test)]
mod help_tests;
mod assets;
mod browser;
mod chosen;
mod clean_link;
mod commands;
mod drag;
mod display_face;
mod drm_note;
mod empty;
mod export;
mod extensions;
mod favorites;
mod files;
mod fly_in;
mod footer;
mod keys;
mod header;
mod history;
mod hover_cards;
mod library;
mod link;
#[cfg(test)]
mod link_target;
mod menu_bar;
mod media_player;
mod menus;
mod naming;
mod new_space;
mod onboarding;
mod page_menu;
mod page_dialog;
mod page_motion;
mod panes;
mod permission;
mod cast;
mod agent_bar;
mod agent_pointer;
mod agent_question;
mod picture;
mod recent_downloads;
#[cfg(test)]
mod recorder;
mod rows;
pub(crate) mod scaled;
mod side_panel;
mod sidebar;
mod sidebar_keys;
mod space_form;
mod split_targets;
mod swap;
mod swipe;
mod themes;
mod trace;
mod tile_theme;
mod trouble;
mod undo;

pub use assets::Assets;
pub use favorites::FavoritesScope;
pub use browser::{
    AgentGrants, Browser, Folders, MediaAction, Page, Split, Trouble, WindowButton,
    WindowButtonGeometry,
};
pub use commands::{
    BAR_ROWS, BAR_TABS_SHOWN, BarCommand, BarTab, CancelName, Command, CreateSpace, KEY_CONTEXT,
    KeepName, Match, bar_commands, bind_keys, copy_from_bar, copy_url_of_preview, find_commands,
    key_name, keystroke_label, Run, menu_item, pane_added, run_from_bar, shown_tab, with_named_rows,
};
pub use drag::dropped_urls;
pub use drm_note::PlayReady;
pub use extensions::{
    Extension, ExtensionAction, ExtensionCommand, menu_item as extension_menu_item,
    pin_menu_item as extension_pin_menu_item,
};
pub use keys::{
    Holder, OFF_THE_MAC, Refused, SecondPress, UserKeys, change_keys, page_keystroke, rebindable,
    recorded, reserved, shortcut_found, user_keys, view_binding, web_keystroke, written_as,
};
pub use history::HistoryPage;
pub use library::{Archived, Captured, Download, DownloadAction, DownloadState};
pub use menu_bar::{MenuBar, ProfileChoice};
pub use themes::{AppAppearance, app_appearance};
pub use menus::{MenuEntry, new_profile_name};
pub use page_menu::{PageMenuItem, made_up as made_up_page_menu};
pub use side_panel::SidePanel;
pub use page_dialog::{DialogKind, PageDialog, PageReply, ShownPageDialog};
pub use panes::page_frame;
pub use permission::{Answer, Question};
pub use cast::{Cast, CastDevice, CastState};
pub use trouble::{NOT_RESPONDING, RELOAD, WAIT};
pub use trace::{RECORDING, STOP, recorded as trace_recorded};
pub use picture::{PagePicture, picture_answered};
pub(crate) use picture::next_request;
pub use recent_downloads::{FilePreview, HOVER_ALL};
use clean_link::clean_link;
use commands::ShellHandle;
use fly_in::Flight;
use library::{MediaVideo, RECEDE_X, Section};
use menus::MenuAction;
use panes::pane_frames;
use recent_downloads::RecentDownloads;
use rows::Breath;
use scaled::scaled;
use swap::{ARROW_DOWN, Swapped, Swaps, UNLOAD};
use tile_theme::TileTheme;
use undo::{Edit, What};

pub use crate::tokens::shell::SIDEBAR_WIDTH;
pub use crate::tokens::shell::CARD_INSET;
pub use crate::tokens::shell::CARD_RADIUS;
use crate::tokens::shell::SPLIT_GAP;
/// The most pages a split view shows, as the core's Space has it.
const MAX_PANES: usize = 3;
use crate::tokens::shell::FOCUS_GLOW;
use crate::tokens::shell::FOCUS_GLOW_ALPHA;
// Dragged, the gap between a split view's pages leaves each a tenth of the card, no less than 50 pt
// and no more than 200, and snaps them even within 15 pt, as Chrome's does.
const PANE_LEAST: (f32, f32) = (50., 200.);
const PANE_SNAP: f32 = 15.;
/// What Expand Current Split leaves each of the other pages (Arc 1.170.0 ax: 220 pt at any width).
const EXPANDED_OTHER: f32 = 220.;

// ⌘S hides and shows the sidebar on Arc's ease-out cubics (docs/ux-spec.md › Motion: Sidebar hide
// and Sidebar show).
const SIDEBAR_HIDE: f32 = 0.11;
const SIDEBAR_SHOW: f32 = 0.1;
// The sidebar's edge drags its width between Arc's limits, stopping at them, and dragged on past
// the narrowest it hides the sidebar (docs/ux-spec.md › Sidebar › Width). A double-click on the
// edge settles it back, as the Space pages settle, at the width it starts at. The edge is the
// margin right of the rows.
use crate::tokens::shell::SIDEBAR_MIN;
use crate::tokens::shell::SIDEBAR_MAX;
const SIDEBAR_EDGE: f32 = 6.;

// The sidebar reveal (docs/ux-spec.md › Sidebar reveal): the hidden sidebar slides over the page as
// a panel once the pointer has been at the window's left edge for a moment, and slides away once
// the pointer is past the hold zone, both on Arc's springs. Its window starts at the browser
// window's top-left corner, so a point is at the same place in both.
pub use crate::tokens::shell::REVEAL_INSET;
const REVEAL_HOLD: f32 = 24.;
const REVEAL_WAIT: Duration = Duration::from_millis(100);
const REVEAL_IN_RESPONSE: f32 = 0.175;
const REVEAL_IN_DAMPING: f32 = 0.8;
const REVEAL_OUT_RESPONSE: f32 = 0.175;
const REVEAL_OUT_DAMPING: f32 = 0.76;
/// The panel's window at rest ends at its edge, and is this much wider while it moves, for the
/// spring's overshoot.
const REVEAL_OVERSHOOT: f32 = 8.;

// Rows make room and close it, a closed tab's row fades and new rows fade in on one spring, Arc's
// (docs/ux-spec.md › Motion: Row motion). A row's hover fill comes and goes in one frame, and only
// the selected row's eases back as the pointer leaves, as Arc's do (Motion: Row hover).
const ROW_RESPONSE: f32 = 0.13;
const ROW_DAMPING: f32 = 0.9;
const SELECTED_UNHOVER: f32 = 0.17;
// A pressed row's plate shrinks about its middle while it's held and grows back once it's let go,
// each way over 70 ms, linear (Arc 1.170 rec at 2×, two presses: 1 pt in at each end).
const PRESS_SCALE: f32 = 0.99;
const PRESS_TIME: f32 = 0.07;
// A pressed header button, back, forward or reload, shrinks the same way, also 1 pt in at each end,
// so a press reads the same on a 32 pt button as on a row (ours: Arc's isn't recorded).
const NAV_PRESS_SCALE: f32 = (NAV_SIZE - 2.) / NAV_SIZE;
// A pressed row comes away once the pointer has moved this far (ours), and the tiles make room for
// a dragged tab critically damped, as Arc's do (docs/ux-spec.md › Motion: Favorites room).
const DRAG_START: f32 = 4.;
const ROOM_OMEGA: f32 = 25.;
// A dragged row turning into a tile has faded its title by this far through the turn (ours).
const TURN_TITLE_FADE: f32 = 0.5;
// The rows under a folder make room as it opens on an ease-out curve, and close up as it closes
// critically damped, neither overshooting, as Arc's (docs/ux-spec.md › Motion: Folder open and
// Folder close).
const FOLD_OPEN: f32 = 0.2;
const FOLD_CLOSE_OMEGA: f32 = 25.;
// The edge under the favorites fades in once the list scrolls past its top and out back there, on
// Dia's Scroll edge curve (docs/ux-spec.md › Motion).
const SCROLL_EDGE_FADE: f32 = 0.1;
// Clear and the URL field's button fade in as the pointer comes onto the sidebar and out as it
// leaves, on the scroll edge's curve. Arc's fade isn't recorded (docs/ux-spec.md › Clear divider).
const TOOLS_FADE: f32 = 0.15;
// Two fingers across the sidebar page between Spaces (docs/ux-spec.md › Spaces › Switching). The
// pages follow at half the fingers' way and rubber-band past the ends. Let go, they settle
// critically damped on the next Space if the last 100 ms moved them faster than a flick, else on
// the nearest, from where the fingers left them and as fast, with Arc's response. A wheel pages
// once per burst of ticks, then rests. AppKit's line is about 10 pt.
// Past the first Space is the Library, which the fingers bring out and put away the same way.
const SWIPE_TRACKING: f32 = 0.5;
const SWIPE_FLICK: f32 = 5.;
const SWIPE_HISTORY: Duration = Duration::from_millis(100);
const SPACE_SETTLE: f32 = 0.19;
/// A switch to the Space next door changes the window's colour all at once, exponentially with a
/// time constant of 30 ms, well ahead of the page's slide (Arc 1.170 rec at 2×); a longer jump's
/// colour goes with the slide.
const SPACE_COLOR_TAU: f32 = 0.03;
const WHEEL_LINE: f32 = 10.;
const WHEEL_BURST: Duration = Duration::from_millis(49);
const WHEEL_REST: Duration = Duration::from_millis(250);

use crate::tokens::shell::NAV_TOP;
use crate::tokens::shell::SIDEBAR_BUTTON_X;
use crate::tokens::shell::BACK_X;
use crate::tokens::shell::FORWARD_X;
use crate::tokens::shell::RELOAD_X;
use crate::tokens::shell::NAV_SIZE;
use crate::tokens::shell::NAV_RADIUS;
use crate::tokens::shell::NAV_GLYPH;
use crate::tokens::shell::NAV_END;

use crate::tokens::shell::URL_FIELD_TOP;
use crate::tokens::shell::URL_FIELD_HEIGHT;
use crate::tokens::shell::URL_FIELD_TEXT_X;
use crate::tokens::shell::URL_BUTTON_SIZE;
use crate::tokens::shell::URL_BUTTON_INSET;
use crate::tokens::shell::URL_BUTTON_GLYPH;
// Arc puts favorites between the URL field and the Space's name. This is their gap while there are
// none, which Arc has no layout for.
const LIST_TOP: f32 = URL_FIELD_TOP + URL_FIELD_HEIGHT + 8.;
use crate::tokens::shell::TILE_HEIGHT;
use crate::tokens::shell::TILE_GAP;
use crate::tokens::shell::TILE_MIN;
use crate::tokens::shell::TILE_RADIUS;
use crate::tokens::shell::{TILE_MARK, TILE_MARK_INSET};
use crate::tokens::shell::TALL_FAVORITES;
use crate::tokens::shell::NAME_HEIGHT;
// The name's left edge. After an icon it's Arc's 41, 2 pt before its titles at 43 (ax, measured on
// a Space with an icon), and after the icon picker's placeholder too. With neither, nothing stands
// in the icon column, so the name starts on it, at the favicons' left edge (ours).
use crate::tokens::shell::NAME_X;
const ICONLESS_NAME_X: f32 = FAVICON_X + (FAVICON_SIZE - FAVICON_DRAWN) / 2.;
// A Space's icon before its name. Arc's frame is 26 wide at x 13, centred on its favicons' (ax),
// so ours is centred there too; its size and look are *interim*, as Arc's isn't recorded. The emoji
// is a tab's size,
// so it sits on the name's cap height as a tab's does on its title; bigger, it outweighed the name
// and sat below its middle.
use crate::tokens::shell::SPACE_ICON_SLOT;
const SPACE_ICON_X: f32 = FAVICON_X + FAVICON_SIZE / 2. - SPACE_ICON_SLOT / 2.;
use crate::tokens::shell::SPACE_ICON_PLATE;
use crate::tokens::shell::PINNED_CARET;
use crate::tokens::shell::PINNED_CARET_PLATE;
use crate::tokens::shell::PINNED_CARET_GAP;
// While the icon picker shows for a Space with no icon, a dashed square holds the icon's place,
// centred in its frame (Arc 1.170 cap at 1×): 24 across, its corners rounded by about 6, a 1 pt
// line of 4 pt dashes and 2 pt gaps, in black at 0.30, seen over a red Space (*interim* over the
// others).
use crate::tokens::shell::ICON_PLACEHOLDER;
use crate::tokens::shell::ICON_PLACEHOLDER_RADIUS;
use crate::tokens::shell::ICON_PLACEHOLDER_LINE;
const ICON_PLACEHOLDER_DASH: (f32, f32) = (4., 2.);
use crate::tokens::shell::ICON_PLACEHOLDER_COLOR;
const SPACE_ICON_TEXT: f32 = EMOJI_SIZE;
use crate::tokens::shell::SPACE_GLYPH;
use crate::tokens::shell::ROW_PITCH;
use crate::tokens::shell::ROW_HEIGHT;
use crate::tokens::shell::PLATE_HEIGHT;
use crate::tokens::shell::PLATE_INSET;
// What's in a folder sits this far in from it (Arc).
use crate::tokens::shell::FOLDER_INDENT;
const NEW_FOLDER: &str = "New Folder";
use crate::tokens::shell::DROP_LINE;
use crate::tokens::shell::DROP_RING;
use crate::tokens::shell::DROP_RING_X;
use crate::tokens::shell::DROP_LINE_END;
use crate::tokens::shell::DROP;
pub use crate::tokens::shell::PLATE_RADIUS;
use crate::tokens::shell::FAVICON_X;
use crate::tokens::shell::FAVICON_SIZE;
// Arc's titles end as far from the sidebar's edge as its favicons start.
const TITLE_END_INSET: f32 = FAVICON_X - PLATE_INSET;
use crate::tokens::shell::FAVICON_DRAWN;
use crate::tokens::shell::EMOJI_SIZE;
use crate::tokens::shell::LOADING_SIZE;
use crate::tokens::shell::LOADING_ROW_MIN;
use crate::tokens::shell::STATUS_DOT;
use crate::tokens::shell::STATUS_LOADING;
use crate::tokens::shell::STATUS_AGENT;
use crate::tokens::shell::BREATH_LOW;
use crate::tokens::shell::STATUS_RING;
use crate::tokens::shell::STATUS_RING_COLOR;
// A loading dot's breath, out and back, eased in and out (ours, #321).
const BREATH: Duration = Duration::from_millis(1600);
use crate::tokens::shell::TITLE_GAP;
/// Where a row's title starts, in from its plate's left.
const FOLDER_TITLE_X: f32 = FAVICON_X - PLATE_INSET + FAVICON_SIZE + TITLE_GAP;
use crate::tokens::shell::SPEAKER_DRAWN;
use crate::tokens::shell::SPEAKER_SIZE;
use crate::tokens::shell::SPEAKER_GAP;
use crate::tokens::shell::CLOSE_SIZE;
use crate::tokens::shell::CLOSE_RIGHT;
use crate::tokens::shell::CLOSE_GLYPH;
use crate::tokens::shell::SEGMENT_FAVICON_X;
use crate::tokens::shell::SEGMENT_TITLE_GAP;
use crate::tokens::shell::SEGMENT_END;
use crate::tokens::shell::SEGMENT_LINE;
use crate::tokens::shell::HAIRLINE;
use crate::tokens::shell::DIVIDER_X;
// The Clear divider, under the pinned tabs. Its line's centre is 25 pt above the centre of the first
// row under it. While Clear shows, the line ends 9 pt before it.
use crate::tokens::shell::DIVIDER_HEIGHT;
const DIVIDER_LINE_Y: f32 = DIVIDER_HEIGHT - (25. - ROW_HEIGHT / 2.);
use crate::tokens::shell::DIVIDER_GAP;
// The arrow before Clear while it would close the last Today tabs, 8 pt tall and 3 pt before the
// word (Arc). Lucide's arrow-down spans 14 of its 24 each way.
const CLEAR_ARROW: f32 = 8. * 24. / 14.;
const CLEAR_ARROW_INSET: f32 = CLEAR_ARROW * 5. / 24.;
use crate::tokens::shell::CLEAR_GAP;
const CLEAR_ARROW_ROOM: f32 = CLEAR_ARROW - 2. * CLEAR_ARROW_INSET + CLEAR_GAP;
use crate::tokens::shell::CLEAR_HEIGHT;
// Clear takes clicks 5 pt past the word each way, as the header's buttons past their glyphs.
const CLEAR_HIT_OUT: f32 = 5.;
// How long the toast after a copy shows (ours: Arc's isn't measured), and after a Space is deleted
// (Arc).
const COPIED_TIME: Duration = Duration::from_secs(2);
const DELETED_TIME: Duration = Duration::from_secs(5);
use crate::tokens::shell::FOOTER_HEIGHT;
use crate::tokens::shell::SWITCHER_INSET;
use crate::tokens::shell::PLUS_INSET;
use crate::tokens::shell::SPACE_PITCH;
use crate::tokens::shell::SPACE_HEIGHT;
use crate::tokens::shell::SPACE_SWATCH_SHOWN;
use crate::tokens::shell::SPACE_SWATCH;
use crate::tokens::shell::SPACE_SWATCH_HOVERED;
// A Space's icon in the switcher takes its dot's place in the same item. Its glyph is 16 pt, a
// favicon's size (ours, *interim*).
const SPACE_ICON_SWITCHER: f32 = EMOJI_SIZE;
// The Create a Space form, in Arc's places and type, without its illustration and subtitle, which
// are Arc's art and copy. The field moves up into the subtitle's place, Arc's 35 pt under the text
// above it. Distances to the buttons are from the window's bottom.
use crate::tokens::shell::FORM_TITLE_TOP;
use crate::tokens::shell::FORM_TITLE_HEIGHT;
const FORM_FIELD_TOP: f32 = FORM_TITLE_TOP + FORM_TITLE_HEIGHT + 35.;
use crate::tokens::shell::FORM_INSET;
use crate::tokens::shell::FORM_PLATE_HEIGHT;
use crate::tokens::shell::FORM_RADIUS;
// Arc's name starts at x 48, after the Space's icon, which Spaces don't have yet. This is the URL
// field's padding.
const FORM_TEXT_X: f32 = FORM_INSET + URL_FIELD_TEXT_X - PLATE_INSET;
// Arc's Profile and Choose a Theme rows, 42 and 86 under the field's top: a glyph centred 30 in, a
// label at 46.5, and the Space's profile in a pill at 95, 10 after the label, its name 8 in and its
// chevron 6.5 after the name and 10 from the pill's end, which the chevron's box takes 4.5 of.
const FORM_PROFILE_TOP: f32 = FORM_FIELD_TOP + 42.;
const FORM_THEME_TOP: f32 = FORM_FIELD_TOP + 86.;
use crate::tokens::shell::FORM_GLYPH_X;
use crate::tokens::shell::FORM_GLYPH;
use crate::tokens::shell::FORM_LABEL_X;
use crate::tokens::shell::FORM_PILL_X;
use crate::tokens::shell::FORM_PILL_HEIGHT;
use crate::tokens::shell::FORM_PILL_LEFT;
use crate::tokens::shell::FORM_CHEVRON;
const FORM_CHEVRON_GAP: f32 = 6.5 - FORM_CHEVRON / 4.;
const FORM_PILL_RIGHT: f32 = 10. - FORM_CHEVRON / 4.;
use crate::tokens::shell::CREATE_BOTTOM;
use crate::tokens::shell::CANCEL_BOTTOM;
use crate::tokens::shell::CANCEL_HEIGHT;

use crate::tokens::shell::PRIVATE_SHADE;
use crate::tokens::shell::TITLE;
use crate::tokens::shell::UNSELECTED_TITLE;
use crate::tokens::shell::CHOSEN;
use crate::tokens::shell::HOVERED;
use crate::tokens::shell::PRESSED;
use crate::tokens::shell::SECONDARY;
use crate::tokens::shell::CLOSE_HOVERED;
use crate::tokens::shell::URL_FIELD;
use crate::tokens::shell::NEW_TAB;
use crate::tokens::shell::NEW_TAB_PLUS;
use crate::tokens::shell::DIVIDER;
use crate::tokens::shell::SCROLL_EDGE;
use crate::tokens::shell::CLEAR;
use crate::tokens::shell::CLEAR_HOVERED;
use crate::tokens::shell::SLEEPING_FAVICON;
use crate::tokens::shell::TILE;
/// A tile, and the URL field, under the pointer: the ladder's hover over its resting plate.
pub(crate) const TILE_HOVERED: f32 = crate::palette::state_alpha(TILE, crate::palette::STATE_HOVERED);
use crate::tokens::shell::TILE_RING;
use crate::tokens::shell::DEVELOPER_OUTLINE;
use crate::tokens::shell::DEVELOPER_OUTLINE_WIDTH;
use crate::tokens::shell::DEVELOPER_SELECTED;
use crate::tokens::shell::DEVELOPER_TILE;
use crate::tokens::shell::TILE_UNTHEMED;
use crate::tokens::shell::TILE_UNTHEMED_LIGHT;
use crate::tokens::shell::NAV;
use crate::tokens::shell::NAV_DISABLED;
use crate::tokens::shell::NAV_HOVERED;
use crate::tokens::shell::SPACE_NAME;
use crate::tokens::shell::SPACE_NAME_SHUT;
use crate::tokens::shell::SPACE_SHOWN;
use crate::tokens::shell::SPACE_OTHER;
use crate::tokens::shell::SPACE_HOVERED;
use crate::tokens::shell::FORM_TEXT;
use crate::tokens::shell::FORM_PLATE;
use crate::tokens::shell::FORM_PLACEHOLDER;
use crate::tokens::shell::FORM_GLYPH_COLOR;
use crate::tokens::shell::FORM_PILL;
use crate::tokens::shell::CREATE_UNNAMED;
// Create sends the form away from the bottom up, as Arc does (docs/ux-spec.md › Creating): the
// buttons fade, then the rows, the field and the title fade as they sink, each over 150 ms.
const FORM_LEAVE: f32 = 0.15;
const FORM_LEAVE_ROWS: f32 = 0.04;
const FORM_LEAVE_FIELD: f32 = 0.08;
const FORM_LEAVE_TITLE: f32 = 0.13;
const FORM_SINK: f32 = 20.;
use crate::tokens::shell::REVEAL_RADIUS;
use crate::tokens::shell::REVEAL_BORDER;
// The panel draws the window's buttons itself, since AppKit's stay in the browser window, under
// it: their centres from the panel's corner, as the docked sidebar's are from the window's, their
// size, and their colours, sampled from AppKit's in a capture of the browser window. The glyphs
// show while the pointer is over any of them, as AppKit's do.
pub const WINDOW_BUTTONS: [(f32, u32); 3] =
    [(20., WINDOW_BUTTON_CLOSE), (43., WINDOW_BUTTON_MINIMIZE), (66., WINDOW_BUTTON_ZOOM)];
// On Linux the browser window is Chromium's without a frame, and on Windows the host takes its
// title bar away (platform/windows/host.cc), so nothing else draws its buttons: the UI draws them
// where AppKit's show.
const DRAWS_WINDOW_BUTTONS: bool = cfg!(any(target_os = "linux", target_os = "windows"));
// The third button maximizes on Linux and Windows, as their title bars' does, where on the Mac it
// goes full screen.
const ZOOM_GLYPH: &str = match cfg!(any(target_os = "linux", target_os = "windows")) {
    true => "icons/window-maximize.svg",
    false => "icons/window-full-screen.svg",
};
use crate::tokens::shell::WINDOW_BUTTON_Y;
use crate::tokens::shell::WINDOW_BUTTON_GLYPH;
use crate::tokens::shell::WINDOW_BUTTON_CLOSE;
use crate::tokens::shell::WINDOW_BUTTON_MINIMIZE;
use crate::tokens::shell::WINDOW_BUTTON_ZOOM;
use crate::tokens::shell::WINDOWS_BUTTON_GLYPH;
use crate::tokens::shell::WINDOWS_BUTTON_PLATE;
use crate::tokens::shell::WINDOWS_CLOSE_HOVERED;
use crate::tokens::text;

#[derive(Clone, Default)]
struct Tab {
    title: SharedString,
    page: Page,
    /// None for a page without one.
    favicon: Option<Arc<Image>>,
    /// What the favicon (by its id) themes the tile with when the tab is a favorite shown, made
    /// the first time that's drawn.
    theme: RefCell<Option<(u64, Option<Rc<TileTheme>>)>>,
}

impl Tab {
    /// Keeps what the core reported, and says whether it differs from what was kept.
    fn keep_title(&mut self, title: SharedString) -> bool {
        replace(&mut self.title, title)
    }

    fn keep_favicon(&mut self, favicon: Option<Arc<Image>>) -> bool {
        replace(&mut self.favicon, favicon)
    }

    fn keep_page(&mut self, page: Page) -> bool {
        replace(&mut self.page, page)
    }
}

fn replace<T: PartialEq>(kept: &mut T, value: T) -> bool {
    let changed = *kept != value;
    if changed {
        *kept = value;
    }
    changed
}

/// A Space as the core reports it: its id, name, colour (0xRRGGBB), profile, icon, the name of the
/// agent it belongs to, and its theme as `Theme` writes it.
pub type SpaceEntry =
    (u64, SharedString, Option<u32>, u64, SharedString, SharedString, SharedString);

/// A picker that waits for the row it points at to settle before it opens over it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pending {
    Icon(Subject),
    Theme(u64),
}

#[derive(Clone)]
struct Space {
    id: u64,
    name: SharedString,
    /// 0xRRGGBB, if it has one.
    color: Option<u32>,
    /// What Arc's theme picker made for it, whose main colour is `color`, if it has one.
    theme: Option<Theme>,
    /// The emoji it shows in place of its dot and before its name, or empty.
    icon: SharedString,
    /// The profile its tabs open in, and its favorites' tabs unless they're each Space's own.
    profile: u64,
    /// The name of the agent it belongs to, or empty for the user's own. It never changes.
    agent: SharedString,
    /// Its pinned tabs and folders and its Today tabs, each top to bottom.
    pinned: Vec<u64>,
    today: Vec<u64>,
    splits: Vec<Split>,
    folders: Folders,
    /// The tab it shows, or showed last.
    shown: u64,
}

impl Space {
    /// The tabs and folders of its pinned rows and its Today rows: each split's first stands for
    /// the split, and what's in a closed folder has none, but the tab shown's (Arc).
    fn rows(&self) -> (Vec<u64>, Vec<u64>) {
        let today = self.today.iter().copied().filter(|&tab| !self.later(tab));
        (self.pinned_rows(self.folders.pinned_shut).collect(), today.collect())
    }

    /// Its pinned rows, with them collapsed or not: what's in a closed folder has none, but the
    /// tab shown's (Arc).
    fn pinned_rows(&self, collapsed: bool) -> impl Iterator<Item = u64> + '_ {
        let shown = self.panes(self.shown)[0];
        let folded = move |row| collapsed || self.holders(row).any(|folder| !self.is_open(folder));
        (self.pinned.iter().copied())
            .filter(move |&row| !self.later(row) && (row == shown || !folded(row)))
    }

    /// Whether `tab` is in a split after its first, which stands for the split's row.
    fn later(&self, tab: u64) -> bool {
        self.splits.iter().any(|split| split.tabs[1..].contains(&tab))
    }

    /// Whether collapsing its pinned rows hides at least one now, which expanding shows again. The
    /// tab shown keeps its row, so with only it pinned there's nothing to collapse (#1208).
    fn collapsing_hides(&self) -> bool {
        self.pinned_rows(true).count() < self.pinned_rows(false).count()
    }

    /// Whether `row`, one of its pinned rows, is in a closed folder or in pinned rows collapsed.
    fn shut(&self, row: u64) -> bool {
        self.folders.pinned_shut || self.holders(row).any(|folder| !self.is_open(folder))
    }

    fn is_folder(&self, row: u64) -> bool {
        self.folders.folders.contains_key(&row)
    }

    fn is_open(&self, folder: u64) -> bool {
        self.folders.folders.get(&folder).is_some_and(|&(_, open)| open)
    }

    /// The folder holding `row`, else 0.
    fn folder_of(&self, row: u64) -> u64 {
        self.folders.parents.get(&row).copied().unwrap_or(0)
    }

    /// The folders `row` is in, innermost first.
    fn holders(&self, row: u64) -> impl Iterator<Item = u64> + '_ {
        std::iter::successors(Some(self.folder_of(row)), |&folder| Some(self.folder_of(folder)))
            .take_while(|&folder| folder != 0)
    }

    fn depth(&self, row: u64) -> usize {
        self.holders(row).count()
    }

    /// Whether `shown` and its split are all of its Today tabs, so Clear has nothing else to close
    /// and closes them too, which an arrow before it says (Arc).
    fn clears_last(&self, shown: u64) -> bool {
        let shown = self.panes(shown);
        !self.today.is_empty() && self.today.iter().all(|tab| shown.contains(tab))
    }

    /// Its pinned tabs and folders as check.sh reads them: each folder followed by `+` if it's
    /// open, else `-`, and what's in it in brackets.
    fn outline(&self, folder: u64) -> String {
        let rows = self.pinned.iter().filter(|&&row| self.folder_of(row) == folder);
        let rows = rows.map(|&row| match self.is_folder(row) {
            true => {
                let open = if self.is_open(row) { '+' } else { '-' };
                format!("{row}{open}[{}]", self.outline(row))
            }
            false => row.to_string(),
        });
        rows.collect::<Vec<_>>().join(" ")
    }

    /// The tabs shown with `tab`, left to right: its split's, else `tab` alone.
    fn panes(&self, tab: u64) -> Vec<u64> {
        let split = self.splits.iter().find(|split| split.tabs.contains(&tab));
        split.map_or_else(|| vec![tab], |split| split.tabs.clone())
    }

    /// Whether `tab`'s split view is stacked.
    fn stacked(&self, tab: u64) -> bool {
        self.splits.iter().any(|split| split.stacked && split.tabs.contains(&tab))
    }

    /// The shares of `tab`'s split view, or none while they're even.
    fn shares(&self, tab: u64) -> Vec<f64> {
        let split = self.splits.iter().find(|split| split.tabs.contains(&tab));
        split.map_or_else(Vec::new, |split| split.shares.clone())
    }

    /// The tab of `tab`'s split view that took the card, else 0.
    fn expanded(&self, tab: u64) -> u64 {
        let split = self.splits.iter().find(|split| split.tabs.contains(&tab));
        split.map_or(0, |split| split.expanded)
    }

    /// Where its rows are in the list, from its top.
    fn slots(&self) -> HashMap<Slot, f32> {
        let (pinned, today) = self.rows();
        slots(&pinned, &today)
    }
}

/// The Create a Space form, which a Space New Space made shows until it's named, cancelled or has
/// a tab.
struct SpaceForm {
    space: u64,
    /// The Space shown before, which Cancel shows again.
    from: u64,
    name: Entity<TextInput>,
    /// The tab Move to › New Space… moves into the Space once it's made.
    tab: Option<u64>,
}

/// What the renaming field names.
#[derive(Clone, Copy, PartialEq)]
enum Naming {
    Space(u64),
    Tab(u64),
    /// A pinned tab's pinned URL, in its row in place of its title.
    PinnedUrl(u64),
    Folder(u64),
    /// A new profile's name, in the New Space form's Profile row.
    Profile(u64),
}

impl Naming {
    /// The tab whose row the field is in, if it's a tab's.
    fn tab(self) -> Option<u64> {
        match self {
            Naming::Tab(tab) | Naming::PinnedUrl(tab) => Some(tab),
            Naming::Space(_) | Naming::Folder(_) | Naming::Profile(_) => None,
        }
    }
}

/// The field a Space's name, a tab's title or a folder's name turns into for renaming it.
struct Renaming {
    naming: Naming,
    name: Entity<TextInput>,
    /// Whether the field is in the Space's card in the Library rather than in the sidebar.
    in_library: bool,
    /// Keeps the name once the field loses GPUI's focus, as Finder does.
    _blur: Option<Subscription>,
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    NewTab,
    Tab(u64),
}

/// A place in the list that moves as tabs come and go.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Slot {
    Tab(u64),
    Divider,
    NewTab,
}

/// Tabs' rows new in the list, fading in since `start`: a new tab's plate, or whole rows when
/// several come at once, as after Undo.
struct Opened {
    tabs: Vec<u64>,
    /// Whether whole rows fade in, else only a new tab's plate.
    whole: bool,
    start: Instant,
}

impl Opened {
    /// How far they've faded in, or None once they have.
    fn done(&self) -> Option<f32> {
        row_spring(self.start.elapsed().as_secs_f32())
    }
}

/// A closed tab's row, fading where it was.
struct Closing {
    title: SharedString,
    favicon: Option<Arc<Image>>,
    /// The emoji it had in place of its favicon, or empty.
    icon: SharedString,
    /// A folder's row's, if it was open.
    folder: Option<bool>,
    /// How far in from a tab's place it was, in its folders.
    indent: f32,
    top: f32,
    start: Instant,
}

/// How a row slides to its place.
#[derive(Clone, Copy)]
enum Curve {
    /// Making room for a tab's row or closing it (Row motion).
    Slide,
    /// The tiles making room for a dragged tab, and the tab turning between its row and its tile
    /// (Favorites room).
    Room,
    /// Making room as a folder opens, or closing it up as it closes (Folder open, Folder close).
    Fold { open: bool },
}

impl Curve {
    /// How far along the slide is `t` seconds in, or None once it's done.
    fn at(self, t: f32) -> Option<f32> {
        match self {
            Curve::Slide => row_spring(t),
            Curve::Room => critically_damped(t, ROOM_OMEGA),
            Curve::Fold { open: true } => {
                (t < FOLD_OPEN).then(|| cubic_bezier(0., 0., 0.58, 1., t / FOLD_OPEN))
            }
            Curve::Fold { open: false } => critically_damped(t, FOLD_CLOSE_OMEGA),
        }
    }
}

/// Where a dragged tab is or would go: among the favorites, or in the pinned tabs or Today, at an
/// index of them without it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Place {
    Favorite(usize),
    List(bool, usize),
}

/// A row the pointer pressed, when, and when it let go.
struct HeldRow {
    tab: u64,
    pressed: Instant,
    let_go: Option<Instant>,
}

/// A header button the pointer pressed, known by where it is, as reload and stop are one button;
/// the command it ran then, when, when it let go, and whether it has been drawn all the way in.
struct HeldNav {
    x: f32,
    command: Command,
    pressed: Instant,
    let_go: Option<Instant>,
    drawn_in: bool,
}

/// A tab's row or tile the pointer pressed, which follows the pointer once it has moved far enough.
/// Over the favorites it's drawn as a tile, and over the list as a row. Or links or files dragged
/// over the list from the page or another app, which the rows make room for in the same way.
struct Drag {
    /// For links or files, INCOMING until they're dropped, then the first one's tab.
    tab: u64,
    incoming: bool,
    /// Where the pointer pressed, and the sidebar's top-left corner in that window.
    press: Point<Pixels>,
    origin: Point<Pixels>,
    /// Where the pointer is now, from the sidebar's corner.
    pointer: Point<f32>,
    /// Where the pointer holds the tab: down from its row's top, and from its tile's centre.
    grab: (f32, Point<f32>),
    /// The row's top in the list when it was pressed. A favorite's is 0.
    top: f32,
    /// Whether the tab has come away, and how far up and down the list its row goes: between the
    /// list's first and last places.
    moving: bool,
    span: (f32, f32),
    from: Place,
    to: Place,
    /// How many folders it was in, and would be in where it goes.
    from_depth: usize,
    depth: usize,
    /// Over the list, where the drop line is drawn, down from the list's top: in the gap it would
    /// go to, unless that's where it was. The rows stay where they are until it's let go.
    line: Option<f32>,
    /// The tab whose row's middle the pointer is on, and the wait for it to rest there, after which
    /// it's the tab the dragged one would join in a split view, as Arc's, in place of a gap.
    resting: Option<(u64, Task<()>)>,
    onto: Option<u64>,
    /// Let go, until the browser reports the lists it made.
    dropped: bool,
    /// Over the page, where it can join the page shown in a split view.
    over_page: Option<split_targets::OverPage>,
    /// Its turn between its row and its tile, and whether the pointer let go mid-turn, which
    /// holds the drop until it has turned.
    shape: Shape,
    held: bool,
    /// The Space it was pressed in, and while it's held at the sidebar's left or right edge, which
    /// and since when, for paging to the Space on that side.
    space: u64,
    edge: Option<(bool, Instant)>,
    /// Pressed with ⌥, whose click splits once it's let go without having moved, so that a drag
    /// with ⌥ held can drop a copy instead (#690).
    alt_click: bool,
}

/// How far a dragged tab has turned from its row into its tile as it crossed into or out of the
/// favorites, on the spring the tiles make room on (ours, docs/ux-spec.md › Favorites).
#[derive(Clone, Copy)]
struct Shape {
    /// Where it turned from, whether it's turning into a tile, and since when, until it has.
    from: f32,
    tile: bool,
    start: Option<Instant>,
}

impl Shape {
    fn new(tile: bool) -> Self {
        Shape { from: if tile { 1. } else { 0. }, tile, start: None }
    }

    /// How far from its row to its tile it is at `now`: 0 a row, 1 a tile.
    fn at(&self, now: Instant) -> f32 {
        let to = if self.tile { 1. } else { 0. };
        let done = self.start.and_then(|start| {
            Curve::Room.at(now.duration_since(start).as_secs_f32())
        });
        done.map_or(to, |done| self.from + (to - self.from) * done)
    }

    /// Turns it toward its tile or its row from where it is at `now`, so a crossing back mid-turn
    /// reverses with no jump.
    fn turn(&mut self, tile: bool, now: Instant) {
        if tile != self.tile {
            *self = Shape { from: self.at(now), tile, start: Some(now) };
        }
    }

    /// Whether it's still turning at `now`.
    fn turning(&self, now: Instant) -> bool {
        self.start.is_some_and(|start| {
            Curve::Room.at(now.duration_since(start).as_secs_f32()).is_some()
        })
    }
}

/// Two fingers on the trackpad over the sidebar, which may page between Spaces.
struct Swipe {
    /// The Space shown when they came down.
    from: u64,
    /// Whether they came down in the Library, or as it moved, so they move it.
    library: bool,
    /// Across the sidebar, to page, or along it, to scroll the list. None until they first move.
    across: Option<bool>,
    /// How far they've taken the page of the Space they began at, before the rubber band.
    way: f32,
    /// Where that page was over the last SWIPE_HISTORY, for how fast it's let go.
    samples: VecDeque<(Instant, f32)>,
    /// The Space nearest its place, for the detent as it changes.
    nearest: u64,
    /// Whether they've pulled the last Space far enough to ready a new Space, and when that last
    /// changed.
    ready: bool,
    turned: Option<Instant>,
}

/// A Space's page settling on its place, after a swipe or a switch.
struct Settle {
    space: u64,
    /// The page it pushes out: the Space shown before a switch. After a swipe it's the Space next
    /// to it on the side it's coming from.
    beside: Option<u64>,
    /// How far from its place it started, and how fast it was going.
    from: f32,
    velocity: f32,
    start: Instant,
    /// A switch to the Space next to the one before, whose colour changes ahead of the slide.
    next_door: bool,
}

/// A page's full screen resizing the window, while `Shell::full_screen_step` waits to time it.
struct FullScreenTiming {
    entering: bool,
    /// The window's content size before.
    before: Size<Pixels>,
    laid: bool,
    drawn: bool,
}

impl Settle {
    /// How much of the Space before's colour is left, for a switch next door.
    fn color_left(&self) -> Option<f32> {
        let t = self.start.elapsed().as_secs_f32();
        self.next_door.then(|| (-t / SPACE_COLOR_TAU).exp())
    }

    /// How far the page is from its place, or None once it's there.
    fn offset(&self) -> Option<f32> {
        let omega = std::f32::consts::TAU / SPACE_SETTLE;
        let t = self.start.elapsed().as_secs_f32();
        let decay = (-omega * t).exp();
        let rise = self.velocity + omega * self.from;
        let offset = (self.from + rise * t) * decay;
        let speed = (self.velocity - omega * rise * t) * decay;
        (offset.abs() > 0.25 || speed.abs() > 10.).then_some(offset)
    }
}

/// What the user asked to show, timed until the core shows it: a Space's switch ends only at the
/// Space's report and a tab's only at a tab's, whatever the core reports between.
#[derive(Clone, Copy)]
enum Switch {
    Space(u64),
    Tab(u64),
}

pub struct Shell {
    browser: Rc<dyn Browser>,
    /// Holds GPUI's focus while nothing in the shell takes it, so the shortcuts have their context.
    focus_handle: FocusHandle,
    /// What the reveal's window focuses, so its keys match the shell's bindings.
    reveal_focus: FocusHandle,
    /// In the switcher's order.
    spaces: Vec<Space>,
    /// The Space the window shows.
    space: u64,
    form: Option<SpaceForm>,
    /// The form Create sent away, and when, until it's gone and its Space shows its rows.
    leaving_form: Option<(SpaceForm, Instant)>,
    /// The form a swipe back cancelled, while its page goes out.
    form_going: Option<SpaceForm>,
    /// The form's field takes GPUI's focus at the next draw, which is when the shell has a window.
    focus_form: bool,
    renaming: Option<Renaming>,
    /// The same for the renaming field.
    focus_renaming: bool,
    /// The pinned tabs and folders changed and haven't been drawn since.
    outline_changed: bool,
    /// The favorites the shown Space shows over its own tabs, left to right.
    favorites: Vec<u64>,
    /// Every favorite, and each one's holder: the Space it belongs to under Each Space, else the
    /// profile whose tab it is. The ids come from one counter, so a Space's never is a profile's.
    all_favorites: Vec<u64>,
    favorite_holders: HashMap<u64, u64>,
    /// The profiles, by id and name, the one the browser started with first.
    profiles: Vec<(u64, SharedString)>,
    /// The tabs of every Space, and the favorites.
    tabs: HashMap<u64, Tab>,
    /// The loading dots' clock.
    breath: Breath,
    /// The tab each agent last acted in, by the agent's id, as the activity log has it: an agent
    /// is at work there, which its row's dot shows while the tab is in an agent's Space.
    agents_at: HashMap<SharedString, u64>,
    selected: u64,
    /// Tabs of every Space, shown most recently first.
    recent: Vec<u64>,
    /// ⌃ is held after a ⌃Tab, which showed this tab. The tabs a walk shows don't count as shown
    /// until ⌃ goes up, and then its last one does, though a click may have shown another since.
    walked: Option<u64>,
    hovered: Option<Row>,
    /// A shortcut the page handed back ran since the pointer last moved. The sidebar draws no row
    /// or favicon under the pointer until it moves, as GPUI hides its hover after a key of its own,
    /// so a row the pointer rests on doesn't read as selected beside the tab a key showed.
    page_keyed: bool,
    /// A drag ended, so the tooltip of the tile or folder the pointer is on, which it kept from
    /// asking while it was hovered under the drag, is asked for once the rows and tiles settle.
    retip: bool,
    /// The tab whose menu is up, whose row keeps its hover fill until the menu closes.
    menu_row: Option<u64>,
    /// The rows ⌘- and ⇧-click chose, by their first tabs (chosen.rs), and the row the next
    /// ⇧-click chooses from, or 0 for the one shown.
    chosen: Vec<u64>,
    chosen_from: u64,
    /// ilo's own menu is up, which keeps the reveal's panel out while the pointer is on it.
    menu_open: bool,
    /// Rows the pointer left, whose hover fill is fading, and since when.
    unhovered: Vec<(Row, Instant)>,
    /// The row the pointer pressed, for its plate's shrinking and growing back.
    held_row: Option<HeldRow>,
    /// The header button the pointer pressed, for its shrinking and growing back.
    held_nav: Option<HeldNav>,
    /// Rows on their way to their place: how far from it each started, and when.
    slides: HashMap<Slot, (f32, Instant, Curve)>,
    /// Where the frame last drawn had each tab's row in the list, which the pointer's presses hit
    /// until the next one is drawn (`press_at_rest`).
    drawn_rows: RefCell<Vec<(u64, f32)>>,
    /// The same for the shown Space's URL field and favorites: their page's offset, and each
    /// tile's place under the favorites' top (`press_top_at_rest`).
    drawn_top: RefCell<(f32, HashMap<u64, [f32; 3]>)>,
    /// What a press `press_top_at_rest` took rests on, for its release to click.
    rest_press: Option<beside::AtRest>,
    /// The user opened a folder, or closed one, whose rows move once the browser reports it.
    folding: Option<bool>,
    /// The same for tiles, by their left, top and width, and for the list's top as the favorites
    /// take a row more or fewer.
    tile_slides: HashMap<u64, ([f32; 3], Instant, Curve)>,
    list_slide: Option<(f32, Instant, Curve)>,
    /// The list's scroll, and the sidebar's top-left corner in the window the last press was in,
    /// for where the pointer is in them.
    list_scroll: ScrollHandle,
    sidebar_origin: Rc<Cell<Point<Pixels>>>,
    /// The frames the favorites' tiles and the folders' plates were last drawn in, in the window,
    /// for their tooltips to hang from.
    tip_frames: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// A closed folder whose row lost the hover with the pointer last seen on it, so its preview
    /// hasn't heard it left: the next move off the row tells it.
    preview_row: Option<u64>,
    /// Where the last move or press in the window put the pointer. GPUI's own mouse position
    /// becomes the system cursor's whenever the window moves or resizes, as it can when the
    /// preview's window is put over it, with no move, which is never where a driven pointer is.
    pointer_at: Point<Pixels>,
    /// How long each frame drawn while the Spaces' pages moved took the sidebar, in ms, for the
    /// settle's log.
    paging_frames: Rc<RefCell<Vec<f32>>>,
    /// Where the page of the Space beside drew its tiles, by tab, and its URL field, as 0.
    #[cfg(test)]
    beside_frames: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// The plates the sidebar's buttons with a tooltip were last drawn in, by their command.
    button_frames: Rc<RefCell<HashMap<&'static str, Bounds<Pixels>>>>,
    /// The icon slots the Library's Space cards were last drawn with, by Space, for their
    /// tooltips and the icon picker to hang from.
    slot_frames: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// The brushes the Library's Space cards were last drawn with, by Space, for the theme picker
    /// to open beside.
    brush_frames: Rc<RefCell<HashMap<u64, Bounds<Pixels>>>>,
    /// Whether the press on a card's icon slot or brush came while its own picker showed, so the
    /// click closes it rather than opening it again: the press outside the picker has closed it,
    /// but the picker hears so only after the press.
    card_press_closes: bool,
    /// Whether each Space card's field was last drawn with its plate, for its log lines.
    fields_drawn: Rc<RefCell<HashMap<u64, bool>>>,
    /// Where the Library's empty line was last laid out, for its log line.
    empty_drawn: Rc<RefCell<String>>,
    /// Whether the list is past its top, and the edge under the favorites' fade since it last
    /// went past or back, from how far in it was then.
    scrolled: bool,
    scroll_edge_fade: Option<(Instant, f32)>,
    /// The edge the selected tab's marker was last drawn at, true for the top, and its frame in
    /// the window (`shown_marker`).
    marker_drawn: Cell<Option<(bool, Bounds<Pixels>)>>,
    /// Whether the pinned rows' chevron was last drawn for collapsed ones, and where.
    caret_drawn: Rc<Cell<Option<(bool, Bounds<Pixels>)>>>,
    /// The row whose ring the keys move while the sidebar has the keyboard (sidebar_keys.rs), the
    /// focus it takes them with, whether it takes them at the next draw, and where the ring was
    /// last drawn.
    keyed: Option<u64>,
    sidebar_keys: FocusHandle,
    focus_keys: bool,
    keyed_drawn: Rc<Cell<Option<(u64, Bounds<Pixels>)>>>,
    /// Whether Clear and the URL field's button were fading in or shown at the last frame, the
    /// fade since it last changed, from how far in they were then, and whether they were drawn
    /// when the last line was logged.
    tools_on: bool,
    tools_fade: Option<(Instant, f32)>,
    drawn_tools: bool,
    /// Whether the tiles scrolled with the list in the last frame.
    tiles_scrolled_last: bool,
    /// Tabs' new rows, which the rows sliding down to make room come out from beneath.
    opened: Option<Opened>,
    closing: Vec<Closing>,
    /// The tabs Clear is closing, whose rows go in one frame.
    clearing: Vec<u64>,
    /// ⌘T's command bar is open, and "New Tab" has the selected plate in place of the tab shown
    /// (Arc).
    new_tab: bool,
    /// When the card started to show no page, for the line on it to fade in from.
    empty_since: Option<Instant>,
    /// The fade has ended and said so.
    empty_settled: bool,
    /// The first run's welcome takes the empty card until a page shows (set_welcome).
    welcome: bool,
    /// A new profile's first launch, or Help › Welcome to ilo, over the window while it shows.
    onboarding: Option<onboarding::Onboarding>,
    /// The first launch's import, made at its first step and kept for the window's life, so an
    /// import under way goes on while the steps after it show and once the flow has ended.
    importing: Option<onboarding::Importing>,
    /// The onboarding has just ended, so the empty card says what the command bar over it is,
    /// until a page shows.
    in_hint: bool,
    /// The press under way was on a target that opens the command bar while it was open, so the
    /// click that follows only closes it.
    press_closed_bar: bool,
    /// The window shows another Space, whose tab the core says next, for the command bar to follow
    /// once it has.
    bar_follows: bool,
    drag: Option<Drag>,
    /// A drag just ended, so the release isn't a click on the row.
    dragged: bool,
    /// Escape put a dragged tab back, and the frame that shows it all put back isn't drawn yet.
    putting_back: bool,
    swipe: Option<Swipe>,
    settle: Option<Settle>,
    /// The last swipe paged, so its momentum, which comes without a touch, doesn't scroll the list.
    swiped: bool,
    /// A wheel's ticks across the sidebar: their sum and the last one's time, and when they last
    /// paged.
    wheel: Option<(f32, Instant)>,
    wheel_paged: Option<Instant>,
    /// The tab whose favicon is under the pointer, which is Back to Pinned URL while it's away.
    hovered_favicon: Option<u64>,
    /// Whether an assistive app reads the window being drawn, so its elements say what they are
    /// (access.rs). Each render sets it from its window.
    a11y: bool,
    /// The row and favicon hovered when the rows were last drawn, and that row's close button:
    /// "0" for none, "1" for ✕ and "−", for the line check.sh waits for.
    drawn_hover: (Option<Row>, Option<u64>, &'static str),
    /// What ⌘Z undoes and ⇧⌘Z does again (`undo`).
    history: undo::History,
    /// The toast shows an Undo button, which the next thing the user does takes away.
    undo_offered: bool,
    /// The link preview the user closed last, for ⇧⌘T while no tab has closed since.
    closed_preview: Option<crate::link_preview::Closed>,
    /// What the Edit menu was last told Undo and Redo do.
    edit_titles: (Option<&'static str>, Option<&'static str>),
    /// Whether the pointer is over the docked sidebar, which shows Clear and the URL field's button
    /// only then.
    sidebar_hovered: bool,
    /// The hover changed since the sidebar was last drawn.
    hover_changed: bool,
    page_fullscreen: bool,
    /// The link the pointer is on in the page shown, whose address the plate in the card's corner
    /// shows, and what measures it.
    link: Option<link::Link>,
    link_system: Option<WindowTextSystem>,
    /// Whether the window is full screen, as macOS last told once its transition was over.
    full_screen: bool,
    /// On Linux, whether the window is maximized, which its third button shows.
    maximized: bool,
    /// Whether the window went full screen for the page, so it comes back with it.
    window_for_page: bool,
    /// While the window goes full screen with the page, or back: its content size before. The page
    /// stays where it is until the window's resize lays it out, once, at its new size.
    page_held: Option<Size<Pixels>>,
    /// While a page's full screen resizes the window: the steps check.sh has yet to time.
    full_screen_timing: Option<FullScreenTiming>,
    /// Whether ⌘S has hidden the sidebar.
    sidebar_hidden: bool,
    /// Whether the window's own buttons show (`window_buttons_shown`).
    window_buttons: bool,
    /// The window's own buttons as the system draws them, once it has said, which the copies
    /// drawn in their place match.
    window_button_geometry: Option<WindowButtonGeometry>,
    /// The docked sidebar's width, at rest.
    sidebar_width: f32,
    /// While the sidebar's edge is dragged: how far right of the edge the pointer pressed it, and
    /// the width the pointer has it at, before it's held between the limits.
    resize: Option<(f32, f32)>,
    /// While the gap before a split view's page is dragged: that page's index, and how far past
    /// its start the pointer pressed the gap.
    pane_resize: Option<(usize, f32)>,
    /// While the width settles on `sidebar_width`: the width it started from, and when.
    width_motion: Option<(f32, Instant)>,
    /// While the sidebar moves: where the card's left edge started from, and when.
    sidebar_motion: Option<(f32, Instant)>,
    /// Lines check.sh waits on that say the page is where a press or a drag put it, held until
    /// the draw that lays the page out there, as the event changes only what that draw shows.
    logged_once_laid: Vec<String>,
    /// Whether the hidden sidebar is out over the page, or on its way.
    revealed: bool,
    /// The reveal's panel came out for ⌥ with the pointer elsewhere, so it stays out until the
    /// pointer has come over it, or ⌥ again.
    revealed_by_option: bool,
    /// While the reveal panel moves: how far out it started (`reveal_out`), and when.
    reveal_motion: Option<(f32, Instant)>,
    /// The pointer is at the window's left edge, waiting to bring the sidebar out.
    reveal_wait: Option<Task<()>>,
    /// Where across the window the pointer was last, for the reveal, and whether it's on a row's
    /// hover card, which keeps the sidebar out as the panel does.
    reveal_x: f32,
    pointer_on_card: bool,
    /// The Copy URL button shows a checkmark until this one-shot timer ends.
    url_copied: Option<Task<()>>,
    /// The tab, direction and place of the history menu asked for, until its pages come.
    history_asked: Option<(u64, bool, Point<Pixels>)>,
    /// Back or forward held, until it's held for HOLD (history.rs) and lists the history.
    nav_hold: Option<Task<()>>,
    /// Whether the last press on back or forward was held for its menu, so its click goes.
    nav_held: bool,
    /// The tabs the switches since the last quiet moment hid, whose hover cards' pictures are
    /// taken once they've been quiet a while, and that wait.
    card_hidden: Vec<u64>,
    card_wait: Option<Task<()>>,
    /// What the button showed when last drawn, for the lines check.sh waits for.
    drawn_copied: &'static str,
    /// The Space shown at the last draw, and whether its Clear had the arrow, while it had a
    /// Clear divider.
    drawn_arrow: Option<(u64, bool)>,
    /// Whether the pointer is over the divider's Clear, which then draws brighter.
    clear_hovered: bool,
    /// The glyphs changing into others.
    swaps: Swaps,
    /// The window's content size at the last draw, and the frame and corner radius the page was
    /// last laid out at.
    viewport: Size<Pixels>,
    /// The window's device pixels in a point, kept from render as `viewport` is.
    scale: f32,
    page_laid: Option<(Bounds<Pixels>, f32)>,
    /// Whether the core was told the page's frame is moving (`page_motion`).
    page_moving_told: bool,
    /// While the command bar is open for a pane beside the tab shown, that tab. The pages shown
    /// make room on their right for the pane (Arc).
    adding_pane: Option<u64>,
    /// The tab a tab menu's Add Split View showed, to open the pane beside once it's shown.
    split_pending: Option<u64>,
    /// The tab Reveal Tab in Sidebar opened folders for, to scroll to once they've opened.
    revealing: Option<u64>,
    /// The colour last given for a split view's focused page.
    focus_color: Option<u32>,
    /// The colour last given for a page before its first frame.
    page_color: Option<u32>,
    /// The window over the page that draws the drop targets and the agent's pointer shows.
    drop_targets_shown: bool,
    /// The agent's pointer at an element in the page, while it acts there.
    agent_pointer: Option<agent_pointer::AgentPointer>,
    /// A link dragged over the page, and its targets.
    link_over: Option<split_targets::LinkOver>,
    /// How far in from the page frame's top right corner the focused page's is, as last given.
    pane_corner: (f32, f32),
    /// Every Space's archived tabs, newest first, and the favicons asked for their URLs.
    archive: Vec<Archived>,
    page_favicons: HashMap<SharedString, Option<Arc<Image>>>,
    /// The profile's downloads, newest first, and the file icons asked for them.
    downloads: Vec<Download>,
    download_icons: HashMap<u64, Option<Arc<Image>>>,
    /// The download whose row the left button last pressed, where, and whether the press became
    /// the system's drag of its file.
    download_press: Option<(u64, Point<Pixels>, bool)>,
    /// The drag of a download's file from the Library closed it, to come back if nothing takes it.
    library_dragged: bool,
    /// The captures saved to the Library, newest first.
    captures: Vec<Captured>,
    /// The previews asked for the recent downloads' files and the Library's media, by download, or
    /// by `library::capture_id`, and the rows over the Library button.
    download_previews: HashMap<u64, Option<FilePreview>>,
    /// The videos Media's tiles play, by tile, and the tiles drawn since the last frame.
    media_videos: HashMap<u64, MediaVideo>,
    videos_drawn: HashSet<u64>,
    recent_downloads: RecentDownloads,
    /// Every download a report has listed, whether one has come, and the new download's icon on its
    /// way to the Library button.
    downloads_seen: HashSet<u64>,
    /// Every tab a Space has listed, so a tab a Space lists that none did is new, not moved.
    listed: HashSet<u64>,
    /// Every Space listed, so one ⌘Z brings back isn't new; and the new ones the toast tells of,
    /// and when it last did, so Spaces made one after another are told of together.
    spaces_listed: HashSet<u64>,
    spaces_toasted: (Vec<u64>, Option<Instant>),
    downloads_read: bool,
    flight: Option<Flight>,
    /// The picture of the page another window shows, in place of it, and the pictures to let go of.
    picture: Option<picture::Picture>,
    stale_pictures: Vec<Arc<gpui::RenderImage>>,
    /// Each profile's extensions.
    extensions: HashMap<u64, Vec<Extension>>,
    /// What the menu bar was last given for this window, and the window's title.
    menus: MenuBar,
    window_title: SharedString,
    /// What the URL field said when it last drew, for its log line.
    url_field_drawn: RefCell<SharedString>,
    /// Whether the header last drew reload or stop, for its log line.
    reload_drawn: Cell<Option<Command>>,
    /// Whether the header last drew back on or dimmed, for its log line.
    back_drawn: Cell<Option<bool>>,
    /// The tab whose Developer Mode outline last drew, and where, for its log lines.
    developer_outline_drawn: Rc<Cell<Option<(u64, Bounds<Pixels>)>>>,
    /// The pinned actions the window shows, for its tab shown.
    extension_actions: Vec<ExtensionAction>,
    /// The other turned-on extensions' actions, which the core reports while the Site Controls
    /// show, for their grid.
    unpinned_actions: Vec<ExtensionAction>,
    /// The extension's side panel shown right of the page, the width the user gave it, and where
    /// its page was last laid out.
    side_panel: Option<SidePanel>,
    side_panel_width: f32,
    side_panel_laid: Option<Option<Bounds<Pixels>>>,
    /// While the panel's edge is dragged: how far right of the panel's left edge the pointer
    /// pressed it.
    side_panel_resize: Option<f32>,
    /// The Library's section shown last, which it opens on.
    section: Section,
    /// The Library, from when it opens until it has closed.
    library: Option<library::Library>,
    library_open: bool,
    /// While the Library's width goes to another section's: the width it left, and when.
    library_resize: Option<(f32, Instant)>,
    /// While the card moves for the Library on its own: how far out it started (`library_out`),
    /// how fast, and when.
    library_motion: Option<(f32, f32, Instant)>,
    /// While the sidebar under the Library fades: its opacity when it started, and when.
    sidebar_fade: Option<(f32, Instant)>,
    /// While the circle goes back after a pull made a Space.
    circle_exit: Option<new_space::CircleExit>,
    /// What the tabs' pages ask, such as for the camera: only shown tabs ask.
    questions: HashMap<u64, Question>,
    /// Where the question's card was drawn last, which the log says when it changes.
    question_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The tab whose question's card shows, and its moment on screen before Block and Allow work.
    question_settling: RefCell<Option<(u64, crate::settling::Settling)>>,
    /// The Cast dialog the core reports for the tab it's for (cast.rs).
    cast: Option<(u64, Cast)>,
    cast_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// What an agent asks the user first, in every window (agent_question.rs).
    agent_question: Option<crate::agents::Question>,
    agent_question_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// What the agents do and what the user holds from them, for the bar in an agent's Space
    /// (agent_bar.rs).
    agent_status: Vec<crate::agents::Status>,
    agent_bar_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The tabs whose player × put away until their page plays again.
    media_dismissed: HashSet<u64>,
    /// The players last drawn, as the log has them, and where the bottom one is.
    media_logged: RefCell<Vec<u64>>,
    media_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Whether a tooltip the agent bar asked for may be up, and whether its bar showed the agent
    /// at work, so it ends once that bar is gone (agent_bar.rs).
    agent_tip: Cell<Option<bool>>,
    /// The protected-media key system each tab's page asked for that ilo can't play, until the tab
    /// goes to another site.
    drm_notes: HashMap<u64, SharedString>,
    /// The tabs' keys, for those whose page the password manager has something for.
    keys: HashMap<u64, Key>,
    /// Where the note's card was drawn last, which the log says when it changes.
    drm_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The card of each tab whose page died while this window showed it, until it loads again.
    crash_cards: HashMap<u64, trouble::CrashCard>,
    /// Where a crash card and its Reload were drawn last, which the log says when they change.
    crash_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    crash_reload_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The tab the toast's note is about while its page is hung and shown.
    hang_noted: Option<u64>,
    /// The dialogs the tabs' pages ask, such as an alert(): the tab shown's goes over its card.
    page_dialogs: HashMap<u64, PageDialog>,
    /// The page dialog asked over the card last, by its tab and id, and where.
    page_dialog_shown: Option<(u64, u64, (Bounds<Pixels>, f32))>,
    /// The tab or Space the user asked for and when, until it's shown.
    switching: Option<(Switch, Instant)>,
    /// A press on the window's background, until the pointer moves and it moves the window.
    background_pressed: bool,
    /// What the chrome draws with, for the Space shown (palette.rs), and each theme's, worked
    /// out once.
    palette: Palette,
    palettes: RefCell<Vec<((Option<Theme>, Tone), Palette)>>,
    /// The system's appearance as the window last drew, or the one View › Appearance chose in its
    /// place, which a Space without a theme, or with an automatic one, follows; the preview's own
    /// where it forces one; whether the window is the active one, as its texture shows; and the
    /// watches that draw the window again once either changes.
    system_tone: Tone,
    app_appearance: Appearance,
    forced_tone: Option<Tone>,
    window_active: bool,
    window_watches: Vec<Subscription>,
    /// Whether the host was last asked to draw the material light.
    window_light: Option<bool>,
    /// The menus' look capture mode was last given, which it takes again when it changes (#861).
    capture_look: Option<crate::menu::Look>,
    /// The picker that opens once the row it points at stops moving: a Space's or a tab's icon
    /// picker, or a Space's theme picker; and when it was asked for.
    picker_pending: Option<(Pending, Instant)>,
    /// The theme the picker is making for a Space, which the window shows as it's made, until the
    /// core has it.
    theme_preview: Option<(u64, Theme)>,
    /// Where the name row was drawn last, and the frame before; and the same for the pending
    /// tab's row, which draws it only while it is pending.
    name_row_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    tab_row_drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// The icon each tile drew last, for the line check.sh waits on once one changes.
    tile_icons_drawn: Rc<RefCell<HashMap<u64, SharedString>>>,
    name_row_before: Option<Bounds<Pixels>>,
    /// The Space with no icon the icon picker shows for, whose name row holds the icon's place
    /// with a placeholder until the picker closes, and the watch on the picker that tells it so.
    icon_placeholder: Option<(u64, Subscription)>,
    /// The Space whose card in the Library a picker opened from, whose field stays lit while the
    /// picker shows for it, as while its name is typed over; and the watch on the picker.
    card_picker: Option<(u64, Subscription)>,
    /// The watch on a picker that showed for another card when a card's slot or brush was
    /// clicked, which opens it from that card once it has closed.
    card_move: Option<Subscription>,
    /// A private window's: its one Space is its own, unsaved, in an off-the-record session.
    private: bool,
}

impl Shell {
    /// Makes the shell, the one the browser's shortcuts act on.
    pub fn new(browser: Rc<dyn Browser>, cx: &mut Context<Self>) -> Self {
        cx.set_global(ShellHandle(cx.weak_entity()));
        let sidebar_width = browser
            .saved_sidebar_width()
            .map_or(SIDEBAR_WIDTH, |width| width.clamp(SIDEBAR_MIN, SIDEBAR_MAX));
        Self {
            browser,
            focus_handle: cx.focus_handle(),
            reveal_focus: cx.focus_handle(),
            spaces: Vec::new(),
            space: 0,
            form: None,
            leaving_form: None,
            form_going: None,
            focus_form: false,
            renaming: None,
            focus_renaming: false,
            outline_changed: false,
            favorites: Vec::new(),
            all_favorites: Vec::new(),
            favorite_holders: HashMap::new(),
            profiles: Vec::new(),
            tabs: HashMap::new(),
            breath: Breath::default(),
            agents_at: HashMap::new(),
            recent: Vec::new(),
            walked: None,
            selected: 0,
            hovered: None,
            page_keyed: false,
            retip: false,
            menu_row: None,
            chosen: Vec::new(),
            chosen_from: 0,
            menu_open: false,
            unhovered: Vec::new(),
            held_row: None,
            held_nav: None,
            slides: HashMap::new(),
            drawn_rows: RefCell::default(),
            drawn_top: RefCell::default(),
            rest_press: None,
            folding: None,
            tile_slides: HashMap::new(),
            list_slide: None,
            list_scroll: ScrollHandle::new(),
            sidebar_origin: Rc::default(),
            tip_frames: Rc::default(),
            preview_row: None,
            pointer_at: Point::default(),
            paging_frames: Rc::default(),
            #[cfg(test)]
            beside_frames: Rc::default(),
            button_frames: Rc::default(),
            slot_frames: Rc::default(),
            brush_frames: Rc::default(),
            card_press_closes: false,
            fields_drawn: Rc::default(),
            empty_drawn: Rc::default(),
            scrolled: false,
            scroll_edge_fade: None,
            marker_drawn: Cell::new(None),
            caret_drawn: Rc::default(),
            keyed: None,
            sidebar_keys: cx.focus_handle(),
            focus_keys: false,
            keyed_drawn: Rc::default(),
            tools_on: false,
            tools_fade: None,
            drawn_tools: false,
            tiles_scrolled_last: false,
            opened: None,
            closing: Vec::new(),
            clearing: Vec::new(),
            new_tab: false,
            empty_since: None,
            empty_settled: false,
            welcome: false,
            onboarding: None,
            importing: None,
            in_hint: false,
            press_closed_bar: false,
            bar_follows: false,
            drag: None,
            dragged: false,
            putting_back: false,
            swipe: None,
            settle: None,
            swiped: false,
            wheel: None,
            wheel_paged: None,
            hovered_favicon: None,
            a11y: false,
            drawn_hover: (None, None, "0"),
            history: undo::History::default(),
            undo_offered: false,
            closed_preview: None,
            edit_titles: (None, None),
            sidebar_hovered: false,
            hover_changed: false,
            page_fullscreen: false,
            link: None,
            link_system: None,
            full_screen: false,
            maximized: false,
            window_for_page: false,
            page_held: None,
            full_screen_timing: None,
            sidebar_hidden: false,
            window_buttons: true,
            window_button_geometry: None,
            sidebar_width,
            resize: None,
            pane_resize: None,
            width_motion: None,
            sidebar_motion: None,
            logged_once_laid: Vec::new(),
            revealed: false,
            revealed_by_option: false,
            reveal_motion: None,
            reveal_wait: None,
            reveal_x: 0.,
            pointer_on_card: false,
            url_copied: None,
            history_asked: None,
            nav_hold: None,
            nav_held: false,
            card_hidden: Vec::new(),
            card_wait: None,
            drawn_copied: "shows link",
            drawn_arrow: None,
            clear_hovered: false,
            swaps: Swaps::default(),
            viewport: Size::default(),
            scale: 1.,
            page_laid: None,
            page_moving_told: false,
            adding_pane: None,
            split_pending: None,
            revealing: None,
            focus_color: None,
            page_color: None,
            drop_targets_shown: false,
            agent_pointer: None,
            link_over: None,
            pane_corner: (0., 0.),
            archive: Vec::new(),
            page_favicons: HashMap::new(),
            downloads: Vec::new(),
            download_icons: HashMap::new(),
            download_press: None,
            library_dragged: false,
            captures: Vec::new(),
            download_previews: HashMap::new(),
            media_videos: HashMap::new(),
            videos_drawn: HashSet::new(),
            recent_downloads: RecentDownloads::default(),
            downloads_seen: HashSet::new(),
            listed: HashSet::new(),
            spaces_listed: HashSet::new(),
            spaces_toasted: (Vec::new(), None),
            downloads_read: false,
            flight: None,
            picture: None,
            stale_pictures: Vec::new(),
            extensions: HashMap::new(),
            menus: MenuBar::default(),
            window_title: SharedString::default(),
            url_field_drawn: RefCell::default(),
            reload_drawn: Cell::default(),
            back_drawn: Cell::default(),
            developer_outline_drawn: Rc::default(),
            extension_actions: Vec::new(),
            unpinned_actions: Vec::new(),
            side_panel: None,
            side_panel_width: 0.,
            side_panel_laid: None,
            side_panel_resize: None,
            section: Section::Archive,
            library: None,
            library_open: false,
            library_resize: None,
            library_motion: None,
            sidebar_fade: None,
            circle_exit: None,
            questions: HashMap::new(),
            question_drawn: Rc::default(),
            cast: None,
            cast_drawn: Rc::default(),
            question_settling: RefCell::default(),
            agent_question: None,
            agent_question_drawn: Rc::default(),
            agent_status: Vec::new(),
            agent_bar_drawn: Rc::default(),
            media_dismissed: HashSet::new(),
            media_logged: RefCell::default(),
            media_drawn: Rc::default(),
            agent_tip: Cell::default(),
            drm_notes: HashMap::new(),
            keys: HashMap::new(),
            drm_drawn: Rc::default(),
            crash_cards: HashMap::new(),
            crash_drawn: Rc::default(),
            crash_reload_drawn: Rc::default(),
            hang_noted: None,
            page_dialogs: HashMap::new(),
            page_dialog_shown: None,
            picker_pending: None,
            theme_preview: None,
            name_row_drawn: Rc::default(),
            tab_row_drawn: Rc::default(),
            tile_icons_drawn: Rc::default(),
            name_row_before: None,
            icon_placeholder: None,
            card_picker: None,
            card_move: None,
            switching: None,
            background_pressed: false,
            palette: Palette::default(),
            palettes: RefCell::default(),
            system_tone: Tone::Dark,
            app_appearance: Appearance::Automatic,
            forced_tone: None,
            window_active: true,
            window_watches: Vec::new(),
            window_light: None,
            capture_look: None,
            private: false,
        }
    }

    /// A private window's shell (docs/ux-spec.md › Profiles › Incognito): it shows `space` alone,
    /// named `name`, whose tabs are in `profile`'s off-the-record session. It hears nothing of the
    /// other Spaces, the favorites or the Library, and offers none of them.
    pub fn private_window(
        browser: Rc<dyn Browser>,
        space: u64,
        name: SharedString,
        profile: u64,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut shell = Self::new(browser, cx);
        shell.private = true;
        shell.space = space;
        shell.spaces = vec![Space {
            id: space,
            name,
            color: None,
            theme: None,
            icon: SharedString::default(),
            agent: SharedString::default(),
            profile,
            pinned: Vec::new(),
            today: Vec::new(),
            splits: Vec::new(),
            folders: Folders::default(),
            shown: 0,
        }];
        shell
    }

    /// Whether this is a private window's shell.
    pub fn private(&self) -> bool {
        self.private
    }

    /// A shell for another window, which starts out knowing what `from` heard from the core about
    /// every window: the Spaces, their tabs, the Library and the extensions. What's this window's,
    /// such as the Space shown, it hears for itself.
    pub fn seeded_from(
        from: &Entity<Shell>,
        browser: Rc<dyn Browser>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut shell = Self::new(browser, cx);
        let from = from.read(cx);
        shell.spaces = from.spaces.clone();
        shell.tabs = from.tabs.clone();
        shell.favorites = from.favorites.clone();
        shell.all_favorites = from.all_favorites.clone();
        shell.favorite_holders = from.favorite_holders.clone();
        shell.profiles = from.profiles.clone();
        shell.archive = from.archive.clone();
        shell.page_favicons = from.page_favicons.clone();
        shell.downloads = from.downloads.clone();
        shell.download_icons = from.download_icons.clone();
        shell.captures = from.captures.clone();
        shell.download_previews = from.download_previews.clone();
        shell.downloads_seen = from.downloads_seen.clone();
        shell.downloads_read = from.downloads_read;
        shell.extensions = from.extensions.clone();
        shell.questions = from.questions.clone();
        shell.drm_notes = from.drm_notes.clone();
        shell.keys = from.keys.clone();
        shell.crash_cards = from.crash_cards.keys().map(|&tab| (tab, Default::default())).collect();
        shell.page_dialogs = from.page_dialogs.clone();
        shell
    }

    /// seeded_from, for a window that opens behind the key one, as the launch brings saved
    /// windows back: the shortcuts, the menus and the Dock's dot stay with the window in front,
    /// since macOS reports no key change for a window that already was.
    pub fn seeded_behind(
        from: &Entity<Shell>,
        browser: Rc<dyn Browser>,
        cx: &mut Context<Self>,
    ) -> Self {
        let key = cx.try_global::<ShellHandle>().map(|handle| handle.0.clone());
        let shell = Self::seeded_from(from, browser, cx);
        if let Some(key) = key {
            cx.set_global(ShellHandle(key));
        }
        shell
    }

    /// The pinned tabs of the Space shown.
    pub fn pinned(&self) -> &[u64] {
        self.shown_space().map_or(&[], |space| &space.pinned)
    }

    /// The Today tabs of the Space shown.
    pub fn today(&self) -> &[u64] {
        self.shown_space().map_or(&[], |space| &space.today)
    }

    /// The split views of the Space shown.
    pub fn splits(&self) -> Vec<Split> {
        self.shown_space().map_or_else(Vec::new, |space| space.splits.clone())
    }

    /// The favorites, which every Space shows.
    pub fn favorites(&self) -> &[u64] {
        &self.favorites
    }

    /// The tabs of the rows of the Space shown, pinned and Today (`Space::rows`).
    fn rows(&self) -> (Vec<u64>, Vec<u64>) {
        self.shown_space().map_or_else(Default::default, Space::rows)
    }

    /// Where the rows of the Space shown are in the list.
    fn layout(&self) -> HashMap<Slot, f32> {
        self.shown_space().map_or_else(|| slots(&[], &[]), Space::slots)
    }

    /// The tabs shown with `tab` in the Space shown, in their order: its split's, else `tab` alone.
    fn panes(&self, tab: u64) -> Vec<u64> {
        self.shown_space().map_or_else(|| vec![tab], |space| space.panes(tab))
    }

    /// Whether `tab`'s split view in the Space shown is stacked.
    fn stacked(&self, tab: u64) -> bool {
        self.shown_space().is_some_and(|space| space.stacked(tab))
    }

    /// The tab of `tab`'s split view in the Space shown that took the card, else 0.
    fn expanded(&self, tab: u64) -> u64 {
        self.shown_space().map_or(0, |space| space.expanded(tab))
    }

    /// The shares of `tab`'s split view in the Space shown, or none while they're even. An
    /// expanded one's come from the page's frame, as the core's do at its layout.
    fn shares(&self, tab: u64) -> Vec<f64> {
        let expanded = self.expanded(tab);
        let panes = self.panes(tab);
        match panes.iter().position(|&pane| pane == expanded) {
            Some(focused) => {
                let (frame, _) = self.page_frame(self.viewport);
                let length = match self.stacked(tab) {
                    true => frame.size.height,
                    false => frame.size.width,
                };
                panes::expanded_shares(panes.len(), focused, f32::from(length))
            }
            None => self.shown_space().map_or_else(Vec::new, |space| space.shares(tab)),
        }
    }

    /// The tab whose row is `tab`'s: its split's first.
    fn row_of(&self, tab: u64) -> u64 {
        self.panes(tab)[0]
    }

    fn is_folder(&self, row: u64) -> bool {
        self.shown_space().is_some_and(|space| space.is_folder(row))
    }

    /// How many folders `row` is in, in the Space shown.
    fn depth(&self, row: u64) -> usize {
        self.shown_space().map_or(0, |space| space.depth(row))
    }

    /// What a row says: a folder's name, else the tab's title.
    fn label(&self, row: u64) -> SharedString {
        match self.is_folder(row) {
            true => self.folder_name(row),
            false => self.title(row),
        }
    }

    /// The folders of the Space shown, for the preview.
    pub fn folders(&self) -> Folders {
        self.shown_space().map(|space| space.folders.clone()).unwrap_or_default()
    }

    /// The tab a row shows: the one of its split shown last, else the first.
    fn row_tab(&self, row: u64) -> u64 {
        let panes = self.panes(row);
        self.recent.iter().copied().find(|tab| panes.contains(tab)).unwrap_or(row)
    }

    /// One device pixel, for a line that is a box's height, which GPUI drops under a pixel.
    pub(super) fn hairline(&self) -> Pixels {
        px(1. / self.scale)
    }

    pub fn space(&self) -> u64 {
        self.space
    }

    /// Where the page goes in a window whose content is `viewport`, and the radius of its corners.
    /// While a pane is being added beside the tab shown, the pages shown make room for it, and
    /// they make room for a side panel.
    pub fn page_frame(&self, viewport: Size<Pixels>) -> (Bounds<Pixels>, f32) {
        let (frame, _) = self.card_frames(viewport);
        let radius = if self.card_left() == 0. { 0. } else { CARD_RADIUS };
        match self.adding_pane {
            Some(with) => {
                let count = self.panes(with).len();
                let last = pane_frames(frame, count + 1, self.stacked(with), &[])[count - 1];
                (Bounds::from_corners(frame.origin, last.bottom_right()), radius)
            }
            None => (frame, radius),
        }
    }

    /// Where the page goes as the window's content takes `viewport`, unless it's held where it is
    /// while the window goes full screen with the page, or back.
    pub fn window_page_frame(&mut self, viewport: Size<Pixels>) -> Option<(Bounds<Pixels>, f32)> {
        if self.page_held == Some(viewport) {
            return None;
        }
        self.page_held = None;
        Some(self.page_frame(viewport))
    }

    /// Lays the page out at `page_frame`, unless it's there already or held.
    fn lay_out_page(&mut self, cx: &mut Context<Self>) {
        let Some(frame) = self.window_page_frame(self.viewport) else {
            return;
        };
        if self.page_laid != Some(frame) {
            self.page_laid = Some(frame);
            self.browser.lay_out_page(frame.0, frame.1, cx);
            // The card moved, such as with the sidebar, and the plate in its corner with it.
            self.show_link(cx);
        }
        self.lay_out_side_panel(cx);
    }

    pub fn selected(&self) -> u64 {
        self.selected
    }

    /// Whether the window shows `tab`'s page: the tab shown, or a page beside it in its split view.
    pub fn shows(&self, tab: u64) -> bool {
        self.selected != 0 && self.panes(self.selected).contains(&tab)
    }

    /// Whether a link from `tab`'s page to another site opens in a link preview: `tab` is a
    /// favorite or pinned in the Space shown, as such a tab keeps its own site, and the Space is
    /// the user's, as a preview is theirs alone.
    pub fn previews_links_of(&self, tab: u64) -> bool {
        let kept = self.favorites.contains(&tab) || self.pinned().contains(&tab);
        kept && !self.agents_space_shown()
    }

    /// Where the page was laid out last, and the radius of its corners.
    pub fn page_laid(&self) -> Option<(Bounds<Pixels>, f32)> {
        self.page_laid
    }

    /// Where the card's left edge is: past the sidebar, or at the inset once ⌘S has hidden it,
    /// and between the two while it moves. 0 while a page is full screen, which takes the window.
    pub fn card_left(&self) -> f32 {
        if self.page_fullscreen {
            return 0.;
        }
        let to = if self.sidebar_hidden { CARD_INSET } else { self.width() };
        let Some((from, start)) = self.sidebar_motion else {
            return to;
        };
        match self.sidebar_progress(start) {
            Some(progress) => from + (to - from) * progress,
            None => to,
        }
    }

    /// The Spaces, by id, name, colour, profile, icon and the name of the agent each belongs to,
    /// empty for the user's own, in the switcher's order.
    pub fn set_spaces(&mut self, spaces: Vec<SpaceEntry>, cx: &mut Context<Self>) {
        if self.private {
            return;
        }
        // check.sh reads these lines.
        let ids: Vec<String> = spaces.iter().map(|(id, ..)| id.to_string()).collect();
        log!("browser: spaces {}", ids.join(" "));
        let width = self.library_width();
        let filled = !self.spaces.is_empty();
        let listed = &mut self.spaces_listed;
        let made: Vec<u64> =
            spaces.iter().map(|&(id, ..)| id).filter(|&id| listed.insert(id)).collect();
        let mut old = std::mem::take(&mut self.spaces);
        self.spaces = spaces
            .into_iter()
            .map(|(id, name, color, profile, icon, agent, theme)| {
                let Some(space) = old.iter_mut().find(|space| space.id == id) else {
                    log!("browser: added space {id} {name}");
                    if !agent.is_empty() {
                        log!("browser: agent space {id} {agent}");
                    }
                    if let Some(color) = color {
                        log!("browser: colored space {id} {color:06x}");
                    }
                    if !icon.is_empty() {
                        log!("browser: iconed space {id} {icon}");
                    }
                    if !theme.is_empty() {
                        log!("browser: themed space {id} {theme}");
                    }
                    log!("browser: profiled space {id} {profile}");
                    return Space {
                        id,
                        name,
                        color,
                        theme: Theme::parse(&theme),
                        icon,
                        profile,
                        agent,
                        pinned: Vec::new(),
                        today: Vec::new(),
                        splits: Vec::new(),
                        folders: Folders::default(),
                        shown: 0,
                    };
                };
                if space.name != name {
                    log!("browser: renamed space {id} {name}");
                }
                if space.color != color {
                    let color = color.map_or("none".to_string(), |color| format!("{color:06x}"));
                    log!("browser: colored space {id} {color}");
                }
                if space.icon != icon {
                    let shown = if icon.is_empty() { "none" } else { icon.as_ref() };
                    log!("browser: iconed space {id} {shown}");
                }
                if space.profile != profile {
                    log!("browser: profiled space {id} {profile}");
                }
                let theme = Theme::parse(&theme);
                if space.theme != theme {
                    let text = theme.map_or("none".to_string(), |theme| theme.to_text());
                    log!("browser: themed space {id} {text}");
                }
                Space {
                    id,
                    name,
                    color,
                    theme,
                    icon,
                    profile,
                    agent,
                    pinned: std::mem::take(&mut space.pinned),
                    today: std::mem::take(&mut space.today),
                    splits: std::mem::take(&mut space.splits),
                    folders: std::mem::take(&mut space.folders),
                    shown: space.shown,
                }
            })
            .collect();
        self.settle_theme_preview();
        if let Some(form) = &self.form
            && !self.spaces.iter().any(|space| space.id == form.space)
        {
            self.form = None;
        }
        if let Some(Naming::Space(space)) = self.renaming.as_ref().map(|renaming| renaming.naming)
            && !self.spaces.iter().any(|each| each.id == space)
        {
            self.renaming = None;
        }
        // The Spaces section springs to its width as cards come and go, as on a section's change.
        if self.library.is_some() && self.library_width() != width {
            self.library_resize = Some((width, Instant::now()));
        }
        if self.show_favorites() {
            self.log_favorites();
        }
        // The window's first Spaces, as at launch, were made before it.
        if filled {
            self.toast_spaces(made, cx);
        }
        self.follow_dock(cx);
        self.update_menus(cx);
        self.update_title(cx);
        cx.notify();
    }

    /// The profiles, by id and name, the one the browser started with first.
    pub fn set_profiles(&mut self, profiles: Vec<(u64, SharedString)>, cx: &mut Context<Self>) {
        // check.sh waits for these lines.
        for (id, name) in &profiles {
            match self.profiles.iter().find(|(each, _)| each == id) {
                None => log!("browser: added profile {id} {name}"),
                Some((_, was)) if was != name => log!("browser: renamed profile {id} {name}"),
                Some(_) => {}
            }
        }
        for (id, name) in &self.profiles {
            if !profiles.iter().any(|(each, _)| each == id) {
                log!("browser: removed profile {id} {name}");
            }
        }
        self.profiles = profiles;
        self.update_menus(cx);
        cx.notify();
    }

    /// The Space the window shows. Which of its tabs shows follows in set_selected.
    pub fn set_space(&mut self, space: u64, cx: &mut Context<Self>) {
        // check.sh reads these lines.
        match self.switching {
            Some((Switch::Space(asked), start)) if asked == space => {
                self.switching = None;
                log!(
                    "browser: space {space} in {:.1} ms",
                    start.elapsed().as_secs_f64() * 1000.
                );
            }
            _ => log!("browser: space {space}"),
        }
        self.end_hovers(cx);
        let before = std::mem::replace(&mut self.space, space);
        self.bar_follows |= before != space;
        if before != space {
            self.unchoose(cx);
        }
        // Arc says again how to leave full screen in each Space it shows there.
        if before != space && self.full_screen && !self.page_fullscreen {
            self.browser.full_screen_note(true, cx);
        }
        // A swipe that began in another Space is over, and its momentum with it.
        if self.swipe.as_ref().is_some_and(|swipe| swipe.from != space) {
            self.swipe = None;
            self.swiped = true;
        }
        // A Space shown some other way slides in from its side, pushing the one before out. One a
        // swipe settles on is moving already, one a pull made shows where it is, and one shown as
        // the Library moves shows in its place under it.
        let in_place = self.pulled_space_shown(space) || self.library_moving();
        let index = |id| self.spaces.iter().position(|other| other.id == id);
        let width = self.width();
        if !self.settle.as_ref().is_some_and(|settle| settle.space == space)
            && !in_place
            && let (Some(from), Some(to)) = (index(before), index(space))
            && from != to
        {
            self.settle = Some(Settle {
                space,
                beside: Some(before),
                from: if to > from { width } else { -width },
                velocity: 0.,
                start: Instant::now(),
                next_door: from.abs_diff(to) == 1,
            });
        }
        self.slides.clear();
        self.opened = None;
        self.closing.clear();
        self.clearing.clear();
        // A tab dragged to the sidebar's edge pages with it, to drop there; any other drag ends.
        match self.drag.as_mut().filter(|drag| drag.edge.is_some() && !drag.dropped) {
            Some(drag) => {
                drag.to = Place::List(false, usize::MAX);
                drag.line = None;
            }
            None => self.drag = None,
        }
        if self.show_favorites() {
            self.log_favorites();
        }
        self.log_tabs();
        if self.form().is_some() {
            self.focus_form(cx);
        }
        self.follow_dock(cx);
        self.update_menus(cx);
        self.update_title(cx);
        cx.notify();
    }

    pub fn set_tabs(
        &mut self,
        space: u64,
        pinned: Vec<u64>,
        today: Vec<u64>,
        splits: Vec<Split>,
        folders: Folders,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.spaces.iter_mut().find(|entry| entry.id == space) else {
            return;
        };
        // The rows of the Space shown move from where they were, unless it's being filled.
        let had_tabs = !entry.pinned.is_empty() || !entry.today.is_empty();
        let before = (space == self.space && had_tabs).then(|| entry.slots());
        let listed = &mut self.listed;
        let opened: Vec<u64> =
            pinned.iter().chain(&today).copied().filter(|&tab| listed.insert(tab)).collect();
        entry.pinned = pinned;
        entry.today = today;
        entry.splits = splits;
        for (folder, (name, _)) in &folders.folders {
            if entry.folders.folders.get(folder).is_some_and(|(was, _)| was != name) {
                // check.sh waits for this line.
                log!("browser: renamed folder {folder} {name}");
            }
        }
        for (folder, icon) in &folders.icons {
            if entry.folders.icons.get(folder) != Some(icon) {
                // check.sh waits for this line.
                log!("browser: iconed folder {folder} {icon}");
            }
        }
        if space == self.space {
            self.swaps.folders_changed(&entry.folders, &folders, Instant::now());
        }
        entry.folders = folders;
        let has_tabs = !entry.pinned.is_empty() || !entry.today.is_empty();
        // After a Clear, the rows left are in their places at once (Arc).
        let cleared = space == self.space && {
            let clearing = self.clearing.len();
            self.clearing.retain(|tab| entry.today.contains(tab));
            self.clearing.len() < clearing
        };
        if space == self.space {
            self.log_tabs();
        }
        if cleared {
            self.slides.clear();
        } else if let Some(mut before) = before {
            // A favorite dragged into the list comes from where the pointer has it.
            let here = |drag: &&Drag| drag.moving && drag.space == space;
            if let Some(drag) = self.drag.as_ref().filter(here) {
                before.entry(Slot::Tab(drag.tab)).or_insert(drag.top);
            }
            // A folder the user opened or closed moves the rows under it on its own spring.
            let curve = self.folding.take().map_or(Curve::Slide, |open| Curve::Fold { open });
            self.move_rows(before, curve);
        }
        if space == self.space {
            let tiles = self.drawn();
            if self.end_drag(false) {
                self.move_tiles(tiles, Curve::Slide);
            } else if let Some(position) = self.drag_position() {
                // A row carried over the new lists finds its place in them where the pointer is.
                self.drag_to(position, cx);
            }
        }
        // A tab opened in the form's Space makes it a Space like the others, under the name it has.
        if has_tabs && self.form.as_ref().is_some_and(|form| form.space == space) {
            self.form = None;
        }
        self.forget_closed(cx);
        // The folders Reveal Tab in Sidebar opened have moved its row.
        if space == self.space
            && let Some(tab) = self.revealing.take()
        {
            self.scroll_to_row(tab);
        }
        // A Space being filled, as at launch, has opened nothing.
        if space == self.space && had_tabs {
            self.toast_opened(&opened, cx);
        }
        if space == self.space {
            self.keep_chosen(cx);
        }
        // Pinning or unpinning the tab shown renames Tabs › Pin Tab.
        self.update_menus(cx);
        cx.notify();
    }

    /// Every favorite, left to right, each with its holder: the Space it belongs to under Each
    /// Space (the core's `bc_favorite_space`), else its profile.
    pub fn set_favorites(&mut self, favorites: Vec<(u64, u64)>, cx: &mut Context<Self>) {
        if self.private {
            return;
        }
        let tiles = self.drawn();
        self.all_favorites = favorites.iter().map(|&(tab, _)| tab).collect();
        self.favorite_holders = favorites.into_iter().collect();
        self.show_favorites();
        self.log_favorites();
        self.end_drag(true);
        // Nothing moves while the Space shown is being filled, as at launch.
        let (pinned, today) = self.rows();
        if pinned.is_empty() && today.is_empty() {
            self.tile_slides.clear();
            self.list_slide = None;
        } else {
            self.move_tiles(tiles, Curve::Slide);
        }
        self.forget_closed(cx);
        cx.notify();
    }

    /// Shows the favorites the shown Space has (its own, or its profile's), or all of them while no
    /// Space shows, and says whether that changed them. Another Space's swap in at once, as the
    /// Space they come with slides in.
    fn show_favorites(&mut self) -> bool {
        let shown = match self.shown_space() {
            Some(space) => self.favorites_of(space),
            None => self.all_favorites.clone(),
        };
        let changed = shown != self.favorites;
        self.favorites = shown;
        changed
    }

    fn log_favorites(&self) {
        // check.sh reads these lines.
        let ids: Vec<String> = self.favorites.iter().map(u64::to_string).collect();
        log!("browser: favorites {}", ids.join(" "));
    }

    /// What the shell knew of a closed tab goes with it. The core reports a tab in its new place
    /// before it leaves the old, so one moving between a Space and the favorites stays.
    fn forget_closed(&mut self, cx: &mut App) {
        let (spaces, favorites) = (&self.spaces, &self.all_favorites);
        let open = |tab: &u64| {
            favorites.contains(tab)
                || spaces
                    .iter()
                    .any(|space| space.pinned.contains(tab) || space.today.contains(tab))
        };
        // A closed tab's hover card picture goes with it.
        let closed: Vec<u64> = self.tabs.keys().copied().filter(|tab| !open(tab)).collect();
        for tab in closed {
            self.tabs.remove(&tab);
            crate::hover_card::forget(tab, cx);
        }
        // An agent is at work only in its own Space's tabs, so not in one the user took out.
        self.agents_at.retain(|_, tab| {
            spaces.iter().any(|space| {
                !space.agent.is_empty() && (space.pinned.contains(tab) || space.today.contains(tab))
            })
        });
        self.crash_cards.retain(|tab, _| open(tab));
        self.recent.retain(open);
        if let Some(tab) = self.renaming.as_ref().and_then(|renaming| renaming.naming.tab())
            && !open(&tab)
        {
            self.renaming = None;
        }
    }

    fn log_tabs(&mut self) {
        // check.sh reads these lines, the pinned one first.
        let ids = |tabs: &[u64]| tabs.iter().map(u64::to_string).collect::<Vec<_>>().join(" ");
        log!("browser: pinned {}", ids(self.pinned()));
        log!("browser: today {}", ids(self.today()));
        let splits = self.shown_space().map_or_else(Vec::new, |space| {
            // A stacked split's tabs are joined with a slash.
            let join = |split: &Split| if split.stacked { "/" } else { "+" };
            space.splits.iter().map(|split| ids(&split.tabs).replace(' ', join(split))).collect()
        });
        log!("browser: splits {}", splits.join(" "));
        self.outline_changed = true;
    }

    /// The sidebar's tabs' rows in its order, the favorites then the Space's pinned and Today rows,
    /// each by its first tab, leaving out folders and what closed ones hide, the tab shown's too,
    /// so each keeps its number.
    fn sidebar_rows(&self) -> Vec<u64> {
        let (pinned, today) = self.rows();
        let space = self.shown_space();
        let shut = |row: u64| space.is_some_and(|space| space.shut(row));
        let pinned = pinned.into_iter().filter(|&row| !self.is_folder(row) && !shut(row)).collect();
        [self.favorites.clone(), pinned, today].concat()
    }

    pub fn set_selected(&mut self, tab: u64, cx: &mut Context<Self>) {
        // A Space's switch can report a tab, even none, before the Space itself (#1332), so only a
        // tab's switch ends here.
        match self.switching {
            Some((Switch::Tab(asked), start)) => {
                self.switching = None;
                if asked == tab {
                    log!(
                        "browser: selected {tab} in {:.1} ms",
                        start.elapsed().as_secs_f64() * 1000.
                    );
                } else {
                    log!("browser: selected {tab}");
                }
            }
            _ => log!("browser: selected {tab}"),
        }
        let before = self.selected_row();
        let hidden = self.panes(self.selected);
        self.selected = tab;
        self.show_in_space(tab);
        self.scroll_to_row(tab);
        if self.walked.is_none() {
            self.shown(tab);
        }
        self.selected_for_picture(tab, cx);
        self.hid_for_cards(hidden, cx);
        self.cast_for_shown(cx);
        // check.sh reads this line.
        let panes = self.panes(tab).iter().map(u64::to_string).collect::<Vec<_>>().join(" ");
        log!("browser: panes {panes}");
        // With no page, AppKit offers the window's ⌘ shortcuts to the UI's view. On Linux the keys
        // go to the window, where nothing takes them, and on Windows to the hidden page that had
        // them, unless the UI has them, and the next page shown, as a window's first, has them back
        // (#895).
        if tab == 0 && cfg!(any(target_os = "linux", target_os = "windows")) {
            self.browser.hold_keyboard_for_page(cx);
        }
        // The command bar open over the Space before lists this one's tabs and suggestions now, so
        // a pick never goes to the other or its profile (Arc).
        let follows = std::mem::take(&mut self.bar_follows) && self.browser.command_bar_open(cx);
        let new_tab =
            follows && self.browser.command_bar_follows(self.bar_tabs(), self.open_tabs(), cx);
        self.set_new_tab(new_tab, cx);
        self.selection_moved(before);
        self.adding_pane = None;
        self.link_tab_changed(cx);
        self.note_hang(cx);
        if self.split_pending.take() == Some(tab) {
            self.add_pane(cx);
        }
        self.update_title(cx);
        self.update_menus(cx);
        cx.notify();
    }

    /// The window became the one in front: the browser's shortcuts and menus act on this shell,
    /// and the menu bar lists its Space's extensions.
    pub fn became_key(&mut self, cx: &mut Context<Self>) {
        cx.set_global(ShellHandle(cx.weak_entity()));
        // Its tab's dialog goes over its card again, as another window's may have shown since.
        self.page_dialog_shown = None;
        cx.notify();
        self.refresh_menus(cx);
        self.refresh_picture(cx);
        self.follow_dock(cx);
    }

    /// The user left the command bar without choosing anything in it.
    pub fn command_bar_dismissed(&mut self, cx: &mut Context<Self>) {
        self.set_new_tab(false, cx);
        if self.adding_pane.take().is_some() {
            self.lay_out_page(cx);
            cx.notify();
        }
    }

    /// A press on a target that opens the command bar, such as the New Tab row. A click outside the
    /// bar has closed it before the press reaches the shell, and the app reports so only after,
    /// so the press is where to tell that this click closes the bar.
    pub(super) fn pressed_bar_target(&mut self, cx: &mut Context<Self>) {
        self.press_closed_bar = self.browser.command_bar_open(cx);
    }

    /// The release of a click on a target that opens the command bar with `command`: it does,
    /// unless the press closed the bar.
    pub(super) fn clicked_bar_target(
        &mut self,
        command: Command,
        from: &str,
        cx: &mut Context<Self>,
    ) {
        if !std::mem::take(&mut self.press_closed_bar) {
            self.run(command, from, cx);
        }
    }

    /// Gives "New Tab" the selected plate while ⌘T's command bar is open, or gives it back.
    fn set_new_tab(&mut self, new_tab: bool, cx: &mut Context<Self>) {
        if self.new_tab != new_tab {
            self.new_tab = new_tab;
            // check.sh reads this line.
            log!("browser: new tab plate {}", new_tab as u8);
            cx.notify();
        }
    }

    /// Makes `tab` the Space's shown tab, whose row a closed folder keeps, and moves the rows if
    /// that changes them.
    fn show_in_space(&mut self, tab: u64) {
        let space = self.spaces.iter_mut().find(|space| space.id == self.space);
        let Some(space) = space.filter(|space| space.shown != tab) else {
            return;
        };
        // Only a closed folder changes the rows, and a tab switch has nothing more to do.
        if !space.folders.folders.values().any(|&(_, open)| !open) {
            space.shown = tab;
            return;
        }
        let before = space.slots();
        space.shown = tab;
        if space.slots() != before {
            self.move_rows(before, Curve::Slide);
        }
    }

    /// The row with the selected plate: the tab shown's, or its split's.
    fn selected_row(&self) -> Row {
        if self.new_tab { Row::NewTab } else { Row::Tab(self.row_of(self.selected)) }
    }

    fn shown(&mut self, tab: u64) {
        if tab != 0 {
            self.recent.retain(|&other| other != tab);
            self.recent.insert(0, tab);
        }
    }

    /// ⌃ went up, which ends a ⌃Tab walk on the tab it shows. A click ends it too, and the core
    /// may report the tab the click showed before this runs, so the walk's tab counts first.
    pub fn control_released(&mut self, _: &mut Context<Self>) {
        if let Some(walked) = self.walked.take().filter(|tab| self.tabs.contains_key(tab)) {
            self.shown(walked);
        }
        self.shown(self.selected);
    }

    // The core reports a tab's title, favicon and state whenever Chromium touches them, often with
    // what it already has, and for tabs nobody sees. Each is kept, but only a change that shows
    // draws a frame.
    pub fn set_title(&mut self, tab: u64, title: SharedString, cx: &mut Context<Self>) {
        let changed = self.tabs.entry(tab).or_default().keep_title(title);
        if changed && tab == self.selected {
            self.update_title(cx);
        }
        self.tab_changed(tab, changed, cx);
    }

    pub fn set_favicon(&mut self, tab: u64, favicon: Option<Arc<Image>>, cx: &mut Context<Self>) {
        let changed = self.tabs.entry(tab).or_default().keep_favicon(favicon);
        self.tab_changed(tab, changed, cx);
    }

    fn tab_changed(&self, tab: u64, changed: bool, cx: &mut Context<Self>) {
        let (selected, space) = (self.selected, self.shown_space());
        if changed && tab_drawn(tab, selected, &self.favorites, space, self.library.is_some()) {
            cx.notify();
        }
    }

    pub fn set_page(&mut self, tab: u64, page: Page, cx: &mut Context<Self>) {
        let developer_mode = self.developer_mode(tab);
        // check.sh reads these lines.
        let flag = |on| if on { 1 } else { 0 };
        log!(
            "browser: page {tab} back {} forward {} loading {} {}",
            flag(page.can_go_back),
            flag(page.can_go_forward),
            flag(page.loading),
            page.url
        );
        let entry = self.tabs.entry(tab).or_default();
        if page.unloaded != entry.page.unloaded {
            log!("browser: page {tab} unloaded {}", flag(page.unloaded));
            // The picture of a page that's gone is of what the tab no longer holds.
            if page.unloaded {
                crate::hover_card::forget(tab, cx);
            }
        }
        if page.away_from_pinned_url != entry.page.away_from_pinned_url {
            log!("browser: page {tab} away {}", flag(page.away_from_pinned_url));
        }
        if (page.audible, page.muted) != (entry.page.audible, entry.page.muted) {
            let (audible, muted) = (flag(page.audible), flag(page.muted));
            log!("browser: page {tab} audible {audible} muted {muted}");
        }
        let capture = |page: &Page| [page.camera, page.microphone, page.sharing, page.shared];
        if capture(&page) != capture(&entry.page) {
            let [camera, microphone, sharing, shared] = capture(&page).map(flag);
            // check.sh waits for these lines.
            log!(
                "browser: page {tab} camera {camera} microphone {microphone} sharing {sharing} \
                 shared {shared}"
            );
        }
        if page.share_instead != entry.page.share_instead {
            // check.sh waits for this line.
            log!("browser: page {tab} share instead {}", flag(page.share_instead));
        }
        let media = |page: &Page| {
            let pip = page.media_picture_in_picture;
            (page.media, page.media_playing, page.media_next, pip, page.media_title.clone())
        };
        let media_changed = media(&page) != media(&entry.page);
        if media_changed {
            // check.sh waits for this line.
            log!(
                "browser: page {tab} media {} playing {} next {} pip {} {}",
                flag(page.media),
                flag(page.media_playing),
                flag(page.media_next),
                flag(page.media_picture_in_picture),
                page.media_title
            );
        }
        // A player put away comes back once its page plays again.
        if page.media_playing && !entry.page.media_playing {
            self.media_dismissed.remove(&tab);
        }
        if page.icon != entry.page.icon {
            let icon = if page.icon.is_empty() { "none" } else { &page.icon };
            log!("browser: page {tab} icon {icon}");
        }
        if page.developer_mode != entry.page.developer_mode {
            log!("browser: page {tab} developer mode {}", flag(page.developer_mode));
        }
        self.swaps.page_changed(tab, self.selected, &entry.page, &page, Instant::now());
        let trouble = (page.trouble != entry.page.trouble).then_some(page.trouble);
        let changed = entry.keep_page(page);
        // An untitled tab is named by its URL.
        if changed && tab == self.selected {
            self.update_title(cx);
        }
        self.tab_changed(tab, changed, cx);
        // A player is for a tab out of sight, whose row may not be drawn.
        if media_changed && self.lists(tab) {
            cx.notify();
        }
        if let Some(trouble) = trouble {
            self.trouble_changed(tab, trouble, cx);
        }
        // View › Developer names it, so only its change rebuilds the menus, not each load.
        if tab == self.selected && self.developer_mode(tab) != developer_mode {
            self.update_menus(cx);
        }
    }

    /// Shows the page full screen, or back. The window goes full screen with it unless it already
    /// was, and comes back with it only if it went for the page.
    pub fn set_page_fullscreen(
        &mut self,
        fullscreen: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        // Left already, with the window (`bounds_changed`).
        if !fullscreen && !self.page_fullscreen {
            return;
        }
        // On Linux and Windows GPUI's window is the UI's, inside Chromium's, and never full screen
        // itself; the shell hears of Chromium's as it resizes.
        let full_screen = match cfg!(any(target_os = "linux", target_os = "windows")) {
            true => self.full_screen,
            false => window.is_fullscreen(),
        };
        let window_resizes = match fullscreen {
            true => !full_screen,
            false => self.window_for_page && full_screen,
        };
        self.window_for_page = fullscreen && window_resizes;
        self.page_held = window_resizes.then(|| window.viewport_size());
        self.full_screen_timing = window_resizes.then(|| FullScreenTiming {
            entering: fullscreen,
            before: window.viewport_size(),
            laid: false,
            drawn: false,
        });
        self.page_fullscreen = fullscreen;
        if fullscreen {
            self.conceal_now(cx);
            self.close_library_now();
            self.recent_downloads_gone(cx);
            self.hide_link(cx);
        }
        cx.notify();
    }

    /// Logs a `step` of the page's full screen, once, the first time it sees the window at a size
    /// other than the one it started from: `"laid"` as the page goes to its frame there, and
    /// `"drawn"` as the UI draws. check.sh times them against the core's and the host's lines.
    pub fn full_screen_step(&mut self, step: &str, viewport: Size<Pixels>) {
        let Some(timing) = &mut self.full_screen_timing else {
            return;
        };
        let done = match step {
            "laid" => &mut timing.laid,
            _ => &mut timing.drawn,
        };
        if viewport == timing.before || std::mem::replace(done, true) {
            return;
        }
        let way = if timing.entering { "enter" } else { "exit" };
        log!(
            "browser: full screen {way} {step} {}x{} at {:.1} ms",
            f32::from(viewport.width),
            f32::from(viewport.height),
            crate::uptime_ms()
        );
        if timing.laid && timing.drawn {
            self.full_screen_timing = None;
        }
    }

    /// The window has gone full screen or left it: going, it says how to leave, as Arc's does,
    /// unless a page took it there, and leaving takes that away.
    pub fn set_full_screen(&mut self, entered: bool, cx: &mut Context<Self>) {
        self.full_screen = entered;
        if !entered || !self.page_fullscreen {
            self.browser.full_screen_note(entered, cx);
        }
    }

    /// The window's bounds changed, as they do when it's maximized or restored: the third button
    /// shows which it would do next. They change as it goes full screen or comes back too, which
    /// on Linux nothing else says.
    pub fn bounds_changed(&mut self, cx: &mut Context<Self>) {
        let maximized = self.browser.window_maximized();
        if maximized != std::mem::replace(&mut self.maximized, maximized) {
            // The check waits for this line.
            log!("browser: window maximized {maximized}");
            cx.notify();
        }
        let full_screen = self.browser.window_full_screen();
        if full_screen != self.full_screen {
            // check.sh waits for this line, which the Mac's driver writes.
            let way = if full_screen { "entered" } else { "left" };
            log!("browser: window {way} full screen");
            // Chrome lets a page's full screen go as the window leaves the full screen it went to
            // for the page, and the window manager can take it out before the core says so. The
            // page leaves now, so this resize lays it out once, in the card.
            if !full_screen && self.window_for_page {
                self.page_fullscreen = false;
                self.window_for_page = false;
                self.page_held = None;
                self.full_screen_timing = Some(FullScreenTiming {
                    entering: false,
                    before: self.viewport,
                    laid: false,
                    drawn: false,
                });
            }
            match full_screen {
                // Chrome on Windows takes the window full screen for a page before the core says the
                // page asked, so whether a page took it there is known a turn later, as is the note.
                true => {
                    self.full_screen = true;
                    cx.spawn(async move |shell, cx| {
                        shell
                            .update(cx, |shell, cx| {
                                if shell.full_screen {
                                    shell.set_full_screen(true, cx);
                                }
                            })
                            .ok();
                    })
                    .detach();
                }
                false => self.set_full_screen(false, cx),
            }
            cx.notify();
        }
    }

    /// Deletes the Space, asking first if it has tabs, which close with it.
    fn delete_space(&mut self, space: u64, cx: &mut Context<Self>) {
        let Some(found) = self.spaces.iter().find(|each| each.id == space) else {
            return;
        };
        let tabs = found.pinned.len() + found.today.len();
        if tabs == 0 {
            self.delete_space_now(space, cx);
            return;
        }
        let shell = cx.weak_entity();
        self.browser.confirm(
            format!("Delete “{}”?", found.name),
            if tabs == 1 {
                "Its tab closes too.".into()
            } else {
                format!("Its {tabs} tabs close too.")
            },
            "Delete",
            Box::new(move |cx| {
                shell.update(cx, |shell, cx| shell.delete_space_now(space, cx)).ok();
            }),
            cx,
        );
    }

    /// Deletes the Space, for ⌘Z to bring back, and says so in the toast (Arc).
    fn delete_space_now(&mut self, space: u64, cx: &mut Context<Self>) {
        if self.browser.delete_space(space, cx) {
            self.keep(What::DELETE_SPACE, Some(Edit::UndoDeleteSpace(space)), cx);
            let undo = keystroke_label("cmd-z");
            self.browser.toast("Space deleted! Use", Some((&undo, "to undo.")), DELETED_TIME, cx);
        }
    }

    fn shown_space(&self) -> Option<&Space> {
        self.spaces.iter().find(|space| space.id == self.space)
    }

    /// The Dock icon's dot takes the key window's Space's color. A private window has no Space of
    /// the user's, so its dot is the icon's own.
    fn follow_dock(&self, cx: &mut Context<Self>) {
        if cx.try_global::<ShellHandle>().is_some_and(|key| key.0 == cx.weak_entity()) {
            crate::dock_dot::follow(self.shown_space().and_then(|space| space.color), cx);
        }
    }

    /// Whether the Space shown is an agent's, which no tab moves into, as the browser keeps the
    /// user's tabs out of its reach.
    fn agents_space_shown(&self) -> bool {
        self.shown_space().is_some_and(|space| !space.agent.is_empty())
    }

    /// The core let the agent with the id `agent` act in `tab`, for the activity log: it's at work
    /// there now, and no longer in the tab it acted in before.
    pub fn agent_acted(&mut self, agent: SharedString, tab: u64, cx: &mut Context<Self>) {
        let before = self.agents_at.insert(agent.clone(), tab);
        if before != Some(tab) {
            // check.sh reads this line.
            log!("browser: agent {agent} at work in {tab}");
            for tab in before.into_iter().chain([tab]) {
                self.tab_changed(tab, true, cx);
            }
        }
    }

    /// The core ended the session of the agent with the id `agent` with `tab`: its client
    /// disconnected, it released the tab or the user took it back. It's no longer at work there,
    /// though it still is in another tab it has since acted in.
    pub fn agent_session_ended(&mut self, agent: &str, tab: u64, cx: &mut Context<Self>) {
        if self.agents_at.get(agent) == Some(&tab) {
            self.agents_at.remove(agent);
            // check.sh reads this line.
            log!("browser: agent {agent} done in {tab}");
            self.tab_changed(tab, true, cx);
        }
    }

    /// Whether an agent is at work in `tab`. Only an agent's own Space's tabs are its to act in,
    /// so one the user moved out of it, into a Space of theirs, no longer shows one.
    pub(super) fn agent_at_work(&self, tab: u64) -> bool {
        self.agents_space_shown() && self.agents_at.values().any(|&at| at == tab)
    }

    fn show_space_at(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if let Some(space) = index.and_then(|index| self.spaces.get(index)) {
            self.show_space(space.id, cx);
        }
    }

    fn show_space(&mut self, space: u64, cx: &mut Context<Self>) {
        if space == self.space {
            return;
        }
        self.switching = Some((Switch::Space(space), Instant::now()));
        self.browser.show_space(space, cx);
    }

    fn select(&mut self, tab: u64, cx: &mut Context<Self>) {
        // The tab selected whose page another window shows is taken from it by selecting it again.
        if tab == self.selected && !self.pictured(tab) {
            return;
        }
        self.switching = Some((Switch::Tab(tab), Instant::now()));
        self.browser.select(tab, cx);
    }

    /// A double-click on the window's background: in the sidebar below the rows it opens a new tab
    /// (Dia), and elsewhere it does what System Settings says a title bar's does.
    fn double_click_background(
        &mut self,
        at: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let below_rows = at.x < px(self.card_left()) && at.y >= px(self.under_tiles());
        let sidebar = !self.page_fullscreen && self.library.is_none() && self.form().is_none();
        if sidebar && below_rows {
            self.run(Command::NewTab, "sidebar", cx);
        } else if self.empty_card_hit(at) {
            // The empty card is a button, not a title bar: its first click opened the command bar.
        } else if cfg!(any(target_os = "linux", target_os = "windows")) {
            // The browser window is Chromium's, out of GPUI's reach, and on Windows and every
            // Linux desktop a double-click on a title bar maximizes or restores it, as its third
            // button does.
            self.browser.press_window_button(WindowButton::Zoom, cx);
        } else {
            window.titlebar_double_click();
        }
    }

    pub fn page(&self, tab: u64) -> Page {
        self.tabs.get(&tab).map(|tab| tab.page.clone()).unwrap_or_default()
    }

    fn url(&self, tab: u64) -> SharedString {
        self.page(tab).url
    }

    /// A tab's title: the page's, else its host, else "Untitled" (Arc).
    pub fn title(&self, tab: u64) -> SharedString {
        match self.tabs.get(&tab) {
            Some(tab) if !tab.title.is_empty() => tab.title.clone(),
            Some(tab) if !tab.page.url.is_empty() => match file_name(&tab.page.url) {
                Some(name) => name.into(),
                None => host(&tab.page.url).into(),
            },
            _ => "Untitled".into(),
        }
    }

    /// The pointer entered `row` or left it. A row fills at once and empties at once, but the
    /// selected row's fill eases back from its hover.
    fn hover(&mut self, row: Row, hovered: bool, cx: &mut Context<Self>) {
        let left = if hovered {
            self.unhovered.retain(|&(old, _)| old != row);
            self.hovered.replace(row).filter(|&old| old != row)
        } else if self.hovered == Some(row) {
            self.hovered.take()
        } else {
            None
        };
        if let Some(left) = left {
            self.unhovered.retain(|&(old, _)| old != left);
            self.unhover(left);
        }
        cx.notify();
    }

    /// The row under the pointer as the sidebar draws it (`page_keyed`).
    fn pointer_row(&self) -> Option<Row> {
        self.hovered.filter(|_| !self.page_keyed)
    }

    /// The same for the favicon under the pointer.
    fn pointer_favicon(&self) -> Option<u64> {
        self.hovered_favicon.filter(|_| !self.page_keyed)
    }

    /// A shortcut the page handed back hides the row under the pointer, easing the selected one
    /// back as the pointer leaving it does.
    fn hide_hover(&mut self, cx: &mut Context<Self>) {
        if let Some(row) = self.pointer_row() {
            self.unhover(row);
            cx.notify();
        }
        self.page_keyed = true;
    }

    /// The pointer moved or pressed, which shows the row under it again.
    fn pointer_moved(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.page_keyed) {
            cx.notify();
        }
    }

    /// Eases the selected row back from its hover fill once the pointer or its menu has left it.
    fn unhover(&mut self, row: Row) {
        if row == self.selected_row() {
            self.unhovered.push((row, Instant::now()));
        }
    }

    /// The pointer's moves anywhere in the window, for whether it's over the sidebar, whose rows
    /// and buttons keep it from the sidebar's own hover. A press says where the pointer is as a
    /// move does: one that arrives with no move before it, such as the first after the window
    /// became active, would otherwise leave the sidebar unaware of the pointer when what it
    /// pressed, a permission question, closes and uncovers it.
    fn hover_listener(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shell = cx.weak_entity();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let moved = shell.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        let over = bounds.contains(&event.position);
                        moved
                            .update(cx, |shell, cx| {
                                shell.pointer_moved(cx);
                                shell.pointer_to(event.position, cx);
                                shell.sidebar_hover(over, cx)
                            })
                            .ok();
                    }
                });
                let pressed = shell.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        let over = bounds.contains(&event.position);
                        pressed
                            .update(cx, |shell, cx| {
                                shell.pointer_moved(cx);
                                shell.pointer_to(event.position, cx);
                                shell.sidebar_hover(over, cx)
                            })
                            .ok();
                    }
                });
                window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        shell.update(cx, |shell, cx| shell.sidebar_hover(false, cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }

    /// Deletes the Space of the form a swipe cancelled once its page is out, so the window's
    /// colour crosses to the other Space's from its own. After the draw, which the core's report
    /// of the Spaces left would otherwise come into.
    fn step_form_going(&mut self, cx: &mut Context<Self>) {
        let beside = self.settle.as_ref().and_then(|settle| settle.beside);
        if let Some(form) = self.form_going.take_if(|form| Some(form.space) != beside) {
            let browser = self.browser.clone();
            cx.defer(move |cx| {
                browser.delete_space(form.space, cx);
            });
        }
    }

    /// Once a drag has ended and its rows and tiles have stopped, asks for the tooltip or the hover
    /// card of what the pointer is on. A tile that came under the pointer in the drag, as the
    /// dropped row's does, was hovered then, and GPUI tells a handler only when hover changes, so
    /// it would never ask.
    fn step_retip(&mut self, cx: &mut Context<Self>) {
        if !self.retip || !self.slides.is_empty() || !self.tile_slides.is_empty() {
            return;
        }
        self.retip = false;
        if let Some(row) = self.hovered {
            self.hover_tooltip(row, true, cx);
            self.hover_card(row, true, cx);
        }
    }

    /// Drops the list's and the pages' motion that's done, and returns whether any is left.
    fn list_moving(&mut self) -> bool {
        let sliding = !self.slides.is_empty();
        self.slides
            .retain(|_, (_, start, curve)| curve.at(start.elapsed().as_secs_f32()).is_some());
        if sliding && self.slides.is_empty() {
            // check.sh waits for this line before it presses a row that moved, or scrolls the
            // list: a row sliding in from above leaves the list short until it settles.
            log!("browser: rows settled");
        }
        let tiling = !self.tile_slides.is_empty();
        self.tile_slides
            .retain(|_, (_, start, curve)| curve.at(start.elapsed().as_secs_f32()).is_some());
        if tiling && self.tile_slides.is_empty() {
            // check.sh waits for this line before it presses a tile that moved.
            log!("browser: tiles settled");
        }
        self.list_slide
            .take_if(|(_, start, curve)| curve.at(start.elapsed().as_secs_f32()).is_none());
        self.closing.retain(|row| row_spring(row.start.elapsed().as_secs_f32()).is_some());
        self.unhovered.retain(|(_, start)| start.elapsed().as_secs_f32() < SELECTED_UNHOVER);
        if self.slides.is_empty() {
            self.opened.take_if(|opened| opened.done().is_none());
        }
        if let Some(settle) = self.settle.take_if(|settle| settle.offset().is_none()) {
            // check.sh reads these lines.
            log!("browser: settled space {}", settle.space);
            self.log_paging_frames();
        }
        let moving = !(self.slides.is_empty() && self.closing.is_empty() && self.unhovered.is_empty())
            || !self.tile_slides.is_empty()
            || self.list_slide.is_some()
            || self.opened.is_some()
            || self.settle.is_some();
        if !moving && std::mem::take(&mut self.putting_back) {
            // check.sh waits for this line before it presses a row or tile put back, by Escape or
            // by a drop where it was: a press hits the frame drawn, which has it sliding home until
            // now.
            log!("browser: drag put back");
        }
        moving
    }

    /// Draws a dragged tab's turn between its row and its tile on, and once it has turned, lands a
    /// drop the pointer let go of mid-turn.
    fn step_drag_shape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_mut().filter(|drag| drag.shape.start.is_some()) else {
            return;
        };
        if drag.shape.turning(Instant::now()) {
            window.request_animation_frame();
            return;
        }
        drag.shape.start = None;
        // check.sh waits for these lines before it captures a dragged tab.
        log!("browser: drag shape {}", if drag.shape.tile { "tile" } else { "row" });
        if std::mem::take(&mut drag.held) {
            // Out of render, as the browser may report the lists it made at once.
            cx.defer_in(window, |shell, _, cx| shell.drop_row(cx));
        }
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.a11y = window.is_a11y_active();
        self.watch_appearance(window, cx);
        self.full_screen_step("drawn", window.viewport_size());
        self.stop_videos(window, cx);
        // check.sh reads this line. It comes as the rows are drawn, so a click it sends next goes
        // to what's drawn.
        if std::mem::take(&mut self.outline_changed) {
            let outline = self.shown_space().map_or_else(String::new, |space| {
                match space.folders.pinned_shut {
                    true => format!("-[{}]", space.outline(0)),
                    false => space.outline(0),
                }
            });
            log!("browser: outline {outline}");
        }
        // As the outline's, so a click on Clear, which shows only under the pointer, finds it.
        if std::mem::take(&mut self.hover_changed) {
            log!("browser: sidebar hovered {}", self.sidebar_hovered as u8);
        }
        // As the outline's: the ✕ or −, and the way back, show only under the pointer, so a click
        // that follows a move waits for this line.
        let close = match self.pointer_row() {
            Some(Row::Tab(tab)) if self.closable(tab) => match self.close_glyph(tab) {
                UNLOAD => "−",
                _ => "1",
            },
            _ => "0",
        };
        let hover = (self.pointer_row(), self.pointer_favicon(), close);
        if hover != self.drawn_hover {
            self.drawn_hover = hover;
            let row = match hover.0 {
                Some(Row::Tab(tab)) => format!("tab {tab}"),
                Some(Row::NewTab) => "new".into(),
                None => "none".into(),
            };
            let favicon = hover.1.map_or("none".into(), |tab| tab.to_string());
            log!("browser: hover {row} close {close} favicon {favicon}");
        }
        self.step_clear_arrow();
        self.swaps.step(window);
        // As the outline's, so a capture of the URL field's button shows the glyph it logged.
        let copied = match (self.url_copied.is_some(), self.swaps.changing(Swapped::CopyUrl)) {
            (true, true) => "draws check",
            (true, false) => "shows check",
            (false, true) => "brings back link",
            (false, false) => "shows link",
        };
        if copied != self.drawn_copied {
            self.drawn_copied = copied;
            log!("browser: copy url {copied}");
        }
        self.step_keys(window, cx);
        self.step_keyed_drawn();
        if std::mem::take(&mut self.focus_form)
            && let Some(form) = self.form()
        {
            window.focus(&form.name.focus_handle(cx), cx);
        }
        // A new folder's field is drawn once the browser reports the folder, and blurs if focused
        // before.
        let drawn = self.renaming.as_ref().is_some_and(|renaming| match renaming.naming {
            Naming::Folder(folder) => self.is_folder(folder),
            _ => true,
        });
        // A name being typed in a card is kept as the Library closes over it, however it closes.
        if !self.library_open && self.renaming.as_ref().is_some_and(|renaming| renaming.in_library) {
            self.keep_name(window, cx);
        }
        if drawn
            && std::mem::take(&mut self.focus_renaming)
            && let Some(renaming) = &mut self.renaming
        {
            let field = renaming.name.focus_handle(cx);
            window.focus(&field, cx);
            // check.sh waits for these lines, which say the field has the keyboard.
            match renaming.naming {
                Naming::Folder(folder) => {
                    log!("browser: rename folder {folder}");
                    log!("browser: folder name field {}", renaming.name.read(cx).text());
                }
                Naming::Profile(profile) => log!("browser: rename profile {profile}"),
                _ => {}
            }
            renaming._blur =
                Some(text_input::on_leave(&field, window, cx, |shell, window, cx| {
                    shell.keep_name(window, cx)
                }));
        }
        if self.picker_pending.is_some() {
            self.open_picker_when_settled(window, cx);
        }
        if let Some((_, start)) = self.width_motion {
            if settle_progress(start).is_some() {
                window.request_animation_frame();
            } else {
                self.width_motion = None;
                // check.sh waits for these lines.
                log!("browser: sidebar width {}", self.sidebar_width);
            }
        }
        if let Some((_, start)) = self.sidebar_motion {
            if self.sidebar_progress(start).is_some() {
                window.request_animation_frame();
            } else {
                self.sidebar_motion = None;
                // check.sh waits for these lines.
                let state = if self.sidebar_hidden { "hidden" } else { "shown" };
                log!("browser: sidebar {state}");
            }
        }
        self.step_library(window, cx);
        self.step_new_space(window);
        self.step_flight(window);
        self.step_picture(window);
        self.step_recent_downloads(window, cx);
        if let Some((_, left)) = &self.leaving_form {
            if left.elapsed().as_secs_f32() < FORM_LEAVE_TITLE + FORM_LEAVE {
                window.request_animation_frame();
            } else {
                self.leaving_form = None;
                // check.sh waits for this line.
                log!("browser: space form gone");
            }
        }
        if self.list_moving() {
            window.request_animation_frame();
        }
        self.step_retip(cx);
        self.step_drag_shape(window, cx);
        self.step_held_row(window);
        self.step_held_nav(window);
        self.step_form_going(cx);
        self.step_scroll_edge(window);
        self.step_tools(window);
        self.step_empty(window);
        self.step_crash_cards(window);
        let drop_targets = self.drag.as_ref().is_some_and(|drag| drag.over_page.is_some())
            || self.link_over.is_some()
            || self.agent_pointer_shown();
        if drop_targets || self.drop_targets_shown {
            self.drop_targets_shown = drop_targets;
            self.browser.show_drop_targets(drop_targets, cx);
        }
        // The page is laid out at every step, as in Arc.
        let viewport = window.viewport_size();
        if viewport != self.viewport {
            // check.sh waits for this line before it scrolls the list in a resized window.
            let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
            log!("browser: drawn at {width}x{height}");
        }
        self.viewport = viewport;
        self.scale = window.scale_factor();
        if self.tiles_scroll() != self.tiles_scrolled_last {
            self.tiles_scrolled_last = !self.tiles_scrolled_last;
            // check.sh waits for these lines.
            match self.tiles_scrolled_last {
                true => log!("browser: tiles scroll with the list"),
                false => log!("browser: tiles stay put"),
            }
        }
        self.lay_out_moving_page(cx);
        for line in self.logged_once_laid.drain(..) {
            log!("{line}");
        }
        self.show_page_dialog(cx);
        let card_left = self.card_left();
        self.update_palette(cx);
        let focus_color = packed(self.palette.accent.into());
        if self.focus_color != Some(focus_color) {
            self.focus_color = Some(focus_color);
            self.browser.set_focus_color(focus_color, cx);
        }
        // A page shows the card until its first frame, never white (Ardia). Not while the sidebar
        // pages between Spaces, whose colours cross-fade at every step.
        let page_color = packed(self.window_fill().blend(self.palette.page.into()));
        if self.pages().len() == 1 && self.page_color != Some(page_color) {
            self.page_color = Some(page_color);
            self.browser.set_page_color(page_color, cx);
        }
        // The find bar and the zoom HUD sit at the focused page's corner.
        let (frame, _) = self.page_frame(self.viewport);
        let panes = self.panes(self.selected);
        let focused = panes.iter().position(|&pane| pane == self.selected).unwrap_or(0);
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        let pane = pane_frames(frame, panes.len(), stacked, &shares)[focused];
        let pane_corner =
            (f32::from(frame.right() - pane.right()), f32::from(pane.top() - frame.top()));
        if self.pane_corner != pane_corner {
            self.pane_corner = pane_corner;
            self.browser.set_pane_corner(pane_corner.0, pane_corner.1, cx);
        }
        let window_buttons = self.window_buttons_shown();
        if window_buttons != self.window_buttons {
            self.window_buttons = window_buttons;
            self.browser.show_window_buttons(window_buttons, cx);
        }
        // AppKit's say how they're drawn once they show, out of full screen, as at rest.
        if self.window_button_geometry.is_none() && window_buttons {
            self.window_button_geometry = self.browser.window_button_geometry(cx);
        }
        // The Library slides in from the left with the card's edge, over the sidebar receding.
        let library = self.library.is_some() && !self.page_fullscreen;
        // Full screen has no window buttons, so the panel draws none.
        let buttons = self.library_buttons() && !window.is_fullscreen();
        // The sidebar and the Library fade against each other, so both take this frame's one read.
        let opacity = self.sidebar_opacity();
        let library = library.then(|| self.library_panel(buttons, opacity, cx));
        // The Spaces section's cards slide in over the receding sidebar, as Arc's do; the other
        // sections slide in under it, its buttons receding over them.
        let (under, over) = match self.section {
            Section::Spaces => (None, library),
            _ => (library, None),
        };
        // The sidebar leaves with the card's edge, all of it by the time the edge stops.
        let width = self.width();
        let sidebar_left = (card_left - width) * width / (width - CARD_INSET);
        let sidebar_shown = !self.page_fullscreen
            && opacity > 0.
            && (!self.sidebar_hidden || self.sidebar_motion.is_some());
        let sidebar = sidebar_shown.then(|| {
            let scale = self.sidebar_scale();
            let origin = point(px(width + RECEDE_X), self.viewport.height / 2.);
            // Whole: the Library fades in over it rather than cutting it (`library_opacity`).
            let sidebar = self.sidebar(cx).left(px(sidebar_left)).child(self.hover_listener(cx));
            let sidebar = self.with_keys(sidebar, cx);
            // Its buttons recede with it, over the Library, as the window's do in Arc. Full screen
            // has none.
            let buttons = self.sidebar_buttons() && !window.is_fullscreen();
            let buttons = buttons.then(|| self.window_buttons(cx));
            let receding = div().absolute().size_full().opacity(opacity);
            scaled(receding.child(sidebar).children(buttons), scale, origin)
        });
        let docked = sidebar_shown
            && !self.sidebar_hidden
            && self.sidebar_motion.is_none()
            && self.library.is_none();
        let edge = docked.then(|| self.sidebar_edge(cx));
        let own_buttons = self.draws_own_window_buttons().then(|| self.window_buttons(cx));
        self.breath.arm(cx);
        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            // The Edit menu's Undo and Redo, which a field of ours takes for itself first.
            .on_action(cx.listener(|shell, _: &crate::text_input::Undo, _, cx| {
                shell.undo(cx);
            }))
            .on_action(cx.listener(|shell, _: &crate::text_input::Redo, _, cx| {
                shell.redo(cx);
            }))
            .size_full()
            .relative()
            .font_family(crate::face::UI)
            .bg(self.palette.tint)
            .capture_any_mouse_down(cx.listener(|shell, _, _, cx| shell.leave_keys("press", cx)))
            // The rows, buttons and fields keep their presses. Everywhere else is the window's
            // background, which moves the window as a title bar does.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, _| shell.background_pressed = true),
            )
            .on_mouse_move(cx.listener(|shell, event: &MouseMoveEvent, window, cx| {
                if shell.background_pressed && event.dragging() {
                    shell.background_pressed = false;
                    // check.sh reads this line.
                    log!("browser: window move");
                    window.start_window_move();
                    // GPUI's window is inside the browser window on Linux and Windows, so the
                    // browser moves that. On Linux GPUI's own still lets go of the X server's
                    // pointer grab its press took, which would keep the window manager from taking
                    // the pointer.
                    #[cfg(any(target_os = "linux", target_os = "windows"))]
                    shell.browser.start_window_move(cx);
                }
                shell.pointer_at(event.position.x.into(), cx);
            }))
            .on_scroll(cx.listener(|shell, _, _, cx| shell.end_hovers(cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|shell, event: &MouseUpEvent, window, cx| {
                    if std::mem::take(&mut shell.background_pressed) && event.click_count == 2 {
                        shell.double_click_background(event.position, window, cx);
                    }
                }),
            )
            .child(self.window_background())
            // The page views go over the cards, as native views.
            .children(self.cards())
            .children(self.picture_element(cx))
            .children(self.empty_card(cx))
            .children(self.crash_cards(cx))
            .children(self.pane_gaps(cx))
            .children(self.pane_handles(cx))
            .children(self.side_panel(cx))
            .children(under)
            .children(sidebar)
            .children(over)
            .children(edge)
            .children(own_buttons)
            .child(self.scroll_listener(true, cx))
            .child(self.navigate_listener(cx))
            .children(self.onboarding_layer(window, cx))
    }
}

impl Shell {
    /// The cards under the pages shown: a page's own in a split view, so the gaps between them
    /// show the window, with the focused page's glowing in its ring's colour. While a pane is
    /// being added, its card is there, empty.
    fn cards(&self) -> Vec<gpui::Div> {
        let (frame, _) = self.page_frame(self.viewport);
        let panes = self.panes(self.selected);
        let glow = BoxShadow {
            color: Hsla::from(self.palette.accent).opacity(FOCUS_GLOW_ALPHA),
            offset: point(px(0.), px(0.)),
            blur_radius: px(FOCUS_GLOW),
            spread_radius: px(0.),
            inset: false,
        };
        let split = panes.len() > 1;
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        let mut cards: Vec<_> = pane_frames(frame, panes.len(), stacked, &shares)
            .into_iter()
            .zip(&panes)
            .map(|(bounds, &tab)| {
                let focused = split && tab == self.selected;
                card(bounds, self.palette.page).when(focused, |card| card.shadow(vec![glow.clone()]))
            })
            .collect();
        if let Some(with) = self.adding_pane {
            let count = self.panes(with).len() + 1;
            let (full, _) = self.card_frames(self.viewport);
            cards.push(card(pane_frames(full, count, self.stacked(with), &[])[count - 1], self.palette.page));
        }
        cards
    }

    /// The sidebar: the header's buttons, the URL field and the Space's tabs, or the Create a
    /// Space form, over the switcher. Docked, or in the reveal's panel.
    fn sidebar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let started = Instant::now();
        let sidebar = div()
            .absolute()
            .top_0()
            .bottom_0()
            .w(px(self.width()))
            .children(self.nav_buttons(cx));
        let leaving = self.leaving_form.as_ref().filter(|(form, _)| form.space == self.space);
        let sidebar = match (self.paged_form(), self.form(), leaving) {
            // Paged, the form goes as a page does, and the URL field and the favorites go with the
            // Space beside's page, as its rows do.
            (Some((form, x)), ..) => {
                let beside = self.beside_form(form.space, cx);
                let page = div().absolute().top_0().bottom_0().left(px(x)).w(px(self.width()));
                let page = page.child(self.space_form(form, None, cx));
                let form = div().absolute().size_full().overflow_hidden().child(page);
                sidebar.child(self.space_pages(cx)).children(beside).child(form)
            }
            (None, Some(form), _) => sidebar.child(self.space_form(form, None, cx)),
            (None, None, Some((form, left))) => {
                let left = left.elapsed().as_secs_f32();
                sidebar.child(self.space_form(form, Some(left), cx))
            }
            // The URL field and the favorites are the Space's page's, and page with it.
            (None, None, None) => {
                self.drawn_top.replace((self.top_offset(), self.drawn_tiles()));
                let rest = self.rest_press_listener(Self::press_top_at_rest, cx);
                let rest = rest.absolute().top(px(URL_FIELD_TOP)).left_0().w(px(self.width()));
                sidebar
                    .child(self.with_page(self.url_field(cx)))
                    .children(self.scroll_edge_line().map(|line| self.with_page(line)))
                    .child(self.space_pages(cx))
                    // Over the pages, which they scroll with when they're tall.
                    .children(self.tiles(cx).map(|tiles| self.with_page(tiles)))
                    .children(self.beside_pages())
                    .child(rest.h(px(self.under_tiles() - URL_FIELD_TOP)))
                    .children(self.dragged_tile(cx))
                    .children(self.turning_drag())
                    .children(self.question_card(cx))
                    .children(self.cast_card(cx))
                    .children(self.agent_question_card(cx))
                    .children(self.media_player_cards(cx))
                    .children(self.agent_bar_card(cx))
                    .children(self.drm_note_card(cx))
            }
        };
        sidebar
            .children(self.new_space_circle())
            .children(self.recent_download_rows(cx))
            .children((!self.private).then(|| self.library_button(cx)))
            .children((!self.private).then(|| self.plus_button(cx)))
            .children(self.switcher(cx))
            .children(self.flying_icon())
            .child(self.press_listener())
            .children(self.paging_frame(started))
    }

    /// The Spaces' pages under the URL field, side by side while they move. Each keeps its own
    /// scroll.
    fn space_pages(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let width = self.width();
        let form = self.paged_form().map(|(form, _)| form.space);
        let mut pages = Vec::new();
        for (space, x) in self.pages() {
            let page = match self.spaces.iter().find(|other| other.id == space) {
                _ if Some(space) == form => continue,
                _ if space == self.space => self.list(cx).into_any_element(),
                // Beside the form, the page's URL field and favorites are the shown Space's, over
                // it (beside_form). Elsewhere it's drawn whole, with its own (beside_pages), over
                // its place here, which keeps its menu.
                Some(other) if form.is_some() => {
                    self.still_page(other, self.tiles_room()).into_any_element()
                }
                Some(_) => div().into_any_element(),
                None => continue,
            };
            // A right-click under the rows, rows, buttons and fields taking their own, is for the
            // Space whose page is there. A Space left for one sliding in has gone already: its
            // page's part is the incoming Space's (`press_at_rest`).
            let menu = space == self.space || self.swipe.is_some();
            pages.push(
                div()
                    .id(("page", space))
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(x))
                    .w(px(width))
                    .when(menu, |page| {
                        page.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                                if shell.drag.is_none() && shell.renaming.is_none() {
                                    shell.empty_space_menu(space, event.position, cx)
                                }
                            }),
                        )
                    })
                    .child(page)
                    .when(space == self.space, |page| page.children(self.shown_marker(cx))),
            );
        }
        div()
            .absolute()
            .top(px(self.pages_top()))
            .left_0()
            .w(px(self.width()))
            .bottom(px(self.list_bottom()))
            .overflow_hidden()
            .children(pages)
            .child(self.rest_press_listener(Self::press_at_rest, cx).absolute().size_full())
    }

    /// Hears a press ahead of what's drawn under it, for one that would go to another element
    /// than the one at rest there: `press`, which says whether it took it, on the list's rows
    /// (`press_at_rest`) or the URL field and favorites (`press_top_at_rest`). Its hitbox leaves
    /// a press on what's drawn over them, such as a question, to that.
    fn rest_press_listener(
        &self,
        press: fn(&mut Self, &MouseDownEvent, &mut Context<Self>) -> bool,
        cx: &mut Context<Self>,
    ) -> gpui::Canvas<gpui::Hitbox> {
        let shell = cx.weak_entity();
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let pressed = shell.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble || !hitbox.is_hovered(window) {
                        return;
                    }
                    let took = pressed.update(cx, |shell, cx| press(shell, event, cx));
                    if took.unwrap_or(false) {
                        cx.stop_propagation();
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        _ = shell.update(cx, |shell, cx| shell.release_at_rest(event, cx));
                    }
                });
            },
        )
    }

    /// Another Space's page as it shows once shown, while it slides in or out: its name, and its
    /// rows with the tab it would show selected, `room` under its top. It takes no input.
    fn still_page(&self, space: &Space, room: f32) -> impl IntoElement + use<> {
        let selected = self.browser.space_tab(space.id);
        let row = |tab: u64| {
            let row = row_box().pl(px(PLATE_INSET + space.depth(tab) as f32 * FOLDER_INDENT));
            if let Some((name, open)) = space.folders.folders.get(&tab) {
                let name = title(&self.palette, name.clone(), false).pr(px(TITLE_END_INSET));
                return row.child(plate(folder_icon(&self.palette, *open), name));
            }
            let panes = space.panes(tab);
            if panes.len() > 1 {
                let width = self.segment_width(space.depth(tab), panes.len());
                let segments = panes
                    .iter()
                    .map(|&pane| self.segment(&self.palette, pane, width, pane == tab, pane == selected));
                let developer = panes.contains(&selected) && self.page(selected).developer_mode;
                return row.child(
                    split_plate(segments)
                        .when(panes.contains(&selected), |plate| {
                            selected_plate(&self.palette, plate, 1., 0., developer)
                        })
                        .when(developer, |plate| {
                            plate.child(developer_outline(&self.palette, PLATE_RADIUS))
                        }),
                );
            }
            let page = self.page(tab);
            let icon = div()
                .when(page.unloaded, |icon| icon.opacity(SLEEPING_FAVICON))
                .child(self.tab_icon(tab));
            let bright = tab == selected || page.developer_mode;
            let developer = tab == selected && page.developer_mode;
            row.child(
                plate(icon, title(&self.palette, self.title(tab), bright).pr(px(TITLE_END_INSET)))
                    .when(tab == selected, |plate| {
                        selected_plate(&self.palette, plate, 1., 0., developer)
                    })
                    .when(developer, |plate| {
                        plate.child(developer_outline(&self.palette, PLATE_RADIUS))
                    }),
            )
        };
        let (plus, new_tab) = new_tab_label(&self.palette);
        let icon = space_icon(&self.palette, &space.icon);
        let (pinned, today) = space.rows();
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(room > 0., |page| page.child(div().flex_none().h(px(room))))
            .child(
                name_box(&self.palette, icon.is_some())
                    .children(icon)
                    .child(div().truncate().child(space.name.clone())),
            )
            .children(pinned.into_iter().map(row))
            .when(!today.is_empty(), |page| {
                let clear = self.tools_shown().then(|| {
                    still_clear(&self.palette, space.clears_last(space.shown)).opacity(self.tools_opacity())
                });
                page.child(divider_box(&self.palette, self.hairline(), clear))
            })
            .child(row_box().child(plate(plus, new_tab)))
            .children(today.into_iter().map(row))
    }

    /// The Space's name, its pinned tabs, the divider, New Tab and its Today tabs, under the URL
    /// field, after room for the tiles when they scroll with it. Closed tabs' rows fade under the
    /// rest, and while rows slide down to make room for a tab's new row, they come out from
    /// beneath it.
    fn list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let room = self.tiles_room();
        let mut rows = vec![(None, self.name_row(cx).into_any_element())];
        let (pinned, today) = self.rows();
        let drawn = self.layout().into_iter().filter_map(|(slot, top)| match slot {
            Slot::Tab(tab) => Some((tab, top + self.slide(slot))),
            _ => None,
        });
        self.drawn_rows.replace(drawn.collect());
        for tab in pinned {
            rows.push((Some(tab), self.list_row(tab, cx)));
        }
        if !today.is_empty() {
            rows.push((None, self.divider(cx).into_any_element()));
        }
        rows.push((None, self.new_tab_row(cx).into_any_element()));
        for tab in today {
            rows.push((Some(tab), self.list_row(tab, cx)));
        }
        // A dragged row's ghost, over the rest where the pointer has it, unless it's over the
        // favorites or the page. The system draws links and files dragged in.
        let over_list = self.drag.as_ref().filter(|drag| {
            drag.moving && drag.over_page.is_none() && matches!(drag.to, Place::List(..))
        });
        let line = over_list.and_then(|drag| Some(self.drop_line(drag.line?, drag.depth, room)));
        let dragged = over_list.filter(|drag| !drag.incoming);
        // Mid-turn into or out of a tile, it's drawn over the sidebar instead.
        let dragged = dragged.filter(|drag| drag.shape.start.is_none()).map(|drag| {
            let row = self.tab_row(drag.tab, true, cx);
            let top = room + self.drag_at(drag);
            let ghost = div().id("ghost").absolute().top(px(top)).left_0();
            ghost.w(px(self.width())).child(row)
        });
        let under = self
            .opened
            .as_ref()
            .and_then(|opened| {
                rows.iter().position(|&(tab, _)| tab.is_some_and(|tab| opened.tabs.contains(&tab)))
            })
            .map_or(rows.len(), |row| row + 1);
        let under = rows.split_off(under).into_iter().map(|(_, row)| row).collect::<Vec<_>>();
        div()
            .id("today")
            .when(self.a11y, |list| {
                list.role(Role::List).a11y_synthetic_children(access::group_rows)
            })
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(&self.list_scroll)
            // Fingers across the list page between Spaces instead.
            .restrict_scroll_to_axis()
            .when(room > 0., |list| list.child(div().flex_none().h(px(room))))
            .children(self.closing.iter().map(|row| self.closing_row(row, room)))
            .children(rows.into_iter().map(|(_, row)| row))
            .when(!under.is_empty(), |list| {
                list.child(div().flex_none().flex().flex_col().overflow_hidden().children(under))
            })
            .children(line)
            .children(dragged)
            .child(self.drag_listeners(cx))
    }

    /// A tab's row in the list, which stays in its place while it's dragged (Arc).
    fn list_row(&self, tab: u64, cx: &mut Context<Self>) -> AnyElement {
        self.tab_row(tab, false, cx)
    }

    /// The line that marks where a dragged row would drop, `y` down the list from its top, a row
    /// `depth` folders in, with a ring at its start, as Arc's (docs/ux-spec.md › Tab rows).
    fn drop_line(&self, y: f32, depth: usize, room: f32) -> impl IntoElement + use<> {
        let left = PLATE_INSET + depth as f32 * FOLDER_INDENT + DROP_RING_X;
        let ring = div()
            .flex_none()
            .size(px(DROP_RING))
            .rounded_full()
            .border(px(DROP_LINE))
            .border_color(rgba(DROP));
        div()
            .absolute()
            .top(px(room + y - DROP_RING / 2.))
            .left(px(left))
            .w(px(self.width() - PLATE_INSET - DROP_LINE_END - left))
            .h(px(DROP_RING))
            .flex()
            .items_center()
            .child(ring)
            .child(div().flex_1().h(px(DROP_LINE)).bg(rgba(DROP)))
    }

    /// What the reveal's window shows: the sidebar in a panel, as far out as it is.
    pub fn render_reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.a11y = window.is_a11y_active();
        if self.reveal_motion.is_some() {
            if self.reveal_step().is_some() {
                window.request_animation_frame();
            } else {
                self.reveal_motion = None;
                // check.sh waits for these lines.
                if self.revealed {
                    log!("browser: reveal shown");
                    // Its window ends at its edge, so the page past it takes clicks again.
                    self.browser.show_reveal(REVEAL_INSET + self.width(), cx);
                } else {
                    log!("browser: reveal hidden");
                    self.browser.hide_reveal(cx);
                }
            }
        }
        // Hidden, its window still draws as the shell changes, and its list shares the docked
        // sidebar's scroll handle: drawn short there, it would scroll the docked list.
        if !self.reveal_shown() {
            return div().into_any_element();
        }
        if self.list_moving() {
            window.request_animation_frame();
        }
        self.step_retip(cx);
        self.step_drag_shape(window, cx);
        self.step_held_row(window);
        self.step_held_nav(window);
        self.step_form_going(cx);
        self.step_scroll_edge(window);
        self.step_tools(window);
        let left = REVEAL_INSET - (REVEAL_INSET + self.width()) * (1. - self.reveal_out());
        // Unless a field in the panel has it.
        if !self.reveal_focus.contains_focused(window, cx) {
            window.focus(&self.reveal_focus, cx);
        }
        let panel = div()
            // GPUI matches a binding against the focused element's contexts, so with none focused a
            // key here falls through to the menu's key equivalents, where ⌘S found Save Page.
            .key_context(KEY_CONTEXT)
            .track_focus(&self.reveal_focus)
            .on_action(cx.listener(|shell, _: &crate::text_input::Undo, _, cx| {
                shell.undo(cx);
            }))
            .on_action(cx.listener(|shell, _: &crate::text_input::Redo, _, cx| {
                shell.redo(cx);
            }))
            .size_full()
            .font_family(crate::face::UI)
            // A double-click below the rows opens a new tab, as in the docked sidebar.
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|shell, event: &MouseUpEvent, _, cx| {
                    let below_rows = event.position.y >= px(REVEAL_INSET + shell.under_tiles());
                    if event.click_count == 2 && below_rows && shell.form().is_none() {
                        shell.run(Command::NewTab, "sidebar", cx);
                    }
                }),
            )
            .child(
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(REVEAL_INSET))
                    .bottom(px(REVEAL_INSET))
                    .w(px(self.width()))
                    .rounded(px(REVEAL_RADIUS))
                    .border(px(0.5))
                    .border_color(self.palette.faint(REVEAL_BORDER))
                    .bg(self.gradient(Some(self.palette.underlay.into())))
                    .child(self.window_buttons(cx))
                    .child(self.sidebar(cx).left_0())
                    .child(self.scroll_listener(false, cx)),
            )
            .into_any_element();
        self.breath.arm(cx);
        panel
    }

    /// Whether the UI draws the window's own buttons, where nothing else does, at `WINDOW_BUTTONS`.
    pub fn draws_own_window_buttons(&self) -> bool {
        DRAWS_WINDOW_BUTTONS && self.window_buttons && !self.full_screen
    }

    /// Where the copies of the window's buttons go, in the window or in a panel at its corner: on
    /// the window's own, at their size (#1310).
    pub(super) fn window_button_frames(&self) -> [Bounds<Pixels>; 3] {
        self.window_button_geometry.unwrap_or_default().frames()
    }

    /// The window's close, minimize and zoom buttons, drawn in the reveal's panel.
    fn window_buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        if cfg!(target_os = "windows") {
            return self.windows_buttons(cx);
        }
        let zoom = if self.maximized { "icons/window-restore.svg" } else { ZOOM_GLYPH };
        let buttons = [
            (WindowButton::Close, "icons/window-close.svg"),
            (WindowButton::Minimize, "icons/window-minimize.svg"),
            (WindowButton::Zoom, zoom),
        ];
        let frames = self.window_button_frames();
        let row = Bounds::from_corners(frames[0].origin, frames[2].bottom_right());
        div()
            .absolute()
            .left(row.left())
            .top(row.top())
            .w(row.size.width)
            .h(row.size.height)
            .group("window-buttons")
            // The row, not each button: a button that blocked the mouse would hide the row behind
            // it from the hover test, and the glyphs would show only between the buttons.
            .block_mouse_except_scroll()
            .children(buttons.into_iter().zip(WINDOW_BUTTONS).zip(frames).map(
                |(((button, glyph), (_, colour)), frame)| {
                    div()
                        .id(glyph)
                        .absolute()
                        .left(frame.left() - row.left())
                        .top(frame.top() - row.top())
                        .size(frame.size.width)
                        .rounded_full()
                        .bg(rgb(colour))
                        .on_click(cx.listener(move |shell, _, _, cx| {
                            // check.sh waits for this line.
                            log!("browser: window button {button:?}");
                            shell.browser.press_window_button(button, cx);
                        }))
                        .child(
                            svg()
                                .path(glyph)
                                .size(frame.size.width)
                                .text_color(gpui::transparent_black())
                                .group_hover("window-buttons", |style| {
                                    style.text_color(rgba(WINDOW_BUTTON_GLYPH))
                                }),
                        )
                },
            ))
            .into_any_element()
    }

    /// Windows' own buttons where the Mac's show, in Windows' order: minimize, maximize and close,
    /// so a Windows user doesn't read the window as a Mac port's (#1244). The host offers Snap
    /// layouts over maximize, the middle one (platform/windows/host.cc's FrameHit).
    fn windows_buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        let maximize = match self.maximized {
            true => "icons/window-windows-restore.svg",
            false => "icons/window-windows-maximize.svg",
        };
        let buttons = [
            (WindowButton::Minimize, "icons/window-windows-minimize.svg"),
            (WindowButton::Zoom, maximize),
            (WindowButton::Close, "icons/window-windows-close.svg"),
        ];
        let (first, _) = WINDOW_BUTTONS[0];
        let (second, _) = WINDOW_BUTTONS[1];
        let ink = Hsla::from(self.palette.glyph(NAV));
        let plate = self.palette.plate(NAV_HOVERED);
        div()
            .absolute()
            .left(px(first - WINDOWS_BUTTON_PLATE / 2.))
            .top(px(WINDOW_BUTTON_Y - WINDOWS_BUTTON_PLATE / 2.))
            .flex()
            .gap(px(second - first - WINDOWS_BUTTON_PLATE))
            .block_mouse_except_scroll()
            .children(buttons.into_iter().map(|(button, glyph)| {
                let close = matches!(button, WindowButton::Close);
                div()
                    .id(glyph)
                    .size(px(WINDOWS_BUTTON_PLATE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(NAV_RADIUS))
                    .group(glyph)
                    .map(|button| match close {
                        true => button.hover(|style| style.bg(rgb(WINDOWS_CLOSE_HOVERED))),
                        false => button.hover(move |style| style.bg(plate)),
                    })
                    .on_hover(move |hovered, _, _| {
                        if *hovered {
                            // check.sh waits for this line before it captures the plate.
                            log!("browser: window button {button:?} hovered");
                        }
                    })
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        // check.sh waits for this line.
                        log!("browser: window button {button:?}");
                        shell.browser.press_window_button(button, cx);
                    }))
                    .child(
                        svg()
                            .path(glyph)
                            .size(px(WINDOWS_BUTTON_GLYPH))
                            .text_color(ink)
                            .when(close, |svg| {
                                svg.group_hover(glyph, |style| style.text_color(gpui::white()))
                            }),
                    )
            }))
            .into_any_element()
    }
}

/// `color`, opaque, as 0xRRGGBB.
fn packed(color: Hsla) -> u32 {
    let color = Rgba::from(color);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u32;
    channel(color.r) << 16 | channel(color.g) << 8 | channel(color.b)
}

/// A card at `bounds` in the window, of `fill`.
fn card(bounds: Bounds<Pixels>, fill: Rgba) -> gpui::Div {
    div()
        .absolute()
        .left(bounds.left())
        .top(bounds.top())
        .w(bounds.size.width)
        .h(bounds.size.height)
        .rounded(px(CARD_RADIUS))
        .bg(fill)
}

/// `way`, but past `low` or `high` only a little further, on the Space pages' rubber band.
fn rubber_band(way: f32, low: f32, high: f32) -> f32 {
    match way {
        way if way > high => high + band(way - high, 255., 0.15),
        way if way < low => low - band(low - way, 255., 0.15),
        way => way,
    }
}

/// How far something goes for `past` beyond its end: `stiffness` of the way at first, and never
/// as far as `limit`.
fn band(past: f32, limit: f32, stiffness: f32) -> f32 {
    limit * (1. - 1. / (stiffness * past / limit + 1.))
}

/// How far something settling critically damped since `start`, as the Space pages do, has come
/// from where it started, or None once it's there.
fn settle_progress(start: Instant) -> Option<f32> {
    critically_damped(start.elapsed().as_secs_f32(), std::f32::consts::TAU / SPACE_SETTLE)
}

/// How far the list's motion is `t` seconds in, or None once it's done.
fn row_spring(t: f32) -> Option<f32> {
    spring(t, ROW_RESPONSE, ROW_DAMPING)
}

/// Where a spring going from `from` toward `to`, at `speed` a second as it starts, is `t` seconds
/// in, and how fast it's going, or None once it's there.
fn spring_to(
    t: f32,
    from: f32,
    to: f32,
    speed: f32,
    response: f32,
    damping: f32,
) -> Option<(f32, f32)> {
    let omega = std::f32::consts::TAU / response;
    let decay = (-damping * omega * t).exp();
    if decay < 0.001 {
        return None;
    }
    let damped = omega * (1. - damping * damping).sqrt();
    let (away, pull) = (from - to, damping * omega);
    let swing = (speed + pull * away) / damped;
    let (sin, cos) = (damped * t).sin_cos();
    let at = to + decay * (away * cos + swing * sin);
    let going =
        decay * ((swing * damped - pull * away) * cos - (away * damped + pull * swing) * sin);
    Some((at, going))
}

/// How far a critically damped spring from 0 to 1, at rest at 0, has come `t` seconds in, at
/// `omega` rad/s, or None once it's within a thousandth of 1.
fn critically_damped(t: f32, omega: f32) -> Option<f32> {
    let left = (1. + omega * t) * (-omega * t).exp();
    (left > 0.001).then_some(1. - left)
}

/// Where a spring from 0 to 1, at rest at 0, is `t` seconds in, by SwiftUI's response and damping
/// fraction. None once it's within a thousandth of 1 for good.
pub(crate) fn spring(t: f32, response: f32, damping: f32) -> Option<f32> {
    let omega = std::f32::consts::TAU / response;
    let decay = (-damping * omega * t).exp();
    if decay < 0.001 {
        return None;
    }
    let damped = omega * (1. - damping * damping).sqrt();
    Some(1. - decay * ((damped * t).cos() + damping * omega / damped * (damped * t).sin()))
}

/// Where the list's rows are from its top, by the heights they're drawn at in `list`.
fn slots(pinned: &[u64], today: &[u64]) -> HashMap<Slot, f32> {
    let rows = pinned
        .iter()
        .map(|&tab| (Slot::Tab(tab), ROW_PITCH))
        .chain((!today.is_empty()).then_some((Slot::Divider, DIVIDER_HEIGHT)))
        .chain([(Slot::NewTab, ROW_PITCH)])
        .chain(today.iter().map(|&tab| (Slot::Tab(tab), ROW_PITCH)));
    let mut top = NAME_HEIGHT;
    rows.map(|(slot, height)| {
        top += height;
        (slot, top - height)
    })
    .collect()
}

/// A row of the list, around its plate.
fn row_box() -> gpui::Div {
    div()
        .flex_none()
        .h(px(ROW_HEIGHT))
        .mb(px(ROW_PITCH - ROW_HEIGHT))
        .px(px(PLATE_INSET))
        .py(px((ROW_HEIGHT - PLATE_HEIGHT) / 2.))
}

/// The row with the Space's name, over its tabs, with room before the name for the Space's icon if
/// it has one.
fn name_box(palette: &Palette, icon: bool) -> gpui::Div {
    div()
        .flex_none()
        .h(px(NAME_HEIGHT))
        .flex()
        .items_center()
        .pl(px(if icon { NAME_X } else { ICONLESS_NAME_X }))
        .pr(px(FAVICON_X))
        .text_size(px(text::BODY))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.text(SPACE_NAME))
}

/// The dashed square that holds the icon's place before a Space's name, in the name row `name_box`
/// makes, while the icon picker shows for a Space with no icon.
fn icon_placeholder() -> impl IntoElement {
    dashed_square(rgba(ICON_PLACEHOLDER_COLOR).into())
        .absolute()
        .left(px(FAVICON_X + FAVICON_SIZE / 2. - ICON_PLACEHOLDER / 2.))
        .top(px((NAME_HEIGHT - ICON_PLACEHOLDER) / 2.))
        .size(px(ICON_PLACEHOLDER))
}

/// A square of Arc's dashes in `color`, as big as it's laid out.
pub(super) fn dashed_square(color: Hsla) -> gpui::Canvas<()> {
    let line = ICON_PLACEHOLDER_LINE;
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            // The line is drawn inside the square, as Arc's is.
            let inset = px(line / 2.);
            let inner = Bounds::new(
                bounds.origin + point(inset, inset),
                bounds.size - size(inset * 2., inset * 2.),
            );
            let (dash, gap) = ICON_PLACEHOLDER_DASH;
            let mut path = PathBuilder::stroke(px(line)).dash_array(&[px(dash), px(gap)]);
            split_targets::rounded_rect(&mut path, inner, px(ICON_PLACEHOLDER_RADIUS) - inset);
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
}

/// A Space's icon before its name, in the name row `name_box` makes, if it has one: its emoji, or
/// its glyph in the name's colour.
pub(super) fn space_icon(palette: &Palette, icon: &str) -> Option<gpui::Div> {
    space_icon_drawn(palette, icon).map(icon_slot)
}

/// A Space's emoji, or its glyph in the name's colour, if it has an icon.
pub(super) fn space_icon_drawn(palette: &Palette, icon: &str) -> Option<gpui::AnyElement> {
    match SpaceIcon::parse(icon) {
        SpaceIcon::Dot => None,
        SpaceIcon::Emoji(emoji) => Some(SharedString::from(emoji.to_string()).into_any_element()),
        SpaceIcon::Glyph(index) => Some(
            svg()
                .path(GLYPHS[index].1)
                .size(px(SPACE_GLYPH))
                .text_color(palette.text(SPACE_NAME))
                .into_any_element(),
        ),
    }
}

/// The place before a Space's name that its icon takes, with `drawn` in it.
pub(super) fn icon_slot(drawn: impl IntoElement) -> gpui::Div {
    div()
        .absolute()
        .left(px(SPACE_ICON_X))
        .top_0()
        .h_full()
        .w(px(SPACE_ICON_SLOT))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(SPACE_ICON_TEXT))
        .child(drawn)
}

/// The divider under the pinned tabs: a line, and `clear` at its end while it shows.
fn divider_box(palette: &Palette, line: Pixels, clear: Option<impl IntoElement>) -> gpui::Div {
    // Clear ends where the titles do, 17 in from the edge.
    let right = if clear.is_some() { FAVICON_X } else { DIVIDER_X };
    div().flex_none().h(px(DIVIDER_HEIGHT)).child(
        div()
            .absolute()
            .top(px(DIVIDER_LINE_Y - CLEAR_HEIGHT / 2.))
            .left(px(DIVIDER_X))
            .right(px(right))
            .h(px(CLEAR_HEIGHT))
            .flex()
            .items_center()
            .child(div().flex_1().h(line).bg(palette.faint(DIVIDER)))
            .children(clear),
    )
}

/// The divider's Clear: the word, after `arrow` while it would close the last Today tabs (Arc).
/// The word stays at the titles' end, and the arrow's room opens `room` of the way before it, which
/// moves the line's end, so nothing jumps as the arrow comes and goes. It hangs out by a margin
/// that takes its clicks, and the word keeps its place.
fn clear_label(palette: &Palette, arrow: AnyElement, room: f32, color: f32) -> gpui::Div {
    div()
        .flex_none()
        .ml(px(DIVIDER_GAP - CLEAR_HIT_OUT))
        .mr(px(-CLEAR_HIT_OUT))
        .px(px(CLEAR_HIT_OUT))
        .h_full()
        .flex()
        .items_center()
        .text_size(px(text::SMALL))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(palette.text(color))
        .child(
            div().relative().flex_none().h_full().w(px(room * CLEAR_ARROW_ROOM)).child(
                div()
                    .absolute()
                    .top(px((CLEAR_HEIGHT - CLEAR_ARROW) / 2.))
                    .right(px(CLEAR_GAP - CLEAR_ARROW_INSET))
                    .child(arrow),
            ),
        )
        .child("Clear")
}

/// Clear, with the arrow while `arrow`, where it doesn't change on the button: a Space's page
/// beside the one shown, and a Library card.
fn still_clear(palette: &Palette, arrow: bool) -> gpui::Div {
    let glyph = match arrow {
        true => svg().path(ARROW_DOWN).size(px(CLEAR_ARROW)).text_color(palette.text(CLEAR)),
        false => return clear_label(palette, div().into_any_element(), 0., CLEAR),
    };
    clear_label(palette, glyph.into_any_element(), 1., CLEAR)
}

/// "+ New Tab": its plus and its label. The plus is drawn as a favicon is, so it centres on the
/// label as a favicon does on its title; a "+" set in text sat on its font's baseline, 2 pt low.
fn new_tab_label(palette: &Palette) -> (gpui::Svg, gpui::Div) {
    let plus = svg().path("icons/plus.svg").size(px(FAVICON_DRAWN)).text_color(palette.glyph(NEW_TAB_PLUS));
    let label = div().text_size(px(text::BODY)).text_color(palette.text(NEW_TAB)).child("New Tab");
    (plus, label)
}

/// A row's plate: its icon in the favicon's square, then its label.
fn plate(icon: impl IntoElement, label: impl IntoElement) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .rounded(px(PLATE_RADIUS))
        .child(
            div()
                .flex_none()
                .ml(px(FAVICON_X - PLATE_INSET))
                .mr(px(TITLE_GAP))
                .size(px(FAVICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(icon),
        )
        .child(label)
}

/// A folder's icon, open or closed.
fn folder_icon(palette: &Palette, open: bool) -> AnyElement {
    svg()
        .path(swap::folder_glyph(open))
        .size(px(FAVICON_DRAWN))
        .text_color(palette.text(SECONDARY))
        .into_any_element()
}

/// A tab's favicon, or a globe for a page without one.
fn favicon(palette: &Palette, favicon: Option<Arc<Image>>) -> AnyElement {
    match favicon {
        Some(favicon) => img(favicon).size(px(FAVICON_DRAWN)).into_any_element(),
        None => svg()
            .path("icons/globe.svg")
            .size(px(FAVICON_DRAWN))
            .text_color(palette.text(SECONDARY))
            .into_any_element(),
    }
}

/// A split view's plate, holding its segments. Each segment's favicon is as far from its start as
/// from the line before it, so the first's is where a tab's is.
fn split_plate(segments: impl IntoIterator<Item = impl IntoElement>) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .rounded(px(PLATE_RADIUS))
        .pl(px(FAVICON_X - PLATE_INSET - SEGMENT_FAVICON_X))
        .children(segments)
}

/// The selected tab's plate, at `opacity` while it fades in, a step firmer at rest in Developer
/// Mode, which the pointer takes to the same plate as another tab's.
fn selected_plate(
    palette: &Palette,
    plate: gpui::Div,
    opacity: f32,
    hover: f32,
    developer: bool,
) -> gpui::Div {
    let fill = palette.selected(hover);
    let firmer = if developer { DEVELOPER_SELECTED - crate::palette::STATE_SELECTED } else { 0. };
    let fill = Hsla { a: (fill.a + firmer * (1. - hover)).min(1.), ..fill };
    plate.bg(fill.opacity(fill.a * opacity)).shadow(palette.selected_shadow(opacity))
}

/// The shown tab's outline in Developer Mode, inside its plate's or tile's `radius` corners, over
/// its fill.
fn developer_outline(palette: &Palette, radius: f32) -> gpui::Div {
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius))
        .border(px(DEVELOPER_OUTLINE_WIDTH))
        .border_dashed()
        .border_color(palette.text(DEVELOPER_OUTLINE))
}

/// A tab row's title, cut off at the row's trailing edge.
fn title(palette: &Palette, title: SharedString, selected: bool) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .text_size(px(text::BODY))
        .text_color(palette.text(if selected { TITLE } else { UNSELECTED_TITLE }))
        .child(title)
}

/// How far a fade from `from` to `to` has come `elapsed` seconds into `duration`, on the scroll
/// edge's ease-out curve.
pub(crate) fn fade_at(from: f32, to: f32, elapsed: f32, duration: f32) -> f32 {
    from + (to - from) * cubic_bezier(0.25, 0.46, 0.45, 0.94, (elapsed / duration).min(1.))
}

/// Where a CSS cubic-bezier(`x1`, `y1`, `x2`, `y2`) curve is at `x`, from 0 to 1: the x of each
/// control point, then its y.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let at = |t: f32, p1: f32, p2: f32| {
        let u = 1. - t;
        3. * u * u * t * p1 + 3. * u * t * t * p2 + t * t * t
    };
    // x rises with t for every curve CSS allows, so halving finds t to well under a pixel.
    let (mut low, mut high) = (0., 1.);
    for _ in 0..20 {
        let t = (low + high) / 2.;
        if at(t, x1, x2) < x {
            low = t;
        } else {
            high = t;
        }
    }
    at((low + high) / 2., y1, y2)
}

/// Whether a change to `tab` can show in the window: it's among the favorites or the Space's
/// rows, it's the page the toolbar names, or the Library is open and lists tabs. Another Space's
/// tab shows only as that Space slides in, which draws at every step.
fn tab_drawn(
    tab: u64,
    selected: u64,
    favorites: &[u64],
    space: Option<&Space>,
    library: bool,
) -> bool {
    library
        || tab == selected
        || favorites.contains(&tab)
        || space.is_some_and(|space| space.pinned.contains(&tab) || space.today.contains(&tab))
}

/// The URL field's text for `url`: its host without "www." (Arc), or the URL itself if it has none.
fn host(url: &str) -> &str {
    // A data: URL can hold "://" further in, after characters no scheme has.
    let Some((scheme, rest)) = url.split_once("://") else {
        return url;
    };
    if !scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)) {
        return url;
    }
    let end = rest.find(['/', '?', '#', ':']).unwrap_or(rest.len());
    let host = &rest[..end];
    if host.is_empty() {
        return url;
    }
    host.strip_prefix("www.").unwrap_or(host)
}

/// The name of the file at a `file:` URL, with its escapes undone, which titles its page when the
/// page has no title, as in Chrome's tab strip. None for a folder.
fn file_name(url: &str) -> Option<String> {
    let path = url.strip_prefix("file://")?.split(['?', '#']).next()?;
    let name = path.rsplit('/').next().filter(|name| !name.is_empty())?;
    let mut bytes = Vec::new();
    let mut rest = name.as_bytes();
    while let Some((&byte, after)) = rest.split_first() {
        let escaped = after.get(..2).and_then(|hex| std::str::from_utf8(hex).ok());
        match escaped.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
            Some(decoded) if byte == b'%' => {
                bytes.push(decoded);
                rest = &after[2..];
            }
            _ => {
                bytes.push(byte);
                rest = after;
            }
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The `file:` URL of the file at `path`, with what a URL can't hold escaped, and a Windows path's
/// drive and backslashes as Chromium reads them.
pub fn file_url(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut url = String::from(if path.starts_with('/') { "file://" } else { "file:///" });
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'.' | b'_' | b'~' => {
                url.push(char::from(byte))
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use std::sync::Arc;

    use gpui::{Image, ImageFormat};

    use super::{Curve, Folders, Page, Shape, Space, Tab, fade_at, file_name, file_url, tab_drawn};

    /// A fade reversed partway starts from the opacity it had, so nothing jumps, and ends at the
    /// other end.
    #[test]
    fn a_reversed_fade_starts_where_it_was() {
        let halfway = fade_at(0., 1., 0.05, 0.15);
        assert!(halfway > 0. && halfway < 1.);
        assert!((fade_at(halfway, 0., 0., 0.15) - halfway).abs() < 1e-3);
        assert!(fade_at(halfway, 0., 0.05, 0.15) < halfway);
        assert!(fade_at(halfway, 0., 0.15, 0.15).abs() < 1e-3);
        assert_eq!(fade_at(halfway, 0., 9., 0.15), fade_at(halfway, 0., 0.15, 0.15));
    }

    /// A file's page is titled by its name, as the user wrote it; a folder's and a site's aren't.
    #[test]
    fn a_file_is_titled_by_its_name() {
        let report = "file:///Users/someone/Report%20final.pdf";
        assert_eq!(file_name(report).unwrap(), "Report final.pdf");
        assert_eq!(file_name("file:///tmp/%E6%97%A5%E8%A8%98.txt#top").unwrap(), "日記.txt");
        assert_eq!(file_name("file:///tmp/100%.txt").unwrap(), "100%.txt");
        assert_eq!(file_name("file:///Users/someone/"), None);
        assert_eq!(file_name("https://example.com/notes.txt"), None);
    }

    /// The licences the app carries open in a tab from wherever it's installed, and come back to
    /// the same name.
    #[test]
    fn a_path_becomes_a_file_url() {
        let licence = "/Applications/My Apps/ilo.app/Contents/Resources/LICENSE";
        let url = file_url(licence);
        assert_eq!(url, "file:///Applications/My%20Apps/ilo.app/Contents/Resources/LICENSE");
        assert_eq!(file_name(&url).unwrap(), "LICENSE");
        assert_eq!(file_name(&file_url("/tmp/日記#1.txt")).unwrap(), "日記#1.txt");
        let windows = r"C:\Program Files\ilo\THIRD-PARTY.txt";
        assert_eq!(file_url(windows), "file:///C:/Program%20Files/ilo/THIRD-PARTY.txt");
    }

    fn png(bytes: &[u8]) -> Option<Arc<Image>> {
        Some(Arc::new(Image::from_bytes(ImageFormat::Png, bytes.to_vec())))
    }

    /// A report that says what the shell already has changes nothing, so it draws no frame, even
    /// for a favicon the core encoded again into a new image.
    #[test]
    fn a_repeated_report_changes_nothing() {
        let mut tab = Tab::default();
        assert!(tab.keep_title("Inbox".into()));
        assert!(!tab.keep_title("Inbox".into()));
        assert!(tab.keep_title("Inbox (1)".into()));

        assert!(tab.keep_favicon(png(b"one")));
        assert!(!tab.keep_favicon(png(b"one")));
        assert!(tab.keep_favicon(png(b"two")));
        assert!(tab.keep_favicon(None));
        assert!(!tab.keep_favicon(None));

        let page = Page { url: "https://example.com/".into(), ..Page::default() };
        assert!(tab.keep_page(page.clone()));
        assert!(!tab.keep_page(page.clone()));
        assert!(tab.keep_page(Page { loading: true, ..page.clone() }));
        assert!(tab.keep_page(Page { audible: true, loading: true, ..page.clone() }));
        assert!(tab.keep_page(Page { icon: "🔥".into(), ..page }));
    }

    fn space(pinned: &[u64], today: &[u64]) -> Space {
        Space {
            id: 1,
            name: "Space 1".into(),
            color: None,
            theme: None,
            icon: Default::default(),
            profile: 0,
            agent: Default::default(),
            pinned: pinned.to_vec(),
            today: today.to_vec(),
            splits: Vec::new(),
            folders: Folders::default(),
            shown: 0,
        }
    }

    /// What the window draws of a tab is in the Space shown's rows, the favorites and the
    /// toolbar, or in the Library's lists; another Space's tab, shown nowhere, is not drawn.
    #[test]
    fn only_a_tab_that_shows_is_drawn() {
        let shown = space(&[1, 2], &[3]);
        let drawn = |tab, library| tab_drawn(tab, 3, &[9], Some(&shown), library);
        for tab in [1, 2, 3, 9] {
            assert!(drawn(tab, false), "tab {tab} shows");
        }
        assert!(!drawn(4, false), "a tab of another Space shows nowhere");
        assert!(drawn(4, true), "the Library lists the tabs of every Space");
        // The page the toolbar names, before the Space lists it, and with no Space shown.
        assert!(tab_drawn(5, 5, &[], Some(&shown), false));
        assert!(!tab_drawn(5, 3, &[], None, false));
    }

    /// A row turning into a tile is partway there 60 ms in, and turned for good once the spring
    /// has settled.
    #[test]
    fn a_dragged_row_turns_into_a_tile() {
        let start = Instant::now();
        let mut shape = Shape::new(false);
        shape.turn(true, start);
        let partway = shape.at(start + Duration::from_millis(60));
        assert!(partway > 0.3 && partway < 0.8, "60 ms in it's {partway} of the way");
        assert!(shape.turning(start + Duration::from_millis(60)));
        assert!(!shape.turning(start + Duration::from_millis(400)));
        assert_eq!(shape.at(start + Duration::from_millis(400)), 1.);
    }

    /// Crossing back mid-turn turns it back from where it was, with no jump, to a whole row.
    #[test]
    fn crossing_back_reverses_from_where_it_is() {
        let start = Instant::now();
        let mut shape = Shape::new(false);
        shape.turn(true, start);
        let back = start + Duration::from_millis(15);
        let there = shape.at(back);
        shape.turn(false, back);
        assert_eq!(shape.at(back), there);
        let later = shape.at(back + Duration::from_millis(5));
        assert!(later < there && later > 0., "5 ms after turning back it's at {later}");
        assert_eq!(shape.at(back + Duration::from_millis(400)), 0.);
        // Crossing again the way it's going changes nothing.
        let at = back + Duration::from_millis(20);
        let going = shape.at(at);
        shape.turn(false, back + Duration::from_millis(10));
        assert_eq!(shape.at(at), going);
    }

    /// How far `curve` is `ms` in, 1 once it's done.
    fn along(curve: Curve, ms: f32) -> f32 {
        curve.at(ms / 1000.).unwrap_or(1.)
    }

    /// The folder's and the favorites' curves pass within 0.05 of Arc's, frame by frame, never
    /// overshoot, and are done, or within a hundredth, soon after Arc's are.
    #[test]
    fn folders_and_favorites_move_as_arcs_do() {
        // Arc 1.170 rec, the share of the way at each ms (docs/ux-spec.md › Motion).
        let open = [(50., 0.38), (100., 0.69), (150., 0.91), (192., 0.996)];
        let close = [(69., 0.5), (128., 0.835), (187., 0.968), (228., 0.998)];
        let room = [(17., 0.095), (50., 0.38), (75., 0.57), (100., 0.73), (142., 0.88)];
        let curves = [
            (Curve::Fold { open: true }, &open[..]),
            (Curve::Fold { open: false }, &close[..]),
            (Curve::Room, &room[..]),
        ];
        for (curve, arcs) in curves {
            for &(ms, arc) in arcs {
                let ours = along(curve, ms);
                assert!((ours - arc).abs() < 0.05, "{ms} ms in it's {ours}, Arc's {arc}");
            }
            let steps: Vec<f32> = (0..=500).map(|ms| along(curve, ms as f32)).collect();
            assert!(steps.windows(2).all(|pair| pair[0] <= pair[1] && pair[1] <= 1.));
        }
        assert_eq!(along(Curve::Fold { open: true }, 200.), 1.);
        assert!(1. - along(Curve::Fold { open: false }, 280.) < 0.01);
        assert!(1. - along(Curve::Room, 280.) < 0.01);
    }
}
