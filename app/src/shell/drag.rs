//! Rows and tiles pressed and dragged: reordered, into and out of folders and the
//! favorites, to another Space, and over the page for a split view. And links and files
//! dragged onto the list from the page or another app, which open there.

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use gpui::{
    Context, DispatchPhase, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Window, canvas, point, prelude::*, px,
};

use super::{
    Closing, Curve, DRAG_START, Drag, Edit, FOLDER_INDENT, HeldRow, NAME_HEIGHT, Opened,
    PLATE_HEIGHT, PLATE_INSET, PLATE_RADIUS, PRESS_SCALE, PRESS_TIME, Place, ROW_HEIGHT, ROW_PITCH,
    Shape, Shell, Slot, TILE_HEIGHT, TILE_RADIUS, What, files, slots,
};

/// The tab that links or files dragged in stand for until they're dropped: none, as the core
/// counts.
const INCOMING: u64 = 0;
/// A tab held this close to the sidebar's left or right edge for EDGE_WAIT pages to the Space on
/// that side, and on again after as long (ours: Arc's isn't measured).
const EDGE: f32 = 24.;
pub(super) const EDGE_WAIT: Duration = Duration::from_millis(500);
/// How long the pointer rests on the middle half of another tab's row before the dragged tab
/// would join it in a split view, so passing rows still reorders (ours, interim: Arc's isn't
/// measured).
pub(super) const SPLIT_REST: Duration = Duration::from_millis(400);

