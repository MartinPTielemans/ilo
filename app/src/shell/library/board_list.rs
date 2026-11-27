//! Library › Boards (#3073): the window's profile's Boards, the last changed first, each opening
//! from its row, and Delete taking one aside, where ⌘Z brings it back from. The disk is read and
//! changed off the main thread (crate::boards), and no log line carries what a Board holds or is
//! named: its id is random, and the only thing of it a line names.

use std::path::PathBuf;

use gpui::{
    AnyElement, Context, MouseButton, MouseDownEvent, ObjectFit, Pixels, Point, div, img,
    prelude::*, px, svg,
};

use super::{
    ARCHIVED_FAVICON_X, ARCHIVED_LEFT, ARCHIVED_PITCH, ARCHIVED_PLATE, ARCHIVED_RADIUS,
    ARCHIVED_RIGHT, Edit, FAVICON_SIZE, Item, MenuEntry, POINTED_ROW, SECONDARY, STATE_PRESSED,
    Section, Shell, What,
    history_list::{history_text, history_when},
};
use crate::boards;
use crate::menu::Look;

impl Shell {
    /// The window's profile and the folder it keeps its Boards' in; none in a private window,
    /// which keeps no Boards.
    fn boards_profile(&self) -> Option<(u64, PathBuf)> {
        let profile = self.learning_profile()?;
        Some((profile, self.browser.profile_folder(profile)?))
    }

