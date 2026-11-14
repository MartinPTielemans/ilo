//! The list's rows: a tab's, a split view's and a folder's, the Space's name, and the
//! divider with Clear and New Tab.

use std::{cell::Cell, f32::consts::FRAC_PI_2, time::Duration};
use web_time::Instant;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, FontWeight, Global, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Point, Radians, Role, SharedString, Transformation,
    Window, canvas, div, prelude::*, px, svg,
};

use super::Space;
use super::{
    BREATH, BREATH_LOW, CHOSEN, CLEAR, CLEAR_ARROW, CLEAR_HOVERED, CLOSE_GLYPH, CLOSE_HOVERED,
    CLOSE_RIGHT, CLOSE_SIZE, CancelName, Closing, Command, Curve, DIVIDER, EMOJI_SIZE,
    FAVICON_DRAWN, FAVICON_SIZE, FAVICON_X, FOLDER_INDENT, FORM_TEXT, HOVERED, KeepName,
    LOADING_SIZE, NAME_HEIGHT, NAV, NAV_HOVERED, NAV_RADIUS, Naming, PINNED_CARET,
    PINNED_CARET_GAP, PINNED_CARET_PLATE, PINNED_TURN, PLATE_HEIGHT, PLATE_INSET, PLATE_RADIUS,
    Page, Pending, REMOVE_GLYPH, ROW_HEIGHT, ROW_PITCH, Row, SECONDARY, SEGMENT_END,
    SEGMENT_FAVICON_X, SEGMENT_LINE, SEGMENT_TITLE_GAP, SELECTED_UNHOVER, SLEEPING_FAVICON,
    SPACE_DONE_HEIGHT, SPACE_DONE_WIDTH, SPACE_ICON_PLATE, SPACE_MORE, SPACE_MORE_INSET,
    SPACE_NAME, SPACE_NAME_SHUT, SPEAKER_DRAWN, SPEAKER_GAP, SPEAKER_SIZE, STATUS_DOT,
    STATUS_LOADING, Shell, Slot, Swapped, TITLE_END_INSET, URL_BUTTON_GLYPH, clear_label,
    cubic_bezier, developer_outline, divider_box, favicon, favicon_source, folder_icon, host,
    icon_placeholder, icon_slot, name_box, new_tab_label, plate, row_box, row_spring,
    scaled::scaled_from_middle,
    selected_plate, space_icon, split_plate,
    swap::{ARROW_DOWN, NONE, folder_glyph},
    title,
};
use super::{
    access::{Accessible, NEW_TAB},
    drag::in_sidebar,
    return_mark::Bitmap,
};
use crate::{
    icon_picker::{self, Subject},
    palette::Palette,
    space_icon::has_icon,
    tokens::shell::{
        DROP_SLOT_INSET, HOST_LINE, HOST_TITLE_LINE, NAME_HIDDEN, NAME_HIDDEN_UNDER_FAVORITES,
        SEGMENT_MARK_GAP, SPACE_PROFILE_GAP,
    },
    tokens::text,
    tooltip::{Anchor, Tip},
};

/// Whether Show Space Header keeps a lone unnamed Space's name row in every window: Arc 1.171's
/// `showSpaceHeaderOnSingleUnnamedSpaces`, off until it's chosen. The user's: agents have no call
/// to change it.
#[derive(Clone, Copy, Default)]
pub struct LoneSpaceHeader(pub bool);

impl Global for LoneSpaceHeader {}

/// Whether `name` is one ilo gives a Space the user doesn't name, "Space 2" (the form's and the
/// first launch's), where Arc 1.171 keeps no name and shows one like it.
pub(super) fn given_name(name: &str) -> bool {
    let number = name.strip_prefix("Space ").unwrap_or_default();
    !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
}

/// What Clear's tooltip says (Arc 1.170.0 cap).
pub(super) const CLEAR_TIP: &str = "Clear unpinned tabs in this space";
/// What an unloaded pinned row's remove button's tooltip says, and the Edit menu after Undo.
pub(super) const REMOVE_TIP: &str = "Remove Tab";
/// The remove button's glyph, in the icon picker's trash's style.
const REMOVE: &str = "icons/trash.svg";
/// What a split row's ✕ or − shows while ⌥ is held over it: a click separates its tabs.
const SEPARATE: &str = "icons/ungroup.svg";
/// The close button's glyphs, its two bars crossed or level, drawn rather than a file.
pub(super) const CLOSE: &str = "close";
pub(super) const UNLOAD: &str = "unload";

impl Shell {
    /// Opens `folder` of the Space shown, with the folders it's in, and scrolls the sidebar to it,
    /// as the command bar's Open does.
    pub(super) fn open_folder(&mut self, folder: u64, cx: &mut Context<Self>) {
        let Some(space) = self.shown_space() else {
            return;
        };
        let folders = std::iter::once(folder).chain(space.holders(folder));
        let shut: Vec<u64> = folders.filter(|&each| !space.is_open(each)).collect();
        for &each in &shut {
            self.browser.set_folder_open(each, true, cx);
        }
        self.revealing = (!shut.is_empty()).then_some(folder);
        self.scroll_to_row(folder);
    }

    /// Opens or closes every folder of the Space shown, all on one fold.
    pub(super) fn fold_folders(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(space) = self.shown_space() else {
            return;
        };
        let folders = space.folders.folders.keys().copied();
        let changed: Vec<u64> = folders
            .filter(|&folder| space.is_open(folder) != open)
            .collect();
        // The core reports each, and the first report has them all folded.
        self.folding = (!changed.is_empty()).then_some(open);
        for folder in changed {
            self.browser.set_folder_open(folder, open, cx);
        }
    }

    /// Collapses the pinned rows of the Space shown or expands them, on a folder's fold.
    pub(super) fn set_pinned_open(&mut self, open: bool, cx: &mut Context<Self>) {
        // The key and the menu bar's item act only as the caret is offered.
        let command = if open {
            Command::ExpandPinned
        } else {
            Command::CollapsePinned
        };
        let shown = self.shown_space().filter(|_| self.offers(command));
        let Some(space) = shown.map(|space| space.id) else {
            return;
        };
        self.folding = Some(open);
        self.browser.set_pinned_open(space, open, cx);
    }

    /// Whether the Space's pinned rows can collapse or expand now, which the name row then does
    /// as it's pressed.
    pub(super) fn pinned_folds(&self) -> bool {
        self.offers(Command::CollapsePinned) || self.offers(Command::ExpandPinned)
    }

    /// Starts the chevron's quarter turn as the Space shown's pinned rows collapse or expand, and
    /// runs frames while it turns.
    pub(super) fn step_pinned_turn(&mut self, window: &mut Window) {
        let seen = self
            .shown_space()
            .map(|space| (space.id, space.folders.pinned_shut));
        match (self.pinned_seen, seen) {
            (Some((was_in, was)), Some((space, shut))) if was_in == space && was != shut => {
                self.pinned_turn = Some(Instant::now());
            }
            (Some((was_in, _)), Some((space, _))) if was_in != space => self.pinned_turn = None,
            _ => {}
        }
        self.pinned_seen = seen;
        if let Some(start) = self.pinned_turn {
            if start.elapsed().as_secs_f32() < PINNED_TURN {
                window.request_animation_frame();
            } else {
                self.pinned_turn = None;
            }
        }
    }

    /// The chevron that says the name row collapses the Space's pinned rows or expands them (Arc
    /// 1.171): in the icon's slot if `in_slot`, else after the name, while the row is `shown` and
    /// while they're collapsed, so the way back shows, with no fade. It turns a quarter, down to
    /// pointing right, as they fold.
    fn pinned_caret(&self, in_slot: bool, shown: bool) -> Option<AnyElement> {
        let shut = self.shown_space()?.folders.pinned_shut;
        let drawn = self.caret_drawn.clone();
        if !self.pinned_folds() {
            if drawn.take().is_some() {
                log!("browser: pinned caret hidden");
            }
            return None;
        }
        let name = if shut {
            "Expand Pinned Tabs"
        } else {
            "Collapse Pinned Tabs"
        };
        // check.sh clicks where this says it's drawn.
        let frame = canvas(
            move |bounds, _, _| {
                if drawn.replace(Some((shut, bounds))) != Some((shut, bounds)) {
                    let (origin, size) = (bounds.origin, bounds.size);
                    log!(
                        "browser: pinned caret {} at {} {} {} {}",
                        if shut { "shut" } else { "open" },
                        f32::from(origin.x),
                        f32::from(origin.y),
                        f32::from(size.width),
                        f32::from(size.height),
                    );
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let t = (self.pinned_turn).map_or(1., |start| {
            (start.elapsed().as_secs_f32() / PINNED_TURN).min(1.)
        });
        let turned = cubic_bezier(0.42, 0., 0.58, 1., t);
        let turned = if shut { turned } else { 1. - turned };
        let glyph = svg()
            .path("icons/chevron-down.svg")
            .size(px(PINNED_CARET))
            .text_color(self.palette.text(SPACE_NAME))
            .with_transformation(Transformation::rotate(Radians(-FRAC_PI_2 * turned)));
        // The row takes the press; VoiceOver's goes to the row through it.
        let caret = div()
            .id("pinned-caret")
            .flex_none()
            .size(px(PINNED_CARET_PLATE))
            .flex()
            .items_center()
            .justify_center()
            .opacity(if shut || shown { 1. } else { 0. })
            .child(frame)
            .child(glyph)
            .when(self.a11y, |caret| {
                caret
                    .button_named(name)
                    .aria_expanded(!shut)
                    .pressed_by_mouse_down(true, false)
            });
        Some(match in_slot {
            true => icon_slot(caret).into_any_element(),
            false => caret.ml(px(PINNED_CARET_GAP)).into_any_element(),
        })
    }

    /// Opens or closes the folder as it's pressed, as Arc's does, even if the press becomes a drag.
    pub(super) fn press_folder(&mut self, folder: u64, cx: &mut Context<Self>) {
        let head = self.list_head();
        let Some(space) = self.spaces.iter_mut().find(|space| space.id == self.space) else {
            return;
        };
        let open = space.is_open(folder);
        let before = space.slots(head);
        let shown = space.panes(space.shown)[0];
        let holds_shown = space.holders(shown).any(|holder| holder == folder);
        // A closed folder still listing tabs shown before closes over them for good, as Arc's
        // does, and opens on the next click. Opening or closing one lets go of them too, so it
        // closes whole next time.
        if space.drop_peeks(folder) && !open && !holds_shown {
            log!("browser: folder {folder} closed for good");
            self.move_rows(before, Curve::Slide, cx);
            cx.notify();
            return;
        }
        log!("browser: command toggle-folder via sidebar");
        self.folding = Some(!open);
        self.browser.set_folder_open(folder, !open, cx);
    }

    /// A sidebar row: the plate, with the favicon slot and the label. The `ghost` is a dragged
    /// row's copy the pointer holds, drawn over the list while the row stays in its place.
    fn row(
        &self,
        row: Row,
        content: gpui::Div,
        ghost: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let selected = Some(row) == self.selected_row();
        let (id, slot) = match row {
            Row::NewTab => (("new-tab", 0), Slot::NewTab),
            Row::Tab(tab) => (("tab", tab), Slot::Tab(tab)),
        };
        let folder = matches!(row, Row::Tab(tab) if self.is_folder_anywhere(tab));
        // Full for a dragged row's ghost, which the pointer holds, and for the row it has rested
        // on, and none for the others then.
        let dragging = self.dragging();
        let onto = self.drag.as_ref().and_then(|drag| drag.onto);
        let onto = onto.is_some_and(|(tab, _)| row == Row::Tab(tab));
        let hover = match dragging {
            Some(_) => (ghost || onto).then_some(1.),
            None => self.hover_fill(row),
        };
        // A new tab's plate fades in, and rows that come together fade in whole.
        let opened = self
            .opened
            .as_ref()
            .filter(|opened| matches!(row, Row::Tab(tab) if opened.tabs.contains(&tab)));
        let (row_in, plate_in) = opened.map_or((1., 1.), |opened| {
            let done = opened
                .done(self.since(opened.start))
                .unwrap_or(1.)
                .clamp(0., 1.);
            if opened.whole { (done, 1.) } else { (1., done) }
        });
        // A page Open sent from a task's plate shows as its flight lands on its row.
        let row_in = match row {
            Row::Tab(tab) if !ghost && self.landing(tab) => 0.,
            _ => row_in,
        };
        // In from its folders, or where a dragged row would go.
        let depth = match (row, &self.drag) {
            (_, Some(drag)) if ghost => drag.depth,
            (Row::Tab(tab), _) => self.listed_depth(tab),
            (Row::NewTab, _) => 0,
        };
        let ring = match row {
            Row::Tab(tab) if !ghost => self.keyed_ring(tab, PLATE_RADIUS),
            _ => None,
        };
        let chosen = matches!(row, Row::Tab(tab) if self.chosen.contains(&tab)) && !selected;
        let developer = selected && !folder && self.page(self.selected).developer_mode;
        let plate = content
            .relative()
            .children(ring)
            .when(selected, |plate| {
                selected_plate(
                    &self.palette,
                    plate,
                    plate_in,
                    hover.unwrap_or(0.),
                    developer,
                )
            })
            .when(developer && !ghost, |plate| {
                plate.child(self.logged_developer_outline(self.selected, PLATE_RADIUS))
            })
            .when(developer && ghost, |plate| {
                plate.child(developer_outline(&self.palette, PLATE_RADIUS))
            })
            .when(selected && !developer && !ghost, |plate| {
                self.developer_outline_gone();
                plate
            })
            .id("plate")
            .when(chosen, |plate| plate.bg(self.palette.plate(CHOSEN)))
            .when_some(hover.filter(|_| !selected), |plate, hover| {
                plate.bg(self.palette.plate(HOVERED).opacity(hover))
            })
            // Pressed, a tab's row has the selected fill before the core answers its press, and a
            // folder's keeps its hover fill, as Arc's do. The held row says so, not GPUI's pressed
            // state, which outlives the release of a press that selected its row.
            .when(
                self.pressed(row) && !selected && dragging != Some(slot) && !folder,
                |plate| {
                    plate
                        .bg(self.palette.selected(0.))
                        .shadow(self.palette.selected_shadow(1.))
                },
            )
            .when(row == Row::NewTab && !selected, |plate| {
                // GPUI keeps an element's pressed state only once a listener gives it a hitbox.
                plate
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .group_active("row", |style| style.bg(self.palette.selected(0.)))
            })
            // A ghost covers the rows it passes, in the colours its row has in place.
            .when(ghost, |plate| {
                let fill = if selected {
                    self.palette.selected(0.)
                } else {
                    self.palette.plate(HOVERED)
                };
                plate.bg(self.window_fill().blend(fill))
            });
        let scale = match row {
            Row::Tab(tab) => self.press_scale(tab),
            Row::NewTab => 1.,
        };
        row_box()
            .pl(px(PLATE_INSET + depth as f32 * FOLDER_INDENT))
            .id(id)
            .group("row")
            .opacity(row_in)
            // A dragged row stays in its place, and its ghost is drawn over the list, from `list`.
            .top(px(if dragging == Some(slot) {
                0.
            } else {
                self.slide(slot)
            }))
            .block_mouse_except_scroll()
            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                shell.hover(row, *hovered, cx);
                shell.hover_tooltip(row, *hovered, cx);
                shell.hover_card(row, *hovered, cx);
                shell.folder_preview(row, *hovered, cx)
            }))
            // The ghost is a picture of a row that stays in the list.
            .when(self.a11y && !ghost, |element| {
                self.row_access(element, row, selected)
            })
            .child(scaled_from_middle(plate, scale))
    }

    /// What assistive apps read of a row: a tab, selected or not, a folder, open or closed, or New
    /// Tab. A split view's row is its segments, each a tab.
    fn row_access(
        &self,
        element: gpui::Stateful<gpui::Div>,
        row: Row,
        selected: bool,
    ) -> gpui::Stateful<gpui::Div> {
        match row {
            Row::NewTab => element.accessibility_id(NEW_TAB).button_named("New Tab"),
            Row::Tab(folder) if self.is_folder_anywhere(folder) => {
                let open = self.folder_open(folder);
                element
                    .role(Role::Button)
                    .accessibility_id(Self::folder_access_id(folder))
                    .aria_label(self.folder_name(folder))
                    .aria_expanded(open)
                    .a11y_synthetic_children(|builder| {
                        let node = builder.parent_node();
                        node.set_role_description("folder");
                        node.add_action(gpui::AccessibleAction::Click);
                        node.add_action(gpui::AccessibleAction::ShowContextMenu);
                    })
            }
            Row::Tab(tab) if self.panes(tab).len() > 1 => element,
            Row::Tab(tab) => element
                .role(Role::Tab)
                .accessibility_id(Self::tab_access_id(tab))
                .aria_label(self.tab_label(tab))
                .aria_selected(selected)
                .pressed_by_mouse_down(true, true),
        }
    }

    /// Whether `row` is a tab's that's pressed and not yet let go.
    pub(super) fn pressed(&self, row: Row) -> bool {
        let held = self.held_row.as_ref().filter(|held| held.let_go.is_none());
        held.is_some_and(|held| row == Row::Tab(held.tab))
    }

    /// How full `row`'s hover fill is: full under the pointer or with its menu up, and for the
    /// selected row, easing back once it's left. None for none.
    pub(super) fn hover_fill(&self, row: Row) -> Option<f32> {
        let onto = self.drag.as_ref().and_then(|drag| drag.onto);
        if self.pointer_row() == Some(row)
            || self.menu_row.is_some_and(|tab| row == Row::Tab(tab))
            || onto.is_some_and(|(tab, _)| row == Row::Tab(tab))
        {
            return Some(1.);
        }
        let selected = Some(row) == self.selected_row();
        let left = self
            .unhovered
            .iter()
            .find(|&&(old, _)| old == row)
            .filter(|_| selected);
        left.map(|(_, start)| {
            let t = (start.elapsed().as_secs_f32() / SELECTED_UNHOVER).min(1.);
            1. - cubic_bezier(0., 0., 0.58, 1., t)
        })
    }

    /// A closed tab's row where it was, without the selected fill it may have had, fading as the
    /// rows below slide over it.
    pub(super) fn closing_row(&self, row: &Closing, room: f32) -> impl IntoElement + use<> {
        let done = row_spring(self.since(row.start)).unwrap_or(1.);
        div()
            .absolute()
            .top(px(room + row.top))
            .left_0()
            .w(px(self.rows_width()))
            .h(px(ROW_HEIGHT))
            .px(px(PLATE_INSET))
            .pl(px(PLATE_INSET + row.indent))
            .py(px((ROW_HEIGHT - PLATE_HEIGHT) / 2.))
            .opacity((1. - done).clamp(0., 1.))
            .child(plate(
                match row.folder {
                    Some(open) => folder_icon(&self.palette, open, false),
                    None if !row.icon.is_empty() => emoji(row.icon.clone()),
                    None => favicon(&self.palette, row.favicon.clone()),
                },
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .pr(px(TITLE_END_INSET))
                    .child(title(&self.palette, row.title.clone(), false)),
            ))
    }

    /// `developer_outline` for the shown `tab`, which logs where it draws once that changes.
    pub(super) fn logged_developer_outline(&self, tab: u64, radius: f32) -> gpui::Div {
        let drawn = self.developer_outline_drawn.clone();
        let logged = move |bounds: Bounds<Pixels>, _: &mut Window, _: &mut App| {
            if drawn.get() == Some((tab, bounds)) {
                return;
            }
            drawn.set(Some((tab, bounds)));
            // check.sh reads this line.
            log!(
                "browser: developer outline {tab} at {:.0} {:.0} {:.0} {:.0}",
                f32::from(bounds.origin.x),
                f32::from(bounds.origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height)
            );
        };
        developer_outline(&self.palette, radius).child(canvas(logged, |_, _, _, _| {}).size_full())
    }

    /// The shown tab drew without its Developer Mode outline: logs that it went, once.
    pub(super) fn developer_outline_gone(&self) {
        if self.developer_outline_drawn.take().is_some() {
            // check.sh reads this line.
            log!("browser: developer outline none");
        }
    }

