//! The capture menu's Boards (#3073): Add to Board's submenu of the profile's Boards, which opens
//! beside the menu as menu.rs opens one, with its safe triangle and its keys, and what the core
//! answers of a capture put on a Board. No log line names a Board, says how many there are, or
//! where one is in the list: the log is no record of the user's Boards.

use super::*;
use crate::tokens::menu::{MAX_WIDTH, OVERLAP, SUBMENU_REST};

/// The most Boards the submenu lists, the last changed first, so it fits any page: an eleventh is
/// reached by opening it.
pub(super) const LISTED: usize = 10;
/// A Board takes no more while this many captures wait to land on it, which they do when it opens.
/// It's the core's `board::kMaxArrived` (core/board_store.h), which refuses the next one.
pub(super) const MOST_WAITING: usize = 100;
/// What the log says of the toasts that name a Board, in place of their words.
const ADDED: &str = "Added to a Board";
const NO_ROOM: &str = "A Board has no room";

/// A Board of the profile's: its id, which the core takes, and its name, which only the menu and
/// its toasts show.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub id: String,
    pub name: SharedString,
}

/// A capture put on a Board, waiting for the core's word on it, in the order they were put.
pub(super) struct Boarding {
    board: Board,
    png: Rc<Vec<u8>>,
    /// New Board's, whose Board opens with it: only a refusal says anything.
    new: bool,
}

/// What the core said of a capture put on Board `id`: whether it arrived.
pub struct BoardAnswer<'a> {
    pub id: &'a str,
    pub arrived: bool,
}

/// What the pointer is over in the menu and the submenu.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    Row(Action),
    /// The menu's panel off its rows, or the submenu's (`true`).
    Panel(bool),
    Outside,
}

/// The Boards of the profile of `tab`'s page, the last changed first, which Add to Board lists.
pub fn boards_listed(tab: u64, mut boards: Vec<Board>, cx: &mut App) {
    update(cx, |capture, window, cx| {
        let Some(session) = capture
            .session
            .as_mut()
            .filter(|session| session.tab == tab)
        else {
            return;
        };
        boards.truncate(LISTED);
        let system = window.text_system().clone();
        let widest = (boards.iter())
            .map(|board| crate::tooltip::text_width(&board.name, TEXT, FontWeight::NORMAL, &system))
            .fold(0., f32::max);
        session.boards_width =
            (2. * (PADDING + ROW_X) + GLYPH + GAP + widest).clamp(MENU_WIDTH, MAX_WIDTH);
        session.boards = boards;
        // check.sh reads this line.
        log!("browser: capture boards listed");
        cx.notify();
    });
}

/// The core's word on a capture put on a Board: a toast says it's there, as Save to Library's
/// does, or that the Board took none of it, naming the Board and what to do.
pub fn board_answered(answer: BoardAnswer, cx: &mut App) {
    update(cx, |capture, _, cx| capture.board_answered(answer, cx));
}

impl Session {
    /// The menu's panel, in the window, which covers the page.
    fn menu_bounds(&self, chosen: Bounds<Pixels>) -> Bounds<Pixels> {
        let origin = self.menu_origin(chosen);
        Bounds::new(origin, size(px(MENU_WIDTH), px(self.menu_height())))
    }

    /// The menu's rows, its field first, each where it is in the window.
    fn menu_rows(&self, chosen: Bounds<Pixels>) -> Vec<(Action, Bounds<Pixels>)> {
        let origin = self.menu_origin(chosen);
        let row = |y: f32| {
            Bounds::new(
                point(origin.x + px(PADDING), origin.y + px(y)),
                size(px(MENU_WIDTH - 2. * PADDING), px(ROW)),
            )
        };
        let mut rows = vec![(Action::SendTo, row(PADDING))];
        let mut y = PADDING + ROW + LINE;
        for each in self.rows() {
            match each {
                Some(action) => {
                    rows.push((action, row(y)));
                    y += ROW;
                }
                None => y += LINE,
            }
        }
        rows
    }

    /// Add to Board's submenu, as menu.rs places one: OVERLAP over the menu's right edge with its
    /// first row level with Add to Board's, or over its left edge where the page has no room on
    /// the right, inside the page.
    pub(super) fn submenu_bounds(&self, chosen: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
        let (_, parent) = (self.menu_rows(chosen).into_iter())
            .find(|(action, _)| *action == Action::AddToBoard)?;
        let (menu, page) = (self.menu_bounds(chosen), self.page());
        let (width, height) = (
            self.boards_width,
            2. * PADDING + ROW * self.boards.len() as f32,
        );
        let right = f32::from(menu.right()) - OVERLAP;
        let x = match right + width > f32::from(page.size.width) - MARGIN {
            true => f32::from(menu.left()) + OVERLAP - width,
            false => right,
        };
        let y = f32::from(parent.top()) - PADDING;
        let bottom = f32::from(page.size.height) - height - MARGIN;
        Some(Bounds::new(
            point(px(x.max(MARGIN)), px(y.min(bottom).max(MARGIN))),
            size(px(width), px(height)),
        ))
    }

