//! Settings' column (docs/ux-spec.md › Settings): its sections down the window's ground in three
//! groups, as the sidebar's rows are, the one shown on its plate, and Chromium's settings at its
//! foot. The Settings window draws it beside the card its section shows on.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, BoxShadow, MouseButton, Rgba, ScrollHandle, Window,
    accesskit::{Orientation, Role},
    div, point,
    prelude::*,
    px, rgb, rgba, svg,
};

use crate::palette::{INK, PAPER};
use crate::settings_card::Follow;
use crate::settings_rows::{drawn, named_button, ring};
use crate::tokens::radius;
use crate::tokens::settings::{
    COLUMN, COLUMN_TOP, FOOT_BOTTOM, FOOT_GLYPH, FOOT_GLYPH_ALPHA, FOOT_TEXT, NAME, PLATE_GAP,
    SEARCH_DIMMED, SEARCH_GAP, SECTION_GLYPH, SECTION_GLYPH_ALPHA, SECTION_GLYPH_GAP,
    SECTION_GROUP_GAP, SECTION_HOVERED, SECTION_SELECTED, SECTION_SELECTED_LIGHT, SECTION_SHADOW,
    SECTION_TEXT, SECTION_X,
};
use crate::tokens::shell::{PLATE_HEIGHT, PLATE_INSET};

#[cfg(test)]
mod tests;

/// A section of Settings, a page of its own on the card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    General,
    TabsAndSidebar,
    Features,
    Agents,
    Links,
    Shortcuts,
    Profiles,
    Passwords,
    Privacy,
    AdBlocking,
    Sync,
    ImportExport,
    About,
}

impl Section {
    /// The column's groups, in its order: everyday behaviour first, then the user's private
    /// things, then the account and the app.
    pub const GROUPS: [&'static [Section]; 3] = [
        &[
            Section::General,
            Section::TabsAndSidebar,
            Section::Features,
            Section::Agents,
            Section::Links,
            Section::Shortcuts,
        ],
        &[
            Section::Profiles,
            Section::Passwords,
            Section::Privacy,
            Section::AdBlocking,
        ],
        &[Section::Sync, Section::ImportExport, Section::About],
    ];

    /// Every section, in the column's order.
    pub fn all() -> impl Iterator<Item = Section> {
        Section::GROUPS.into_iter().flatten().copied()
    }