impl Shell {
    /// Starts the list's rows from where they were in `before` to where they are now: rows still
    /// there slide, a new tab's row is in its place at once with its plate fading in, several new
    /// ones fade in whole, and a closed one fades where it was (docs/ux-spec.md › Tab rows).
    pub(super) fn move_rows(&mut self, before: HashMap<Slot, f32>, curve: Curve) {
        let now = Instant::now();
        let after = self.layout();
        let slides = after
            .iter()
            .filter_map(|(&slot, &top)| {
                let from = before.get(&slot)? + self.slide(slot) - top;
                (from != 0.).then_some((slot, (from, now, curve)))
            })
            .collect();
        let mut opened = after
            .iter()
            .filter_map(|(&slot, &top)| match slot {
                Slot::Tab(tab) if !before.contains_key(&slot) => Some((top, tab)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !opened.is_empty() {
            opened.sort_by(|(a, _), (b, _)| a.total_cmp(b));
            let tabs: Vec<u64> = opened.into_iter().map(|(_, tab)| tab).collect();
            // Rows a folder shows as it opens come whole, as several new tabs do.
            let whole = tabs.len() > 1 || tabs.iter().any(|&tab| self.depth(tab) > 0);
            if !whole {
                self.scroll_to_row(tabs[0]);
            }
            self.opened = Some(Opened { tabs, whole, start: now });
        }
        for (&slot, &top) in &before {
            let Slot::Tab(tab) = slot else { continue };
            // One gone to the favorites is a tile now.
            if !after.contains_key(&slot) && !self.favorites.contains(&tab) {
                let space = self.shown_space();
                let folder = space.filter(|space| space.is_folder(tab));
                self.closing.push(Closing {
                    title: self.label(tab),
                    favicon: self.tabs.get(&tab).and_then(|tab| tab.favicon.clone()),
                    icon: self.page(tab).icon,
                    folder: folder.map(|space| space.is_open(tab)),
                    indent: self.depth(tab) as f32 * FOLDER_INDENT,
                    top: top + self.slide(slot),
                    start: now,
                });
            }
        }
        self.slides = slides;
    }

    /// How far a row is drawn from its place: where the pointer has a dragged one, and others while
    /// they slide there.
    pub(super) fn slide(&self, slot: Slot) -> f32 {
        let drag = self.drag.as_ref().filter(|drag| drag.moving);
        if let Some(drag) = drag.filter(|drag| slot == Slot::Tab(drag.tab)) {
            return self.drag_at(drag) - drag.top;
        }
        self.slides.get(&slot).map_or(0., |&(from, start, curve)| {
            curve.at(start.elapsed().as_secs_f32()).map_or(0., |done| from * (1. - done))
        })
    }

    /// Where a dragged tab's row is drawn in the list: with the pointer, held between the list's
    /// first and last places.
    pub(super) fn drag_at(&self, drag: &Drag) -> f32 {
        let (first, last) = drag.span;
        (self.list_y(drag.pointer.y) - drag.grab.0).clamp(first, last)
    }

    /// A tab dragged across the favorites' edge, mid-turn between its row and its tile: its
    /// plate's left, top, width, height and radius in the sidebar, and how far it has turned. The
    /// point the pointer holds goes from where it held the row to where it holds the tile, so the
    /// plate grows and shrinks around the pointer.
    pub(super) fn turning_shape(&self) -> Option<(u64, [f32; 5], f32)> {
        let drag = self.drag.as_ref().filter(|drag| drag.moving && drag.shape.start.is_some())?;
        if drag.incoming || drag.over_page.is_some() {
            return None;
        }
        let indent = drag.depth as f32 * FOLDER_INDENT;
        let scrolled = f32::from(self.list_scroll.offset().y);
        let row_top = self.drawn_list_top() + scrolled + self.drag_at(drag);
        let row = [
            PLATE_INSET + indent,
            row_top + (ROW_HEIGHT - PLATE_HEIGHT) / 2.,
            self.width() - 2. * PLATE_INSET - indent,
            PLATE_HEIGHT,
            PLATE_RADIUS,
        ];
        let others = self.favorites.iter().filter(|&&tab| tab != drag.tab).count();
        let width = self.tile_places(others + 1)[0][2];
        let centre = drag.pointer - drag.grab.1;
        let tile = [
            centre.x - width / 2.,
            centre.y - TILE_HEIGHT / 2.,
            width,
            TILE_HEIGHT,
            TILE_RADIUS,
        ];
        let turned = drag.shape.at(Instant::now());
        let blend = |row: f32, tile: f32| row + (tile - row) * turned;
        let (width, height) = (blend(row[2], tile[2]), blend(row[3], tile[3]));
        let pointer = drag.pointer;
        let held = point(
            blend(pointer.x - row[0], pointer.x - tile[0]),
            blend(pointer.y - row[1], pointer.y - tile[1]),
        );
        let radius = blend(row[4], tile[4]);
        Some((drag.tab, [pointer.x - held.x, pointer.y - held.y, width, height, radius], turned))
    }

    /// Scrolls the list the least that shows `tab`'s row whole, if it has one: for a new tab's row,
    /// as Dia's does for a tab opened behind, and for the tab shown, as Chrome's tab strip does.
    pub(super) fn scroll_to_row(&self, tab: u64) {
        let Some(&top) = self.layout().get(&Slot::Tab(self.row_of(tab))) else {
            return;
        };
        let top = top + self.tiles_room();
        let height = f32::from(self.list_scroll.bounds().size.height);
        let offset = self.list_scroll.offset();
        let scrolled = -f32::from(offset.y);
        let to = if top < scrolled {
            top
        } else if top + ROW_PITCH > scrolled + height {
            top + ROW_PITCH - height
        } else {
            return;
        };
        // Not before the list's first layout.
        if height > 0. {
            // check.sh reads this line.
            log!("browser: list scrolled to {to} for {tab}");
            self.list_scroll.set_offset(point(offset.x, px(-to)));
        }
    }

    /// Where `y`, down from the sidebar's top, is in the list, scrolled as it is.
    pub(super) fn list_y(&self, y: f32) -> f32 {
        y - self.drawn_list_top() - f32::from(self.list_scroll.offset().y)
    }

    /// The row the pointer drags, once it has come away.
    pub(super) fn dragging(&self) -> Option<Slot> {
        self.drag.as_ref().filter(|drag| drag.moving).map(|drag| Slot::Tab(drag.tab))
    }

    /// The pointer pressed `tab`'s row at `position`, which may start a drag.
    pub(super) fn press_row(&mut self, tab: u64, position: Point<Pixels>) {
        let pinned = self.pinned().contains(&tab);
        let (pinned_rows, today_rows) = self.rows();
        let rows = if pinned { pinned_rows } else { today_rows };
        let Some(index) = rows.iter().position(|&other| other == tab) else {
            return;
        };
        let top = self.layout()[&Slot::Tab(tab)];
        let pointer = in_sidebar(position, self.sidebar_origin.get());
        let grab = (self.list_y(pointer.y) - top, point(0., 0.));
        self.press(tab, position, Place::List(pinned, index), top, grab);
        self.held_row = Some(HeldRow { tab, pressed: Instant::now(), let_go: None });
    }

    /// How small `tab`'s row's plate is drawn: smaller while it's held, growing back from where it
    /// is once it's let go. Full size while it's dragged, as it has come away, and while the
    /// sidebar recedes under the Library, whose scale this one can't sit inside.
    pub(super) fn press_scale(&self, tab: u64) -> f32 {
        let Some(held) = self.held_row.as_ref().filter(|held| held.tab == tab) else {
            return 1.;
        };
        if self.dragging().is_some() || self.sidebar_scale() != 1. {
            return 1.;
        }
        1. - (1. - PRESS_SCALE) * held.depth(Instant::now())
    }

    /// Lets the held row go once it has grown back, or has come away, and asks for the next frame
    /// while it's shrinking or growing.
    pub(super) fn step_held_row(&mut self, window: &mut Window) {
        let Some(held) = &self.held_row else {
            return;
        };
        let depth = held.depth(Instant::now());
        match held.let_go {
            _ if self.dragging().is_some() => self.held_row = None,
            Some(_) if depth == 0. => self.held_row = None,
            None if depth == 1. => {}
            _ => window.request_animation_frame(),
        }
    }

    /// The pointer pressed `tab`'s tile at `position`, which may start a drag.
    pub(super) fn press_tile(&mut self, tab: u64, position: Point<Pixels>) {
        let Some(index) = self.favorites.iter().position(|&other| other == tab) else {
            return;
        };
        let Some(&[left, top, width]) = self.drawn_tiles().get(&tab) else {
            return;
        };
        let pointer = in_sidebar(position, self.sidebar_origin.get());
        let centre = point(left + width / 2., self.tiles_top() + top + TILE_HEIGHT / 2.);
        // Its row, over the list, is centred on the pointer.
        let grab = (ROW_HEIGHT / 2., pointer - centre);
        self.press(tab, position, Place::Favorite(index), 0., grab);
    }

    fn press(
        &mut self,
        tab: u64,
        position: Point<Pixels>,
        from: Place,
        top: f32,
        grab: (f32, Point<f32>),
    ) {
        if self.drag.as_ref().is_some_and(|drag| drag.dropped || drag.held) {
            return;
        }
        let origin = self.sidebar_origin.get();
        self.dragged = false;
        self.drag = Some(Drag {
            tab,
            incoming: tab == INCOMING,
            press: position,
            origin,
            pointer: in_sidebar(position, origin),
            grab,
            top,
            moving: false,
            span: (top, top),
            from,
            to: from,
            from_depth: self.depth(tab),
            depth: self.depth(tab),
            line: None,
            resting: None,
            onto: None,
            dropped: false,
            over_page: None,
            shape: Shape::new(matches!(from, Place::Favorite(_))),
            held: false,
            space: self.space,
            edge: None,
            alt_click: false,
        });
    }

    /// An ⌥ press on `tab`'s row at `position`: a drag, if it moves, else ⌥-click's split once
    /// it's let go. Where the row can't be dragged, the split is at once, as it was.
    pub(super) fn alt_press_list_row(
        &mut self,
        tab: u64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.end_hovers(cx);
        self.press_row(tab, position);
        if let Some(drag) = self.drag.as_mut().filter(|drag| drag.tab == tab && !drag.moving) {
            drag.alt_click = true;
        } else if !self.alt_press_row(tab, cx) {
            self.show_row(tab, cx);
        }
    }

    /// The pointer pressing a row or a tile moved to `position`. Past DRAG_START the tab comes away
    /// and follows it. Over the favorites it's a tile, and the tiles make room for it at the place
    /// nearest it. Over the list it's a row, which goes up and down over rows that stay put, and a
    /// line marks the place, in the pinned tabs or Today, nearest where it's drawn. Where that's
    /// at the end of folders or after one, the pointer moved right takes it into them, a row in for
    /// each FOLDER_INDENT. A split view's row stays in the list, and a folder's in the pinned tabs.
    pub(super) fn drag_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_ref().filter(|drag| !drag.dropped && !drag.held) else {
            return;
        };
        let (moved, incoming) = (position - drag.press, drag.incoming);
        if !drag.moving && !incoming && moved.x.abs().max(moved.y.abs()) < px(DRAG_START) {
            return;
        }
        if !incoming && self.drag_over_page(position, cx) {
            return;
        }
        let Some(drag) = &self.drag else {
            return;
        };
        let (tab, to, grab, span, kept_depth) =
            (drag.tab, drag.to, drag.grab, drag.span, drag.depth);
        let (from, from_depth) = (drag.from, drag.from_depth);
        let pointer = in_sidebar(position, drag.origin);
        // Rows in, from where it was, by how far the pointer has gone right. Links dragged in go
        // in as far as the row under them.
        let moved_in = (pointer.x - in_sidebar(drag.press, drag.origin).x) / FOLDER_INDENT;
        let wants = match incoming {
            true => 0,
            false => (drag.from_depth as f32 + moved_in).round().max(0.) as usize,
        };
        let others = |rows: Vec<u64>| rows.into_iter().filter(|&other| other != tab).collect();
        let (pinned, today) = self.rows();
        let (pinned, today): (Vec<u64>, Vec<u64>) = (others(pinned), others(today));
        let tasks = self.shown_space().map(|space| space.tasks.clone()).unwrap_or_default();
        let favorites = self.favorites.iter().filter(|&&other| other != tab).count();
        let folder = self.is_folder(tab);
        let away = drag.space != self.space;
        self.watch_edge(pointer.x, cx);
        // A favorite stays one in an agent's Space, which it can't move into.
        let kept = self.favorites.contains(&tab) && self.agents_space_shown();
        // Past the most, a row stays one.
        let over_favorites = !incoming
            && !away
            && !folder
            && self.panes(tab).len() == 1
            && self.has_room_for(tab)
            && (kept || pointer.y < self.list_top_with(favorites + 1) - self.tiles_scrolled());
        let mut depth = 0;
        let (place, span, line) = if over_favorites {
            let centre = pointer - grab.1;
            let places = self.tile_places(favorites + 1);
            let distance = |index: &usize| {
                let [left, top, width] = places[*index];
                let x = left + width / 2. - centre.x;
                let y = self.tiles_top() + top + TILE_HEIGHT / 2. - centre.y;
                x * x + y * y
            };
            let index = (0..places.len()).min_by(|a, b| distance(a).total_cmp(&distance(b)));
            // Its row keeps its indent while it turns into a tile.
            depth = kept_depth;
            (Place::Favorite(index.unwrap_or(0)), span, None)
        } else {
            // In another Space it goes where Move to puts it: under its pinned tabs, or at the top
            // of its Today.
            let fits = |&gap: &usize| {
                (!away || gap == pinned.len()) && self.gap_depths(&pinned, gap, tab).is_some()
            };
            let places = (0..=pinned.len()).filter(fits).map(|index| (true, index));
            let today_places = (0..=today.len()).filter(|&index| !folder && (!away || index == 0));
            let places = places.chain(today_places.map(|index| (false, index)));
            let layouts: Vec<_> = places
                .map(|(in_pinned, index)| {
                    let (mut pinned, mut today) = (pinned.clone(), today.clone());
                    let tabs = if in_pinned { &mut pinned } else { &mut today };
                    tabs.insert(index, tab);
                    let layout = slots(&pinned, &today, &tasks);
                    ((in_pinned, index), layout[&Slot::Tab(tab)], layout)
                })
                .collect();
            let (first, last) = (layouts[0].1, layouts[layouts.len() - 1].1);
            let span = (first.min(last), first.max(last));
            // A row goes where its top comes nearest a slot's. What's dragged in has no row in the
            // list, so it goes to the gap nearest the pointer, where its line shows, as Arc's: a
            // slot's top is half the rows' gap under its line.
            let held = match incoming {
                true => -(ROW_PITCH - ROW_HEIGHT) / 2.,
                false => grab.0,
            };
            let at = (self.list_y(pointer.y) - held).clamp(span.0, span.1);
            let Some(((in_pinned, index), _, _)) =
                layouts.into_iter().min_by(|a, b| (a.1 - at).abs().total_cmp(&(b.1 - at).abs()))
            else {
                return;
            };
            if in_pinned && let Some((low, high)) = self.gap_depths(&pinned, index, tab) {
                depth = wants.clamp(low, high);
            }
            let place = Place::List(in_pinned, index);
            let line = self.line_at(&pinned, &today, place);
            let stays = place == from && depth == from_depth && !away;
            (place, span, line.filter(|_| !stays))
        };
        // Rested on another tab's row, it would join it, so the gap it would go to stays where it
        // was. Off the row's middle, it no longer would, and reordering carries on. Held at an
        // edge that pages, it's going to another Space, so it rests on no row there, whose wait is
        // the shorter (#1089).
        let paging = self.drag.as_ref().is_some_and(|drag| drag.edge.is_some());
        let rests = !incoming && !over_favorites && !paging;
        let middle = rests.then(|| self.split_onto(tab, pointer.y)).flatten();
        let resting = self.drag.as_ref().and_then(|drag| drag.resting.as_ref().map(|rest| rest.0));
        if middle != resting {
            let rest = middle.map(|row| {
                let wait = cx.spawn(async move |shell, cx| {
                    cx.background_executor().timer(SPLIT_REST).await;
                    shell.update(cx, |shell, cx| shell.rested_onto(row, cx)).ok();
                });
                (row, wait)
            });
            if let Some(drag) = &mut self.drag {
                drag.resting = rest;
            }
        }
        let was_onto = self.drag.as_ref().and_then(|drag| drag.onto);
        let onto = was_onto.filter(|&row| middle == Some(row));
        let (place, line) = if onto.is_some() { (to, None) } else { (place, line) };
        if let Some(drag) = &mut self.drag {
            drag.moving = true;
            drag.pointer = pointer;
            drag.span = span;
            drag.depth = depth;
            drag.line = line;
            drag.onto = onto;
            drag.shape.turn(over_favorites, Instant::now());
        }
        if onto != was_onto {
            // check.sh waits for this line.
            log!("browser: drop onto none");
        }
        // Links dragged in have no place before their first, which they didn't pass a row to get.
        let entered = incoming && to == Place::List(false, usize::MAX);
        if (place != to || depth != kept_depth || onto != was_onto) && !entered {
            self.drag_detent(cx);
        }
        if place != to {
            // The tiles that make room, or no longer need to, start from where they're drawn. The
            // rows stay put, as Arc's do, so the line, not the list, shows where it would go.
            // check.sh waits for these lines.
            match place {
                Place::List(true, index) if line.is_some() => {
                    log!("browser: drop line in pinned {index}")
                }
                Place::List(false, index) if line.is_some() => {
                    log!("browser: drop line in today {index}")
                }
                _ => {}
            }
            let tiles = self.drawn();
            if let Some(drag) = &mut self.drag {
                drag.to = place;
            }
            self.move_tiles(tiles, Curve::Room);
        }
        cx.notify();
    }

    /// Whether a dragged tab at `x` across the sidebar is held at its left or right edge, where it
    /// pages to the user's Space on that side once it has waited there, as Arc's does. Only a tab
    /// Move to takes: not a favorite, a folder, a split view or links dragged in.
    fn watch_edge(&mut self, x: f32, cx: &mut Context<Self>) {
        let width = self.width();
        let side = (x < EDGE).then_some(false).or((x > width - EDGE).then_some(true));
        let Some(drag) = &self.drag else {
            return;
        };
        let tab = drag.tab;
        let movable = !drag.incoming
            && !self.favorites.contains(&tab)
            && !self.is_folder(tab)
            && self.panes(tab).len() == 1
            && !self.private;
        let side = side.filter(|_| movable && self.edge_space(side == Some(true)).is_some());
        if drag.edge.map(|(right, _)| right) == side {
            return;
        }
        let edge = side.map(|right| (right, Instant::now()));
        if let Some(drag) = &mut self.drag {
            drag.edge = edge;
        }
        if let Some((_, since)) = edge {
            self.page_drag_after(since, cx);
        }
    }

    /// The user's Space beside the one shown, right or left.
    fn edge_space(&self, right: bool) -> Option<u64> {
        let index = self.spaces.iter().position(|space| space.id == self.space)?;
        let index = if right { index + 1 } else { index.checked_sub(1)? };
        self.spaces.get(index).filter(|space| space.agent.is_empty()).map(|space| space.id)
    }

    /// Pages to the Space on the edge's side once the tab has been held there since `since`, and
    /// waits again for the next.
    fn page_drag_after(&self, since: Instant, cx: &mut Context<Self>) {
        cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(EDGE_WAIT).await;
            shell
                .update(cx, |shell, cx| {
                    let held = shell.drag.as_ref().filter(|drag| drag.moving);
                    let edge = held.and_then(|drag| drag.edge);
                    let Some((right, _)) = edge.filter(|&(_, at)| at == since) else {
                        return;
                    };
                    let Some(space) = shell.edge_space(right) else {
                        return;
                    };
                    let now = Instant::now();
                    if let Some(drag) = &mut shell.drag {
                        drag.edge = Some((right, now));
                    }
                    // check.sh waits for this line.
                    log!("browser: drag paged to space {space}");
                    shell.drag_detent(cx);
                    shell.show_space(space, cx);
                    shell.page_drag_after(now, cx);
                })
                .ok();
        })
        .detach();
    }

