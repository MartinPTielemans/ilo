//! The plus and its add menu: one form for each field a thread is asked from, the bar's composer
//! and the reply field under a thread. The field gives the rows; with none the plus stays in its
//! place, disabled, taking no press and showing nothing (the lead's call, 24 Nov). Each row is a
//! `MenuEntry` and what its pick does.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, Bounds, EntityId, Hsla, PathPromptOptions, Pixels, Role, Window, canvas, div, point,
    prelude::*, px, svg,
};

use crate::menu::{Closed, EntriesMenu};
use crate::shell::MenuEntry;

/// FP1: the plus, spoken, its menu's own name.
pub const NAME: &str = "Add";
/// Z11.
pub const FILE_OR_FOLDER: &str = "File or Folder";
const FILE_OR_FOLDER_ICON: &str = "icons/file.svg";

/// The rows of a row's second menu, listed as the add menu opens.
type Under = Box<dyn FnOnce(&mut App) -> Vec<MenuEntry>>;

/// A row of the add menu: its entry, and what a pick of it does; or the rows of the second menu
/// it opens, each of which does its own (`MenuEntry::stays`).
pub struct Row {
    entry: MenuEntry,
    picked: Option<Box<dyn FnOnce(&mut Window, &mut App)>>,
    under: Option<Under>,
}

impl Row {
    pub fn new(entry: MenuEntry, picked: impl FnOnce(&mut Window, &mut App) + 'static) -> Self {
        Row {
            entry,
            picked: Some(Box::new(picked)),
            under: None,
        }
    }

    /// A row that opens a second menu, as the user's tabs' does (`crate::tab_picks`): `under`
    /// lists its rows once, as the add menu opens and not before, and with none the row is left
    /// out of the menu.
    pub fn over(
        entry: MenuEntry,
        under: impl FnOnce(&mut App) -> Vec<MenuEntry> + 'static,
    ) -> Self {
        Row {
            entry,
            picked: None,
            under: Some(Box::new(under)),
        }
    }
}

/// Z11's row: the system's panel, for files and folders, several at once. Once it closes,
/// `added` gets what the user picked there, none on Cancel.
pub fn file_or_folder(added: impl FnOnce(Vec<PathBuf>, &mut App) + 'static) -> Row {
    let entry = MenuEntry::new(FILE_OR_FOLDER, true).icon(FILE_OR_FOLDER_ICON);
    Row::new(entry, move |_, cx| {
        let asked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: true,
            // The system's own word on its button: the table gives no other.
            prompt: None,
        });
        cx.spawn(async move |cx| {
            let paths = (asked.await.ok())
                .and_then(Result::ok)
                .flatten()
                .unwrap_or_default();
            cx.update(|cx| added(paths, cx));
        })
        .detach();
    })
}

/// How a field draws its plus: a square `side` wide at `radius`, its glyph `glyph` wide in `ink`,
/// on `open` while its menu is open, and none at rest; at `off` of its ink with nothing to add.
/// Its menu hangs `drop` under it.
pub struct Look {
    pub side: f32,
    pub radius: f32,
    pub glyph: f32,
    pub ink: Hsla,
    pub open: Hsla,
    pub off: f32,
    pub drop: f32,
    /// Whether a screen reader is on, for its role and name.
    pub a11y: bool,
}

/// A field's plus, kept by the view that draws it: where it was last drawn, for its menu to hang
/// from, and whether its menu is open.
#[derive(Clone, Default)]
pub struct Plus {
    drawn: Rc<Cell<Option<Bounds<Pixels>>>>,
    open: Rc<Cell<bool>>,
}

impl Plus {
    /// The plus `id` over `rows`, in `view`, which is drawn again as its menu opens and closes. A
    /// click shows its menu; with no row it is drawn disabled and takes no press.
    pub fn draw(
        &self,
        id: &'static str,
        rows: Vec<Row>,
        look: &Look,
        view: EntityId,
    ) -> gpui::Stateful<gpui::Div> {
        let live = !rows.is_empty();
        let rows = RefCell::new(rows);
        let (plus, drawn) = (self.clone(), self.drawn.clone());
        let drop = look.drop;
        div()
            .id(id)
            .relative()
            .flex_none()
            .size(px(look.side))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(look.radius))
            .when(self.open.get(), |shown| shown.bg(look.open))
            .when(look.a11y, |plus| {
                plus.role(Role::Button)
                    .aria_label(NAME)
                    .when(!live, |plus| {
                        plus.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
                    })
            })
            .when(live, |plus_box| {
                plus_box.on_click(move |_, window, cx| {
                    plus.open(rows.take(), view, drop, window, cx);
                })
            })
            .child(
                svg()
                    .flex_none()
                    .path("icons/plus.svg")
                    .size(px(look.glyph))
                    .text_color(look.ink)
                    .when(!live, |glyph| glyph.opacity(look.off)),
            )
            .child(
                canvas(move |bounds, _, _| drawn.set(Some(bounds)), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
    }

    /// Shows the menu of `rows` `drop` under the plus as last drawn, as a click on it does: a pick
    /// runs its row. Nothing with no row, or before the plus is drawn.
    pub fn open(
        &self,
        rows: Vec<Row>,
        view: EntityId,
        drop: f32,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (Some(drawn), Some(menu)) = (self.drawn.get(), cx.try_global::<EntriesMenu>()) else {
            return;
        };
        let menu = menu.0.clone();
        // Each row, then those of its second menu under it by its place.
        let (mut entries, mut picks) = (Vec::new(), Vec::new());
        for row in rows {
            let under = row.under.map(|list| list(cx));
            if under.as_ref().is_some_and(Vec::is_empty) {
                continue;
            }
            let parent = entries.len();
            entries.push(row.entry);
            picks.push(row.picked);
            for under in under.into_iter().flatten() {
                entries.push(MenuEntry {
                    parent: Some(parent),
                    ..under
                });
                picks.push(None);
            }
        }
        if entries.is_empty() {
            return;
        }
        self.open.set(true);
        cx.notify(view);
        let open = self.open.clone();
        let chosen = Box::new(move |closed: Closed, window: &mut Window, cx: &mut App| {
            open.set(false);
            cx.notify(view);
            let picked = closed.chosen().and_then(|at| picks.get_mut(at)?.take());
            if let Some(picked) = picked {
                picked(window, cx);
            }
        });
        let at = point(drawn.left(), drawn.bottom() + px(drop));
        menu(entries, at, false, window, chosen, cx);
    }

    /// Where it was last drawn, if the last frame drew it.
    pub fn drawn(&self) -> Option<Bounds<Pixels>> {
        self.drawn.get()
    }

    /// Forgets where it was drawn, before a frame that may not draw it, so its menu never hangs
    /// from where it no longer is.
    pub fn forget(&self) {
        self.drawn.set(None);
    }
}

#[cfg(test)]
mod tests;
