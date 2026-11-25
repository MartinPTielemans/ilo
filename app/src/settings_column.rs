//! Settings' sidebar (spec-settings.md › The sidebar): its pages down the window's glass in the
//! owner's three groups, the one shown on its plate, and the version row and Chromium's settings at
//! its foot. The Settings window draws it beside the plane its page shows on.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, FontWeight, MouseButton, Pixels, Rgba, ScrollHandle, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowKind, WindowOptions,
    accesskit::{Orientation, Role},
    div, point,
    prelude::*,
    px, rgb, rgba, svg,
};

use crate::palette::{BUTTON_PLATE, FIELD_PLATE, INK, PAPER, TEXT_PRIMARY, TEXT_SECONDARY, Tone};
use crate::settings_card::Follow;
use crate::settings_rows::{drawn, named_button, ring};
use crate::tokens::settings::form;
use crate::tokens::settings::{MIN_HEIGHT, MIN_WIDTH, SEARCH_DIMMED};
use crate::tokens::shell::ICON_INK;

/// Settings' window at `bounds`, titled `title`, as the app opens it and the preview shows it: the
/// column runs under the window's buttons, which sit where the browser window's do, and off the Mac
/// there's no title bar of the window manager's, as the UI draws the close button.
pub fn window_options(bounds: Bounds<Pixels>, title: SharedString) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(title),
            appears_transparent: true,
            traffic_light_position: Some(point(px(12.), px(16.))),
        }),
        #[cfg(target_os = "linux")]
        window_decorations: Some(gpui::WindowDecorations::Client),
        kind: WindowKind::Normal,
        is_resizable: true,
        is_minimizable: false,
        window_min_size: Some(gpui::size(px(MIN_WIDTH), px(MIN_HEIGHT))),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests;

/// A page of Settings, as the owner drew the sidebar (spec-settings.md › The sidebar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    General,
    Window,
    Privacy,
    Passwords,
    SyncImport,
    Shortcuts,
    Threads,
    Models,
    Access,
    OtherApps,
    TabsAndSpaces,
    Links,
    /// The version row at the sidebar's foot.
    About,
}

impl Section {
    /// The sidebar's groups, in its order, as the owner drew them.
    pub const GROUPS: [&'static [Section]; 3] = [
        &[
            Section::General,
            Section::Window,
            Section::Privacy,
            Section::Passwords,
            Section::SyncImport,
            Section::Shortcuts,
        ],
        &[
            Section::Threads,
            Section::Models,
            Section::Access,
            Section::OtherApps,
        ],
        &[Section::TabsAndSpaces, Section::Links],
    ];

    /// The heading over each of `GROUPS`, in its order.
    pub const LABELS: [&'static str; 3] = ["Personal", "Agents", "Features"];

    /// Every page, in the sidebar's order: its groups', then About's at the foot.
    pub fn all() -> impl Iterator<Item = Section> {
        Section::GROUPS
            .into_iter()
            .flatten()
            .copied()
            .chain([Section::About])
    }

    /// Its name, the page's title, and what the logs and check.sh call it.
    pub fn name(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Window => "Window",
            Section::Privacy => "Privacy",
            Section::Passwords => "Passwords",
            Section::SyncImport => "Sync & Import",
            Section::Shortcuts => "Shortcuts",
            Section::Threads => "Threads",
            Section::Models => "Models",
            Section::Access => "Access",
            Section::OtherApps => "Other apps",
            Section::TabsAndSpaces => "Tabs & Spaces",
            Section::Links => "Links",
            Section::About => "About",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Section::General => "icons/settings-general.svg",
            Section::Window => "icons/settings-window.svg",
            Section::Privacy => "icons/settings-privacy.svg",
            Section::Passwords => "icons/settings-passwords.svg",
            Section::SyncImport => "icons/settings-sync.svg",
            Section::Shortcuts => "icons/settings-shortcuts.svg",
            Section::Threads => "icons/settings-threads.svg",
            Section::Models => "icons/settings-models.svg",
            Section::Access => "icons/settings-access.svg",
            Section::OtherApps => "icons/settings-apps.svg",
            Section::TabsAndSpaces => "icons/settings-tabs.svg",
            Section::Links => "icons/settings-links.svg",
            Section::About => "icons/info.svg",
        }
    }
}

/// What the column's sections hold between frames: their scroll, at the window's least height, and
/// the section the keys move to kept in sight.
#[derive(Default)]
pub struct Sections {
    pub scroll: ScrollHandle,
    follow: Follow,
}

/// What the column does with a click: shows a section, or opens Chromium's settings.
pub type Pick = Rc<dyn Fn(Section, &mut Window, &mut App)>;
pub type Foot = Rc<dyn Fn(&mut Window, &mut App)>;