    /// A tab's row, its split view's for the split's first tab, or a folder's, or its `ghost`.
    pub(super) fn tab_row(&self, tab: u64, ghost: bool, cx: &mut Context<Self>) -> AnyElement {
        // A folder dragged to another Space is drawn there as one too.
        if self.is_folder_anywhere(tab) {
            return self.folder_row(tab, ghost, cx).into_any_element();
        }
        let panes = self.panes(tab);
        // Rested on, a row shows the split view it would make, the dragged tab on its side.
        let drop = self.drag.as_ref().filter(|_| !ghost).and_then(|drag| {
            drag.onto
                .filter(|&(row, _)| row == tab)
                .map(|(_, left)| (drag.tab, left))
        });
        match panes.len() > 1 || drop.is_some() {
            true => self.split_row(panes, drop, ghost, cx).into_any_element(),
            false => self.single_row(tab, ghost, cx).into_any_element(),
        }
    }

    /// A folder's row: its icon, open or closed, and its name. A press opens or closes it, a
    /// double-click renames it and a right-click shows its menu.
    fn folder_row(
        &self,
        folder: u64,
        ghost: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let open = self.folder_open(folder);
        let renaming = self
            .renaming
            .as_ref()
            .filter(|renaming| renaming.naming == Naming::Folder(folder));
        let label = match renaming {
            Some(renaming) => self.naming_field(renaming, cx),
            None => title(&self.palette, self.folder_name(folder), false).pr(px(TITLE_END_INSET)),
        };
        let renaming = renaming.is_some();
        let emoji_icon = self.folder_icon(folder);
        let icon = match has_icon(&emoji_icon) {
            true => emoji(emoji_icon),
            false => self
                .swaps
                .glyph(
                    Swapped::Folder(folder),
                    folder_glyph(open, self.elides(folder)),
                    FAVICON_DRAWN,
                    self.palette.text(SECONDARY).into(),
                    cx,
                )
                .into_any_element(),
        };
        let plate = plate(icon, label)
            .child(self.framed(folder, 0.))
            .children(self.drawn_for_picker(Subject::Folder(folder)));
        self.row(Row::Tab(folder), plate, ghost, cx)
            .when(!renaming, |row| {
                row.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                        shell.end_hovers(cx);
                        // The first press opened or closed it.
                        if event.click_count == 2 {
                            return shell.rename_folder(folder, cx);
                        }
                        if shell.choose_by_press(folder, event, cx) {
                            return;
                        }
                        shell.press_folder(folder, cx);
                        shell.press_row(folder, event.position)
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                        if shell.dragging().is_none() {
                            shell.tab_menu(folder, event.position, cx)
                        }
                    }),
                )
            })
    }

    /// The button that closes `tabs`, which shows under the pointer at a row's end: − on a pinned
    /// row, which unloads them and keeps the row, else ✕. On a split's, ⌥ held turns it to
    /// separate, which takes the tabs out of the split and closes none, as Arc's ⌥-click does.
    fn close_button(&self, tabs: Vec<u64>, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let row = tabs[0];
        let unload = self.close_glyph(row) == UNLOAD;
        let angle = if unload { 0. } else { 45. };
        let split = tabs.len() > 1;
        let separate = self.on_split_close == Some((row, true));
        let glyph = match separate {
            true => SEPARATE.into(),
            false => super::assets::bars_path(angle),
        };
        let name = match (separate, unload) {
            (true, _) => "Separate All Tabs",
            (false, true) => "Unload Tab",
            (false, false) => "Close Tab",
        };
        div()
            .id(("close", row))
            .when(self.a11y, |button| button.button_named(name))
            .flex_none()
            .ml(px(4.))
            .mr(px(CLOSE_RIGHT))
            .size(px(CLOSE_SIZE))
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .flex()
            .items_center()
            .justify_center()
            // The bars as one glyph, so they don't darken where they cross (assets::bars).
            .child(
                svg()
                    .path(glyph)
                    .flex_none()
                    .size(px(CLOSE_GLYPH))
                    .text_color(self.palette.text(SECONDARY)),
            )
            .when(split, |button| {
                button
                    .on_mouse_move(cx.listener(|shell, event: &MouseMoveEvent, _, cx| {
                        shell.alt_on_close(event.modifiers.alt, cx)
                    }))
                    .on_hover(cx.listener(move |shell, hovered: &bool, window, cx| {
                        // Off it, only its own: the pointer may be on another's already.
                        if *hovered || shell.on_split_close.is_some_and(|(on, _)| on == row) {
                            shell.on_split_close = hovered.then(|| (row, window.modifiers().alt));
                            cx.notify();
                        }
                    }))
            })
            // Not the row's press, which would show the tab before closing it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // A pinned tab closed unloads, which the core does for the same call.
            .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if split && event.modifiers().alt {
                    log!("browser: command separate-split via sidebar");
                    shell.on_split_close = None;
                    return shell.separate_all(row, cx);
                }
                log!("browser: command close-tab via sidebar");
                shell.close_row(row, cx);
            }))
    }

    /// ⌥ is held, or not: a split row's ✕ or − under the pointer shows separate while it is.
    pub(super) fn alt_on_close(&mut self, alt: bool, cx: &mut Context<Self>) {
        if let Some((_, held)) = self
            .on_split_close
            .as_mut()
            .filter(|(_, held)| *held != alt)
        {
            *held = alt;
            cx.notify();
        }
    }

    /// Closes the row whose first tab is `row`, as its ✕ or − does. A pinned row keeps its place
    /// without its pages, and its remove button waits for the pointer to leave it.
    fn close_row(&mut self, row: u64, cx: &mut Context<Self>) {
        if self.pinned().contains(&row) {
            self.remove_held = Some(row);
        }
        self.close_tab(row, true, cx);
    }

    /// The button an unloaded pinned row shows in its close button's place under the pointer,
    /// which takes the row, `tabs`, out of the pinned tabs: unpinned and closed, not into Today
    /// (owner, 2026-11-02, #1349). It names itself after a moment, as a header button does.
    fn remove_button(&self, tabs: Vec<u64>, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let row = tabs[0];
        div()
            .id(("remove", row))
            .when(self.a11y, |button| button.button_named(REMOVE_TIP))
            .flex_none()
            .ml(px(4.))
            .mr(px(CLOSE_RIGHT))
            .size(px(CLOSE_SIZE))
            .rounded(px(NAV_RADIUS))
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path(REMOVE)
                    .size(px(REMOVE_GLYPH))
                    .text_color(self.palette.text(SECONDARY)),
            )
            .child(self.button_frame(REMOVE_TIP))
            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                shell.hover_remove(row, *hovered, cx)
            }))
            // Not the row's press, which would show the tab and load it before removing it.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| {
                    cx.stop_propagation();
                    shell.browser.tooltip(None, cx)
                }),
            )
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                log!("browser: command remove-tab via sidebar");
                shell.remove_pinned(row, cx);
            }))
    }

    /// The pointer has come onto the remove button of the row whose first tab is `row`, or left
    /// it: the button's tooltip names it, and the row's card waits until the pointer is back on
    /// the row, so only one speaks at a time.
    fn hover_remove(&mut self, row: u64, hovered: bool, cx: &mut Context<Self>) {
        let busy = self.dragging().is_some() || self.menu_row.is_some();
        let frame = self.button_frames.borrow().get(REMOVE_TIP).copied();
        let tip = frame.filter(|_| hovered && !busy).map(|frame| {
            (
                Tip::named(REMOVE_TIP.into()),
                Anchor::under(frame, self.viewport),
            )
        });
        self.hover_tip(tip, cx);
        if hovered || self.pointer_row() == Some(Row::Tab(row)) {
            self.hover_card(Row::Tab(row), !hovered, cx);
        }
    }

    /// The button at the end of the row whose first tab is `tabs[0]`, under the pointer: ✕ or −
    /// while it has a page to close, else, on a pinned row, remove.
    fn row_button(
        &self,
        tabs: Vec<u64>,
        closable: bool,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        match (closable, self.removable(tabs[0])) {
            (true, _) => Some(self.close_button(tabs, cx)),
            (false, true) => Some(self.remove_button(tabs, cx)),
            (false, false) => None,
        }
    }

    /// The speaker after `tab`'s favicon while its page plays sound, crossed out while it's muted,
    /// which a click mutes or unmutes (Dia).
    fn speaker(&self, tab: u64, cx: &mut Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        let page = self.page(tab);
        let icon = speaker_glyph(page.muted);
        (page.audible || page.muted).then(|| {
            div()
                .id(("speaker", tab))
                .when(self.a11y, |button| {
                    button.button_named(if page.muted { "Unmute" } else { "Mute" })
                })
                .flex_none()
                .mr(px(SPEAKER_GAP))
                .size(px(SPEAKER_SIZE))
                .rounded(px(NAV_RADIUS))
                .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
                .flex()
                .items_center()
                .justify_center()
                .child(self.swaps.glyph(
                    Swapped::Speaker(tab),
                    icon,
                    SPEAKER_DRAWN,
                    self.palette.text(SECONDARY).into(),
                    cx,
                ))
                // Not the row's press, which would show the tab, nor a double-click's rename.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    cx.stop_propagation();
                    log!("browser: command toggle-mute via sidebar");
                    shell.toggle_mute(tab, cx);
                }))
        })
    }

    /// What `tab`'s page captures or shares, after its favicon and before its speaker, as Chrome's
    /// tab strip marks it (Arc animates it; ours is still, as a call would draw every frame).
    fn capture_mark(&self, tab: u64) -> Option<Div> {
        let glyph = capture_glyph(&self.page(tab))?;
        Some(
            div()
                .flex_none()
                .mr(px(SPEAKER_GAP))
                .size(px(SPEAKER_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(glyph)
                        .size(px(SPEAKER_DRAWN))
                        .text_color(self.palette.text(SECONDARY)),
                ),
        )
    }

    /// Mutes `tab`, or unmutes it.
    pub(super) fn toggle_mute(&mut self, tab: u64, cx: &mut Context<Self>) {
        if tab != 0 {
            let muted = self.page(tab).muted;
            self.browser.set_muted(tab, !muted, cx);
        }
    }

    /// Mutes every tab of the window, or unmutes them, those silent now too, so none starts playing
    /// aloud (Arc's Mute All Tabs; ours, which tabs: the window's, as Arc's isn't recorded).
    pub(super) fn mute_all(&mut self, muted: bool, cx: &mut Context<Self>) {
        let tabs: Vec<u64> = (self.tabs.iter())
            .filter(|(_, tab)| tab.page.muted != muted)
            .map(|(&tab, _)| tab)
            .collect();
        for tab in tabs {
            self.browser.set_muted(tab, muted, cx);
        }
    }

    /// What stands for `tab`: the emoji the user gave it, else its page's favicon.
    pub(super) fn tab_icon(&self, tab: u64) -> AnyElement {
        self.tab_icon_in(&self.palette, tab)
    }

    /// `tab`'s icon drawn in `palette`, such as a Library card's, which is its own Space's.
    pub(super) fn tab_icon_in(&self, palette: &Palette, tab: u64) -> AnyElement {
        match self.tabs.get(&tab) {
            Some(each) if has_icon(&each.page.icon) => emoji(each.page.icon.clone()),
            each => favicon(palette, each.and_then(|each| each.favicon.clone())),
        }
    }

    /// The bitmap `tab_icon_in` draws, if it draws one: not an emoji or the globe.
    pub(super) fn tab_bitmap_in(&self, palette: &Palette, tab: u64) -> Option<Bitmap> {
        let each = self.tabs.get(&tab)?;
        if has_icon(&each.page.icon) {
            return None;
        }
        Some(Bitmap {
            source: favicon_source(palette, each.favicon.clone())?,
            drawn: FAVICON_DRAWN,
        })
    }

    /// Shows `tab` for a press on its row, or its segment of a split view's, at once, as Arc's
    /// does, even if the press becomes a drag. Its page takes the keyboard, as a tab clicked in
    /// Chrome's tab strip does, so the keys that follow, ⌘D or Space, are the page's.
    pub(super) fn show_row(&mut self, tab: u64, cx: &mut Context<Self>) {
        log!("browser: command select via sidebar");
        self.select(tab, cx);
        self.browser.focus_page(cx)
    }

    /// A press on the list where the row drawn isn't the one that rests there. While the page of
    /// a Space switched to slides in, the frame last drawn has it somewhere on its way, which a
    /// busy main thread leaves behind the slide, so the pointer may be over the outgoing page,
    /// which takes no input, over nothing, or over another part of the row, such as its ✕. Among
    /// rows that moved, as ⌘D or a close moves them, it may have another row under the pointer,
    /// one on its way out. Either way it acts on the row that will be there, as the press that it
    /// is (docs/ux-principles.md: act on the press), so a click under a pointer that hasn't moved
    /// acts on the row it rests on, and a right-click where no row rests shows the menu of the
    /// Space's name or of its empty space. Returns whether it took the press; a press on a row
    /// drawn where it rests is the rows' own.
    pub(super) fn press_at_rest(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) -> bool {
        let busy = self.swipe.is_some() || self.form().is_some() || self.renaming.is_some();
        if busy || self.dragging().is_some() || (self.settle.is_some() && self.drag.is_some()) {
            return false;
        }
        let at = in_sidebar(event.position, self.sidebar_origin.get());
        let width = self.width();
        let rows = self.drawn_list_top()..f32::from(self.viewport.height) - self.list_bottom();
        if !(0. ..width).contains(&at.x) || !rows.contains(&at.y) {
            return false;
        }
        // The name row stays put over the rows scrolled under it, which take no press there.
        if self.on_fixed_head(at.y) && self.settle.is_none() {
            if event.button == MouseButton::Right {
                self.space_menu(self.space, event.position, cx);
                return true;
            }
            return false;
        }
        let y = self.list_y(at.y);
        let row_at = |tops: &[(u64, f32)]| {
            tops.iter()
                .find_map(|&(tab, top)| (top..top + ROW_PITCH).contains(&y).then_some(tab))
        };
        let layout: Vec<_> = (self.layout().into_iter())
            .filter_map(|(slot, top)| match slot {
                Slot::Tab(tab) => Some((tab, top)),
                _ => None,
            })
            .collect();
        let tab = row_at(&layout);
        if self.settle.is_none() && row_at(&self.drawn_rows.borrow()) == tab {
            return false;
        }
        let new_tab = (self.layout().into_iter())
            .any(|(slot, top)| matches!(slot, Slot::NewTab) && (top..top + ROW_PITCH).contains(&y));
        if event.button == MouseButton::Right && tab.is_none() && !new_tab {
            match y < NAME_HEIGHT && !self.name_row_hidden() {
                true => self.space_menu(self.space, event.position, cx),
                false => self.empty_space_menu(self.space, event.position, cx),
            }
            return true;
        }
        let Some(tab) = tab.filter(|&tab| self.panes(tab).len() == 1) else {
            return false;
        };
        match event.button {
            MouseButton::Right => self.tab_menu(tab, event.position, cx),
            MouseButton::Left if event.click_count == 2 && self.is_folder(tab) => {
                self.rename_folder(tab, cx)
            }
            MouseButton::Left if event.click_count == 2 => self.rename_tab(tab, cx),
            MouseButton::Left if self.little_press_row(tab, event, cx) => {}
            MouseButton::Left if event.modifiers.alt && !self.is_folder(tab) => {
                self.alt_press_list_row(tab, event.position, cx)
            }
            MouseButton::Left if self.choose_by_press(tab, event, cx) => {}
            MouseButton::Left => {
                self.end_hovers(cx);
                if self.is_folder(tab) {
                    self.press_folder(tab, cx);
                } else {
                    self.show_row(tab, cx);
                }
                self.press_row(tab, event.position);
            }
            _ => return false,
        }
        true
    }

    /// How wide each segment of a split view's row is, `count` of them `depth` folders in.
    pub(super) fn segment_width(&self, depth: usize, count: usize) -> f32 {
        let plate = self.rows_width() - 2. * PLATE_INSET - depth as f32 * FOLDER_INDENT;
        (plate - (FAVICON_X - PLATE_INSET - SEGMENT_FAVICON_X)) / count as f32
    }

    /// A tab's segment of its split view's row, drawn in `palette`: its favicon, its `speaker`, its
    /// title, bright if `focused`, and its loading dot, after a line unless it's the `first`. It's
    /// as wide as those but the title, and `weight` of the row's room past them: 0 folds it to its
    /// favicon and marks, 1 gives it the row's `room` with its title whole, and between it moves.
    pub(super) fn segment(
        &self,
        palette: &Palette,
        tab: u64,
        (weight, room): (f32, f32),
        first: bool,
        focused: bool,
        speaker: Option<AnyElement>,
        cx: &App,
    ) -> gpui::Div {
        let page = self.page(tab);
        let icon = div()
            .flex_none()
            .size(px(FAVICON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .when(page.unloaded, |icon| icon.opacity(SLEEPING_FAVICON))
            .child(self.with_return_mark(
                palette,
                tab,
                FAVICON_SIZE,
                self.tab_bitmap_in(palette, tab),
                || self.tab_icon_in(palette, tab),
                cx,
            ));
        let line = div()
            .absolute()
            .left_0()
            .top(px((PLATE_HEIGHT - SEGMENT_LINE) / 2.))
            .w(px(1.))
            .h(px(SEGMENT_LINE))
            .bg(palette.faint(DIVIDER));
        // While it moves the title keeps the width it has whole, so it's cut by the segment's
        // edge and fades, rather than its ellipsis running along it.
        let title = (weight > 0.).then(|| {
            let title = title(palette, self.title(tab), focused || page.developer_mode)
                .ml(px(SEGMENT_TITLE_GAP));
            let title = match weight < 1. {
                true => title
                    .flex_none()
                    .w(px(room - SEGMENT_TITLE_GAP))
                    .opacity(weight),
                false => title,
            };
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .overflow_hidden()
                .child(title)
        });
        div()
            .relative()
            .flex_basis(px(self.folded_width(tab)))
            .flex_grow(weight)
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .pl(px(SEGMENT_FAVICON_X))
            .pr(px(SEGMENT_END))
            .when(!first, |segment| segment.child(line))
            .child(icon)
            .children(
                speaker.map(|speaker| div().flex_none().ml(px(SEGMENT_MARK_GAP)).child(speaker)),
            )
            .children(title)
            .children(self.status_dot(tab))
    }

    /// `tab`'s speaker as a segment that takes no input draws it, while its page plays or is
    /// muted.
    pub(super) fn still_speaker(&self, palette: &Palette, tab: u64) -> Option<AnyElement> {
        let page = self.page(tab);
        (page.audible || page.muted).then(|| {
            div()
                .size(px(SPEAKER_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(speaker_glyph(page.muted))
                        .size(px(SPEAKER_DRAWN))
                        .text_color(palette.text(SECONDARY)),
                )
                .into_any_element()
        })
    }

    /// The room a split view's row of `count` segments `depth` folders in has past its segments'
    /// favicons and marks, `panes`' and a dropped `tab`'s, and its close button while it `closes`.
    fn fold_room(&self, depth: usize, panes: &[u64], dropped: Option<u64>, closes: bool) -> f32 {
        let folded: f32 = panes
            .iter()
            .chain(&dropped)
            .map(|&tab| self.folded_width(tab))
            .sum();
        // The close button and its margins, as `close_button` draws them.
        let close = if closes {
            4. + CLOSE_SIZE + CLOSE_RIGHT
        } else {
            0.
        };
        (self.segment_width(depth, 1) - folded - close).max(0.)
    }

    /// Where the middle of `tab`'s segment is across the sidebar, as `split_row` lays it out.
    #[cfg(test)]
    pub(super) fn segment_middle(&self, tab: u64) -> f32 {
        let panes = self.panes(tab);
        let first = panes[0];
        let depth = self.listed_depth(first);
        let closes = self.pointer_row() == Some(Row::Tab(first));
        let room = self.fold_room(depth, &panes, None, closes);
        let mut x = FAVICON_X - SEGMENT_FAVICON_X + depth as f32 * FOLDER_INDENT;
        for (&pane, weight) in panes.iter().zip(self.fold_weights(first, &panes)) {
            let width = self.folded_width(pane) + weight * room;
            if pane == tab {
                return x + width / 2.;
            }
            x += width;
        }
        x
    }

    /// `segment`, which a press shows focused, a double-click renames and a right-click shows the
    /// tab's menu for.
    fn split_segment(
        &self,
        tab: u64,
        fold: (f32, f32),
        first: bool,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let speaker = self
            .speaker(tab, cx)
            .map(|speaker| speaker.mr_0().into_any_element());
        self.segment(&self.palette, tab, fold, first, focused, speaker, cx)
            .id(("segment", tab))
            .when(self.a11y, |segment| {
                segment
                    .role(Role::Tab)
                    .accessibility_id(Self::tab_access_id(tab))
                    .aria_label(format!("{}, split view", self.tab_label(tab)))
                    .aria_selected(focused)
                    .pressed_by_mouse_down(true, true)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    // The first press selected the tab (Dia); the row's press may start a drag.
                    if event.click_count == 2 {
                        cx.stop_propagation();
                        return shell.rename_tab(tab, cx);
                    }
                    if (event.modifiers.alt && shell.alt_press_row(tab, cx))
                        || shell.choose_by_press(tab, event, cx)
                    {
                        return cx.stop_propagation();
                    }
                    shell.refold(tab, cx);
                    shell.show_row(tab, cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    if shell.dragging().is_none() {
                        shell.tab_menu(tab, event.position, cx)
                    }
                }),
            )
    }

    /// Where the row or tile of `subject`, a tab or a folder, is, for the icon picker to hang under
    /// while it waits for it to settle.
    pub(super) fn drawn_for_picker(&self, subject: Subject) -> Option<impl IntoElement + use<>> {
        let pending = Some(Pending::Icon(subject));
        (self.picker_pending.map(|(picker, _)| picker) == pending).then(|| {
            let drawn = self.tab_row_drawn.clone();
            canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                .absolute()
                .top_0()
                .left_0()
                .size_full()
        })
    }

    /// A split view's row, for its first tab: a segment per tab, left to right. A press on one
    /// shows the split with its page focused, and the close button closes them all. With a tab
    /// `drop`ped on it, the row of one tab or more shows that tab's slot on its left or right.
    fn split_row(
        &self,
        panes: Vec<u64>,
        drop: Option<(u64, bool)>,
        ghost: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let first = panes[0];
        let row_selected = self.selected_row() == Some(Row::Tab(first));
        let hovered = self.pointer_row() == Some(Row::Tab(first)) && drop.is_none();
        // A pinned tab closed already has nothing to close.
        let pinned = self.pinned().contains(&first);
        let closable = panes
            .iter()
            .any(|&tab| !(self.page(tab).unloaded && pinned));
        let renaming = self.renaming.as_ref().filter(|renaming| {
            renaming
                .naming
                .tab()
                .is_some_and(|tab| panes.contains(&tab))
        });
        let (name, icon): (SharedString, SharedString) =
            self.split(first).map_or_else(Default::default, |split| {
                (split.name.clone(), split.icon.clone())
            });
        let named = !name.is_empty() || has_icon(&icon);
        let content = match renaming {
            Some(renaming) => {
                let icon = match renaming.naming {
                    Naming::Split(_) => self.split_icon(&icon, first),
                    naming => self.tab_icon(naming.tab().unwrap_or(first)),
                };
                plate(icon, self.naming_field(renaming, cx))
            }
            None if named => {
                let close = hovered
                    .then(|| self.row_button(panes.clone(), closable, cx))
                    .flatten();
                let name = match name.is_empty() {
                    true => panes
                        .iter()
                        .map(|&tab| self.title(tab))
                        .collect::<Vec<_>>()
                        .join(", "),
                    false => name.to_string(),
                };
                self.named_split(first, &panes, name.into(), &icon, close, cx)
            }
            None => {
                let close = hovered
                    .then(|| self.row_button(panes.clone(), closable, cx))
                    .flatten();
                let dropped = drop.map(|(tab, _)| tab);
                let depth = self.listed_depth(first);
                let room = self.fold_room(depth, &panes, dropped, close.is_some());
                // A tab dropped on the row takes its width at once: it's what the row would show.
                let weights = match drop {
                    Some(_) => vec![0.; panes.len()],
                    None => self.fold_weights(first, &panes),
                };
                let slot = drop.map(|(_, left)| if left { 0 } else { panes.len() });
                let mut segments: Vec<_> = panes
                    .iter()
                    .zip(weights)
                    .enumerate()
                    .map(|(index, (&tab, weight))| {
                        let focused = row_selected && tab == self.selected;
                        let leads = index == 0 && slot != Some(0);
                        self.split_segment(tab, (weight, room), leads, focused, cx)
                            .into_any_element()
                    })
                    .collect();
                if let Some(((tab, _), slot)) = drop.zip(slot) {
                    segments.insert(slot, self.drop_slot(tab, room, slot == 0, cx));
                }
                split_plate(segments).children(close)
            }
        };
        let renaming = renaming.is_some();
        let bounds = self.drawn_for_picker(Subject::Tab(first));
        let row = self
            .row(Row::Tab(first), content, ghost, cx)
            .children(bounds);
        row.when(!renaming, |row| {
            row.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, _| {
                    shell.press_row(first, event.position)
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |shell, _, _, cx| shell.middle_press(first, closable, cx)),
            )
        })
    }

    /// A split view's row with a name or an icon of its own, as Arc's custom-info cell: one plate
    /// with the split's icon and `name`, in place of its segments. A press shows the page shown
    /// last, a double-click renames the split and a right-click shows its menu, as a segment's do.
    fn named_split(
        &self,
        first: u64,
        panes: &[u64],
        name: SharedString,
        icon: &SharedString,
        close: Option<impl IntoElement>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let row_selected = self.selected_row() == Some(Row::Tab(first));
        let dot = panes.iter().find_map(|&tab| self.status_dot(tab));
        let label = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .when(close.is_none(), |label| label.pr(px(TITLE_END_INSET)))
            .child(title(&self.palette, name.clone(), row_selected))
            .when(close.is_none(), |label| label.children(dot))
            .children(close);
        let content = div()
            .id(("named-split", first))
            .size_full()
            .when(self.a11y, |content| {
                content
                    .role(Role::Tab)
                    .accessibility_id(Self::tab_access_id(first))
                    .aria_label(format!("{name}, split view"))
                    .aria_selected(row_selected)
                    .pressed_by_mouse_down(true, true)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    // The first press showed the split; the row's press may start a drag.
                    if event.click_count == 2 {
                        cx.stop_propagation();
                        return shell.rename_tab(first, cx);
                    }
                    if shell.choose_by_press(first, event, cx) {
                        return cx.stop_propagation();
                    }
                    shell.show_row(shell.row_tab(first), cx)
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    if shell.dragging().is_none() {
                        shell.tab_menu(shell.row_tab(first), event.position, cx)
                    }
                }),
            )
            .child(plate(self.split_icon(icon, first), label));
        div().size_full().child(content)
    }

    /// The icon of `tab`'s split view: the one the user gave it, else Lucide's columns-2, or rows-2
    /// while it's stacked, in a folder's ink.
    fn split_icon(&self, icon: &SharedString, tab: u64) -> AnyElement {
        if has_icon(icon) {
            return emoji(icon.clone());
        }
        let glyph = if self.stacked(tab) {
            "icons/rows-2.svg"
        } else {
            "icons/columns-2.svg"
        };
        svg()
            .path(glyph)
            .flex_none()
            .size(px(FAVICON_DRAWN))
            .text_color(self.palette.text(SECONDARY))
            .into_any_element()
    }

    /// The slot `tab`, dragged onto a row, would take in the split view it would make: its segment,
    /// faint, on a plate inset in its place, so the row reads as the split before it's let go.
    fn drop_slot(&self, tab: u64, room: f32, first: bool, cx: &App) -> AnyElement {
        // The first segment starts inside the split plate's padding, and the slot from its edge.
        let padding = FAVICON_X - PLATE_INSET - SEGMENT_FAVICON_X;
        let left = if first {
            DROP_SLOT_INSET - padding
        } else {
            DROP_SLOT_INSET
        };
        let fill = div()
            .absolute()
            .top(px(DROP_SLOT_INSET))
            .bottom(px(DROP_SLOT_INSET))
            .left(px(left))
            .right(px(DROP_SLOT_INSET))
            .rounded(px(PLATE_RADIUS - DROP_SLOT_INSET))
            .bg(self.palette.plate(HOVERED));
        let speaker = self.still_speaker(&self.palette, tab);
        let segment = self
            .segment(&self.palette, tab, (1., room), first, false, speaker, cx)
            .opacity(SECONDARY);
        let slot = div()
            .relative()
            .flex_basis(px(self.folded_width(tab)))
            .flex_grow(1.)
            .min_w_0()
            .h_full()
            .flex();
        slot.child(fill).child(segment).into_any_element()
    }

    /// A middle press on the row whose first tab is `row` closes it as its ✕ or − does, on the
    /// press as a row's press shows its tab.
    fn middle_press(&mut self, row: u64, closable: bool, cx: &mut Context<Self>) {
        // As every press does: the card would show a tab the press closes.
        self.end_hovers(cx);
        if closable && self.dragging().is_none() {
            log!("browser: command close-tab via sidebar");
            self.close_row(row, cx);
        }
    }

    /// Whether a tab's row has a ✕ or − to show: a pinned tab closed already has nothing to close.
    pub(super) fn closable(&self, tab: u64) -> bool {
        !(self.page(tab).unloaded && self.pinned().contains(&tab))
    }

    /// Whether the row whose first tab is `row` shows the remove button in the −'s place: a pinned
    /// row whose pages are all unloaded, unless the pointer hasn't left it since its − unloaded it.
    pub(super) fn removable(&self, row: u64) -> bool {
        self.pinned().contains(&row)
            && !self.is_folder(row)
            && self.remove_held != Some(row)
            && self.panes(row).iter().all(|&tab| self.page(tab).unloaded)
    }

    /// The close button's glyph on the row whose first tab is `row`: one shape per action (owner,
    /// 2026-11-02, #1349), so − on a pinned row, selected or not, which unloads it and keeps the
    /// row, and ✕ on a Today row, which closes it for good.
    pub(super) fn close_glyph(&self, row: u64) -> &'static str {
        if self.pinned().contains(&row) {
            UNLOAD
        } else {
            CLOSE
        }
    }

    /// The host a tab's row names under its title, as Arc's does: a pinned tab's whose page left
    /// its pinned URL's host, and a tab's in Developer Mode, whose site is what's worked on. A
    /// renamed pinned tab at its pinned host has none (*interim*: Arc's rule for those isn't
    /// known, and one of its two seen has none).
    pub(super) fn host_line(&self, tab: u64) -> Option<SharedString> {
        let kept = self.tabs.get(&tab)?;
        let named = kept.left_pinned_host || kept.page.developer_mode;
        named.then(|| SharedString::from(host(&kept.page.url).to_owned()))
    }

    fn single_row(
        &self,
        tab: u64,
        ghost: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let page = self.page(tab);
        let closable = self.closable(tab);
        let hovered = self.pointer_row() == Some(Row::Tab(tab));
        // Away from its pinned URL, a pinned tab's favicon is a way back.
        let away = page.away_from_pinned_url;
        let back = away && self.pointer_favicon() == Some(tab);
        let label = if back {
            "Back to Pinned URL".into()
        } else {
            self.title(tab)
        };
        let bright = Some(Row::Tab(tab)) == self.selected_row() || page.developer_mode;
        let title = title(&self.palette, label, bright);
        let title = match self.host_line(tab) {
            Some(host) => with_host(&self.palette, title, host),
            None => title,
        };
        // The row's button shows under the pointer, and the title truncates before it.
        let button = hovered
            .then(|| self.row_button(vec![tab], closable, cx))
            .flatten();
        let renaming = self
            .renaming
            .as_ref()
            .filter(|renaming| renaming.naming.tab() == Some(tab));
        let label = match renaming {
            Some(renaming) => self.naming_field(renaming, cx),
            None => div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .when(button.is_none(), |label| label.pr(px(TITLE_END_INSET)))
                // The mark takes the speaker's place while the page has died or hung.
                .children(match self.trouble_mark(tab) {
                    Some(mark) => vec![mark.into_any_element()],
                    None => (self
                        .capture_mark(tab)
                        .map(IntoElement::into_any_element)
                        .into_iter())
                    .chain(self.speaker(tab, cx).map(IntoElement::into_any_element))
                    .collect(),
                })
                .child(title)
                .children(button)
                .when(!hovered, |label| label.children(self.status_dot(tab))),
        };
        let renaming = renaming.is_some();
        let icon = match back {
            true => svg()
                .path("icons/arrow-left.svg")
                .size(px(FAVICON_DRAWN))
                .text_color(self.palette.glyph(NAV))
                .into_any_element(),
            false => {
                let bitmap = self.tab_bitmap_in(&self.palette, tab);
                self.with_return_mark(
                    &self.palette,
                    tab,
                    FAVICON_SIZE,
                    bitmap,
                    || self.tab_icon(tab),
                    cx,
                )
            }
        };
        let icon = div()
            .id(("favicon", tab))
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .when(page.unloaded && !back, |icon| {
                icon.opacity(SLEEPING_FAVICON)
            })
            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                if *hovered {
                    shell.hovered_favicon = Some(tab);
                } else if shell.hovered_favicon == Some(tab) {
                    shell.hovered_favicon = None;
                }
                cx.notify();
            }))
            // It shows the tab after going back, which the row's press leaves to it, so the old
            // page never loads. With ⌘ the page goes on in a copy at the top of Today, shown, as
            // Arc's does.
            .when(away, |icon| {
                icon.on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                    if shell.dragged {
                        return;
                    }
                    if event.modifiers().secondary(cx) {
                        log!("browser: command back-to-pinned-url-keeping-page via sidebar");
                        shell.browser.duplicate(tab, cx);
                        return shell.browser.back_to_pinned_url(tab, cx);
                    }
                    log!("browser: command back-to-pinned-url via sidebar");
                    shell.browser.back_to_pinned_url(tab, cx);
                    shell.show_row(tab, cx)
                }))
            })
            .child(icon);
        let bounds = self.drawn_for_picker(Subject::Tab(tab));
        // The field of a rename takes the row's clicks while it's up.
        let plate = plate(icon.into_any_element(), label).child(self.framed(tab, 0.));
        let row = self.row(Row::Tab(tab), plate, ghost, cx);
        row.children(bounds).when(!renaming, |row| {
            row.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    shell
                        .browser
                        .hover_card(crate::hover_card::Hover::Close, cx);
                    // The first press selected the tab (Dia).
                    if event.click_count == 2 {
                        return shell.rename_tab(tab, cx);
                    }
                    if shell.little_press_row(tab, event, cx) {
                        return;
                    }
                    if event.modifiers.alt {
                        return shell.alt_press_list_row(tab, event.position, cx);
                    }
                    if shell.choose_by_press(tab, event, cx) {
                        return;
                    }
                    // On the way back to the pinned URL, the favicon's click shows the tab.
                    if !back {
                        shell.show_row(tab, cx);
                    }
                    shell.press_row(tab, event.position)
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    if shell.dragging().is_none() {
                        shell.tab_menu(tab, event.position, cx)
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(move |shell, _, _, cx| shell.middle_press(tab, closable, cx)),
            )
        })
    }

    /// The Space's name over its rows, or with the row hidden what Arc leaves of it, which keeps
    /// its place for the theme picker that waits for it to settle.
    pub(super) fn name_row(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.name_row_hidden() {
            return self.shown_name_row(cx).into_any_element();
        }
        let drawn = self.name_row_drawn.clone();
        let bounds = canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {});
        let head = div().flex_none().h(px(self.list_head()));
        head.child(bounds.absolute().top_0().left_0().size_full())
            .into_any_element()
    }

    /// The name row. Under the pointer it's a row of its own (Arc 1.171): a tab row's plate, the
    /// chevron in the icon's slot and the ••• at its end, in one frame, kept while its menu is
    /// up. A press anywhere on it but the ••• collapses the pinned rows or expands them.
    fn shown_name_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let space = self.space;
        let icon = (self.shown_space()).and_then(|each| space_icon(&self.palette, &each.icon));
        let placeholder = self.holds_icon_place().then(icon_placeholder);
        let room = icon.is_some() || placeholder.is_some();
        let renaming = (self.renaming.as_ref())
            .filter(|renaming| renaming.naming == Naming::Space(space) && !renaming.in_library)
            .map(|renaming| renaming.name.clone());
        let lit = renaming.is_some()
            || self.dragging().is_none() && self.name_hovered
            || self.name_menu && self.menu_open;
        // VoiceOver's cursor is no pointer, so the chevron and the ••• stay for it.
        let shown = renaming.is_none() && (lit || self.a11y);
        let in_slot = icon.is_some();
        let caret = self.pinned_caret(in_slot, shown);
        let shut = self
            .shown_space()
            .is_some_and(|space| space.folders.pinned_shut);
        let icon = icon.filter(|_| caret.is_none() || !in_slot || !(shut || shown));
        // A click on the plate under its icon opens the icon picker, or closes it: the click
        // outside the picker has closed it by now, but the picker hears so only after this event.
        // While the row folds the pinned rows, the slot is the chevron's.
        let icon_plate = (icon.is_some() && !self.pinned_folds()).then(|| {
            div()
                .id("space-icon")
                .absolute()
                .left(px(FAVICON_X + FAVICON_SIZE / 2. - SPACE_ICON_PLATE / 2.))
                .top(px((NAME_HEIGHT - SPACE_ICON_PLATE) / 2.))
                .size(px(SPACE_ICON_PLATE))
                .rounded(px(NAV_RADIUS))
                .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                .when(self.a11y, |plate| {
                    plate
                        .button_named("Change Icon")
                        .pressed_by_mouse_down(true, false)
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |shell, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if !icon_picker::is_open(cx) {
                            shell.change_space_icon(space, cx);
                        }
                    }),
                )
        });
        let drawn = self.name_row_drawn.clone();
        // Where the row is, for the icon picker to hang under.
        let bounds = canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full();
        let fill = lit.then(|| {
            div()
                .absolute()
                .left(px(PLATE_INSET))
                .right(px(PLATE_INSET))
                .top(px((NAME_HEIGHT - PLATE_HEIGHT) / 2.))
                .h(px(PLATE_HEIGHT))
                .rounded(px(PLATE_RADIUS))
                .bg(self.palette.plate(HOVERED))
        });
        let name = self
            .shown_space()
            .map(|space| space.name.clone())
            .unwrap_or_default();
        // VoiceOver reads the profile as it's seen, after the name.
        let label: SharedString = match self.name_row_profile(&name) {
            Some(profile) => format!("{name}, {profile}").into(),
            None => name.clone(),
        };
        // A heading VoiceOver can jump to, whose menu is the Space's.
        let row = name_box(&self.palette, room, self.blank)
            .id("space-name")
            // Lit while the pinned rows are collapsed, where Arc's turns bold.
            .when(shut, |row| {
                row.text_color(self.palette.text(SPACE_NAME_SHUT))
            })
            .when(self.a11y, |row| {
                row.role(Role::Heading)
                    .accessibility_id("space-name")
                    .aria_label(label)
                    .aria_level(2)
                    .pressed_by_mouse_down(false, true)
            })
            .block_mouse_except_scroll()
            .on_hover(cx.listener(|shell, hovered: &bool, _, cx| {
                if shell.name_hovered != *hovered {
                    shell.name_hovered = *hovered;
                    // check.sh waits for this line.
                    log!(
                        "browser: space name {}",
                        if *hovered { "hovered" } else { "left" }
                    );
                    cx.notify();
                }
            }))
            .child(bounds)
            .children(fill)
            .children(icon_plate)
            .children(icon)
            .children(placeholder);
        let (caret_in_slot, caret_after) = match in_slot {
            true => (caret, None),
            false => (None, caret),
        };
        // The end of the plate, where the ••• is and Rename Space… puts Done.
        let end = PLATE_INSET + SPACE_MORE_INSET;
        if let Some(field) = renaming {
            let done = div()
                .id("space-done")
                .absolute()
                .right(px(end))
                .top(px((NAME_HEIGHT - SPACE_DONE_HEIGHT) / 2.))
                .w(px(SPACE_DONE_WIDTH))
                .h(px(SPACE_DONE_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(NAV_RADIUS))
                .bg(self.palette.plate(CLOSE_HOVERED))
                .text_size(px(crate::tokens::text::SMALL))
                .font_weight(FontWeight::MEDIUM)
                .text_color(self.palette.text(SECONDARY))
                .child("Done")
                .when(self.a11y, |done| {
                    done.button_named("Done").pressed_by_mouse_down(true, false)
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|shell, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        log!("browser: space name done");
                        shell.keep_name(window, cx);
                    }),
                );
            return row
                .key_context("Naming")
                .on_action(
                    cx.listener(|shell, _: &KeepName, window, cx| shell.keep_name(window, cx)),
                )
                .on_action(
                    cx.listener(|shell, _: &CancelName, window, cx| shell.cancel_name(window, cx)),
                )
                .text_color(self.palette.text(FORM_TEXT))
                .pr(px(end + SPACE_DONE_WIDTH + 4.))
                .children(caret_in_slot)
                .child(field)
                .child(done);
        }
        // Arc's •••, on a tab row's ✕ square in its place, opens the Space's menu as it's pressed.
        let more = div()
            .id("space-edit")
            .absolute()
            .right(px(end))
            .top(px((NAME_HEIGHT - SPACE_MORE) / 2.))
            .size(px(SPACE_MORE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .opacity(if shown { 1. } else { 0. })
            .hover(|style| style.bg(self.palette.plate(CLOSE_HOVERED)))
            .when(self.a11y, |edit| {
                edit.button_named("Edit Space")
                    .pressed_by_mouse_down(true, false)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    shell.name_row_menu(space, event.position, cx)
                }),
            )
            .child(
                svg()
                    .path("icons/ellipsis.svg")
                    .size(px(URL_BUTTON_GLYPH))
                    .text_color(self.palette.text(SECONDARY)),
            );
        row.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |shell, _: &MouseDownEvent, _, cx| {
                if !shell.pinned_folds() {
                    return;
                }
                // Not the window's background, which a drag from it would move.
                cx.stop_propagation();
                let shut = shell
                    .shown_space()
                    .is_some_and(|space| space.folders.pinned_shut);
                let command = if shut {
                    Command::ExpandPinned
                } else {
                    Command::CollapsePinned
                };
                shell.run(command, "sidebar", cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                shell.name_row_menu(space, event.position, cx)
            }),
        )
        // The name ends before the ••• while it shows, as a tab's title does before its ✕.
        .when(shown, |row| row.pr(px(end + SPACE_MORE + 4.)))
        .children(caret_in_slot)
        .child(self.name_and_profile(name))
        .children(caret_after)
        .child(more)
    }

    /// The Space's name, then its profile's in grey while there's more than one profile (#2339),
    /// which gives way first. None where the names are the same, or in a private window, whose
    /// session shares none of the profile's logins or cookies.
    fn name_and_profile(&self, name: SharedString) -> Div {
        let profile = self.name_row_profile(&name);
        // Shrink-wrapped, so the pinned rows' caret still follows the names.
        div()
            .flex()
            .min_w_0()
            .child(div().flex_shrink_0().max_w_full().truncate().child(name))
            .children(profile.map(|profile| {
                div()
                    .ml(px(SPACE_PROFILE_GAP))
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::NORMAL)
                    .text_color(self.palette.text(SECONDARY))
                    .child(profile)
            }))
    }

    /// The profile's name the name row shows after the Space's `name`, if any.
    fn name_row_profile(&self, name: &str) -> Option<SharedString> {
        (self.shown_space())
            .filter(|_| self.profiles.len() > 1 && !self.private)
            .and_then(|space| self.profiles.iter().find(|(id, _)| *id == space.profile))
            .map(|(_, profile)| profile.clone())
            .filter(|profile| profile != name)
    }

    /// The Space's menu from its name row, which keeps the row lit while it's up.
    fn name_row_menu(&mut self, space: u64, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.space_menu(space, position, cx);
        self.name_menu = self.menu_open;
        cx.notify();
    }

    /// Whether the window's Space is the browser's only one, with no icon and the name ilo gives a
    /// Space the user doesn't name, which Arc 1.171 counts as unnamed: its name tells it from no
    /// other. An agent's Space is one more, which the name then tells it from.
    pub(super) fn lone_unnamed(&self) -> bool {
        let unnamed = |space: &Space| !has_icon(&space.icon) && given_name(&space.name);
        !self.private && self.spaces.len() == 1 && self.shown_space().is_some_and(unnamed)
    }

    /// Whether the name row hides, as Arc 1.171's does for a lone unnamed Space unless Show Space
    /// Header keeps it. It shows while the Space is renamed or given an icon there, as both hang
    /// from it.
    pub(super) fn name_row_hidden(&self) -> bool {
        let space = self.space;
        let naming = (self.renaming.as_ref())
            .is_some_and(|naming| naming.naming == Naming::Space(space) && !naming.in_library);
        let asked = Some(Pending::Icon(Subject::Space(space)));
        let picking =
            self.picker_pending.map(|(picker, _)| picker) == asked || self.holds_icon_place();
        !self.lone_header && self.lone_unnamed() && !naming && !picking
    }

    /// How tall the list's head is: the name row, or what Arc leaves of it once it hides.
    pub(super) fn list_head(&self) -> f32 {
        match self.name_row_hidden() {
            false => NAME_HEIGHT,
            true if self.tile_tabs().is_empty() => NAME_HIDDEN,
            true => NAME_HIDDEN_UNDER_FAVORITES,
        }
    }

    /// Show Space Header or Hide Space Header, from a lone unnamed Space's menu: every window's,
    /// kept for the next launch.
    pub(super) fn keep_lone_header(&mut self, kept: bool, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!(
            "browser: space header {}",
            if kept { "kept" } else { "hidden" }
        );
        cx.set_global(LoneSpaceHeader(kept));
        self.lone_header = kept;
        self.browser.keep_lone_header(kept, cx);
        // check.sh waits for the outline the rows are drawn with next.
        self.outline_changed = true;
        cx.refresh_windows();
    }

    /// The line over Today, and Clear at its end, where the titles end.
    pub(super) fn divider(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let live = self.tools_live();
        let clear = self.tools_shown().then(|| {
            let arrow = if self.clears_last() { ARROW_DOWN } else { NONE };
            let what = Swapped::Clear(self.space);
            let room = self.swaps.toward(what, ARROW_DOWN);
            let room = room.unwrap_or(if arrow == NONE { 0. } else { 1. });
            // Fading out, it takes no click and doesn't light up.
            let color = if live && self.clear_hovered {
                CLEAR_HOVERED
            } else {
                CLEAR
            };
            let glyph = self
                .swaps
                .glyph(what, arrow, CLEAR_ARROW, self.palette.text(color), cx);
            let clear = clear_label(&self.palette, glyph, room, color)
                .id("clear")
                .opacity(self.tools_opacity());
            let clear = clear.when(self.a11y, |clear| {
                clear.accessibility_id("clear").button_named("Clear Today")
            });
            let clear = clear.child(self.button_frame(Command::Clear.name()));
            clear.when(live, |clear| {
                clear
                    .block_mouse_except_scroll()
                    .on_hover(
                        cx.listener(|shell, hovered: &bool, _, cx| shell.hover_clear(*hovered, cx)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|shell, _, _, cx| shell.browser.tooltip(None, cx)),
                    )
                    .on_click(
                        cx.listener(|shell, _, _, cx| shell.run(Command::Clear, "sidebar", cx)),
                    )
            })
        });
        divider_box(&self.palette, self.hairline(), clear).top(px(self.slide(Slot::Divider)))
    }

    /// The pointer has come onto Clear or left it: it lights up, and after a moment says what it
    /// does in Arc's words, hung under it as a header button's tooltip is.
    fn hover_clear(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.clear_hovered = hovered;
        // check.sh waits for this line.
        log!("browser: clear hovered {}", hovered as u8);
        let busy = self.dragging().is_some() || self.menu_row.is_some();
        let frame = self
            .button_frames
            .borrow()
            .get(Command::Clear.name())
            .copied();
        let tip = frame.filter(|_| hovered && !busy).map(|frame| {
            (
                Tip::named(CLEAR_TIP.into()),
                Anchor::under(frame, self.viewport),
            )
        });
        self.hover_tip(tip, cx);
        cx.notify();
    }

    /// Whether the Space shown shows only its Today tabs, which Clear then closes.
    pub(super) fn clears_last(&self) -> bool {
        self.shown_space()
            .is_some_and(|space| space.clears_last(self.selected))
    }

    /// Starts the arrow coming or going as Clear starts or stops closing the last Today tabs, on
    /// the divider drawn before. Another Space's, or a divider just drawn, has its own at once.
    pub(super) fn step_clear_arrow(&mut self) {
        let arrow = (!self.today().is_empty()).then(|| (self.space, self.clears_last()));
        if arrow == self.drawn_arrow {
            return;
        }
        if let Some((space, is)) = arrow {
            match self.drawn_arrow {
                Some((was_in, was)) if was_in == space => {
                    let glyph = |arrow: bool| if arrow { ARROW_DOWN } else { NONE };
                    self.swaps
                        .start(Swapped::Clear(space), glyph(was), glyph(is), Instant::now());
                }
                _ => self.swaps.end(Swapped::Clear(space)),
            }
        }
        let shows = |arrow: Option<(u64, bool)>| arrow.is_some_and(|(_, shows)| shows);
        if shows(arrow) != shows(self.drawn_arrow) {
            // check.sh waits for this line.
            log!("browser: clear arrow {}", shows(arrow) as u8);
        }
        self.drawn_arrow = arrow;
    }

    pub(super) fn new_tab_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (plus, label) = new_tab_label(&self.palette);
        // ⌥-click opens the new tab beside the page, as Dia's does.
        self.row(Row::NewTab, plate(plus, label), false, cx)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.pressed_bar_target(cx)),
            )
            .on_click(cx.listener(|shell, event: &ClickEvent, _, cx| {
                let command = if event.modifiers().alt {
                    Command::AddSplit
                } else {
                    Command::NewTab
                };
                shell.clicked_bar_target(command, "sidebar", cx)
            }))
    }
}