    /// The submenu's rows, each where it is in the window.
    fn board_rows(&self, submenu: Bounds<Pixels>) -> Vec<(Action, Bounds<Pixels>)> {
        (0..self.boards.len())
            .map(|at| {
                let top = submenu.top() + px(PADDING + ROW * at as f32);
                let width = submenu.size.width - px(2. * PADDING);
                let row = Bounds::new(
                    point(submenu.left() + px(PADDING), top),
                    size(width, px(ROW)),
                );
                (Action::Board(at), row)
            })
            .collect()
    }

    /// What's at `at`, while the menu shows.
    fn hit(&self, at: Point<Pixels>) -> Hit {
        let Some(chosen) = self.chosen.filter(|_| self.editing.is_none()) else {
            return Hit::Outside;
        };
        if let Some(submenu) = self.submenu_bounds(chosen).filter(|_| self.submenu) {
            let rows = self.board_rows(submenu);
            if let Some((action, _)) = rows.iter().find(|(_, row)| row.contains(&at)) {
                return Hit::Row(*action);
            }
            if submenu.contains(&at) {
                return Hit::Panel(true);
            }
        }
        let rows = self.menu_rows(chosen);
        if let Some((action, _)) = rows.iter().find(|(_, row)| row.contains(&at)) {
            return Hit::Row(*action);
        }
        match self.menu_bounds(chosen).contains(&at) {
            true => Hit::Panel(false),
            false => Hit::Outside,
        }
    }

    /// Lights what's at `at` and opens or closes the submenu, as menu.rs's `point_at` does: on
    /// Add to Board it opens, on another of the menu's rows or its panel it closes, and over the
    /// submenu or off the menu it stays as it is, Add to Board lit while it shows.
    fn point_at(&mut self, at: Point<Pixels>) {
        match self.hit(at) {
            Hit::Row(Action::AddToBoard) => {
                self.lit = Some(Action::AddToBoard);
                self.submenu = true;
            }
            Hit::Row(board @ Action::Board(_)) => self.lit = Some(board),
            Hit::Row(action) => {
                self.lit = Some(action);
                self.submenu = false;
            }
            Hit::Panel(true) => self.lit = None,
            Hit::Panel(false) => {
                self.lit = None;
                self.submenu = false;
            }
            Hit::Outside => self.lit = None,
        }
    }

    /// Whether the pointer's move from `from` to `to` is on its way to the open submenu: from Add
    /// to Board, inside the triangle from where it was to the submenu's two near corners, off Add
    /// to Board's row and not yet in the submenu, as menu.rs's `safe` reads it.
    fn safe(&self, from: Point<Pixels>, to: Point<Pixels>) -> bool {
        let Some(chosen) = self.chosen else {
            return false;
        };
        let Some(submenu) = self.submenu_bounds(chosen).filter(|_| self.submenu) else {
            return false;
        };
        if from == to
            || submenu.contains(&to)
            || self.lit != Some(Action::AddToBoard)
            || self.hit(to) == Hit::Row(Action::AddToBoard)
        {
            return false;
        }
        let x = match submenu.left() >= self.menu_bounds(chosen).center().x {
            true => submenu.left(),
            false => submenu.right(),
        };
        crate::menu::in_triangle(
            to,
            from,
            point(x, submenu.top()),
            point(x, submenu.bottom()),
        )
    }

    /// The rows drawn lit: the one lit, and Add to Board while its submenu shows, as a
    /// submenu's parent row stays lit in menu.rs, so the two read as one menu.
    pub(super) fn lit_rows(&self) -> [Option<Action>; 2] {
        [self.lit, self.submenu.then_some(Action::AddToBoard)]
    }

    /// The rows the keys move through: the submenu's while it shows, else the menu's.
    fn key_rows(&self) -> Vec<Action> {
        match self.submenu {
            true => (0..self.boards.len()).map(Action::Board).collect(),
            false => std::iter::once(Action::SendTo)
                .chain(self.rows().flatten())
                .collect(),
        }
    }
}