    /// The pointer has rested on `row`'s middle for SPLIT_REST with a tab dragged: let go, the tab
    /// joins it in a split view.
    fn rested_onto(&mut self, row: u64, cx: &mut Context<Self>) {
        let resting = |drag: &&mut Drag| drag.resting.as_ref().is_some_and(|rest| rest.0 == row);
        let Some(drag) = self.drag.as_mut().filter(|drag| !drag.dropped).filter(resting) else {
            return;
        };
        drag.onto = Some(row);
        drag.line = None;
        // check.sh waits for this line.
        log!("browser: drop onto {row}");
        self.drag_detent(cx);
        cx.notify();
    }

    /// The tab whose row's middle half is at `y` down the sidebar, which a dragged `tab` would join
    /// in a split view, as Arc's does: a tab alone onto a tab's row, or a split's with room, and
    /// not a folder's.
    fn split_onto(&self, tab: u64, y: f32) -> Option<u64> {
        if self.is_folder(tab) || self.panes(tab).len() > 1 {
            return None;
        }
        let y = self.list_y(y);
        self.layout().into_iter().find_map(|(slot, top)| {
            let Slot::Tab(row) = slot else {
                return None;
            };
            let middle = top + ROW_HEIGHT / 4. <= y && y <= top + ROW_HEIGHT * 3. / 4.;
            let room = !self.is_folder(row) && self.panes(row).len() < super::MAX_PANES;
            (row != tab && middle && room).then_some(row)
        })
    }