    /// Its name, the page's title, the window's, and what the logs and check.sh call it.
    pub fn name(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::TabsAndSidebar => "Tabs & Sidebar",
            Section::Features => "Features",
            Section::Agents => "Agents",
            Section::Links => "Links",
            Section::Shortcuts => "Shortcuts",
            Section::Profiles => "Profiles",
            Section::Passwords => "Passwords",
            Section::Privacy => "Privacy",
            Section::AdBlocking => "Ad Blocking",
            Section::Sync => "Sync",
            Section::ImportExport => "Import & Export",
            Section::About => "About",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Section::General => "icons/settings.svg",
            Section::TabsAndSidebar => "icons/panel-left.svg",
            Section::Features => "icons/shapes.svg",
            Section::Agents => "icons/square-mouse-pointer.svg",
            Section::Links => "icons/link.svg",
            Section::Shortcuts => "icons/keyboard.svg",
            Section::Profiles => "icons/users.svg",
            Section::Passwords => "icons/key-round.svg",
            Section::Privacy => "icons/shield.svg",
            Section::AdBlocking => "icons/shield-ban.svg",
            Section::Sync => "icons/cloud.svg",
            Section::ImportExport => "icons/arrow-down-to-line.svg",
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

/// The column, 208 wide, the search `field` at its top and `shown`'s plate lit, light or dark as
/// the window is. While a search shows what it `found`, no plate is lit and the sections with
/// nothing found dim. Each section logs where it's drawn as "toolbar {name}", as the toolbar's
/// tabs did, for check.sh to click, with `logged`. A press on the column's ground moves the
/// window, as a title bar does. While an assistive app asks, `a11y`, the sections are a tab list, the one shown
/// selected, and Chromium settings a button (#1381). `sections` keeps the list's scroll.
#[allow(clippy::too_many_arguments)]
pub fn column(
    sections: &Sections,
    shown: Section,
    dark: bool,
    logged: Rc<RefCell<Vec<String>>>,
    field: AnyElement,
    found: Option<&[Section]>,
    pick: Pick,
    foot: Foot,
    a11y: bool,
) -> gpui::Div {
    let base = if dark { rgb(PAPER) } else { rgb(INK) };
    let groups = Section::GROUPS
        .into_iter()
        .enumerate()
        .map(|(index, sections)| {
            let plates = sections.iter().map(|&section| {
                let selected = found.is_none() && section == shown;
                let dimmed = found.is_some_and(|found| !found.contains(&section));
                let pick = pick.clone();
                plate(base, selected, dark)
                    .id(section.name())
                    .when(a11y, |plate| {
                        plate
                            .role(Role::Tab)
                            .aria_label(section.name())
                            .aria_selected(selected)
                    })
                    // ⇥ reaches the list, and ↑ and ↓ move in it, as in the sidebar; Space or
                    // Return shows the section.
                    .tab_index(0)
                    .focus_from_keys_only()
                    .focus_visible(ring(base))
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
                    .child(drawn(format!("toolbar {}", section.name()), logged.clone()))
                    // A press on a plate is its click's, not the window's move.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(move |_, window, cx| pick(section, window, cx))
                    .child(glyph(
                        section.glyph(),
                        SECTION_GLYPH,
                        base.opacity(match selected {
                            true => NAME,
                            false => SECTION_GLYPH_ALPHA,
                        }),
                    ))
                    .child(section.name())
            });
            div()
                .when(index > 0, |group| group.mt(px(SECTION_GROUP_GAP)))
                .children(plates)
        });
    // Drawn now, as the foot takes `logged` after them.
    let groups: Vec<gpui::Div> = groups.collect();
    let chromium = plate(base, false, dark)
        .mb_0()
        .id("chromium-settings")
        .when(a11y, |foot| named_button(foot, "Chromium settings"))
        .tab_index(0)
        .focus_from_keys_only()
        .focus_visible(ring(base))
        .text_color(base.opacity(FOOT_TEXT))
        .child(drawn("chromium settings".into(), logged))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| foot(window, cx))
        .child(glyph(
            "icons/external-link.svg",
            FOOT_GLYPH,
            base.opacity(FOOT_GLYPH_ALPHA),
        ))
        .child("Chromium settings");
    let list = div()
        .id("sections")
        .when(a11y, |sections| {
            let sections = sections.role(Role::TabList).aria_label("Sections");
            sections.aria_orientation(Orientation::Vertical)
        })
        .tab_group()
        .tab_index(0)
        // Its sections are the stops, not the list that scrolls them.
        .tab_stop(false)
        .size_full()
        .overflow_y_scroll()
        .track_scroll(&sections.scroll)
        .children(groups);
    div()
        .flex_none()
        .relative()
        .w(px(COLUMN))
        .h_full()
        .pt(px(COLUMN_TOP))
        .pb(px(FOOT_BOTTOM))
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
        // ⇥ goes through the sections, then the field, then the card's controls, then the foot.
        .child(
            div()
                .flex_none()
                .mb(px(SEARCH_GAP))
                .tab_group()
                .tab_index(1)
                .child(field),
        )
        // At the window's least height the sections scroll above the foot, not under it.
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(sections.follow.before())
                .child(list)
                .child(sections.follow.after(sections.scroll.clone(), 0., 0.)),
        )
        .child(
            div()
                .flex_none()
                .w(px(COLUMN))
                .tab_group()
                .tab_index(3)
                .child(chromium),
        )
}

/// A section's plate: the sidebar's row, 8 in, lit under the pointer, and on the sidebar's
/// selected plate while it's `selected`, white with its shadow in light.
fn plate(base: Rgba, selected: bool, dark: bool) -> gpui::Div {
    let shadow = BoxShadow {
        color: rgba(SECTION_SHADOW).into(),
        offset: point(px(0.), px(1.)),
        blur_radius: px(2.),
        spread_radius: px(0.),
        inset: false,
    };
    let hovered = base.opacity(SECTION_HOVERED);
    div()
        .relative()
        .h(px(PLATE_HEIGHT))
        .mx(px(PLATE_INSET))
        .mb(px(PLATE_GAP))
        .pl(px(SECTION_X))
        .flex()
        .items_center()
        .rounded(px(radius::ROW))
        .whitespace_nowrap()
        .text_color(base.opacity(if selected { 1. } else { SECTION_TEXT }))
        // A press on a plate picks it, and doesn't move the window.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .when(selected && dark, |plate| {
            plate.bg(base.opacity(SECTION_SELECTED))
        })
        .when(selected && !dark, |plate| {
            plate.bg(rgba(SECTION_SELECTED_LIGHT)).shadow(vec![shadow])
        })
        .when(!selected, |plate| {
            plate.hover(move |plate| plate.bg(hovered))
        })
}

fn glyph(path: &'static str, size: f32, color: Rgba) -> gpui::Svg {
    svg()
        .flex_none()
        .path(path)
        .size(px(size))
        .mr(px(SECTION_GLYPH_GAP))
        .text_color(color)
}