impl Capture {
    /// The pointer moved to `at` over capture mode while its menu shows: what's under it lights,
    /// unless it's on its way to the submenu, where what's under it lights once it rests there.
    pub(super) fn point(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut().filter(|session| {
            session.chosen.is_some() && session.editing.is_none() && session.pointer != Some(at)
        }) else {
            return;
        };
        let from = session.pointer.replace(at);
        session.moves += 1;
        if from.is_some_and(|from| session.safe(from, at)) {
            let moves = session.moves;
            cx.spawn(async move |capture, cx| {
                cx.background_executor()
                    .timer(Duration::from_secs_f32(SUBMENU_REST))
                    .await;
                capture
                    .update(cx, |capture, cx| {
                        let rested = (capture.session.as_ref())
                            .filter(|session| session.moves == moves)
                            .and_then(|session| session.pointer);
                        if let Some(at) = rested {
                            capture.light(|session| session.point_at(at), cx);
                        }
                    })
                    .ok();
            })
            .detach();
            return;
        }
        self.light(|session| session.point_at(at), cx);
    }

    /// Changes what's lit and whether the submenu shows with `change`, and says so.
    pub(super) fn light(&mut self, change: impl FnOnce(&mut Session), cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let before = (session.lit, session.submenu);
        change(session);
        if (session.lit, session.submenu) == before {
            return;
        }
        if session.submenu != before.1 {
            // check.sh reads this line.
            log!(
                "browser: capture boards {}",
                if session.submenu { "shown" } else { "hidden" }
            );
        }
        if session.lit != before.0 {
            // check.sh reads this line.
            log!(
                "browser: capture menu lit {}",
                session.lit.map_or("nothing", Action::name)
            );
        }
        cx.notify();
    }

    /// ↑ and ↓ move what's lit through the deepest rows that show, staying at the ends, as
    /// NSMenu's and menu.rs's do; from none, ↓ lights the first and ↑ the last.
    pub(super) fn step(&mut self, down: bool, cx: &mut Context<Self>) {
        if !self.menu_keys() {
            return;
        }
        self.light(
            |session| {
                let rows = session.key_rows();
                let at = session
                    .lit
                    .and_then(|lit| rows.iter().position(|&row| row == lit));
                let to = match (at, down) {
                    (Some(at), true) => (at + 1).min(rows.len().saturating_sub(1)),
                    (Some(at), false) => at.saturating_sub(1),
                    (None, true) => 0,
                    (None, false) => rows.len().saturating_sub(1),
                };
                if let Some(&row) = rows.get(to) {
                    session.lit = Some(row);
                    if !matches!(row, Action::Board(_)) {
                        session.submenu = row == Action::AddToBoard && session.submenu;
                    }
                }
            },
            cx,
        );
    }

    /// → or ↩ on Add to Board opens its submenu, its first Board lit, as menu.rs's
    /// `open_submenu` does. Returns whether it did.
    pub(super) fn open_submenu(&mut self, cx: &mut Context<Self>) -> bool {
        let opens = self.menu_keys()
            && (self.session.as_ref()).is_some_and(|s| s.lit == Some(Action::AddToBoard));
        if opens {
            self.light(
                |session| {
                    session.submenu = true;
                    session.lit = Some(Action::Board(0));
                },
                cx,
            );
        }
        opens
    }

    /// ← or esc with the submenu open closes only it, Add to Board lit again. Returns whether it
    /// did.
    pub(super) fn close_submenu(&mut self, cx: &mut Context<Self>) -> bool {
        let closes = self.menu_keys() && (self.session.as_ref()).is_some_and(|s| s.submenu);
        if closes {
            self.light(
                |session| {
                    session.submenu = false;
                    session.lit = Some(Action::AddToBoard);
                },
                cx,
            );
        }
        closes
    }

    /// ↩ with a row lit: Add to Board opens its submenu, and another row does what it says.
    pub(super) fn choose(&mut self, cx: &mut Context<Self>) {
        if self.open_submenu(cx) || !self.menu_keys() {
            return;
        }
        if let Some(lit) = self.session.as_ref().and_then(|session| session.lit) {
            self.run(lit, cx);
        }
    }

    /// Whether the keys are the menu's: it shows, with nothing on its way.
    fn menu_keys(&self) -> bool {
        self.session.as_ref().is_some_and(|session| {
            session.chosen.is_some() && session.editing.is_none() && session.pending.is_none()
        })
    }