/// The sidebar's colours: the palette's neutrals and the sidebar's icon ink, with Settings' own
/// quietest text and shown plate.
#[derive(Clone, Copy)]
pub struct Ink {
    pub text: Rgba,
    pub text_2: Rgba,
    pub text_3: Rgba,
    pub icon: Rgba,
    pub field: Rgba,
    pub button: Rgba,
    pub shown: Rgba,
}

impl Ink {
    pub fn new(dark: bool) -> Ink {
        let pick = |dark_one: u32, light_one: u32| rgba(if dark { dark_one } else { light_one });
        let ink = if dark { Tone::Dark } else { Tone::Light }.ink();
        let at = |a: f32| Rgba { a, ..ink };
        Ink {
            text: at(TEXT_PRIMARY),
            text_2: at(TEXT_SECONDARY),
            text_3: pick(form::TEXT_3, form::LIGHT_TEXT_3),
            icon: at(ICON_INK),
            field: at(FIELD_PLATE),
            button: at(BUTTON_PLATE),
            shown: pick(form::SHOWN, form::LIGHT_SHOWN),
        }
    }
}

/// The version row's words: the app's name and version, and what the updater last said, if it
/// said it's current or checking.
pub struct Version {
    pub name: SharedString,
    pub word: Option<SharedString>,
}

impl Version {
    /// The app's name and version, and "Up to date" or "Checking…" while the updater says so
    /// (values-settings.md › Sidebar).
    pub fn of(cx: &App) -> Version {
        use crate::updates::{self, Stage};
        let word = match updates::stage(cx) {
            Stage::Current => Some("Up to date".into()),
            Stage::Checking => Some("Checking…".into()),
            _ => None,
        };
        Version {
            name: format!(
                "{} {}",
                crate::product::PRODUCT,
                crate::build_info::version()
            )
            .into(),
            word,
        }
    }
}