    /// Where the drop line for `place` in the list goes, down from the list's top: halfway
    /// between the plates of the rows either side of the gap, in the list as it's drawn, without
    /// the dragged tab in `pinned` and `today`. None for the favorites.
    fn line_at(&self, pinned: &[u64], today: &[u64], place: Place) -> Option<f32> {
        let Place::List(in_pinned, index) = place else {
            return None;
        };
        let above = match (in_pinned, index.checked_sub(1)) {
            (true, at) => at.map(|at| Slot::Tab(pinned[at])),
            (false, at) => Some(at.map_or(Slot::NewTab, |at| Slot::Tab(today[at]))),
        };
        let layout = self.layout();
        // Today's top is under the task cards.
        let tasks = self.shown_space().map_or(0., |space| {
            space.tasks.iter().map(|&(_, height)| height).sum::<f32>()
        });
        let after = |slot: Slot| ROW_PITCH + if slot == Slot::NewTab { tasks } else { 0. };
        let below =
            above.map_or(Some(NAME_HEIGHT), |slot| Some(layout.get(&slot)? + after(slot)))?;
        Some(below - (ROW_PITCH - ROW_HEIGHT) / 2.)
    }

    /// A click on the trackpad, as the dragged row passes another or goes into a folder: one for
    /// each change of where it would drop, as Arc's does (docs/ux-spec.md › Reorder).
    fn drag_detent(&self, cx: &mut Context<Self>) {
        // check.sh counts these lines.
        log!("browser: detent");
        self.browser.detent(cx);
    }