    /// Puts `png` onto `board`, for the core's word on it in `board_answered`.
    pub(super) fn add_to_board(
        &mut self,
        tab: u64,
        board: Board,
        png: Vec<u8>,
        cx: &mut Context<Self>,
    ) {
        let png = Rc::new(png);
        if !self.host.add_to_board(tab, &board, &png) {
            return self.refused(board, png, cx);
        }
        self.boarding.push_back(Boarding {
            board,
            png,
            new: false,
        });
    }

    /// New Board with `png` on it: a Board of its own, named as New Board's are until its row's
    /// field names it.
    pub(super) fn new_board(&mut self, tab: u64, png: Vec<u8>) {
        let board = Board {
            id: crate::boards::new_id(),
            name: "Board".into(),
        };
        self.host.new_board(tab, &board.id, &png);
        self.boarding.push_back(Boarding {
            board,
            png: Rc::new(png),
            new: true,
        });
    }

    fn board_answered(&mut self, answer: BoardAnswer, cx: &mut Context<Self>) {
        let Some(at) = (self.boarding.iter()).position(|boarding| boarding.board.id == answer.id)
        else {
            return;
        };
        let Some(boarding) = self.boarding.remove(at) else {
            return;
        };
        // check.sh reads this line.
        log!(
            "browser: capture board {}",
            if answer.arrived { "arrived" } else { "refused" }
        );
        match (answer.arrived, boarding.new) {
            (true, true) => {}
            (true, false) => {
                let text = format!("Added to {}", boarding.board.name);
                cx.defer(move |cx| toast::show_logged_as(&text, ADDED, Vec::new(), TOAST_TIME, cx));
            }
            (false, _) => self.refused(boarding.board, boarding.png, cx),
        }
    }

    /// The Board took none of the capture: a toast names it and says what to do, with Open to go
    /// there and Copy to keep the capture another way, as nothing of it was stored.
    fn refused(&mut self, board: Board, png: Rc<Vec<u8>>, cx: &mut Context<Self>) {
        let open = {
            let (host, id) = (self.host.clone(), board.id.clone());
            toast::Button {
                label: toast::Label::Text("Open".into()),
                name: "open",
                run: Rc::new(move |_, _| host.open_board(&id)),
            }
        };
        let copy = {
            let host = self.host.clone();
            toast::Button {
                label: toast::Label::Text("Copy".into()),
                name: "copy",
                run: Rc::new(move |_, cx| {
                    host.copy(&png);
                    toast::show(COPIED, None, TOAST_TIME, cx);
                }),
            }
        };
        let text = format!(
            "{} has {MOST_WAITING} new captures you haven't seen. Open it, then add this again.",
            board.name
        );
        let buttons = vec![open, copy];
        cx.defer(move |cx| toast::show_logged_as(&text, NO_ROOM, buttons, TOAST_TIME, cx));
    }

    /// Add to Board's submenu, beside the menu at `origin`, while it shows.
    pub(super) fn submenu(
        &self,
        session: &Session,
        chosen: Bounds<Pixels>,
        look: Look,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let at = session.submenu_bounds(chosen).filter(|_| session.submenu)?;
        let origin = session.menu_origin(chosen);
        let (palette, highlight) = (look.palette(), look.plate());
        let rows = (session.boards.iter().enumerate()).map(|(at, board)| {
            let action = Action::Board(at);
            let lit = session.lit == Some(action);
            div()
                .id(("board", at))
                .h(px(ROW))
                .mx(px(PADDING))
                .px(px(ROW_X))
                .flex()
                .items_center()
                .gap(px(GAP))
                .rounded(px(ROW_RADIUS))
                .when(lit, |row| {
                    row.bg(highlight.fill).text_color(highlight.label)
                })
                .active(|style| style.bg(highlight.pressed))
                .on_click(cx.listener(move |capture, _, _, cx| capture.run(action, cx)))
                .child(
                    svg()
                        .path(action.glyph())
                        .size(px(GLYPH))
                        .flex_none()
                        .text_color(match lit {
                            true => highlight.label,
                            false => rgba(palette.secondary),
                        }),
                )
                .child(div().min_w_0().truncate().child(board.name.clone()))
        });
        Some(
            div()
                .id("capture-boards")
                .absolute()
                .left(at.left() - origin.x)
                .top(at.top() - origin.y)
                .w(at.size.width)
                .py(px(PADDING))
                .flex()
                .flex_col()
                .rounded(px(RADIUS))
                .bg(look.fill())
                .shadow(Level::Popover.shadow())
                .occlude()
                .on_mouse_move(cx.listener(|capture, event: &MouseMoveEvent, _, cx| {
                    capture.point(event.position, cx)
                }))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(rgba(palette.rim)),
                )
                .children(rows)
                .into_any_element(),
        )
    }
}