/// The speaker after a playing tab's favicon, crossed out while it's muted.
pub(super) fn speaker_glyph(muted: bool) -> &'static str {
    if muted {
        "icons/volume-x.svg"
    } else {
        "icons/volume-2.svg"
    }
}

// How often the dots are drawn. GPUI can't redraw only a dot, so each frame is the whole shell
// again; 30 ms lands on every second display frame at 60 Hz and every fourth at 120.
const LOADING_FRAME: Duration = Duration::from_millis(30);
// Dots left undrawn this long start their breath again.
const LOADING_GAP: Duration = Duration::from_millis(200);

/// The loading dots' clock: every dot breathes with it, and one timer redraws them all while one
/// is drawn, where an animation for each would ask for the shell at every display frame. With no
/// page loading nothing asks for it, so an idle browser draws nothing.
#[derive(Default)]
pub(super) struct Breath {
    /// When the breath began that the dots are on, and when a dot was last drawn.
    since: Cell<Option<Instant>>,
    last: Cell<Option<Instant>>,
    /// The frame being built draws a loading dot, and a timer is out to draw the next.
    drawn: Cell<bool>,
    pending: Cell<bool>,
    /// Motion is reduced, so the dots hold still at full strength.
    still: Cell<bool>,
}

impl Breath {
    /// How strong a loading dot drawn now is.
    pub(super) fn strength(&self) -> f32 {
        self.through(BREATH).map_or(1., breath_strength)
    }