    /// The pointer let go of a pressed row or tile. A dragged tab goes where it would, and the rows
    /// and tiles settle once the browser has moved it.
    pub(super) fn drop_row(&mut self, cx: &mut Context<Self>) {
        self.drop_row_with(false, cx);
    }

    /// `drop_row`, with ⌥ held or not as it's let go: a row dragged from the list and let go there
    /// with ⌥ held leaves a copy of its tab where it's let go and goes back, as Arc's does.
    pub(super) fn drop_row_with(&mut self, alt: bool, cx: &mut Context<Self>) {
        if let Some(held) = self.held_row.as_mut().filter(|held| held.let_go.is_none()) {
            held.let_go = Some(Instant::now());
            cx.notify();
        }
        if self.drop_over_page(cx) {
            return;
        }
        let Some(drag) = self.drag.as_mut().filter(|drag| !drag.dropped) else {
            return;
        };
        if !drag.moving {
            let alt_click = drag.alt_click.then_some(drag.tab);
            self.drag = None;
            if let Some(tab) = alt_click
                && !self.alt_press_row(tab, cx)
            {
                self.show_row(tab, cx);
            }
            return;
        }
        self.dragged = true;
        // Let go mid-turn, it lands once it has turned, from its tile's or its row's shape.
        if drag.shape.start.is_some() {
            drag.held = true;
            return;
        }
        let (tab, to, drag_depth) = (drag.tab, drag.to, drag.depth);
        if drag.space != self.space {
            let space = self.space;
            self.drag = None;
            self.retip = true;
            if let Place::List(pin, _) = to {
                // check.sh waits for this line.
                let list = if pin { "pinned" } else { "today" };
                log!("browser: dragged {tab} to space {space} {list}");
                self.move_to_space(tab, space, pin, cx);
            }
            cx.notify();
            return;
        }
        if let Some(with) = drag.onto {
            // check.sh waits for this line.
            log!("browser: dragged {tab} onto {with}");
            self.settle_drag();
            self.join_split(tab, with, false, false, cx);
            cx.notify();
            return;
        }
        let listed = matches!((drag.from, to), (Place::List(..), Place::List(..)));
        if alt && listed && self.panes(tab).len() == 1 && !self.is_folder(tab) {
            self.settle_drag();
            self.putting_back = true;
            self.drop_copy(tab, to, drag_depth, cx);
            cx.notify();
            return;
        }
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let favorite = matches!(to, Place::Favorite(_));
        if drag.to == drag.from && (favorite || drag.depth == drag.from_depth) {
            self.settle_drag();
            self.putting_back = true;
        } else {
            drag.dropped = true;
            match to {
                Place::Favorite(index) => {
                    // check.sh waits for this line.
                    log!("browser: dragged {tab} to favorites {index}");
                    self.favorite_tab(tab, index, cx);
                }
                Place::List(true, gap) => {
                    let (rows, _) = self.rows();
                    let rows: Vec<u64> = rows.into_iter().filter(|&row| row != tab).collect();
                    let (folder, index) = self.pinned_place(&rows, gap, drag_depth, tab, false);
                    // check.sh waits for these lines.
                    match folder {
                        0 => log!("browser: dragged {tab} to pinned {gap}"),
                        _ => log!("browser: dragged {tab} to folder {folder} {index}"),
                    }
                    self.move_tab_to(tab, true, folder, index, cx);
                }
                Place::List(false, index) => {
                    // check.sh waits for this line.
                    log!("browser: dragged {tab} to today {index}");
                    // The browser counts tabs, and the rows above may be split views.
                    let (_, rows) = self.rows();
                    let rows = rows.into_iter().filter(|&row| row != tab).take(index);
                    let index = rows.map(|row| self.panes(row).len()).sum();
                    self.move_tab_to(tab, false, 0, index, cx);
                }
            }
        }
        cx.notify();
    }