    /// Reads the window's profile's Boards off the main thread, and lists them again if they
    /// changed. A read asked for after it keeps its own list rather than this one's.
    pub(super) fn read_boards(&mut self, cx: &mut Context<Self>) {
        let Some((_, folder)) = self.boards_profile() else {
            self.board_list.clear();
            return;
        };
        self.boards_asked += 1;
        let asked = self.boards_asked;
        let read = cx.background_spawn(async move { boards::list(&folder) });
        cx.spawn(async move |shell, cx| {
            let listed = read.await;
            shell
                .update(cx, |shell, cx| {
                    if shell.boards_asked != asked || listed == shell.board_list {
                        return;
                    }
                    // check/board.sh reads this line.
                    log!("browser: library boards {}", listed.len());
                    shell.board_list = listed;
                    if shell.section == Section::Boards {
                        shell.filter(true, cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    /// The window's tabs that show Board `id`, or are pinned at it, in the order they opened.
    pub(super) fn board_tabs(&self, id: &str) -> Vec<u64> {
        let mut tabs: Vec<u64> = (self.tabs.iter())
            .filter(|(tab, each)| {
                boards::id_in(&each.page.url) == Some(id)
                    || boards::id_in(&self.browser.pinned_url(**tab)) == Some(id)
            })
            .map(|(tab, _)| *tab)
            .collect();
        tabs.sort_unstable();
        tabs
    }

    /// A Board's row: its picture, else the Board glyph, beside its name over when it changed. A
    /// click opens it; a right-click opens its menu.
    pub(super) fn board_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let (Some(library), Some(board)) = (&self.library, self.board_list.get(index)) else {
            return div().into_any_element();
        };
        let when = history_when(board.changed, library.starts, crate::clock::twelve_hour());
        let picture = match &board.thumb {
            // On the Board's paper in the window's look, as the page saves its picture with no
            // ground, so it shows in either look whichever it was taken in.
            Some(thumb) => div()
                .size(px(FAVICON_SIZE))
                .bg(gpui::rgb(match self.palette.tone {
                    crate::palette::Tone::Dark => crate::tokens::board::CANVAS_DARK,
                    crate::palette::Tone::Light => crate::tokens::board::CANVAS,
                }))
                .child(img(thumb.clone()).size_full().object_fit(ObjectFit::Cover))
                .into_any_element(),
            None => svg()
                .path("icons/presentation.svg")
                .size(px(FAVICON_SIZE))
                .text_color(self.palette.text(SECONDARY))
                .into_any_element(),
        };
        let (open, menu) = (board.id.clone(), board.id.clone());
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                div()
                    .id(("board", index))
                    .map(|plate| self.library_row(plate, Item::Board(index), 0, ARCHIVED_RADIUS))
                    .size_full()
                    .relative()
                    .flex()
                    .items_center()
                    .rounded(px(ARCHIVED_RADIUS))
                    .block_mouse_except_scroll()
                    .group(POINTED_ROW)
                    .hover(|style| style.bg(self.palette.hovered_plate()))
                    .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
                    .on_click(
                        cx.listener(move |shell, _, _, cx| shell.open_listed_board(&open, cx)),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            shell.board_menu(menu.clone(), event.position, cx)
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(ARCHIVED_FAVICON_X))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(picture),
                    )
                    .child(history_text(
                        board.name.clone().into(),
                        format!("Changed {when}").into(),
                        &self.palette,
                    )),
            )
            .into_any_element()
    }

    /// Shows Board `id`, as a click on its row in Library › Boards does: capture's toast's Open.
    pub fn open_board(&mut self, id: &str, cx: &mut Context<Self>) {
        self.open_listed_board(id, cx);
    }

    /// A click on a Board's row shows the window's tab of it, if it has one, and otherwise opens
    /// it in a new tab named as its row was.
    pub(super) fn open_listed_board(&mut self, id: &str, cx: &mut Context<Self>) {
        let open = self.board_tabs(id).first().copied();
        // check/board.sh reads this line.
        log!(
            "browser: library board {id} {}",
            if open.is_some() { "shown" } else { "opened" }
        );
        match open {
            Some(tab) => self.show_row(tab, cx),
            None => {
                let name = (self.board_list.iter())
                    .find(|board| board.id == id)
                    .map(|board| board.name.clone());
                self.open_board_tab(id, name, cx);
            }
        }
        self.set_library(false, cx);
    }

    /// Opens Board `id` in a new tab, named `name` as its row is, unless that's the product's.
    fn open_board_tab(&mut self, id: &str, name: Option<String>, cx: &mut Context<Self>) {
        let tab = self.browser.open_at(&[boards::url(id)], false, 0, 0, cx);
        if let Some(name) = name.filter(|name| tab != 0 && name != "Board") {
            self.browser.rename_tab(tab, &name, cx);
        }
    }

    /// Duplicate, on a Board's row or from the bar over its tab: a copy of Board `id`, with its
    /// cards and pictures, opens in a new tab named as its row once it's whole on disk, and the
    /// Library closes as on Open. It isn't a ⌘Z step: Delete on the copy is the way out, and says
    /// what it does.
    pub(in crate::shell) fn duplicate_board(&mut self, id: String, cx: &mut Context<Self>) {
        let Some((_, folder)) = self.boards_profile() else {
            return;
        };
        let copy = boards::new_id();
        let made = cx.background_spawn({
            let (id, copy) = (id.clone(), copy.clone());
            async move { boards::duplicate(&folder, &id, &copy) }
        });
        cx.spawn(async move |shell, cx| {
            let made = made.await;
            shell
                .update(cx, |shell, cx| match made {
                    Ok(name) => {
                        // check/board.sh reads this line.
                        log!("browser: board {id} duplicated as {copy}");
                        shell.open_board_tab(&copy, Some(name), cx);
                        shell.set_library(false, cx);
                        shell.read_boards(cx);
                    }
                    Err(error) => log!("browser: board {id} duplicate {copy} failed: {error}"),
                })
                .ok();
        })
        .detach();
    }

    /// A Board's menu in Library › Boards: Open, Duplicate, and Delete, which ⌘Z takes back.
    pub(super) fn board_menu(
        &mut self,
        id: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let entries = vec![
            MenuEntry::new("Open", true),
            MenuEntry::new("Duplicate", true),
            MenuEntry::line(),
            MenuEntry::new("Delete", true).destructive(),
        ];
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            Look {
                light: false,
                ..self.menu_look()
            },
            Box::new(move |chosen, cx| {
                shell
                    .update(cx, |shell, cx| match chosen {
                        Some(0) => shell.open_listed_board(&id, cx),
                        Some(1) => shell.duplicate_board(id.clone(), cx),
                        Some(3) => shell.delete_board(id.clone(), cx),
                        _ => {}
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// Delete on a Board's row: a step of the window's ⌘Z history, as Delete Folder is.
    pub(super) fn delete_board(&mut self, id: String, cx: &mut Context<Self>) {
        let Some((profile, _)) = self.boards_profile() else {
            return;
        };
        let edit = self.take_board(profile, id, cx);
        self.keep(What::DELETE_BOARD, edit, cx);
    }

    /// Takes Board `id` of `profile` out of the list, into the folder ⌘Z brings it back from,
    /// off the main thread, and closes the window's tabs of it, whose page would find nothing.
    /// The step that brings it all back.
    pub(in crate::shell) fn take_board(
        &mut self,
        profile: u64,
        id: String,
        cx: &mut Context<Self>,
    ) -> Option<Edit> {
        let folder = self.browser.profile_folder(profile)?;
        // check/board.sh reads this line.
        log!("browser: board {id} deleted");
        // A pinned row is removed, as its remove button does; a Today tab closes. A split's pane
        // that went with its row is no longer there.
        let tabs: Vec<Edit> = (self.board_tabs(&id).into_iter())
            .filter_map(|tab| {
                self.spot_of(tab)?;
                (Edit::Remove(tab).apply(self, cx))
                    .or_else(|| Edit::Close(vec![tab]).apply(self, cx))
            })
            .collect();
        let removed = cx.background_spawn({
            let id = id.clone();
            async move { boards::remove(&folder, &id) }
        });
        cx.spawn({
            let id = id.clone();
            async move |shell, cx| {
                let removed = removed.await;
                shell
                    .update(cx, |shell, cx| {
                        if let Err(error) = removed {
                            log!("browser: board {id} delete failed: {error}");
                        }
                        shell.read_boards(cx);
                    })
                    .ok();
            }
        })
        .detach();
        Some(Edit::RestoreBoard { profile, id, tabs })
    }

    /// ⌘Z on a Delete: the Board's folder comes back off the main thread, and only then its tabs,
    /// the last closed first, so their pages find it.
    pub(in crate::shell) fn restore_board(
        &mut self,
        profile: u64,
        id: String,
        tabs: Vec<Edit>,
        cx: &mut Context<Self>,
    ) {
        let Some(folder) = self.browser.profile_folder(profile) else {
            return;
        };
        // check/board.sh reads this line.
        log!("browser: board {id} restored");
        let restored = cx.background_spawn({
            let id = id.clone();
            async move { boards::restore(&folder, &id) }
        });
        cx.spawn(async move |shell, cx| {
            let restored = restored.await;
            shell
                .update(cx, |shell, cx| {
                    if let Err(error) = restored {
                        log!("browser: board {id} restore failed: {error}");
                    }
                    for tab in tabs.into_iter().rev() {
                        tab.apply(shell, cx);
                    }
                    shell.read_boards(cx);
                })
                .ok();
        })
        .detach();
    }

    /// A Board's row renamed keeps its name in the Board's folder, for Library › Boards, off the
    /// main thread.
    pub(in crate::shell) fn keep_board_name(
        &mut self,
        tab: u64,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        let url = (self.tabs.get(&tab)).map_or(String::new(), |each| each.page.url.to_string());
        let pinned = self.browser.pinned_url(tab);
        let Some(id) = (boards::id_in(&url).or(boards::id_in(&pinned))).map(str::to_owned) else {
            return;
        };
        let Some((_, folder)) = self.boards_profile() else {
            return;
        };
        let written = cx.background_spawn({
            let (id, name) = (id.clone(), name.to_owned());
            async move { boards::write_name(&folder, &id, &name) }
        });
        cx.spawn(async move |shell, cx| {
            if let Err(error) = written.await {
                log!("browser: board {id} name not kept: {error}");
            }
            shell.update(cx, |shell, cx| shell.read_boards(cx)).ok();
        })
        .detach();
    }
}