    /// How far round a loading ring drawn now is, as a fraction of a turn each `turn`: none while
    /// motion is reduced.
    pub(super) fn turned(&self, turn: Duration) -> f32 {
        self.through(turn).unwrap_or(0.)
    }

    /// How far through its `period` something drawn now on this clock is, or None while motion is
    /// reduced and it holds still.
    fn through(&self, period: Duration) -> Option<f32> {
        self.drawn.set(true);
        if self.still.get() {
            return None;
        }
        let now = Instant::now();
        if self
            .last
            .replace(Some(now))
            .is_none_or(|last| now.duration_since(last) > LOADING_GAP)
        {
            self.since.set(Some(now));
        }
        let since = self.since.get().unwrap_or(now);
        Some((now.duration_since(since).as_secs_f32() / period.as_secs_f32()).fract())
    }

    /// Something drawn now moves on this clock, as a loading dot does.
    pub(super) fn keep(&self) {
        self.drawn.set(true);
    }

    /// Whether a timer is out to draw the next frame.
    #[cfg(test)]
    pub(super) fn pending(&self) -> bool {
        self.pending.get()
    }

    /// Once a frame is built: if it drew a loading dot, draws the next in a moment.
    pub(super) fn arm(&self, cx: &mut Context<Shell>) {
        self.still.set(cx.reduce_motion());
        if !self.drawn.take() || self.still.get() || self.pending.replace(true) {
            return;
        }
        cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(LOADING_FRAME).await;
            shell
                .update(cx, |shell, cx| {
                    shell.breath.pending.set(false);
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}

/// A dot's strength `phase` of the way through its breath: from BREATH_LOW up to full half way
/// and back, eased in and out.
fn breath_strength(phase: f32) -> f32 {
    BREATH_LOW + (1. - BREATH_LOW) * (1. - (std::f32::consts::TAU * phase).cos()) / 2.
}

impl Shell {
    /// A tab's status dot, in the slot at its row's end, while its page loads: the row's text,
    /// breathing.
    fn status_dot(&self, tab: u64) -> Option<impl IntoElement + use<>> {
        if !self.page(tab).loading {
            return None;
        }
        let dot = div()
            .size(px(STATUS_DOT))
            .rounded_full()
            .bg(self.palette.text(STATUS_LOADING));
        let strength = self.breath.strength();
        Some(
            div()
                .flex_none()
                .ml(px(4.))
                .size(px(LOADING_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .child(dot.opacity(strength)),
        )
    }
}

/// An emoji in a favicon's place.
pub(super) fn emoji(icon: SharedString) -> AnyElement {
    div()
        .text_size(px(EMOJI_SIZE))
        .line_height(px(FAVICON_SIZE))
        .child(icon)
        .into_any_element()
}

/// The glyph for what `page` captures or shares: sharing a screen, a window or a tab, or being
/// shared, then the camera, then the microphone alone.
pub(super) fn capture_glyph(page: &Page) -> Option<&'static str> {
    if page.sharing || page.shared {
        Some("icons/screen-share.svg")
    } else if page.camera {
        Some("icons/video.svg")
    } else if page.microphone {
        Some("icons/mic.svg")
    } else {
        None
    }
}

/// A row's title with its host under it, the two centred in the row together.
fn with_host(palette: &Palette, title: gpui::Div, host: SharedString) -> gpui::Div {
    let host = div()
        .truncate()
        .text_size(px(text::SMALL))
        .line_height(px(HOST_LINE))
        .text_color(palette.text(SECONDARY))
        .child(host);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .justify_center()
        .child(title.flex_none().line_height(px(HOST_TITLE_LINE)))
        .child(host)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        rc::Rc,
        time::{Duration, Instant},
    };

    use gpui::{
        Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext,
        VisualTestContext,
        accesskit::{Action, Role},
        point, px,
    };

    use super::{
        super::{
            CLOSE_RIGHT, CLOSE_SIZE, FAVICON_SIZE, FAVICON_X, Folders, NAME_HEIGHT, PINNED_TURN,
            PLATE_INSET, PRESS_SCALE, PRESS_TIME, Page, ROW_HEIGHT, ROW_PITCH, Row,
            SELECTED_UNHOVER, SPACE_ICON_PLATE, SPACE_MORE, SPACE_MORE_INSET, Shell, Slot, Swapped,
            commands::{Command, bind_keys},
            drag::detents::shell_with,
            recorder::Recorder,
        },
        BREATH, BREATH_LOW, Breath, CLOSE, LOADING_GAP, Pending, Subject, breath_strength,
    };
    use crate::icon_picker::{self, Picked, Recents};

    /// A loading dot breathes from BREATH_LOW to full half a breath in, eased, and back, all
    /// together; left undrawn for a while they start again, and with reduced motion they hold at
    /// full strength.
    #[test]
    fn the_loading_dots_breathe_together() {
        let breath = Breath::default();
        assert!((breath.strength() - BREATH_LOW).abs() < 0.01);
        assert!((breath_strength(0.25) - (1. + BREATH_LOW) / 2.).abs() < 0.001);
        assert!((breath_strength(1.) - BREATH_LOW).abs() < 0.001);
        breath.since.set(Some(Instant::now() - BREATH / 2));
        let (first, second) = (breath.strength(), breath.strength());
        assert!(first > 0.99, "half a breath in, {first}");
        assert!((second - first).abs() < 0.01);

        breath.last.set(Some(
            Instant::now() - LOADING_GAP - Duration::from_millis(1),
        ));
        assert!(
            breath.strength() < BREATH_LOW + 0.01,
            "undrawn for a while, it starts over"
        );

        breath.since.set(Some(Instant::now() - BREATH / 4));
        breath.still.set(true);
        assert_eq!(breath.strength(), 1.);
    }

    /// A row's hover fill comes and goes at once, and only the selected row's eases back, from
    /// full to none over SELECTED_UNHOVER, as Arc's does.
    #[gpui::test]
    fn only_the_selected_rows_hover_eases_away(cx: &mut TestAppContext) {
        let (shell, _) = shell_with(cx, &[], &[1, 2], Folders::default());
        let fill = |shell: &Entity<Shell>, cx: &mut TestAppContext, tab| {
            shell.update(cx, |shell, _| shell.hover_fill(Row::Tab(tab)))
        };
        shell.update(cx, |shell, cx| shell.set_selected(1, cx));
        shell.update(cx, |shell, cx| shell.hover(Row::Tab(2), true, cx));
        assert_eq!(fill(&shell, cx, 2), Some(1.));
        shell.update(cx, |shell, cx| shell.hover(Row::Tab(2), false, cx));
        assert_eq!(fill(&shell, cx, 2), None);

        shell.update(cx, |shell, cx| shell.hover(Row::Tab(1), true, cx));
        assert_eq!(fill(&shell, cx, 1), Some(1.));
        shell.update(cx, |shell, cx| shell.hover(Row::Tab(1), false, cx));
        let easing = fill(&shell, cx, 1).unwrap_or(0.);
        assert!(
            easing > 0.9,
            "just left, the selected row's fill is {easing}"
        );
        shell.update(cx, |shell, _| {
            for (_, start) in &mut shell.unhovered {
                *start -= Duration::from_secs_f32(SELECTED_UNHOVER);
            }
        });
        assert_eq!(fill(&shell, cx, 1), Some(0.));
    }

    /// The Space's icon answers the pointer on its plate, Arc's 26 pt frame, as a header button
    /// does: a press anywhere on the plate opens the picker, even past the glyph, and one above
    /// it, still in the name's row, doesn't.
    #[gpui::test]
    fn the_space_icon_opens_the_picker_from_its_plate(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (
                1,
                "Space".into(),
                None,
                1,
                "🌱".into(),
                "".into(),
                "".into(),
            );
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
        });
        cx.run_until_parked();
        let row = shell
            .read_with(cx, |shell, _| shell.name_row_drawn.get())
            .expect("a name row");
        let middle = point(
            row.left() + px(FAVICON_X + FAVICON_SIZE / 2.),
            row.center().y,
        );
        // Whether a press at `at` asked for the picker.
        let press = |cx: &mut VisualTestContext, at| {
            cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
            let asked = shell.update(cx, |shell, _| shell.picker_pending.take().is_some());
            cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            asked
        };
        let edge = SPACE_ICON_PLATE / 2. - 1.5;
        assert!(
            press(cx, middle + point(px(edge), px(0.))),
            "on the plate, past the glyph"
        );
        let above = (SPACE_ICON_PLATE / 2. + NAME_HEIGHT / 2.) / 2.;
        assert!(
            !press(cx, middle - point(px(0.), px(above))),
            "above the plate"
        );
    }

    /// The … at the end of the Space's name row, under the pointer, opens the Space's menu where
    /// it's pressed, as a right-click on the name does (#1090), with the pinned rows' caret beside
    /// it or without.
    #[gpui::test]
    fn the_name_rows_edit_button_shows_the_spaces_menu(cx: &mut TestAppContext) {
        for (pinned, a11y) in [(&[][..], false), (&[5][..], false), (&[5][..], true)] {
            let host = Rc::new(Recorder::default());
            let browser = host.clone();
            let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
            if a11y {
                cx.simulate_a11y_activation();
            }
            shell.update(cx, |shell, cx| {
                let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
                shell.set_spaces(vec![space], cx);
                shell.set_space(1, cx);
                shell.set_tabs(
                    1,
                    pinned.to_vec(),
                    vec![2],
                    Vec::new(),
                    Folders::default(),
                    cx,
                );
                shell.set_selected(2, cx);
            });
            cx.run_until_parked();
            let row = shell
                .read_with(cx, |shell, _| shell.name_row_drawn.get())
                .expect("a row");
            let at = point(row.right() - px(MORE_END), row.center().y);
            cx.simulate_mouse_move(at, None, Modifiers::none());
            cx.run_until_parked();
            host.menus.take();
            host.take();
            cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            let menus = host.menus.take();
            let titles: Vec<_> = (menus.last().into_iter().flatten())
                .filter(|(_, on)| *on)
                .map(|(title, _)| title.as_str())
                .collect();
            assert_eq!(
                titles.first().copied(),
                Some("Change Space Icon…"),
                "pinned {pinned:?}, a11y {a11y}"
            );
            assert_eq!(
                host.menu_at.get(),
                Some(at),
                "pinned {pinned:?}, a11y {a11y}"
            );
            let calls = host.take();
            assert!(
                !calls.iter().any(|call| call.starts_with("pinned open")),
                "{calls:?}"
            );
        }
    }

    /// The click that makes the window active, which GPUI on the Mac takes as its first mouse,
    /// shows the Space's menu from the … as it selects a tab from its row (#1090).
    #[gpui::test]
    fn the_click_that_activates_the_window_reaches_the_edit_button_as_a_row(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4], 4);
        let first_click = |cx: &mut VisualTestContext, position| {
            cx.simulate_mouse_move(position, None, Modifiers::none());
            cx.deactivate_window();
            let (button, modifiers) = (MouseButton::Left, Modifiers::none());
            cx.simulate_event(MouseDownEvent {
                position,
                button,
                modifiers,
                click_count: 1,
                first_mouse: true,
            });
            cx.simulate_event(MouseUpEvent {
                position,
                button,
                modifiers,
                click_count: 1,
            });
            cx.run_until_parked();
        };
        let (x, y) = close_button(&shell, cx, 2);
        host.take();
        first_click(cx, point(px(x - 100.), px(y)));
        assert!(
            host.take().contains(&"select 2".into()),
            "the first click on a row"
        );