    /// A drag from the page or another app, carrying links or files, is at `position` in the
    /// window. Over the list of the docked sidebar, a line marks the place nearest, as for a
    /// dragged row, and it returns whether they'd open there. Not while the Library, a swipe or a
    /// row's drag has the sidebar.
    pub fn drag_in(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let busy = self.sidebar_hidden || self.library_open || self.swipe.is_some();
        let other = self.drag.as_ref().is_some_and(|drag| !drag.incoming || drag.dropped);
        if busy || other || !self.list_scroll.bounds().contains(&position) {
            self.drag_away(cx);
            return false;
        }
        if self.drag.is_none() {
            // Held by the middle of the row it stands for, where the system draws what it carries.
            let grab = (ROW_HEIGHT / 2., point(0., 0.));
            // From no place, so the first one it has gets its line.
            self.press(INCOMING, position, Place::List(false, usize::MAX), 0., grab);
            if let Some(drag) = &mut self.drag {
                // The docked sidebar's corner, the window's.
                drag.origin = Point::default();
                drag.moving = true;
            }
        }
        self.drag_to(position, cx);
        true
    }

    /// The drag from the page or another app left the list, or ended without a drop there: the
    /// rows settle back.
    pub fn drag_away(&mut self, cx: &mut Context<Self>) {
        if self.drag.as_ref().is_some_and(|drag| drag.incoming && !drag.dropped) {
            self.settle_drag();
            cx.notify();
        }
    }

    /// The drag from the page or another app was let go at `position`. Over the list, `urls` open
    /// as tabs where the line marks, and the first shows. Returns whether they opened.
    pub fn drop_in(
        &mut self,
        position: Point<Pixels>,
        urls: &[String],
        cx: &mut Context<Self>,
    ) -> bool {
        if urls.is_empty() || !self.drag_in(position, cx) {
            return false;
        }
        let Some(drag) = &self.drag else {
            return false;
        };
        let (pinned, today) = self.rows();
        let (in_pinned, folder, index) = match drag.to {
            Place::List(true, gap) => {
                let (folder, index) = self.pinned_place(&pinned, gap, drag.depth, INCOMING, false);
                (true, folder, index)
            }
            // The browser counts tabs, and the rows above may be split views.
            Place::List(false, index) => {
                (false, 0, today.iter().take(index).map(|&row| self.panes(row).len()).sum())
            }
            Place::Favorite(_) => return false,
        };
        // check.sh waits for these lines.
        let count = urls.len();
        match (in_pinned, folder) {
            (false, _) => log!("browser: dropped {count} in today {index}"),
            (true, 0) => log!("browser: dropped {count} in pinned {index}"),
            (true, _) => log!("browser: dropped {count} in folder {folder} {index}"),
        }
        // A folder from the desktop comes in as one of the sidebar's folders, after what else
        // was dropped with it in the pinned tabs, else at their bottom, as folders are only there.
        let (folders, urls): (Vec<&String>, Vec<&String>) =
            urls.iter().partition(|url| files::folder_path(url).is_some());
        let urls: Vec<String> = urls.into_iter().cloned().collect();
        let folders: Vec<PathBuf> =
            folders.into_iter().filter_map(|url| files::file_path(url)).collect();
        let tab = match urls.is_empty() {
            true => 0,
            false => self.browser.open_at(&urls, in_pinned, folder, index, cx),
        };
        let made = !folders.is_empty();
        match in_pinned {
            true => self.add_folders(folders, folder, index + urls.len(), cx),
            false => self.add_folders(folders, 0, usize::MAX, cx),
        }
        match self.drag.as_mut().filter(|_| tab != 0) {
            // The first one's row lands from where the pointer let go, once the browser reports
            // the lists it made.
            Some(drag) => {
                drag.tab = tab;
                drag.dropped = true;
            }
            None => self.settle_drag(),
        }
        cx.notify();
        tab != 0 || made
    }

    /// The depths a row dropped at `gap` in the pinned rows `rows`, which leave out `dragged`, may
    /// be at: from the row's below, out of the folders it ends, to the row's above, or one in if
    /// that's a folder. None at a gap inside `dragged`, a folder.
    fn gap_depths(&self, rows: &[u64], gap: usize, dragged: u64) -> Option<(usize, usize)> {
        let space = self.shown_space()?;
        let above = gap.checked_sub(1).map(|at| rows[at]);
        let below = rows.get(gap).copied();
        let inside = |row: u64| space.holders(row).any(|folder| folder == dragged);
        if below.is_some_and(inside) {
            return None;
        }
        let low = below.map_or(0, |row| space.depth(row));
        let high = match above {
            None => 0,
            // Not into itself.
            Some(row) if inside(row) => space.depth(dragged),
            Some(row) => space.depth(row) + space.is_folder(row) as usize,
        };
        Some((low, high.max(low)))
    }