/// The sidebar, 216 wide: the window's buttons in its first row, the search `field`, the pages in
/// their groups with `shown`'s on its plate, light or dark as the window is, then at its foot the
/// version row, which is About's, and Chromium's settings. While a search shows what it `found`,
/// no plate is lit and the pages with nothing found dim. Each page logs where it's drawn as
/// "toolbar {name}", as the toolbar's tabs did, for check.sh to click, with `logged`. A press on
/// the sidebar's ground moves the window, as a title bar does. While an assistive app asks,
/// `a11y`, the pages are a tab list, the one shown selected, and Chromium settings a button
/// (#1381). `sections` keeps the list's scroll.
#[allow(clippy::too_many_arguments)]
pub fn column(
    sections: &Sections,
    shown: Section,
    dark: bool,
    logged: crate::settings_rows::Drawn,
    field: AnyElement,
    found: Option<&[Section]>,
    pick: Pick,
    foot: Foot,
    version: Version,
    a11y: bool,
) -> gpui::Div {
    let ink = Ink::new(dark);
    let ring_base = if dark { rgb(PAPER) } else { rgb(INK) };
    let entry = |section: Section, logged: crate::settings_rows::Drawn| {
        let selected = found.is_none() && section == shown;
        let dimmed = found.is_some_and(|found| !found.contains(&section));
        let pick = pick.clone();
        plate(ink, selected)
            .id(section.name())
            .when(a11y, |plate| {
                plate
                    .role(Role::Tab)
                    .aria_label(section.name())
                    .aria_selected(selected)
            })
            // ⇥ reaches the list, and ↑ and ↓ move in it, as in the sidebar; Space or Return
            // shows the page.
            .tab_index(0)
            .focus_from_keys_only()
            .focus_visible(ring(ring_base))
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let all: Vec<Section> = Section::all().collect();
                let at = all.iter().position(|&each| each == section).unwrap_or(0);
                match key {
                    "down" if at + 1 < all.len() => window.focus_next(cx),
                    "up" if at > 0 => window.focus_prev(cx),
                    "down" | "up" => {}
                    _ => return,
                }
                cx.stop_propagation();
            })
            .when(dimmed, |plate| plate.opacity(SEARCH_DIMMED))
            .child(drawn(format!("toolbar {}", section.name()), logged))
            .on_click(move |_, window, cx| pick(section, window, cx))
    };
    let groups = Section::GROUPS
        .into_iter()
        .enumerate()
        .map(|(index, sections)| {
            let rows = sections.iter().map(|&section| {
                let selected = found.is_none() && section == shown;
                entry(section, logged.clone())
                    .child(glyph(section.glyph(), form::ROW_GLYPH, ink.icon))
                    .child(
                        div()
                            .when(selected, |name| name.font_weight(FontWeight::MEDIUM))
                            .child(section.name()),
                    )
            });
            // The group's heading, on the rows' text; the list's tab stops are the rows.
            let heading = div()
                .h(px(form::HEADING))
                .px(px(form::ROW_X))
                .pb(px(form::HEADING_BOTTOM))
                .flex()
                .items_end()
                .text_size(px(form::HEADING_TEXT))
                .line_height(px(form::HEADING_LINE))
                .text_color(ink.text_3)
                .child(Section::LABELS[index]);
            div().child(heading).children(rows)
        });
    // Drawn now, as the foot takes `logged` after them.
    let groups: Vec<gpui::Div> = groups.collect();
    let selected = found.is_none() && shown == Section::About;
    let about = entry(Section::About, logged.clone())
        .h(px(form::FOOT_ROW))
        .gap(px(form::FOOT_GAP))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(form::FOOT_TEXT))
                .line_height(px(form::FOOT_LINE))
                .text_color(ink.text_2)
                .when(selected, |name| name.font_weight(FontWeight::MEDIUM))
                .child(version.name),
        )
        .children(version.word.map(|word| {
            div()
                .flex_none()
                .text_size(px(form::FOOT_WORD))
                .line_height(px(form::FOOT_WORD_LINE))
                .text_color(ink.text_3)
                .child(word)
        }));
    let chromium = div()
        .id("chromium-settings")
        .h(px(form::CHROMIUM_ROW))
        .px(px(form::ROW_X))
        .flex()
        .items_center()
        .gap(px(form::CHROMIUM_GAP))
        .rounded(px(form::ROW_RADIUS))
        .when(a11y, |foot| named_button(foot, "Chromium settings"))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(ring_base))
        .text_size(px(form::FOOT_TEXT))
        .line_height(px(form::FOOT_LINE))
        .text_color(ink.text_2)
        .hover(move |row| row.bg(ink.button))
        .child(drawn("chromium settings".into(), logged))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| foot(window, cx))
        .child("Chromium settings")
        // After its name, as a link out of the window is marked.
        .child(
            svg()
                .flex_none()
                .path("icons/settings-out.svg")
                .size(px(form::CHROMIUM_GLYPH))
                .text_color(ink.icon),
        );
    let list = div()
        .id("sections")
        .when(a11y, |sections| {
            let sections = sections.role(Role::TabList).aria_label("Sections");
            sections.aria_orientation(Orientation::Vertical)
        })
        .tab_group()
        .tab_index(0)
        // Its pages are the stops, not the list that scrolls them.
        .tab_stop(false)
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&sections.scroll)
        .children(groups);
    div()
        .flex_none()
        .relative()
        .w(px(form::SIDEBAR))
        .h_full()
        .px(px(form::SIDEBAR_X))
        .pb(px(form::SIDEBAR_BOTTOM))
        .flex()
        .flex_col()
        .text_color(ink.text)
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        // The window's buttons, the OS's own, sit in this row.
        .child(div().flex_none().h(px(form::TITLE_ROW)))
        // ⇥ goes through the pages, then the field, then the page's controls, then the foot.
        .child(
            div()
                .flex_none()
                .mb(px(form::SEARCH_AFTER))
                .tab_group()
                .tab_index(1)
                .child(field),
        )
        // At the window's least height the pages scroll above the foot, not under it. The
        // version row is About's page, so the keys reach it after the list's last, as one list.
        .child(
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .tab_group()
                .tab_index(0)
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .child(sections.follow.before())
                        .child(list)
                        // A heading's height kept above the page the keys move to, so a group's
                        // first brings its heading into sight.
                        .child(
                            sections
                                .follow
                                .after(sections.scroll.clone(), form::HEADING, 0.),
                        ),
                )
                .child(about.tab_index(1)),
        )
        .child(div().flex_none().tab_group().tab_index(3).child(chromium))
}

/// A page's row: 30 tall at radius 8, its glyph 9 before its name, on the `shown` plate while
/// it's `selected`, and the button's fill under the pointer.
fn plate(ink: Ink, selected: bool) -> gpui::Div {
    div()
        .relative()
        .flex_none()
        .h(px(form::ROW))
        .px(px(form::ROW_X))
        .flex()
        .items_center()
        .gap(px(form::ROW_GAP))
        .rounded(px(form::ROW_RADIUS))
        .whitespace_nowrap()
        .text_size(px(form::ROW_TEXT))
        .line_height(px(form::ROW_LINE))
        // A press on a row picks it, and doesn't move the window.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .when(selected, |plate| plate.bg(ink.shown))
        .when(!selected, |plate| {
            plate.hover(move |plate| plate.bg(ink.button))
        })
}

fn glyph(path: &'static str, size: f32, color: Rgba) -> gpui::Svg {
    svg()
        .flex_none()
        .path(path)
        .size(px(size))
        .text_color(color)
}