        let row = shell
            .read_with(cx, |shell, _| shell.name_row_drawn.get())
            .expect("a name row");
        let at = point(row.right() - px(MORE_END), row.center().y);
        host.menus.take();
        first_click(cx, at);
        let menus = host.menus.take();
        let first = menus
            .last()
            .and_then(|menu| menu.first())
            .map(|(title, _)| title.clone());
        assert_eq!(
            first.as_deref(),
            Some("Change Space Icon…"),
            "the first click on the …"
        );
    }

    /// The ••• 's middle, in from the name row's end: its square's end is 6 in from the plate's.
    const MORE_END: f32 = PLATE_INSET + SPACE_MORE_INSET + SPACE_MORE / 2.;

    /// A Space with an icon whose pinned rows can collapse, shown with a Today tab.
    fn header_shell(
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (
                1,
                "Space".into(),
                None,
                1,
                "🌱".into(),
                "".into(),
                "".into(),
            );
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, vec![5, 6], vec![2], Vec::new(), Folders::default(), cx);
            shell.set_selected(2, cx);
        });
        cx.run_until_parked();
        (shell, cx, host)
    }

    /// A press anywhere on the name row but its •••, the name and the icon's slot too, collapses
    /// the pinned rows, and the next expands them (Arc 1.171), each turning the chevron a quarter
    /// over PINNED_TURN. A double-click no longer renames the Space.
    #[gpui::test]
    fn a_press_on_the_name_row_folds_the_pinned_rows(cx: &mut TestAppContext) {
        let (shell, cx, host) = header_shell(cx);
        let row = shell
            .read_with(cx, |shell, _| shell.name_row_drawn.get())
            .expect("a name row");
        let slot = point(
            row.left() + px(FAVICON_X + FAVICON_SIZE / 2.),
            row.center().y,
        );
        let name = point(row.left() + px(80.), row.center().y);
        for (at, open) in [(slot, false), (name, true)] {
            cx.simulate_mouse_move(at, None, Modifiers::none());
            host.take();
            cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            assert!(
                host.take().contains(&format!("pinned open 1 {open}")),
                "at {at:?}"
            );
            shell.update(cx, |shell, cx| {
                shell.set_tabs(1, vec![5, 6], vec![2], Vec::new(), folded(!open), cx)
            });
            cx.run_until_parked();
            let turn = shell.read_with(cx, |shell, _| shell.pinned_turn);
            assert!(turn.is_some_and(|start| start.elapsed().as_secs_f32() < PINNED_TURN));
        }
        assert!(
            shell.read_with(cx, |shell, _| shell.renaming.is_none()),
            "a press renamed"
        );
    }

    fn folded(shut: bool) -> Folders {
        Folders {
            pinned_shut: shut,
            ..Folders::default()
        }
    }

    /// Under the pointer the row is lit, its chevron in the icon's place and its ••• showing, all
    /// at once; the menu the ••• opens keeps them while it's up.
    #[gpui::test]
    fn the_name_row_lights_under_the_pointer_and_keeps_it_for_its_menu(cx: &mut TestAppContext) {
        let (shell, cx, host) = header_shell(cx);
        let row = shell
            .read_with(cx, |shell, _| shell.name_row_drawn.get())
            .expect("a name row");
        let lit = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                shell.name_hovered || shell.name_menu && shell.menu_open
            })
        };
        assert!(!lit(cx), "lit before the pointer came");
        let more = point(row.right() - px(MORE_END), row.center().y);
        cx.simulate_mouse_move(more, None, Modifiers::none());
        cx.run_until_parked();
        assert!(lit(cx), "the pointer over the row didn't light it");
        cx.simulate_mouse_down(more, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(more, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
        cx.run_until_parked();
        assert!(lit(cx), "the row went dark under its menu");
        // Arc 1.171's order, with Remove Icon (ours) after what it undoes and Export as Markdown…
        // where Arc shares and exports; the lines between its groups.
        let top: Vec<String> = (host.entries.borrow().iter())
            .filter(|entry| entry.parent.is_none())
            .map(|entry| {
                if entry.title.is_empty() {
                    "—".into()
                } else {
                    entry.title.to_string()
                }
            })
            .collect();
        let order = [
            "Change Space Icon…",
            "Remove Icon",
            "Rename Space…",
            "Edit Theme Color…",
            "Set Profile",
            "—",
            "New Folder…",
            "—",
            "Export as Markdown…",
            "—",
            "Manage Spaces…",
            "—",
            "Delete Space",
        ];
        assert_eq!(top, order);
        cx.update(|_, cx| host.choose("Manage Spaces…", cx));
        cx.run_until_parked();
        assert!(!lit(cx), "the row stayed lit after its menu closed");
        let library = shell.read_with(cx, |shell, _| shell.library_open);
        assert!(library, "Manage Spaces… didn't open the Library");
    }

    /// Collapsed, the name row's chevron stays with the pointer gone, so the way back shows. The
    /// sidebar is at rest here: closing the Library just before would leave it faded out, and
    /// undrawn, for as long as the fade back waits, which reads the wall clock.
    #[gpui::test]
    fn the_chevron_stays_for_collapsed_rows_with_the_pointer_gone(cx: &mut TestAppContext) {
        let (shell, cx, _) = header_shell(cx);
        let row = shell
            .read_with(cx, |shell, _| shell.name_row_drawn.get())
            .expect("a name row");
        cx.simulate_mouse_move(row.center(), None, Modifiers::none());
        cx.run_until_parked();
        cx.simulate_mouse_move(point(px(600.), px(400.)), None, Modifiers::none());
        cx.run_until_parked();
        assert!(!shell.read_with(cx, |shell, _| shell.name_hovered));
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![5, 6], vec![2], Vec::new(), folded(true), cx)
        });
        cx.run_until_parked();
        let caret = shell.read_with(cx, |shell, _| shell.caret_drawn.get());
        assert!(
            caret.is_some_and(|(shut, _)| shut),
            "no chevron for the collapsed rows"
        );
    }

    /// Rename Space… puts Done in the •••'s place, and a press on it keeps the name typed.
    #[gpui::test]
    fn done_keeps_the_spaces_new_name(cx: &mut TestAppContext) {
        let (shell, cx, host) = header_shell(cx);
        shell.update(cx, |shell, cx| shell.rename_space(1, cx));
        cx.run_until_parked();
        cx.simulate_input("Work");
        let row = shell
            .read_with(cx, |shell, _| shell.name_row_drawn.get())
            .expect("a name row");
        let done = point(
            row.right() - px(PLATE_INSET + SPACE_MORE_INSET + 16.),
            row.center().y,
        );
        host.take_edits();
        cx.simulate_mouse_down(done, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(done, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let edits = host.take_edits();
        assert!(
            edits.contains(&"rename space 1 Work".into()),
            "Done kept {edits:?}"
        );
        assert!(
            shell.read_with(cx, |shell, _| shell.renaming.is_none()),
            "the field stayed"
        );
    }

    /// Change Icon… on a tab asks for the icon picker under that tab's row, which draws where the
    /// picker is to hang: only the row asked for tells where it is.
    #[gpui::test]
    fn change_icon_has_the_tabs_row_say_where_it_is(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
        });
        report(&shell, cx, vec![2, 3], 2);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.tab_row_drawn.get()),
            None
        );

        shell.update(cx, |shell, cx| shell.change_icon(3, cx));
        cx.run_until_parked();
        let (pending, row) = shell.read_with(cx, |shell, _| {
            (
                shell.picker_pending.map(|(picker, _)| picker),
                shell.tab_row_drawn.get(),
            )
        });
        assert_eq!(pending, Some(Pending::Icon(Subject::Tab(3))));
        let row = row.expect("the tab's row didn't say where it is");
        // The other row is a row's pitch off.
        shell.update(cx, |shell, cx| shell.change_icon(2, cx));
        cx.run_until_parked();
        let first = shell
            .read_with(cx, |shell, _| shell.tab_row_drawn.get())
            .unwrap();
        assert_eq!(
            f32::from(row.center().y - first.center().y).abs(),
            ROW_PITCH
        );
    }

    /// A favorite's Change Icon… has its tile say where it is, for the picker to hang under its
    /// middle.
    #[gpui::test]
    fn change_icon_on_a_favorite_has_its_tile_say_where_it_is(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_favorites(vec![(20, 1), (21, 1)], cx);
        });
        report(&shell, cx, vec![2], 2);
        shell.update(cx, |shell, cx| shell.change_icon(21, cx));
        cx.run_until_parked();
        let tile = shell.read_with(cx, |shell, _| shell.tab_row_drawn.get());
        let tile = tile.expect("the favorite's tile didn't say where it is");
        // Inside the tile's hairline.
        let height = f32::from(tile.size.height);
        assert!(
            (height - crate::tokens::shell::TILE_HEIGHT).abs() <= 1.,
            "{height} tall"
        );
        // The first tile is a tile's pitch left of it.
        shell.update(cx, |shell, cx| shell.change_icon(20, cx));
        cx.run_until_parked();
        let first = shell
            .read_with(cx, |shell, _| shell.tab_row_drawn.get())
            .unwrap();
        let places = shell.read_with(cx, |shell, _| shell.tile_places(2));
        let pitch = f32::from(tile.center().x - first.center().x);
        assert!(
            (pitch - (places[1][0] - places[0][0])).abs() < 0.5,
            "{pitch} apart"
        );
    }

    /// An icon picker's window that shows nowhere.
    struct NoWhere;

    impl icon_picker::Host for NoWhere {
        fn show(&self, _: u64, _: f32, _: f32) {}

        fn hide(&self) {}

        fn keep_recents(&self, _: &str) {}
    }

    /// While the icon picker shows for a Space with no icon, its name row holds the icon's place,
    /// the name after it as after an icon. It lets go once the picker closes, and the Space's icon
    /// takes the place once it has one.
    #[gpui::test]
    fn the_name_row_holds_the_icons_place_while_the_picker_shows_for_a_space_with_none(
        cx: &mut TestAppContext,
    ) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(|cx| icon_picker::open(Rc::new(NoWhere), Recents::default(), cx));
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let spaces = |icon: &str| {
            vec![(
                1,
                "Space".into(),
                None,
                1,
                icon.to_string().into(),
                "".into(),
                "".into(),
            )]
        };
        shell.update(cx, |shell, cx| {
            shell.set_spaces(spaces(""), cx);
            shell.set_space(1, cx);
        });
        let holds =
            |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.holds_icon_place());
        let show = |cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| {
                let picked: Picked = Rc::new(|_, _| {});
                icon_picker::show(1, Subject::Space(1), "".into(), point(0., 0.), picked, cx);
                shell.hold_icon_place(1, cx);
            });
            cx.run_until_parked();
        };
        assert!(!holds(cx));
        show(cx);
        assert!(holds(cx));
        cx.update(|_, cx| icon_picker::dismissed(cx));
        cx.run_until_parked();
        assert!(!holds(cx), "the picker closed");

        show(cx);
        shell.update(cx, |shell, cx| shell.set_spaces(spaces("glyph:moon"), cx));
        assert!(!holds(cx), "the icon took its place");
    }

    /// The window, showing a Space whose Today tabs the core reports as `today`, with `shown`.
    fn report(shell: &Entity<Shell>, cx: &mut VisualTestContext, today: Vec<u64>, shown: u64) {
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx);
            shell.set_selected(shown, cx);
        });
        cx.run_until_parked();
    }

    /// Whether the window drew Clear with its arrow, without it, or no Clear, and whether the
    /// arrow was coming or going.
    fn arrow(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Option<bool>, bool) {
        shell.read_with(cx, |shell, _| {
            let drawn = shell.drawn_arrow.map(|(_, arrow)| arrow);
            (drawn, shell.swaps.changing(Swapped::Clear(1)))
        })
    }

    /// With two Today tabs, Clear keeps the one shown and has no arrow. Once one is left, the arrow
    /// comes before Clear, and Clear closes that one too. ⌘Z brings it back with its arrow, shown
    /// at once on the divider drawn again, and a second tab takes it away until it closes.
    #[gpui::test]
    fn clear_shows_its_arrow_when_one_today_tab_is_left(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(bind_keys);
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
        });
        // The keys reach the shell as they do with the UI focused.
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        report(&shell, cx, vec![2, 3], 2);
        assert_eq!(arrow(&shell, cx), (Some(false), false));

        cx.simulate_keystrokes("shift-cmd-k");
        assert!(host.take().contains(&"clear".to_string()));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.clearing.clone()),
            vec![3]
        );
        report(&shell, cx, vec![2], 2);
        assert_eq!(
            arrow(&shell, cx),
            (Some(true), true),
            "the arrow comes on the same Clear"
        );

        cx.simulate_keystrokes("shift-cmd-k");
        assert!(host.take().contains(&"clear".to_string()));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.clearing.clone()),
            vec![2]
        );
        report(&shell, cx, Vec::new(), 0);
        assert_eq!(arrow(&shell, cx).0, None);

        cx.simulate_keystrokes("cmd-z");
        assert!(host.take().contains(&"undo clear".to_string()));
        report(&shell, cx, vec![2], 2);
        assert_eq!(
            arrow(&shell, cx),
            (Some(true), false),
            "a divider drawn again has it at once"
        );

        // A tab that opens takes it away again, on the same Clear, and closing it brings it back.
        report(&shell, cx, vec![4, 2], 4);
        assert_eq!(arrow(&shell, cx), (Some(false), true));
        report(&shell, cx, vec![2], 2);
        assert_eq!(arrow(&shell, cx), (Some(true), true));
    }

    /// Under Reduce Motion the rows are in their places at once, and the frame after a move still
    /// sees it end, which check.sh waits for, when the core's next answer with the same lists
    /// comes before that frame, as a ⌘Z's for each tab it brings back do (#1380).
    #[gpui::test]
    fn under_reduce_motion_rows_moved_before_a_frame_still_settle(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx, vec![10, 11], vec![20, 21], 20);
        shell.update(cx, |shell, cx| {
            cx.set_reduce_motion(true);
            for _ in 0..2 {
                shell.set_tabs(
                    1,
                    vec![10],
                    vec![11, 20, 21],
                    Vec::new(),
                    Folders::default(),
                    cx,
                );
            }
            assert_eq!(
                shell.slide(Slot::Tab(11)),
                0.,
                "the row slid under Reduce Motion"
            );
            assert!(
                !shell.slides.is_empty(),
                "the second answer took the move from its frame"
            );
            cx.notify();
        });
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert!(shell.slides.is_empty(), "the rows didn't settle")
        });
    }

    /// A window showing one Space with `pinned` and `today` tabs and `shown`.
    fn window(
        cx: &mut TestAppContext,
        pinned: Vec<u64>,
        today: Vec<u64>,
        shown: u64,
    ) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        window_with(cx, pinned, today, shown, Folders::default())
    }

    /// `window`, with `folders` among the pinned tabs.
    fn window_with(
        cx: &mut TestAppContext,
        pinned: Vec<u64>,
        today: Vec<u64>,
        shown: u64,
        folders: Folders,
    ) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, pinned, today, Vec::new(), folders, cx);
            shell.set_selected(shown, cx);
        });
        cx.run_until_parked();
        (shell, cx, host)
    }

    /// Where `tab`'s row's close button is in the window, and its row's middle.
    fn close_button(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) -> (f32, f32) {
        shell.read_with(cx, |shell, _| {
            let top = shell.drawn_list_top() + shell.layout()[&Slot::Tab(tab)];
            let x = shell.width() - PLATE_INSET - CLOSE_RIGHT - CLOSE_SIZE / 2.;
            (x, top + ROW_HEIGHT / 2.)
        })
    }

    fn move_to(cx: &mut VisualTestContext, (x, y): (f32, f32)) {
        cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
        cx.run_until_parked();
    }

    /// The glyph the close button of the row under the pointer shows.
    fn hovered_glyph(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<&'static str> {
        shell.read_with(cx, |shell, _| match shell.hovered {
            Some(Row::Tab(row)) => Some(shell.close_glyph(row)),
            _ => None,
        })
    }

    /// One shape per action (owner, 2026-11-02, #1349): under the pointer, a loaded pinned row's
    /// button is −, selected or not, and a click on it unloads the tab with the call that closes
    /// one, without showing it. A Today row's is ✕, selected or not.
    #[gpui::test]
    fn a_loaded_pinned_row_shows_minus_and_a_today_row_x(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4, 5], 2);
        let minus = close_button(&shell, cx, 3);
        move_to(cx, minus);
        assert!(shell.read_with(cx, |shell, _| shell.hovered == Some(Row::Tab(3))));
        assert_eq!(drawn_button(&shell, cx), "−", "a pinned row not selected");

        host.take();
        cx.simulate_click(point(px(minus.0), px(minus.1)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["close 3"]);

        let selected = close_button(&shell, cx, 2);
        move_to(cx, selected);
        assert_eq!(drawn_button(&shell, cx), "−", "the selected pinned row");
        let today = close_button(&shell, cx, 5);
        move_to(cx, today);
        assert_eq!(drawn_button(&shell, cx), "1", "a Today row");
        shell.update(cx, |shell, cx| shell.set_selected(5, cx));
        assert_eq!(
            hovered_glyph(&shell, cx),
            Some(CLOSE),
            "a selected Today row"
        );
    }

    /// A loaded pinned row in a folder, and a pinned split's row, show − as well.
    #[gpui::test]
    fn pinned_rows_in_a_folder_or_a_split_show_minus(cx: &mut TestAppContext) {
        let folders = Folders {
            folders: HashMap::from([(5, ("Reading".into(), true))]),
            parents: HashMap::from([(6, 5)]),
            ..Default::default()
        };
        let (shell, cx, _) = window_with(cx, vec![2, 5, 6, 7, 8], vec![4], 4, folders.clone());
        let in_folder = close_button(&shell, cx, 6);
        move_to(cx, in_folder);
        assert_eq!(drawn_button(&shell, cx), "−", "a pinned row in a folder");

        shell.update(cx, |shell, cx| {
            let split = crate::shell::Split {
                tabs: vec![7, 8],
                ..Default::default()
            };
            shell.set_tabs(1, vec![2, 5, 6, 7, 8], vec![4], vec![split], folders, cx)
        });
        cx.run_until_parked();
        let split = close_button(&shell, cx, 7);
        move_to(cx, split);
        assert_eq!(drawn_button(&shell, cx), "−", "a pinned split's row");
    }

    /// A pressed row's plate shrinks to PRESS_SCALE over PRESS_TIME while it's held and grows
    /// back over as long once it's let go. Once it comes away to be dragged, it's full size.
    #[gpui::test]
    fn a_pressed_row_shrinks_a_little_until_it_is_let_go(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx, Vec::new(), vec![2, 3], 2);
        let (x, y) = close_button(&shell, cx, 3);
        let middle = point(px(x - 100.), px(y));
        let scale = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| shell.press_scale(3))
        };
        // As if the press were `ago` seconds back, and the release `let_go` seconds back.
        let back = |shell: &Entity<Shell>, cx: &mut VisualTestContext, ago: f32, let_go: f32| {
            shell.update(cx, |shell, _| {
                let held = shell.held_row.as_mut().unwrap();
                let now = Instant::now();
                held.pressed = now - Duration::from_secs_f32(ago);
                held.let_go = held.let_go.map(|_| now - Duration::from_secs_f32(let_go));
            })
        };
        assert_eq!(scale(&shell, cx), 1.);
        cx.simulate_mouse_down(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        back(&shell, cx, PRESS_TIME / 2., 0.);
        let half = scale(&shell, cx);
        assert!(half < 1. && half > PRESS_SCALE, "half way in, {half}");
        back(&shell, cx, PRESS_TIME * 2., 0.);
        assert_eq!(scale(&shell, cx), PRESS_SCALE);

        cx.simulate_mouse_up(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        back(&shell, cx, PRESS_TIME * 3., PRESS_TIME / 2.);
        let out = scale(&shell, cx);
        assert!(out < 1. && out > PRESS_SCALE, "half way out, {out}");
        back(&shell, cx, PRESS_TIME * 3., PRESS_TIME);
        assert_eq!(scale(&shell, cx), 1.);

        cx.simulate_mouse_down(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        back(&shell, cx, PRESS_TIME * 2., 0.);
        assert_eq!(scale(&shell, cx), PRESS_SCALE);
        let away = point(middle.x, middle.y + px(30.));
        cx.simulate_mouse_move(away, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.dragging().is_some()));
        assert_eq!(scale(&shell, cx), 1.);
    }

    /// A press on a row shows its tab at once, as Arc's does, before it's let go, and gives its
    /// page the keyboard, as Chrome's tab strip does; letting go asks for nothing more. A press on
    /// the row shown gives its page the keyboard too. A row's − or ✕ closes its tab without showing
    /// it, and a right-click shows the row's menu without showing its tab or moving the keyboard.
    #[gpui::test]
    fn a_press_shows_its_tab_before_it_is_let_go(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2], vec![3, 4], 3);
        let (x, y) = close_button(&shell, cx, 4);
        let middle = point(px(x - 100.), px(y));
        host.take();
        cx.simulate_mouse_down(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["card close", "select 4", "focus page"]);
        cx.simulate_mouse_up(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().is_empty(), "letting go");
        shell.update(cx, |shell, cx| shell.set_selected(4, cx));
        cx.run_until_parked();
        cx.simulate_click(middle, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["card close", "focus page"], "the row shown");

        let close = close_button(&shell, cx, 2);
        move_to(cx, close);
        cx.simulate_click(point(px(close.0), px(close.1)), Modifiers::none());
        cx.run_until_parked();
        // The pointer left the row whose card the press closed.
        assert_eq!(
            host.take(),
            ["card left 4", "close 2"],
            "the − of a pinned row not shown"
        );

        let middle = point(px(close.0 - 100.), px(close.1));
        cx.simulate_mouse_down(middle, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert!(
            !calls
                .iter()
                .any(|call| ["select 2", "focus page"].contains(&call.as_str()))
        );
    }

    /// A click on a row while the rows still slide, as they do once a link opens a tab under its
    /// opener, selects the row resting under it, pressed and let go before a frame as the Mac
    /// check's driver does (#2658).
    #[gpui::test]
    fn a_click_on_settling_rows_selects_the_row_resting_there(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2], vec![3, 4, 5], 3);
        shell.update(cx, |shell, cx| {
            let today = vec![3, 6, 4, 5];
            shell.set_tabs(1, vec![2], today, Vec::new(), Folders::default(), cx);
            shell.set_selected(6, cx);
        });
        cx.run_until_parked();
        let moving = shell.read_with(cx, |shell, _| shell.slide(Slot::Tab(4)));
        assert!(moving < 0., "4 is still on its way down: {moving}");
        let (x, y) = close_button(&shell, cx, 4);
        let at = point(px(x - 100.), px(y));
        host.take();
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert!(calls.iter().any(|call| call == "select 4"), "{calls:?}");
    }

    fn pressed(position: gpui::Point<gpui::Pixels>) -> gpui::PlatformInput {
        gpui::PlatformInput::MouseDown(gpui::MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::none(),
            click_count: 1,
            first_mouse: false,
        })
    }

    fn released(position: gpui::Point<gpui::Pixels>) -> gpui::PlatformInput {
        gpui::PlatformInput::MouseUp(gpui::MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::none(),
            click_count: 1,
        })
    }

    fn moved(
        position: gpui::Point<gpui::Pixels>,
        button: Option<MouseButton>,
    ) -> gpui::PlatformInput {
        gpui::PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position,
            pressed_button: button,
            modifiers: Modifiers::none(),
        })
    }

    /// Row 4's resting place in today's three rows.
    fn row_four(
        cx: &mut TestAppContext,
    ) -> (
        Entity<Shell>,
        &mut VisualTestContext,
        Rc<Recorder>,
        gpui::Point<gpui::Pixels>,
    ) {
        let (shell, cx, host) = window(cx, Vec::new(), vec![2, 3, 4], 2);
        let (x, y) = close_button(&shell, cx, 4);
        host.take();
        (shell, cx, host, point(px(x - 100.), px(y)))
    }

    /// A press and release on a row that come while the App is in an update, as a nested event
    /// pump delivers them, show the row once, as a click at rest does, once the update is over
    /// (#2658).
    #[gpui::test]
    fn a_click_that_comes_mid_update_still_selects_its_row(cx: &mut TestAppContext) {
        let (shell, cx, host, at) = row_four(cx);
        let mut input = cx.test_window(gpui::VisualContext::window_handle(cx));
        shell.update(cx, |_, _| {
            input.simulate_input(pressed(at));
            input.simulate_input(released(at));
        });
        cx.run_until_parked();
        assert_eq!(host.take(), ["card close", "select 4", "focus page"]);
    }

    /// A press held for the update it came in goes ahead of input that comes once it's over: the
    /// release after it lands and ends the press, so a move then drags nothing.
    #[gpui::test]
    fn a_press_held_mid_update_lands_before_the_release_after_it(cx: &mut TestAppContext) {
        let (shell, cx, host, at) = row_four(cx);
        let mut input = cx.test_window(gpui::VisualContext::window_handle(cx));
        shell.update(cx, |_, _| {
            input.simulate_input(pressed(at));
        });
        input.simulate_input(released(at));
        input.simulate_input(moved(point(at.x, at.y - px(2. * ROW_PITCH)), None));
        cx.run_until_parked();
        // The move leaves the row, so its hover card goes.
        assert_eq!(
            host.take(),
            ["card close", "select 4", "focus page", "card left 4"]
        );
        assert!(shell.read_with(cx, |shell, _| shell.dragging().is_none()));
        assert_eq!(host.moves.get(), 0);
    }

    /// A drag begun mid-update keeps its moves behind its press, so it moves the tab as a drag at
    /// rest does, whether it ends in the update or after.
    #[gpui::test]
    fn a_drag_begun_mid_update_still_drags(cx: &mut TestAppContext) {
        for release_after in [false, true] {
            let (shell, cx, host, at) = row_four(cx);
            let mut input = cx.test_window(gpui::VisualContext::window_handle(cx));
            let to = point(at.x, at.y - px(2. * ROW_PITCH));
            shell.update(cx, |_, _| {
                input.simulate_input(pressed(at));
                input.simulate_input(moved(
                    point(at.x, at.y - px(ROW_PITCH)),
                    Some(MouseButton::Left),
                ));
                input.simulate_input(moved(to, Some(MouseButton::Left)));
                if !release_after {
                    input.simulate_input(released(to));
                }
            });
            if release_after {
                input.simulate_input(released(to));
            }
            cx.run_until_parked();
            assert_eq!(host.moves.get(), 1, "release after: {release_after}");
            let calls = host.take();
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call.starts_with("select"))
                    .count(),
                1,
                "{calls:?}"
            );
        }
    }

    /// What a window holds while its callback is out (the Mac's, when a nested event pump delivers
    /// input inside another event) or the App is in an update: presses and releases, and the
    /// pointer's input behind them, in the order it came; never a key, which is answered at once,
    /// nor a lone move, which the next one replaces.
    #[test]
    fn held_input_keeps_a_press_and_what_follows_it_in_order() {
        let at = point(px(10.), px(10.));
        let key = gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("a").unwrap(),
            is_held: false,
            prefer_character_input: false,
        });
        let mut held = gpui::HeldInput::default();
        assert!(held.hold(moved(at, None)).is_some());
        assert!(held.hold(key.clone()).is_some());
        assert!(held.is_empty());
        assert!(held.hold(pressed(at)).is_none());
        assert!(held.hold(moved(at, Some(MouseButton::Left))).is_none());
        assert!(held.hold(key).is_some());
        assert!(held.hold(released(at)).is_none());
        let order: Vec<_> = std::iter::from_fn(|| held.next())
            .map(|event| match event {
                gpui::PlatformInput::MouseDown(_) => "down",
                gpui::PlatformInput::MouseMove(_) => "move",
                gpui::PlatformInput::MouseUp(_) => "up",
                _ => "other",
            })
            .collect();
        assert_eq!(order, ["down", "move", "up"]);
        assert!(held.is_empty());
    }

    /// A press that becomes a drag has shown its tab, and still drags it: let go two rows up, the
    /// tab moves there, and nothing is shown again.
    #[gpui::test]
    fn a_press_that_becomes_a_drag_still_drags(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, Vec::new(), vec![2, 3, 4], 2);
        let (x, y) = close_button(&shell, cx, 4);
        let from = point(px(x - 100.), px(y));
        host.take();
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["card close", "select 4", "focus page"]);
        let to = point(from.x, from.y - px(2. * ROW_PITCH));
        cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.dragging() == Some(Slot::Tab(4))));
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.moves.get(), 1);
        assert!(!host.take().iter().any(|call| call.starts_with("select")));
    }

    /// Expand All Folders opens the Space's closed folders and Collapse All Folders closes its
    /// open ones, each on one fold, leaving the rest alone.
    #[gpui::test]
    fn expand_and_collapse_all_fold_only_the_folders_that_change(cx: &mut TestAppContext) {
        let docs = [(20, ("Docs".into(), true)), (22, ("Work".into(), false))];
        let folders = Folders {
            folders: docs.into(),
            ..Default::default()
        };
        let (shell, cx, host) = window_with(cx, vec![20, 21, 22, 23], vec![4], 4, folders);
        let folds = |command, cx: &mut VisualTestContext| {
            host.take();
            shell.update(cx, |shell, cx| shell.run(command, "ui", cx));
            let calls = host.take();
            let folds = calls
                .into_iter()
                .filter(|call| call.starts_with("folder open"));
            (
                folds.collect::<Vec<_>>(),
                shell.read_with(cx, |shell, _| shell.folding),
            )
        };
        assert_eq!(
            folds(Command::ExpandFolders, cx),
            (vec!["folder open 22 true".into()], Some(true))
        );
        assert_eq!(
            folds(Command::CollapseFolders, cx),
            (vec!["folder open 20 false".into()], Some(false))
        );
    }

    /// A tab shown in closed folders stays listed there, with its dots, as another tab shows, as
    /// Arc keeps it (#1405). A click on the folder then closes it over the tab for good, and the
    /// next click opens it. While the tab shown is in the folder, a click opens it as ever.
    #[gpui::test]
    fn a_peeked_tab_stays_listed_until_its_folder_is_clicked(cx: &mut TestAppContext) {
        let (shell, cx, host) = window_with(cx, vec![10, 11, 3, 6], vec![4], 3, work_and_plans());
        let listed = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                let space = shell.shown_space().unwrap();
                (space.pinned_rows(false).collect::<Vec<_>>(), space.dotted())
            })
        };
        assert_eq!(listed(cx), (vec![10, 3], vec![(10, 3)]));
        shell.update(cx, |shell, cx| shell.set_selected(4, cx));
        cx.run_until_parked();
        assert_eq!(
            listed(cx),
            (vec![10, 3], vec![(10, 3)]),
            "kept after 4 shows"
        );
        // A second tab shown there is kept with the first.
        shell.update(cx, |shell, cx| shell.set_selected(6, cx));
        shell.update(cx, |shell, cx| shell.set_selected(4, cx));
        cx.run_until_parked();
        assert_eq!(listed(cx), (vec![10, 3, 6], vec![(10, 3)]));

        host.take();
        shell.update(cx, |shell, cx| shell.press_folder(10, cx));
        cx.run_until_parked();
        assert_eq!(
            host.take(),
            Vec::<String>::new(),
            "closed for good, not opened"
        );
        assert_eq!(listed(cx), (vec![10], vec![]));
        shell.update(cx, |shell, cx| shell.press_folder(10, cx));
        assert_eq!(host.take(), ["folder open 10 true"]);
    }

    /// A tab shown two closed folders down slides in from under the outer folder's row over
    /// 0.15 s, fading in as it comes, and the rows under it make room on the same curve, as Arc
    /// 1.171's does (#1405).
    #[gpui::test]
    fn a_tab_shown_in_closed_folders_slides_in_under_them(cx: &mut TestAppContext) {
        let (shell, cx, _host) =
            window_with(cx, vec![10, 11, 3, 6, 7], vec![4], 4, work_and_plans());
        let before = shell.read_with(cx, |shell, _| shell.layout());
        shell.update(cx, |shell, cx| {
            shell.set_selected(3, cx);
            // A loaded runner's late first frame, longer than the slide: it holds, as the list's
            // motion is on the test's clock (#2630).
            std::thread::sleep(Duration::from_secs_f32(crate::shell::PEEK_IN + 0.05));
        });
        shell.read_with(cx, |shell, _| {
            let after = shell.layout();
            let (folder, tab) = (after[&Slot::Tab(10)], after[&Slot::Tab(3)]);
            let (from, _, curve) = shell.slides[&Slot::Tab(3)];
            assert_eq!(from, folder - tab, "it starts under the folder's row");
            assert!(matches!(curve, crate::shell::Curve::Peek));
            let (from, _, curve) = shell.slides[&Slot::Tab(7)];
            assert_eq!(from, before[&Slot::Tab(7)] - after[&Slot::Tab(7)]);
            assert!(
                from < 0. && matches!(curve, crate::shell::Curve::Peek),
                "7 makes room"
            );
            let opened = shell.opened.as_ref().expect("the row fades in");
            assert!(
                opened.tabs == [3]
                    && opened.whole
                    && matches!(opened.curve, crate::shell::Curve::Peek)
            );
        });
    }

    /// While the tab shown is in a folder that keeps tabs shown before, a click opens it as ever,
    /// and lets go of the kept tabs, so it closes whole next time.
    #[gpui::test]
    fn a_click_with_the_tab_shown_in_the_folder_opens_it(cx: &mut TestAppContext) {
        let (shell, cx, host) = window_with(cx, vec![10, 11, 3, 6], vec![4], 3, work_and_plans());
        shell.update(cx, |shell, cx| shell.set_selected(6, cx));
        cx.run_until_parked();
        host.take();
        shell.update(cx, |shell, cx| shell.press_folder(10, cx));
        assert_eq!(host.take(), ["folder open 10 true"]);
        let peeked = shell.read_with(cx, |shell, _| shell.shown_space().unwrap().peeked.clone());
        assert_eq!(peeked, Vec::<u64>::new());
    }

    /// A folder opened and closed again from anywhere, as by Expand All Folders, lets go of the
    /// tabs it kept, so it closes whole.
    #[gpui::test]
    fn a_folder_opened_elsewhere_lets_go_of_its_kept_tabs(cx: &mut TestAppContext) {
        let (shell, cx, _) = window_with(cx, vec![10, 11, 3, 6], vec![4], 3, work_and_plans());
        shell.update(cx, |shell, cx| shell.set_selected(4, cx));
        let fold = |open, cx: &mut VisualTestContext| {
            let mut folders = work_and_plans();
            folders.folders.insert(10, ("Work".into(), open));
            shell.update(cx, |shell, cx| {
                shell.set_tabs(1, vec![10, 11, 3, 6], vec![4], Vec::new(), folders, cx)
            });
            cx.run_until_parked();
        };
        fold(true, cx);
        fold(false, cx);
        let listed = shell.read_with(cx, |shell, _| {
            let space = shell.shown_space().unwrap();
            space.pinned_rows(false).collect::<Vec<_>>()
        });
        assert_eq!(listed, [10]);
    }

    /// Work, closed, holds Plans, closed, which holds 3; Work also holds 6.
    fn work_and_plans() -> Folders {
        Folders {
            folders: [(10, ("Work".into(), false)), (11, ("Plans".into(), false))].into(),
            parents: [(11, 10), (3, 11), (6, 10)].into(),
            ..Default::default()
        }
    }

    /// A dragged row that rests on another tab's row's middle for less than SPLIT_REST still goes
    /// where it's let go; one that rests there longer joins that tab in a split view, as Arc's
    /// does, with the row lit and no drop line.
    #[gpui::test]
    fn a_rest_on_a_rows_middle_splits_and_a_short_one_reorders(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, Vec::new(), vec![2, 3, 4], 2);
        let (x, y) = close_button(&shell, cx, 4);
        let from = point(px(x - 100.), px(y));
        let to = point(from.x, from.y - px(2. * ROW_PITCH));
        let drag_and_rest = |cx: &mut VisualTestContext, rest: u64| {
            cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(rest));
            cx.run_until_parked();
        };
        let onto = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                shell.drag.as_ref().map(|drag| (drag.onto, drag.line))
            })
        };

        drag_and_rest(cx, 200);
        assert!(
            matches!(onto(&shell, cx), Some((None, Some(_)))),
            "a short rest keeps the line"
        );
        cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.moves.get(), 1, "a short rest reorders");
        assert!(
            !host
                .take_edits()
                .iter()
                .any(|edit| edit.starts_with("split"))
        );

        shell.update(cx, |shell, cx| {
            shell.set_tabs(
                1,
                Vec::new(),
                vec![2, 3, 4],
                Vec::new(),
                Folders::default(),
                cx,
            )
        });
        cx.run_until_parked();
        drag_and_rest(cx, 500);
        // The pointer is on the row's left half, where the tab would join it on the left.
        assert_eq!(onto(&shell, cx), Some((Some((2, true)), None)));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.hover_fill(Row::Tab(2))),
            Some(1.)
        );
        // Across the plate's middle, it would join on the right, with no second wait.
        let right = point(px(x - 40.), to.y);
        cx.simulate_mouse_move(right, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(onto(&shell, cx), Some((Some((2, false)), None)));
        cx.simulate_mouse_up(right, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.moves.get(), 1, "a rest doesn't reorder");
        assert_eq!(host.take_edits(), ["split 4 2 false"]);
    }

    /// ⌥-click in the sidebar splits as Arc's does: another tab joins the page shown, the page
    /// shown splits with a copy of itself, and a split's tab leaves it.
    #[gpui::test]
    fn an_option_click_on_a_row_splits_duplicates_or_separates(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, Vec::new(), vec![2, 3, 4], 2);
        let option = Modifiers {
            alt: true,
            ..Default::default()
        };
        let option_click = |cx: &mut VisualTestContext, tab: u64, inset: f32| {
            let (x, y) = close_button(&shell, cx, tab);
            let at = point(px(x - inset), px(y));
            host.take();
            host.take_edits();
            cx.simulate_mouse_down(at, MouseButton::Left, option);
            cx.simulate_mouse_up(at, MouseButton::Left, option);
            cx.run_until_parked();
            (host.take(), host.take_edits())
        };

        let (_, edits) = option_click(cx, 3, 100.);
        assert_eq!(edits, ["split 3 2 false"]);

        let (calls, edits) = option_click(cx, 2, 100.);
        assert!(calls.contains(&"duplicate 2".to_string()), "{calls:?}");
        assert_eq!(edits.len(), 1, "{edits:?}");
        assert!(
            edits[0].starts_with("split ") && edits[0].ends_with(" 2 false"),
            "{edits:?}"
        );

        shell.update(cx, |shell, cx| {
            let split = crate::shell::Split {
                tabs: vec![2, 3],
                ..Default::default()
            };
            shell.set_tabs(
                1,
                Vec::new(),
                vec![2, 3, 4],
                vec![split],
                Folders::default(),
                cx,
            )
        });
        cx.run_until_parked();
        let (_, edits) = option_click(cx, 2, 40.);
        assert_eq!(
            edits,
            ["separate 3"],
            "the press on the second segment takes its tab out"
        );
    }

    /// Shift+Alt-click does what ⌥-click does, for Linux desktops whose window manager takes every
    /// Alt-click but leaves one with Shift (#2755): another tab joins the page shown, and the page
    /// shown splits with a copy of itself, where Shift-click alone would select.
    #[gpui::test]
    fn a_shift_alt_click_on_a_row_splits_as_an_option_click_does(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, Vec::new(), vec![2, 3, 4], 2);
        let held = Modifiers {
            alt: true,
            shift: true,
            ..Default::default()
        };
        let click = |cx: &mut VisualTestContext, tab: u64| {
            let (x, y) = close_button(&shell, cx, tab);
            let at = point(px(x - 100.), px(y));
            host.take();
            host.take_edits();
            cx.simulate_mouse_down(at, MouseButton::Left, held);
            cx.simulate_mouse_up(at, MouseButton::Left, held);
            cx.run_until_parked();
            (host.take(), host.take_edits())
        };

        let (_, edits) = click(cx, 3);
        assert_eq!(edits, ["split 3 2 false"]);

        let (calls, edits) = click(cx, 2);
        assert!(calls.contains(&"duplicate 2".to_string()), "{calls:?}");
        assert_eq!(edits.len(), 1, "{edits:?}");
    }

    /// ⌥ held over a split row's ✕ turns it to separate, as Arc's does: the glyph follows the key
    /// as the pointer moves or GPUI hears it, a click with ⌥ takes every tab out of the split from
    /// the right and closes none, and without ⌥ the ✕ closes them. A tab's own ✕ never turns.
    #[gpui::test]
    fn option_on_a_split_rows_close_separates_it(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, Vec::new(), vec![2, 3, 4, 5], 5);
        shell.update(cx, |shell, cx| {
            let split = crate::shell::Split {
                tabs: vec![2, 3, 4],
                ..Default::default()
            };
            shell.set_tabs(
                1,
                Vec::new(),
                vec![2, 3, 4, 5],
                vec![split],
                Folders::default(),
                cx,
            )
        });
        cx.run_until_parked();
        let option = Modifiers {
            alt: true,
            ..Default::default()
        };
        let (x, y) = close_button(&shell, cx, 2);
        let at = point(px(x), px(y));
        let shown =
            |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.on_split_close);
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(shown(cx), Some((2, false)));
        cx.simulate_mouse_move(at + point(px(1.), px(0.)), None, option);
        cx.run_until_parked();
        assert_eq!(shown(cx), Some((2, true)), "⌥ held as the pointer moves");
        // GPUI hears the keys while it has the keyboard, as in a field of the sidebar's.
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        cx.simulate_modifiers_change(Modifiers::none());
        cx.run_until_parked();
        assert_eq!(shown(cx), Some((2, false)), "⌥ let go, heard by GPUI");
        cx.simulate_modifiers_change(option);
        cx.run_until_parked();
        assert_eq!(shown(cx), Some((2, true)));

        host.take();
        host.take_edits();
        cx.simulate_mouse_down(at, MouseButton::Left, option);
        cx.simulate_mouse_up(at, MouseButton::Left, option);
        cx.run_until_parked();
        assert_eq!(host.take_edits(), ["separate 4", "separate 3"]);
        assert!(!host.take().iter().any(|call| call.starts_with("close")));

        let own = close_button(&shell, cx, 5);
        move_to(cx, own);
        assert_eq!(shown(cx), None, "a tab's own ✕ never turns");
        cx.simulate_mouse_move(point(px(x), px(y + 300.)), None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(shown(cx), None);
    }

    /// A row dragged onto a page shown alone finds a target at each of the card's edges, and let go
    /// on the top one it joins the page above it in a stacked split, as Arc's does.
    #[gpui::test]
    fn a_row_let_go_on_the_top_target_stacks_its_split(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, Vec::new(), vec![2, 3], 2);
        let (x, y) = close_button(&shell, cx, 3);
        let from = point(px(x - 100.), px(y));
        cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let card = shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport).0);
        let top = point(card.center().x, card.top() + px(60.));
        for to in [point(from.x, from.y - px(ROW_PITCH)), top] {
            cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
        }
        host.take();
        host.take_edits();
        cx.simulate_mouse_up(top, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take_edits(), ["split 3 2 true"]);
        assert!(host.take().contains(&"split stacked 2 true".to_string()));
    }

    /// A press on a folder's row closes it at once, as Arc's does, and a press again opens it.
    #[gpui::test]
    fn a_press_opens_or_closes_a_folder(cx: &mut TestAppContext) {
        let docs = [(20, ("Docs".into(), true))];
        let folders = Folders {
            folders: docs.into(),
            ..Default::default()
        };
        let (shell, cx, host) = window_with(cx, vec![20, 21], vec![4], 4, folders);
        let (x, y) = close_button(&shell, cx, 20);
        let middle = point(px(x - 100.), px(y));
        host.take();
        cx.simulate_mouse_down(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(shell.read_with(cx, |shell, _| shell.folding), Some(false));
        let calls = host.take();
        assert!(
            !calls.iter().any(|call| call.starts_with("select")),
            "{calls:?}"
        );
    }

    /// The chevron after the Space's name collapses its pinned rows as it's pressed, which hides
    /// them but the tab shown's from the list and from the tabs' order, and expands them again.
    #[gpui::test]
    fn the_caret_collapses_and_expands_the_pinned_rows(cx: &mut TestAppContext) {
        let (shell, cx, host) = window_with(cx, vec![1, 2], vec![4], 4, Folders::default());
        let caret =
            |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.caret_drawn.get());
        let (shut, frame) = caret(cx).expect("no caret with pinned rows");
        assert!(!shut);
        cx.simulate_mouse_move(frame.center(), None, Modifiers::none());
        cx.simulate_mouse_down(frame.center(), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"pinned open 1 false".to_owned()));
        assert_eq!(shell.read_with(cx, |shell, _| shell.folding), Some(false));

        let shut = Folders {
            pinned_shut: true,
            ..Folders::default()
        };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![1, 2], vec![4], Vec::new(), shut, cx)
        });
        cx.run_until_parked();
        assert_eq!(shell.read_with(cx, |shell, _| shell.sidebar_rows()), [4]);
        let stays = caret(cx).is_some_and(|(shut, _)| shut);
        assert!(stays, "the caret didn't stay for the way back");
        shell.update(cx, |shell, cx| shell.set_selected(2, cx));
        assert_eq!(shell.read_with(cx, |shell, _| shell.rows().0), [2]);
        let offered = shell.read_with(cx, |shell, _| {
            (
                shell.offers(Command::CollapsePinned),
                shell.offers(Command::ExpandPinned),
            )
        });
        assert_eq!(offered, (false, true));

        shell.update(cx, |shell, cx| shell.run(Command::ExpandPinned, "ui", cx));
        assert!(host.take().contains(&"pinned open 1 true".to_owned()));
        assert_eq!(shell.read_with(cx, |shell, _| shell.folding), Some(true));
    }

    /// Collapse and Expand Pinned Tabs are offered only while they'd hide or show a row now
    /// (#1208): the tab shown keeps its row, so with it the only pinned tab there's no caret, no
    /// command bar row and a dimmed View item, and the key does nothing. A Today tab shown leaves
    /// the pinned row to hide, and once it's hidden Expand brings it back; the pinned tab shown
    /// again from the command bar is its row. A tab shown in a folder with another collapses them.
    #[gpui::test]
    fn the_pinned_rows_collapse_only_when_that_hides_one(cx: &mut TestAppContext) {
        use crate::shell::commands::{ShellHandle, bar_commands};
        let (shell, cx, host) = window_with(cx, vec![1], vec![4], 1, Folders::default());
        cx.update(|_, cx| cx.set_global(ShellHandle(shell.downgrade())));
        let caret = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            shell.read_with(cx, |shell, _| shell.caret_drawn.get().map(|(shut, _)| shut))
        };
        let bar = |cx: &mut VisualTestContext| {
            let rows = cx.update(|_, cx| bar_commands("pinned", cx));
            let pinned = [Command::CollapsePinned, Command::ExpandPinned];
            let commands = rows.into_iter().map(|row| row.command);
            commands
                .filter(|command| pinned.contains(command))
                .collect::<Vec<_>>()
        };
        let menu = || {
            let bar = host.menu_bar.borrow();
            bar.pinned_offered.then(|| bar.pinned_item().1)
        };
        assert_eq!(caret(cx), None, "a caret with only the tab shown pinned");
        assert_eq!(bar(cx), []);
        assert_eq!(menu(), None);
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::CollapsePinned, "ui", cx));
        assert!(
            !host
                .take()
                .iter()
                .any(|call| call.starts_with("pinned open"))
        );

        shell.update(cx, |shell, cx| shell.set_selected(4, cx));
        assert_eq!(
            caret(cx),
            Some(false),
            "no caret with the pinned row to hide"
        );
        assert_eq!(bar(cx), [Command::CollapsePinned]);
        assert_eq!(menu(), Some(Command::CollapsePinned));
        shell.update(cx, |shell, cx| shell.run(Command::CollapsePinned, "ui", cx));
        assert!(host.take().contains(&"pinned open 1 false".to_owned()));
        let shut = Folders {
            pinned_shut: true,
            ..Folders::default()
        };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![1], vec![4], Vec::new(), shut, cx)
        });
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.rows().0),
            Vec::<u64>::new()
        );
        assert_eq!(caret(cx), Some(true), "no caret for the way back");
        assert_eq!(bar(cx), [Command::ExpandPinned]);
        assert_eq!(menu(), Some(Command::ExpandPinned));

        // As the core answers the command bar's row for the pinned tab.
        shell.update(cx, |shell, cx| shell.set_selected(1, cx));
        assert_eq!(shell.read_with(cx, |shell, _| shell.rows().0), [1]);
        assert_eq!(caret(cx), None, "a caret with nothing hidden");
        assert_eq!(bar(cx), []);

        let docs = Folders {
            folders: [(20, ("Docs".into(), true))].into(),
            parents: [(21, 20), (22, 20)].into(),
            ..Folders::default()
        };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![20, 21, 22], vec![4], Vec::new(), docs, cx);
            shell.set_selected(21, cx);
        });
        assert_eq!(
            caret(cx),
            Some(false),
            "no caret with the folder's other tab to hide"
        );
        assert_eq!(bar(cx), [Command::CollapsePinned]);
    }

    /// Presses in the first frames after ⌃2, while the second Space's rows still slide in from the
    /// right, act on its row at the pointer, as they do once it has settled (docs/ux-principles.md:
    /// act on the press): a right-click shows its menu, and a left-click shows its tab.
    #[gpui::test]
    fn a_press_while_a_space_slides_in_acts_on_its_row(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    1,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
            shell.set_spaces(vec![space(1), space(2)], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![2, 3],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_tabs(
                2,
                Vec::new(),
                vec![4, 5],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_selected(2, cx);
        });
        cx.run_until_parked();
        shell.update(cx, |shell, cx| {
            shell.set_space(2, cx);
            shell.set_selected(4, cx);
        });
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.settle.is_some()),
            "the Space didn't slide"
        );

        let (x, y) = close_button(&shell, cx, 5);
        let at = point(px(x - 100.), px(y));
        host.take();
        host.menus.borrow_mut().clear();
        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            host.menus.borrow().len(),
            1,
            "a right-click mid-slide showed no menu"
        );
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let calls = host.take();
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.starts_with("select"))
                .count(),
            1,
            "{calls:?}"
        );
        assert!(
            calls.contains(&"select 5".to_string()),
            "a left-click mid-slide: {calls:?}"
        );

        // Where the page has come, its drawn row takes the press, and only once.
        shell.update(cx, |shell, cx| {
            if let Some(settle) = shell.settle.as_mut() {
                settle.start -= Duration::from_millis(60);
            }
            cx.notify();
        });
        cx.run_until_parked();
        let page = shell.read_with(cx, |shell, _| shell.pages()[0].1);
        assert!(
            page > 0. && page < 100.,
            "the page is {page} from its place"
        );
        host.menus.borrow_mut().clear();
        let drawn = point(px(x + page - 100.), px(y));
        cx.simulate_mouse_down(drawn, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            host.menus.borrow().len(),
            1,
            "a right-click on the drawn row"
        );
    }

    /// A click on a row while its Space's page slides back in from the left shows that row's tab,
    /// as it rests under the pointer, whatever the frame last drawn has there (#1437): the page
    /// further back than it is by now, as a busy main thread draws it, or the row's ✕ under a
    /// pointer that rested on the sidebar as the page slid under it.
    #[gpui::test]
    fn a_click_while_a_space_slides_back_in_shows_the_row_at_rest(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    1,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
            shell.set_spaces(vec![space(1), space(2)], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![10, 4],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_tabs(2, Vec::new(), vec![7], Vec::new(), Folders::default(), cx);
            shell.set_selected(4, cx);
        });
        cx.run_until_parked();
        let (close, y) = close_button(&shell, cx, 10);
        let at = point(px(100.), px(y));
        let width = shell.read_with(cx, |shell, _| shell.width());
        let back = |cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| {
                shell.set_space(2, cx);
                shell.set_selected(7, cx);
                shell.settle = None;
                cx.notify();
            });
            cx.run_until_parked();
        };
        let slide_in = |cx: &mut VisualTestContext, ago: u64| {
            shell.update(cx, |shell, cx| {
                shell.set_space(1, cx);
                shell.set_selected(4, cx);
                if let Some(settle) = shell.settle.as_mut() {
                    settle.start -= Duration::from_millis(ago);
                }
                cx.notify();
            });
            cx.run_until_parked();
            shell.read_with(cx, |shell, _| shell.pages()[0])
        };
        let click = |cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>| {
            host.take();
            cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            host.take()
        };

        // Drawn with the pointer still over the outgoing page, then on, undrawn, past the pointer.
        back(cx);
        cx.simulate_mouse_move(point(px(97.), px(y + 400.)), None, Modifiers::none());
        let (space, drawn) = slide_in(cx, 0);
        assert!(
            space == 1 && drawn < 100. - width,
            "the page was drawn at {drawn}"
        );
        shell.update(cx, |shell, _| {
            if let Some(settle) = shell.settle.as_mut() {
                settle.start -= Duration::from_millis(100);
            }
        });
        let now = shell.read_with(cx, |shell, _| shell.pages()[0].1);
        assert!(now > 100. - width && now < 0., "the page is at {now}");
        let calls = click(cx, at);
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.starts_with("select"))
                .count(),
            1,
            "{calls:?}"
        );
        assert!(
            calls.contains(&"select 10".to_string()),
            "behind the slide: {calls:?}"
        );

        // The pointer rests where row 10's ✕ is drawn as the page slides under it, a point that
        // at rest is on the row, left of its ✕. Taken from the frame drawn, not a time into the
        // slide, so the ✕'s place in the row doesn't move it off the pointer.
        back(cx);
        cx.simulate_mouse_move(at, None, Modifiers::none());
        let (_, drawn) = slide_in(cx, 60);
        let over_close = point(px(close + drawn), px(y));
        assert!(
            close + drawn > 0. && close + drawn < close - CLOSE_SIZE,
            "the ✕ is drawn at {}, where it rests at {close}",
            close + drawn
        );
        cx.simulate_mouse_move(over_close, None, Modifiers::none());
        let calls = click(cx, over_close);
        assert!(
            calls.contains(&"select 10".to_string()),
            "over the ✕: {calls:?}"
        );
        assert!(
            !calls.iter().any(|call| call.starts_with("close")),
            "over the ✕: {calls:?}"
        );
    }

    /// A press on an away pinned tab's favicon leaves showing the tab to its click, which takes
    /// the tab back to its pinned URL first, so its old page never loads.
    #[gpui::test]
    fn the_way_back_shows_the_tab_once_it_is_back(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2], vec![3], 3);
        shell.update(cx, |shell, cx| {
            let page = Page {
                away_from_pinned_url: true,
                ..shell.page(2)
            };
            shell.set_page(2, page, cx)
        });
        let (_, y) = close_button(&shell, cx, 2);
        let favicon = (FAVICON_X + FAVICON_SIZE / 2., y);
        move_to(cx, favicon);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.hovered_favicon),
            Some(2)
        );
        host.take();
        let at = point(px(favicon.0), px(favicon.1));
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["card close"], "the press");
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            host.take(),
            ["back to pinned url 2", "select 2", "focus page"]
        );
    }

    /// ⌘-click on an away pinned tab's favicon keeps its page in a copy, which the core shows, and
    /// takes the tab back without showing it.
    #[gpui::test]
    fn a_command_click_on_the_way_back_keeps_the_page_in_a_new_tab(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2], vec![3], 3);
        shell.update(cx, |shell, cx| {
            let page = Page {
                away_from_pinned_url: true,
                ..shell.page(2)
            };
            shell.set_page(2, page, cx)
        });
        let (_, y) = close_button(&shell, cx, 2);
        let favicon = (FAVICON_X + FAVICON_SIZE / 2., y);
        move_to(cx, favicon);
        host.take();
        let at = point(px(favicon.0), px(favicon.1));
        let command = cx.update(|_, cx| Modifiers::secondary_key(cx));
        cx.simulate_mouse_down(at, MouseButton::Left, command);
        cx.simulate_mouse_up(at, MouseButton::Left, command);
        cx.run_until_parked();
        assert_eq!(
            host.take(),
            ["card close", "duplicate 2", "back to pinned url 2"]
        );
    }

    /// Arc's ⌘} and ⌘{ show the next and previous tab, as ⌥⌘↓ and ⌥⌘↑ do, either way GPUI names them.
    #[gpui::test]
    fn the_brace_keys_show_the_next_and_previous_tab(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4], 3);
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        for (key, tab) in [
            ("shift-cmd-}", 4),
            ("shift-cmd-]", 4),
            ("shift-cmd-{", 2),
            ("shift-cmd-[", 2),
        ] {
            shell.update(cx, |shell, cx| shell.set_selected(3, cx));
            host.take();
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            assert!(host.take().contains(&format!("select {tab}")), "{key}");
        }
    }

    /// How the user goes back to the tab they came from.
    #[derive(Clone, Copy, Debug)]
    enum Back {
        Click,
        /// A key in the UI.
        Key,
        /// A key in the page, which hands it back.
        PageKey,
    }

    /// The issue's steps: the pointer on pinned tab 2, shown, a click on tab 3 below it, whose page
    /// may show the protected-media note, then `back` to tab 2. Returns the rows other than the one
    /// shown that draw a hover fill, then the shell and where the pointer is. The two rows are
    /// below where the note hangs, so a click reaches each.
    fn switch_back(
        cx: &mut TestAppContext,
        back: Back,
        note: bool,
    ) -> (Vec<u64>, Entity<Shell>, &mut VisualTestContext) {
        cx.update(bind_keys);
        let (shell, cx, host) = window(cx, vec![5, 6, 7, 8, 2, 3], vec![4], 2);
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        let middle = |cx: &mut VisualTestContext, tab| {
            let (x, y) = close_button(&shell, cx, tab);
            point(px(x - 100.), px(y))
        };
        let click = |cx: &mut VisualTestContext, tab| {
            let at = middle(cx, tab);
            cx.simulate_mouse_move(at, None, Modifiers::none());
            cx.simulate_click(at, Modifiers::none());
        };
        let at = middle(cx, 2);
        cx.simulate_mouse_move(at, None, Modifiers::none());
        cx.run_until_parked();
        for tab in [3, 2] {
            host.take();
            match (tab, back) {
                (3, _) | (_, Back::Click) => click(cx, tab),
                (_, Back::Key) => cx.simulate_keystrokes("alt-cmd-up"),
                (_, Back::PageKey) => {
                    shell.update(cx, |shell, cx| shell.run_for_page(Command::PreviousTab, cx));
                }
            }
            cx.run_until_parked();
            assert!(host.take().contains(&format!("select {tab}")), "{back:?}");
            shell.update(cx, |shell, cx| {
                shell.set_selected(tab, cx);
                let system = if tab == 3 && note {
                    "com.widevine.alpha"
                } else {
                    ""
                };
                shell.set_media_unsupported(3, system, cx);
            });
            cx.run_until_parked();
        }
        (filled(&shell, cx), shell, cx)
    }

    /// The rows other than the one shown that draw a hover fill.
    fn filled(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<u64> {
        shell.read_with(cx, |shell, _| {
            let rows = [2, 3, 4, 5, 6, 7, 8]
                .into_iter()
                .filter(|&tab| tab != shell.selected);
            rows.filter(|&tab| {
                shell
                    .hover_fill(Row::Tab(tab))
                    .is_some_and(|fill| fill > 0.)
            })
            .collect()
        })
    }

    /// A row whose press showed its tab before it was let go reads as pressed only while it's
    /// held: once another tab is shown, by a key or the core, it reads as neither pressed nor
    /// selected (#427).
    #[gpui::test]
    fn a_row_let_go_is_not_drawn_pressed_once_another_is_shown(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2], vec![3, 4], 3);
        let (x, y) = close_button(&shell, cx, 4);
        let middle = point(px(x - 100.), px(y));
        let pressed = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| shell.pressed(Row::Tab(4)))
        };
        host.take();
        cx.simulate_mouse_down(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(pressed(&shell, cx), "held");
        // The core answers the press before the row is let go.
        shell.update(cx, |shell, cx| shell.set_selected(4, cx));
        cx.run_until_parked();
        cx.simulate_mouse_up(middle, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert!(!pressed(&shell, cx), "let go");
        // Then the pointer leaves for the page, and a key shows the tab before.
        cx.simulate_mouse_move(point(px(900.), px(y)), None, Modifiers::none());
        shell.update(cx, |shell, cx| shell.set_selected(3, cx));
        cx.run_until_parked();
        assert!(!pressed(&shell, cx), "another tab shown");
        assert_eq!(filled(&shell, cx), Vec::<u64>::new());
    }

    /// After a click on a tab and back to the one before, by a click or a key in the UI or in the
    /// page, with or without the note on the other tab's page, only the shown tab's row reads as
    /// selected. A key in the page hides the row the pointer rests on, as one in the UI does,
    /// until the pointer moves.
    #[gpui::test]
    fn switching_back_leaves_one_row_selected(cx: &mut TestAppContext) {
        for back in [Back::Click, Back::Key, Back::PageKey] {
            for note in [false, true] {
                let (filled, ..) = switch_back(cx, back, note);
                assert_eq!(filled, Vec::<u64>::new(), "{back:?}, note {note}");
            }
        }
        let (_, shell, cx) = switch_back(cx, Back::PageKey, false);
        let (x, y) = close_button(&shell, cx, 3);
        move_to(cx, (x - 101., y));
        assert_eq!(
            filled(&shell, cx),
            [3],
            "the pointer moved on the row it rests on"
        );
    }

    /// Under its title, a pinned tab's row names the host its page went to from its pinned URL's,
    /// but not another page on the same host, and a tab's row in Developer Mode names its host,
    /// without "www." or the port, as the URL field does.
    #[gpui::test]
    fn a_row_names_its_host_off_its_pinned_host_and_in_developer_mode(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4, 5], 4);
        host.pinned_urls
            .borrow_mut()
            .insert(2, "https://p.example/".into());
        host.pinned_urls
            .borrow_mut()
            .insert(3, "https://p.example/".into());
        let report = |cx: &mut VisualTestContext, tab, url: &str, away, developer_mode| {
            shell.update(cx, |shell, cx| {
                let page = Page {
                    url: gpui::SharedString::from(url.to_owned()),
                    away_from_pinned_url: away,
                    developer_mode,
                    ..shell.page(tab)
                };
                shell.set_page(tab, page, cx);
            });
        };
        report(cx, 2, "https://www.other.example/a", true, false);
        report(cx, 3, "https://p.example/signin", true, false);
        report(cx, 4, "http://localhost:3000/", false, true);
        report(cx, 5, "https://q.example/", false, false);
        let lines = shell.read_with(cx, |shell, _| [2, 3, 4, 5].map(|tab| shell.host_line(tab)));
        assert_eq!(
            lines,
            [
                Some("other.example".into()),
                None,
                Some("localhost".into()),
                None
            ]
        );
        // Back at its pinned URL's host, the line goes.
        report(cx, 2, "https://p.example/", false, false);
        assert_eq!(shell.read_with(cx, |shell, _| shell.host_line(2)), None);
    }

    /// A middle press on a tab's row closes it, as its ✕ does: a Today tab goes, a pinned one lets
    /// go of its page, and a pinned one without its page has nothing to close. The tab shown stays.
    #[gpui::test]
    fn a_middle_press_on_a_row_closes_its_tab(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4, 5], 4);
        shell.update(cx, |shell, cx| {
            let page = Page {
                unloaded: true,
                ..shell.page(2)
            };
            shell.set_page(2, page, cx);
        });
        let middle = |cx: &mut VisualTestContext, tab| {
            let (_, y) = close_button(&shell, cx, tab);
            cx.simulate_mouse_down(
                point(px(60.), px(y)),
                MouseButton::Middle,
                Modifiers::none(),
            );
            cx.simulate_mouse_up(
                point(px(60.), px(y)),
                MouseButton::Middle,
                Modifiers::none(),
            );
            cx.run_until_parked();
        };
        host.take();
        middle(cx, 5);
        middle(cx, 3);
        middle(cx, 2);
        // Each press also ends the hovers, which hover_cards' tests check.
        let asked = host.take();
        let closes: Vec<_> = asked
            .iter()
            .filter(|call| call.starts_with("close "))
            .collect();
        assert_eq!(closes, ["close 5", "close 3"]);
        assert_eq!(shell.read_with(cx, |shell, _| shell.selected), 4);
    }

    /// A Today tab whose page was let go after hours unseen keeps its ✕, as the tab itself is
    /// still there to close, and a click on it closes it without loading it.
    #[gpui::test]
    fn a_today_tab_without_its_page_still_closes(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2], vec![4, 5], 4);
        shell.update(cx, |shell, cx| {
            let page = Page {
                unloaded: true,
                ..shell.page(5)
            };
            shell.set_page(5, page, cx);
        });
        let today = close_button(&shell, cx, 5);
        move_to(cx, today);
        assert!(shell.read_with(cx, |shell, _| shell.closable(5)));
        assert_eq!(hovered_glyph(&shell, cx), Some(CLOSE));
        host.take();
        cx.simulate_click(point(px(today.0), px(today.1)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take(), ["close 5"]);
    }

    /// VoiceOver reads the button the row under the pointer shows by what it does, and the speaker
    /// by what a press on it would do (#1381). Its press closes the tab as a click does.
    #[gpui::test]
    fn the_row_buttons_say_what_they_do(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4, 5], 4);
        shell.update(cx, |shell, cx| {
            shell.set_page(
                5,
                Page {
                    audible: true,
                    ..Page::default()
                },
                cx,
            );
            shell.set_page(
                3,
                Page {
                    unloaded: true,
                    ..Page::default()
                },
                cx,
            );
        });
        cx.simulate_a11y_activation();
        cx.run_until_parked();
        let button = |cx: &mut VisualTestContext, name: &str| {
            let tree = cx.a11y_tree().expect("GPUI built a tree");
            let found = tree
                .nodes
                .into_iter()
                .find(|(_, node)| node.role() == Role::Button && node.label() == Some(name));
            found.map(|(id, _)| id)
        };
        assert!(button(cx, "Mute").is_some());
        assert!(button(cx, "Close Tab").is_none(), "only under the pointer");
        for (tab, name) in [(2, "Unload Tab"), (3, "Remove Tab"), (5, "Close Tab")] {
            let at = close_button(&shell, cx, tab);
            move_to(cx, at);
            assert!(button(cx, name).is_some(), "{name}");
        }

        host.take();
        let close = button(cx, "Close Tab").expect("Video's ✕");
        cx.simulate_a11y_action(close, Action::Click);
        cx.run_until_parked();
        assert!(host.take().contains(&"close 5".to_owned()));
        shell.update(cx, |shell, cx| {
            shell.set_page(
                5,
                Page {
                    audible: true,
                    muted: true,
                    ..Page::default()
                },
                cx,
            )
        });
        cx.run_until_parked();
        assert!(button(cx, "Unmute").is_some());
    }

    /// The button the row under the pointer draws, as check.sh reads it: "1" for ✕, "−" for −,
    /// "remove" for remove and "0" for none.
    fn drawn_button(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> &'static str {
        shell.read_with(cx, |shell, _| shell.drawn_hover.2)
    }

    /// The core reports the Space's tabs, as it does a moment after each call.
    fn report_lists(
        shell: &Entity<Shell>,
        cx: &mut VisualTestContext,
        pinned: Vec<u64>,
        today: Vec<u64>,
    ) {
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, pinned, today, Vec::new(), Folders::default(), cx)
        });
        cx.run_until_parked();
    }

    /// A pinned tab without its page has nothing to close, so under the pointer its row shows the
    /// remove button in the −'s place (#1349), selected or not. A click on it takes the tab out of
    /// the pinned tabs without showing or loading it: the core unpins it, which puts it at the top
    /// of Today, and closes it there, into the archive, so Today doesn't gain it. The toast offers
    /// its Undo, as a pinned tab is kept as a favorite is, and ⌘Z pins it again at its place, with
    /// its pinned URL.
    #[gpui::test]
    fn an_unloaded_pinned_rows_button_removes_it_and_undo_pins_it_again(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4, 5], 4);
        host.pinned_urls
            .borrow_mut()
            .insert(3, "https://p.example/".into());
        shell.update(cx, |shell, cx| {
            let page = Page {
                unloaded: true,
                ..shell.page(3)
            };
            shell.set_page(3, page, cx);
        });
        let remove = close_button(&shell, cx, 3);
        move_to(cx, remove);
        assert_eq!(drawn_button(&shell, cx), "remove");
        host.take();
        host.take_edits();
        cx.simulate_click(point(px(remove.0), px(remove.1)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take_edits(), ["pinned 3 false", "close 3"]);
        assert!(!host.take().iter().any(|call| call.starts_with("select")));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.history.titles().0),
            Some("Remove Tab")
        );
        assert_eq!(host.undo_toasts.borrow().as_slice(), ["Tab Removed"]);
        report_lists(&shell, cx, vec![2], vec![4, 5]);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.today().to_vec()),
            [4, 5]
        );

        // The core forgot its pinned URL when it unpinned it.
        host.pinned_urls.borrow_mut().clear();
        shell.update(cx, |shell, cx| shell.run(Command::Undo, "ui", cx));
        assert_eq!(
            host.take_edits(),
            [
                "reopen 3 as 1001",
                "move 1001 true 0 1",
                "pinned url 1001 https://p.example/"
            ]
        );
        assert!(!host.take().iter().any(|call| call.starts_with("select")));
        report_lists(&shell, cx, vec![2, 1001], vec![4, 5]);
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.pinned().to_vec()),
            [2, 1001]
        );
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.today().to_vec()),
            [4, 5]
        );
    }

    /// A click on a pinned row's − unloads it, and the remove button it now has waits until the
    /// pointer has left the row or pressed it, so a second click meant for the − never removes the
    /// tab. A middle press on the unloaded row does nothing, as removing is the button's alone.
    #[gpui::test]
    fn the_remove_button_waits_for_the_pointer_to_leave_the_row_it_unloaded(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx, host) = window(cx, vec![2, 3], vec![4], 4);
        let button = close_button(&shell, cx, 2);
        move_to(cx, button);
        assert_eq!(drawn_button(&shell, cx), "−");
        host.take_edits();
        cx.simulate_click(point(px(button.0), px(button.1)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take_edits(), ["close 2"]);
        // The core lets go of its page and keeps its row.
        shell.update(cx, |shell, cx| {
            let page = Page {
                unloaded: true,
                ..shell.page(2)
            };
            shell.set_page(2, page, cx);
        });
        cx.run_until_parked();
        assert_eq!(drawn_button(&shell, cx), "0");
        cx.simulate_click(point(px(button.0), px(button.1)), Modifiers::none());
        cx.run_until_parked();
        assert!(
            !host
                .take_edits()
                .iter()
                .any(|edit| edit.starts_with("pinned"))
        );
        cx.simulate_mouse_down(
            point(px(60.), px(button.1)),
            MouseButton::Middle,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            point(px(60.), px(button.1)),
            MouseButton::Middle,
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(host.take_edits().is_empty(), "a middle press");
        // The core hasn't loaded it yet, so the row has the button once pressed.
        cx.simulate_click(point(px(60.), px(button.1)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(drawn_button(&shell, cx), "remove", "pressed");

        let below = close_button(&shell, cx, 3);
        move_to(cx, below);
        cx.simulate_click(point(px(below.0), px(below.1)), Modifiers::none());
        cx.run_until_parked();
        shell.update(cx, |shell, cx| {
            let page = Page {
                unloaded: true,
                ..shell.page(3)
            };
            shell.set_page(3, page, cx);
        });
        cx.run_until_parked();
        assert_eq!(drawn_button(&shell, cx), "0");
        move_to(cx, button);
        move_to(cx, below);
        assert_eq!(drawn_button(&shell, cx), "remove", "left and back");
    }

    /// A pinned tab in a folder has the same button, and ⌘Z pins it again in its folder, at its
    /// place there.
    #[gpui::test]
    fn a_pinned_tab_in_a_folder_is_removed_and_comes_back_in_it(cx: &mut TestAppContext) {
        let folders = Folders {
            folders: HashMap::from([(5, ("Reading".into(), true))]),
            parents: HashMap::from([(6, 5), (7, 5)]),
            ..Default::default()
        };
        let (shell, cx, host) = window_with(cx, vec![2, 5, 6, 7], vec![4], 4, folders.clone());
        shell.update(cx, |shell, cx| {
            let page = Page {
                unloaded: true,
                ..shell.page(7)
            };
            shell.set_page(7, page, cx);
        });
        let remove = close_button(&shell, cx, 7);
        move_to(cx, remove);
        assert_eq!(drawn_button(&shell, cx), "remove");
        host.take_edits();
        cx.simulate_click(point(px(remove.0), px(remove.1)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(host.take_edits(), ["pinned 7 false", "close 7"]);
        let folders_after = Folders {
            parents: HashMap::from([(6, 5)]),
            ..folders
        };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![2, 5, 6], vec![4], Vec::new(), folders_after, cx)
        });
        shell.update(cx, |shell, cx| shell.run(Command::Undo, "ui", cx));
        assert_eq!(
            host.take_edits(),
            ["reopen 7 as 1001", "move 1001 true 5 1"]
        );
    }

    /// ⌘D, from the UI or the page, unpins the tab shown under a pointer that doesn't move, and
    /// the tab below slides up under it. A click there, as a trackpad tap makes, shows that tab,
    /// whether the rows are still on their way or not yet drawn where they moved, and with no
    /// row before it drawing a hover fill (#271).
    #[gpui::test]
    fn a_click_after_a_key_acts_on_the_row_under_the_pointer(cx: &mut TestAppContext) {
        cx.update(bind_keys);
        for (page, drawn) in [(false, false), (false, true), (true, false), (true, true)] {
            let (shell, cx, host) = window(cx, vec![2, 3], vec![4], 4);
            cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
            let (x, y) = close_button(&shell, cx, 2);
            let at = point(px(x - 100.), px(y));
            cx.simulate_mouse_move(at, None, Modifiers::none());
            cx.simulate_click(at, Modifiers::none());
            cx.run_until_parked();
            assert!(host.take().contains(&"select 2".into()));
            shell.update(cx, |shell, cx| shell.set_selected(2, cx));
            cx.run_until_parked();
            if page {
                shell.update(cx, |shell, cx| shell.run_for_page(Command::TogglePin, cx));
            } else {
                cx.simulate_keystrokes("cmd-d");
            }
            shell.update(cx, |shell, cx| {
                shell.set_tabs(1, vec![3], vec![2, 4], Vec::new(), Folders::default(), cx)
            });
            if drawn {
                cx.run_until_parked();
                assert_eq!(filled(&shell, cx), Vec::<u64>::new(), "page {page}");
            }
            cx.simulate_click(at, Modifiers::none());
            assert!(
                host.take().contains(&"select 3".into()),
                "page {page}, drawn {drawn}"
            );
        }
    }

    /// ⌘-click chooses rows along with the one shown and ⇧-click the rows through one, drawn with
    /// the secondary fill without showing them. ⇧⌘C copies their links, ⌘W closes them as one
    /// step, and a plain click or Esc lets them go, as Arc's do.
    #[gpui::test]
    fn command_and_shift_clicks_choose_rows_to_close_or_copy_together(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx, vec![2], vec![3, 4, 5, 6], 3);
        shell.update(cx, |shell, cx| {
            for tab in 2..=6 {
                let url = format!("https://{tab}.example/").into();
                shell.set_page(
                    tab,
                    Page {
                        url,
                        ..shell.page(tab)
                    },
                    cx,
                );
            }
        });
        let at = |cx: &mut VisualTestContext, tab| {
            let (x, y) = close_button(&shell, cx, tab);
            point(px(x - 100.), px(y))
        };
        let chosen =
            |cx: &mut VisualTestContext| shell.read_with(cx, |shell, _| shell.chosen_in_order());
        let command = cx.update(|_, cx| Modifiers::secondary_key(cx));
        host.take();
        let five = at(cx, 5);
        cx.simulate_click(five, command);
        assert_eq!(chosen(cx), [3, 5]);
        let six = at(cx, 6);
        cx.simulate_click(six, Modifiers::shift());
        assert_eq!(chosen(cx), [3, 5, 6]);
        assert!(!host.take().iter().any(|call| call.starts_with("select")));

        shell.update(cx, |shell, cx| shell.run(Command::CopyUrl, "keys", cx));
        let links = "copy https://3.example/\nhttps://5.example/\nhttps://6.example/";
        assert!(host.take().contains(&links.to_string()));
        shell.update(cx, |shell, cx| shell.run(Command::Stop, "keys", cx));
        assert!(chosen(cx).is_empty());

        cx.simulate_click(five, command);
        cx.simulate_click(six, command);
        let two = at(cx, 2);
        cx.simulate_click(two, Modifiers::shift());
        assert_eq!(chosen(cx), [2, 3, 4, 5, 6]);
        cx.simulate_click(two, command);
        host.take_edits();
        shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "keys", cx));
        assert_eq!(
            host.take_edits(),
            ["close 6", "close 5", "close 4", "close 3"]
        );
        assert!(chosen(cx).is_empty());
        shell.update(cx, |shell, cx| shell.undo(cx));
        let reopened = host.take_edits();
        assert!(reopened.len() == 4 && reopened.iter().all(|edit| edit.starts_with("reopen")));

        cx.simulate_click(five, command);
        assert_eq!(chosen(cx), [3, 5]);
        cx.simulate_click(six, Modifiers::none());
        assert!(chosen(cx).is_empty());
    }
}