    /// Where a copy of `tab`, a row dragged from the list with ⌥ held, goes once let go at `to`,
    /// `depth` folders in: the browser makes it, which shows it, as Duplicate does, and puts it
    /// there, counting `tab`, which stays where it was. One step for ⌘Z, which closes it.
    fn drop_copy(&mut self, tab: u64, to: Place, depth: usize, cx: &mut Context<Self>) {
        let (pinned_rows, today_rows) = self.rows();
        let (pinned, folder, index) = match to {
            Place::List(true, gap) => {
                let rows: Vec<u64> = pinned_rows.into_iter().filter(|&row| row != tab).collect();
                let (folder, index) = self.pinned_place(&rows, gap, depth, tab, true);
                (true, folder, index)
            }
            Place::List(false, gap) => {
                let before = today_rows.iter().filter(|&&row| row != tab).nth(gap).copied();
                let above = today_rows.iter().take_while(|&&row| Some(row) != before);
                (false, 0, above.map(|&row| self.panes(row).len()).sum())
            }
            Place::Favorite(_) => return,
        };
        let copy = self.browser.duplicate(tab, cx);
        if copy == 0 {
            return;
        }
        // check.sh waits for this line.
        let list = if pinned { "pinned" } else { "today" };
        log!("browser: dragged {tab} copy {copy} to {list} {folder} {index}");
        self.browser.move_tab(copy, pinned, folder, index, cx);
        self.keep(What::DUPLICATE, Some(Edit::Close(vec![copy])), cx);
    }

    /// Where a row dropped at `gap` in the pinned rows `rows`, without `dragged`, at `depth`
    /// goes: the folder, else 0, and its index among what's directly in that, counted as the
    /// browser counts, without `dragged` unless it's a `copy` of it that goes.
    fn pinned_place(
        &self,
        rows: &[u64],
        gap: usize,
        depth: usize,
        dragged: u64,
        copy: bool,
    ) -> (u64, usize) {
        let Some(space) = self.shown_space() else {
            return (0, gap);
        };
        let above = gap.checked_sub(1).map(|at| rows[at]);
        let folder = match above {
            _ if depth == 0 => 0,
            Some(row) if space.is_folder(row) && depth == space.depth(row) + 1 => row,
            // The folder holding the row above at that depth.
            Some(row) => space.holders(row).nth(space.depth(row) - depth).unwrap_or(0),
            None => 0,
        };
        let before = rows.get(gap).copied().filter(|&row| space.folder_of(row) == folder);
        let panes = if copy { Vec::new() } else { space.panes(dragged) };
        let index = space
            .pinned
            .iter()
            .take_while(|&&row| Some(row) != before)
            .filter(|&&row| space.folder_of(row) == folder && !panes.contains(&row))
            .count();
        (folder, index)
    }

    /// Puts the tab the pointer drags back where it was, and returns whether there was one.
    pub(super) fn cancel_drag(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.drag.as_ref().is_some_and(|drag| drag.moving && !drag.dropped && !drag.held) {
            return false;
        }
        // The release that ends the drag isn't a click.
        self.dragged = true;
        self.settle_drag();
        self.putting_back = true;
        cx.notify();
        true
    }

    /// Ends a drag that moves nothing in the browser: the rows and tiles settle from where they're
    /// drawn.
    pub(super) fn settle_drag(&mut self) {
        let tiles = self.drawn();
        self.move_rows(self.layout(), Curve::Slide);
        self.drag = None;
        self.retip = true;
        self.move_tiles(tiles, Curve::Slide);
    }

    /// The browser reported its favorites, or else the Space shown's lists. A row still held whose
    /// tab is still in its list goes on from where it is now, as when its own press opened or
    /// closed a folder (#1118); any other drag still held goes back. A dragged tab let go lands once
    /// it's where it was dropped: moved in one list, once that list comes, and between them, once
    /// it's in the one and out of the other. Returns whether the drag ended.
    pub(super) fn end_drag(&mut self, favorites: bool) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        // One paged to another Space is held over lists it isn't in.
        if !drag.dropped && drag.space != self.space {
            return false;
        }
        if !drag.dropped {
            let moving = drag.moving;
            if !favorites && self.carry_drag() {
                return false;
            }
            // The rows have already slid from where they're drawn, the dragged one home with them;
            // the release that ends the drag isn't a click.
            self.dragged |= moving;
            self.putting_back |= moving;
            self.drag = None;
            self.retip = true;
            return true;
        }
        let tab = drag.tab;
        let in_favorites = self.favorites.contains(&tab);
        let in_space = self
            .shown_space()
            .is_some_and(|space| space.pinned.contains(&tab) || space.today.contains(&tab));
        let landed = match (drag.from, drag.to) {
            (Place::Favorite(_), Place::Favorite(_)) => favorites,
            (Place::List(..), Place::List(..)) => !favorites,
            (_, Place::Favorite(_)) => in_favorites && !in_space,
            (_, Place::List(..)) => in_space && !in_favorites,
        };
        if !landed {
            return false;
        }
        self.drag = None;
        self.retip = true;
        true
    }

    /// The lists changed under a row still held in them: it goes on from its row's place in the
    /// new ones, if its tab is still in the list it came from. Returns whether it does.
    fn carry_drag(&mut self) -> bool {
        let Some((tab, Place::List(in_pinned, _))) =
            self.drag.as_ref().map(|drag| (drag.tab, drag.from))
        else {
            return false;
        };
        let (pinned, today) = self.rows();
        let rows = if in_pinned { pinned } else { today };
        let (Some(index), Some(&top)) =
            (rows.iter().position(|&row| row == tab), self.layout().get(&Slot::Tab(tab)))
        else {
            return false;
        };
        let depth = self.depth(tab);
        let Some(drag) = &mut self.drag else {
            return false;
        };
        drag.from = Place::List(in_pinned, index);
        drag.from_depth = depth;
        drag.top = top;
        if !drag.moving {
            (drag.to, drag.depth) = (drag.from, depth);
        }
        true
    }

    /// Where the pointer holding a drag is in the window, for its place to be found again.
    pub(super) fn drag_position(&self) -> Option<Point<Pixels>> {
        let held = |drag: &&Drag| drag.moving && !drag.dropped && drag.space == self.space;
        let drag = self.drag.as_ref().filter(held)?;
        Some(drag.origin + point(px(drag.pointer.x), px(drag.pointer.y)))
    }

    /// Where the sidebar's top-left corner is in the window a press is in, heard before the row or
    /// tile pressed hears it, for where a drag is in the sidebar.
    pub(super) fn press_listener(&self) -> impl IntoElement + use<> {
        let origin = self.sidebar_origin.clone();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window.on_mouse_event(move |_: &MouseDownEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture {
                        origin.set(bounds.origin);
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }

    /// The pointer's moves and release anywhere in the window, for a pressed row or tile, which
    /// only hears the pointer while it's over it.
    pub(super) fn drag_listeners(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shell = cx.weak_entity();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let moved = shell.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        moved.update(cx, |shell, cx| shell.drag_to(event.position, cx)).ok();
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        let alt = event.modifiers.alt;
                        shell.update(cx, |shell, cx| shell.drop_row_with(alt, cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_0()
    }
}

/// The URLs of what a drag from the page or another app carries that open as tabs: files', web
/// links, and for text that isn't a link, `search`'s URL for it with the default engine (Arc). A
/// dragged link never opens a `javascript:` or `data:` URL, which could run script or pass for a
/// site, or a `file:` one, which could open a file the user didn't drag (Ardia), nor is any link
/// but a web one searched for, as it isn't words.
pub fn dropped_urls<'a>(
    items: impl IntoIterator<Item = (&'a str, bool)>,
    search: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    /// The scheme `url` starts with, if it starts with one.
    fn scheme(url: &str) -> Option<&str> {
        let (scheme, _) = url.split_once(':')?;
        let mut letters = scheme.chars();
        let named = letters.next().is_some_and(|c| c.is_ascii_alphabetic())
            && letters.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c));
        named.then_some(scheme)
    }
    let is = |scheme: &str, names: &[&str]| names.iter().any(|n| scheme.eq_ignore_ascii_case(n));
    let spaced = |url: &str| url.contains(char::is_whitespace);
    let items = items.into_iter().filter_map(|(url, file)| match scheme(url) {
        _ if file => Some(url.to_owned()),
        Some(s) if is(s, &["http", "https"]) && !spaced(url) && url.len() > s.len() + 3 => {
            Some(url.to_owned())
        }
        Some(s) if is(s, &["javascript", "data", "file"]) || !spaced(url) => None,
        _ => {
            let words = url.split_whitespace().collect::<Vec<_>>().join(" ");
            (!words.is_empty()).then(|| search(&words)).flatten()
        }
    });
    items.collect()
}

/// Where `position` in a window is from the corner, `origin`, of the sidebar it draws.
pub(super) fn in_sidebar(position: Point<Pixels>, origin: Point<Pixels>) -> Point<f32> {
    let at = position - origin;
    point(f32::from(at.x), f32::from(at.y))
}

impl HeldRow {
    fn depth(&self, now: Instant) -> f32 {
        held_depth(self.pressed, self.let_go, now)
    }
}

/// How far a plate pressed at `pressed` has shrunk at `now`, 0 to 1: in over PRESS_TIME while it's
/// held, then back out from as far as it got, at the same pace, once it's `let_go`.
pub(super) fn held_depth(pressed: Instant, let_go: Option<Instant>, now: Instant) -> f32 {
    let way = |from: Instant, to: Instant| {
        (to.saturating_duration_since(from).as_secs_f32() / PRESS_TIME).min(1.)
    };
    match let_go {
        None => way(pressed, now),
        Some(let_go) => (way(pressed, let_go) - way(let_go, now)).max(0.),
    }
}

#[cfg(test)]
pub(in crate::shell) mod detents;

#[cfg(test)]
mod drop_line;

#[cfg(test)]
mod copy;

#[cfg(test)]
mod to_space;

#[cfg(test)]
mod tests {
    use super::dropped_urls;

    /// Files and web links open, links that aren't web links don't, and other text is searched.
    #[test]
    fn files_and_web_links_open_and_text_is_searched() {
        let dragged = [
            ("https://example.com/a?b#c", false),
            ("HTTP://EXAMPLE.COM", false),
            ("file:///Users/someone/Report%20final.pdf", true),
            ("javascript:alert(1)", false),
            ("JavaScript:alert(1)", false),
            ("data:text/html,<h1>Sign in</h1>", false),
            ("file:///etc/hosts", false),
            ("mailto:someone@example.com", false),
            ("https://", false),
            ("https://example.com and  some\nwords", false),
            ("example.com", false),
            ("", false),
            (" \n ", false),
        ];
        let search = |text: &str| Some(format!("search:{text}"));
        let opened = [
            "https://example.com/a?b#c",
            "HTTP://EXAMPLE.COM",
            "file:///Users/someone/Report%20final.pdf",
            "search:https://example.com and some words",
            "search:example.com",
        ];
        assert_eq!(dropped_urls(dragged, search), opened);
    }
}
