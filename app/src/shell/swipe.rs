//! Two fingers or a wheel across the sidebar, paging between Spaces and out to the
//! Library (docs/ux-spec.md › Spaces › Switching), and the list's scroll edge.

use std::{collections::VecDeque, time::Instant};

use gpui::{
    Context, DispatchPhase, Point, ScrollDelta, ScrollEvent, TouchPhase, Window, canvas, div,
    prelude::*, px,
};

use super::{
    SCROLL_EDGE, SCROLL_EDGE_FADE, SWIPE_FLICK, SWIPE_HISTORY, SWIPE_TRACKING,
    Settle, Shell, Space, Swipe, WHEEL_BURST, WHEEL_LINE, WHEEL_REST, fade_at, rubber_band,
    library::library_band,
};

impl Shell {
    /// The Spaces' pages the sidebar draws, and where from its left edge: the shown Space's, and
    /// while a swipe or a settle moves one, it and the page beside it.
    pub(super) fn pages(&self) -> Vec<(u64, f32)> {
        let swipe = self.swipe.as_ref().filter(|swipe| {
            swipe.across == Some(true) && self.library_swiped(swipe).is_none()
        });
        let (space, x, beside) = match (swipe, &self.settle) {
            (Some(swipe), _) => (swipe.from, self.swiped_to(swipe), None),
            (None, Some(settle)) => (settle.space, settle.offset().unwrap_or(0.), settle.beside),
            (None, None) => return vec![(self.space, self.held_page().unwrap_or(0.))],
        };
        let beside = beside.or_else(|| self.beside(space, x));
        let mut pages = vec![(space, x)];
        if x != 0.
            && let Some(beside) = beside
        {
            pages.push((beside, x - x.signum() * self.width()));
        }
        pages
    }

    /// The sidebar's last child while the Spaces' pages move, which notes how long the frame took
    /// it from `started`, as it was built, to its paint: the hot path's number (principle 1).
    pub(super) fn paging_frame(&self, started: Instant) -> Option<impl IntoElement + use<>> {
        let frames = self.paging_frames.clone();
        let paint = move |_, _, _: &mut Window, _: &mut gpui::App| {
            frames.borrow_mut().push(started.elapsed().as_secs_f32() * 1000.)
        };
        (self.pages().len() > 1).then(|| canvas(|_, _, _| {}, paint).absolute().size_0())
    }

    /// Logs the frames drawn while the pages moved, as they settle. check.sh reads the line.
    pub(super) fn log_paging_frames(&self) {
        let mut frames = std::mem::take(&mut *self.paging_frames.borrow_mut());
        frames.sort_by(f32::total_cmp);
        let median = frames.get(frames.len().saturating_sub(1) / 2);
        if let (Some(median), Some(max)) = (median, frames.last()) {
            log!("browser: paged frames {}, median {median:.2} ms, max {max:.2} ms", frames.len());
        }
    }

    /// Where a swipe has taken the page of the Space it began at: with the fingers, but
    /// rubber-banding past the first and last Spaces, and past one Space either side. Past the
    /// first Space is the Library, its width away. A swipe in the Library goes back that far, to
    /// the Space shown, and past where it opens takes it and the card on, on the Library's band.
    pub(super) fn swiped_to(&self, swipe: &Swipe) -> f32 {
        if swipe.library && swipe.way > 0. {
            return self.past_library(swipe.way);
        }
        if swipe.library {
            return rubber_band(swipe.way, -self.library_width(), 0.);
        }
        let index = self.spaces.iter().position(|space| space.id == swipe.from).unwrap_or(0);
        let width = self.width();
        let library = index == 0 && self.library_beside(swipe.from);
        // From the first Space, fingers that brought the Library out and go on meet its band too.
        if library && swipe.way > self.library_width() {
            return self.library_width() + self.past_library(swipe.way - self.library_width());
        }
        let high = match index {
            0 if library => self.library_width(),
            0 => 0.,
            _ => width,
        };
        if index + 1 < self.spaces.len() {
            return rubber_band(swipe.way, -width, high);
        }
        rubber_band(swipe.way, 0., high)
    }

    /// Where fingers `way` past where the Library opens take the page: the card goes on by the
    /// Library's band, and the page's way is the card's over its push.
    fn past_library(&self, way: f32) -> f32 {
        let push = (self.library_width() - self.card_left()).max(1.);
        self.library_width() * library_band(way) / push
    }

    /// Shows the first Space as fingers or a wheel leave the Library, which sits left of it, though
    /// it was opened from another (Arc 1.170 rec, three Spaces).
    fn show_first_space(&mut self, cx: &mut Context<Self>) {
        if let Some(first) = self.spaces.first() {
            self.show_space(first.id, cx);
        }
    }

    /// Whether fingers going right from `space` bring out the Library: from the first Space, in the
    /// docked sidebar, of a regular window.
    fn library_beside(&self, space: u64) -> bool {
        let first = self.spaces.first().is_some_and(|first| first.id == space);
        first && !self.sidebar_hidden && !self.page_fullscreen && !self.private
    }

    /// How far out a swipe has taken the Library (`library_out`), while it moves the Library.
    pub(super) fn library_swiped(&self, swipe: &Swipe) -> Option<f32> {
        if swipe.across != Some(true) {
            return None;
        }
        let x = self.swiped_to(swipe);
        match swipe.library {
            true => Some((1. + x / self.library_width()).max(0.)),
            false if x > 0. && self.library_beside(swipe.from) => Some(x / self.library_width()),
            false => None,
        }
    }

    /// The Space whose page comes in beside `space`'s while that's at `x` (`beside`).
    fn beside(&self, space: u64, x: f32) -> Option<u64> {
        let form = self.form.as_ref().map(|form| (form.space, form.from));
        beside(&self.spaces, form, space, x)
    }

    /// The Space whose page is nearest its place while `from`'s is at `x` (`nearest`).
    fn nearest(&self, from: u64, x: f32) -> Option<u64> {
        let form = self.form.as_ref().map(|form| (form.space, form.from));
        nearest(&self.spaces, form, from, x, self.width())
    }

    /// Fingers or a wheel over the sidebar. Across it they page between Spaces, and along it they
    /// scroll the list. Returns whether paging took the event.
    fn scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // The form pages back to the Space it was made from, but not while another Space shows.
        let form_hidden = self.form.is_some() && self.form().is_none();
        if form_hidden || self.renaming.is_some() || self.drag.is_some() {
            return false;
        }
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => delta.map(f32::from),
            ScrollDelta::Lines(lines) => return self.wheel(lines, window, cx),
        };
        match event.touch_phase {
            TouchPhase::Started => {
                self.swiped = false;
                self.swipe = Some(Swipe {
                    from: self.space,
                    library: self.library_open || self.library_motion.is_some(),
                    across: None,
                    way: 0.,
                    samples: VecDeque::new(),
                    nearest: self.space,
                    ready: false,
                    turned: None,
                });
            }
            TouchPhase::Ended | TouchPhase::Cancelled => return self.release(window, cx),
            TouchPhase::Moved => {}
        }
        let Some(mut swipe) = self.swipe.take() else {
            // A page swipe's momentum, across, doesn't scroll the list.
            return self.swiped && delta.x.abs() > delta.y.abs();
        };
        if swipe.across.is_none() && (delta.x != 0. || delta.y != 0.) {
            let across = delta.x.abs() > delta.y.abs();
            swipe.across = Some(across);
            // check.sh waits for this line.
            log!("browser: swipe {}", if across { "across" } else { "along" });
            // The page moves from under what floats over it, and no click outside closes it.
            if across {
                self.leave_popovers(cx);
            }
            // Fingers catch a page, or the Library, that's still settling where it is.
            if across && swipe.library {
                swipe.way = (self.library_out() - 1.) * self.library_width();
                self.library_motion = None;
            } else if across
                && let Some(settle) = self.settle.take()
                && let Some(x) = settle.offset()
            {
                (swipe.from, swipe.way, swipe.nearest) = (settle.space, x, settle.space);
            }
        }
        let across = swipe.across == Some(true);
        if across {
            let library_near = |shell: &Self, swipe: &Swipe| {
                shell.library_swiped(swipe).map(|out| out > 0.5)
            };
            let past_end = |shell: &Self, swipe: &Swipe| {
                shell.library_swiped(swipe).is_some_and(|out| out > 1.)
            };
            let was_near = library_near(self, &swipe);
            let was_past = past_end(self, &swipe);
            swipe.way += SWIPE_TRACKING * delta.x;
            if !was_past && past_end(self, &swipe) {
                // check.sh waits for this line.
                log!("browser: library past its end");
            }
            let (now, x) = (Instant::now(), self.swiped_to(&swipe));
            swipe.samples.push_back((now, x));
            while swipe.samples.front().is_some_and(|&(at, _)| now - at > SWIPE_HISTORY) {
                swipe.samples.pop_front();
            }
            self.ready_new_space(&mut swipe, cx);
            let near = library_near(self, &swipe);
            if near.is_some() {
                if self.library.is_none() {
                    self.show_section(self.section, cx);
                }
                if was_near.is_some() && was_near != near {
                    self.browser.detent(cx);
                }
            } else if let Some(nearest) = self.nearest(swipe.from, x)
                && nearest != swipe.nearest
            {
                swipe.nearest = nearest;
                self.browser.detent(cx);
            }
            cx.notify();
        }
        self.swipe = Some(swipe);
        across
    }

    /// The fingers lifted. The pages settle on the next Space in the direction of a flick, else
    /// on the one nearest its place, keeping the release's speed if it's heading there. Off the
    /// form, that's Cancel, its page going out as the Space it was made from settles.
    fn release(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(swipe) = self.swipe.take().filter(|swipe| swipe.across == Some(true)) else {
            return false;
        };
        self.swiped = true;
        let (now, x) = (Instant::now(), self.swiped_to(&swipe));
        let recent = swipe.samples.iter().filter(|&&(at, _)| now - at <= SWIPE_HISTORY);
        let velocity = match (recent.clone().next(), recent.last()) {
            (Some(&(start, from)), Some(&(end, to))) if end > start => {
                (to - from) / (end - start).as_secs_f32()
            }
            _ => 0.,
        };
        if let Some(out) = self.library_swiped(&swipe) {
            let open = if velocity.abs() >= SWIPE_FLICK { velocity > 0. } else { out > 0.5 };
            if open != self.library_open {
                // check.sh waits for this line.
                log!("browser: command view-library via swipe");
            }
            if !open {
                self.show_first_space(cx);
            }
            self.move_library(open, out, velocity / self.library_width(), None, cx);
            return true;
        }
        if swipe.ready {
            self.make_pulled_space(x, cx);
            return true;
        }
        let toward = heading(x, velocity, self.width());
        let Some(space) = self.nearest(swipe.from, toward) else {
            return true;
        };
        let from = if space == swipe.from { x } else { x - toward.signum() * self.width() };
        let velocity = if velocity * from < 0. { velocity } else { 0. };
        let cancel = space != swipe.from && self.form().is_some();
        let beside = cancel.then_some(swipe.from);
        self.settle =
            Some(Settle { space, beside, from, velocity, start: now, next_door: false });
        if cancel {
            self.form_going = self.close_form(window, cx);
        } else if space != self.space {
            // check.sh waits for this line.
            log!("browser: command select-space via swipe");
            self.show_space(space, cx);
        }
        cx.notify();
        true
    }

    /// A wheel's ticks across the sidebar page once per burst, then rest. Past the first Space they
    /// open the Library, and back they close it. Back off the form they cancel it.
    fn wheel(&mut self, lines: Point<f32>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if lines.x.abs() <= lines.y.abs() {
            return false;
        }
        let now = Instant::now();
        if self.wheel_paged.is_some_and(|paged| now - paged < WHEEL_REST) {
            return true;
        }
        let sum = match self.wheel {
            Some((sum, last)) if now - last < WHEEL_BURST => sum + lines.x,
            _ => lines.x,
        };
        self.wheel = Some((sum, now));
        if (sum * WHEEL_LINE).abs() > 1. {
            self.wheel = None;
            self.wheel_paged = Some(now);
            self.leave_popovers(cx);
            let library = sum > 0. && (self.library_open || self.library_beside(self.space));
            if library != self.library_open {
                // check.sh waits for this line.
                log!("browser: command view-library via wheel");
                if !library {
                    self.show_first_space(cx);
                }
                self.set_library(library, cx);
            } else if !self.library_open
                && let Some(space) = self.nearest(self.space, sum.signum() * self.width())
                && space != self.space
            {
                if self.form().is_some() {
                    self.cancel_space(window, cx);
                    return true;
                }
                // check.sh waits for this line.
                log!("browser: command select-space via wheel");
                self.show_space(space, cx);
            }
        }
        true
    }

    /// Fingers and wheels over the sidebar or the Library, heard ahead of the list, which would
    /// scroll with them. In the browser's window that's left of the card's edge, and in the
    /// reveal's, its panel. The rest of a swipe comes here wherever the pointer goes, as the
    /// sidebar or the Library may slide or fade from under it.
    pub(super) fn scroll_listener(
        &self,
        in_window: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let shell = cx.weak_entity();
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    let took = shell.update(cx, |shell, cx| {
                        let over = match in_window {
                            true => event.position.x < px(shell.card_left() + shell.card_push()),
                            false => bounds.contains(&event.position),
                        };
                        (over || shell.swipe.is_some()) && shell.scroll(event, window, cx)
                    });
                    if took.unwrap_or(false) {
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }

    /// Fades the edge under the favorites in once the list is past its top, and out back there.
    pub(super) fn step_scroll_edge(&mut self, window: &mut Window) {
        let scrolled = self.list_scroll.offset().y < px(0.);
        if scrolled != self.scrolled {
            self.scroll_edge_fade = Some((Instant::now(), self.scroll_edge()));
            self.scrolled = scrolled;
        }
        if let Some((start, _)) = self.scroll_edge_fade {
            if start.elapsed().as_secs_f32() < SCROLL_EDGE_FADE {
                window.request_animation_frame();
            } else {
                self.scroll_edge_fade = None;
                // check.sh waits for these lines.
                log!("browser: scroll edge {}", if scrolled { "shown" } else { "hidden" });
            }
        }
    }

    /// How far the edge under the favorites has faded in.
    fn scroll_edge(&self) -> f32 {
        let to = if self.scrolled { 1. } else { 0. };
        self.scroll_edge_fade.map_or(to, |(start, from)| {
            fade_at(from, to, start.elapsed().as_secs_f32(), SCROLL_EDGE_FADE)
        })
    }

    /// The edge across the sidebar 4 under the favorites, or the URL field without them or while
    /// they scroll with the list, while the rows go under it (Dia).
    pub(super) fn scroll_edge_line(&self) -> Option<impl IntoElement + use<>> {
        let shown = self.scroll_edge();
        (shown > 0.).then(|| {
            div()
                .absolute()
                .top(px(self.pages_top() - 2.))
                .left_0()
                .w(px(self.width()))
                .h(px(1.))
                .bg(self.palette.faint(SCROLL_EDGE))
                .opacity(shown)
        })
    }
}

/// The Space in `spaces` whose page comes in beside `space`'s while that's at `x`: the one before
/// it for `x` right of its place, else the one after. The form's, `(space, from)`, comes in from
/// the Space it was made from, before it, and has none after.
fn beside(spaces: &[Space], form: Option<(u64, u64)>, space: u64, x: f32) -> Option<u64> {
    if let Some((_, from)) = form.filter(|&(form, _)| form == space) {
        let made_from = spaces.iter().any(|other| other.id == from);
        return (x > 0. && made_from).then_some(from);
    }
    let index = spaces.iter().position(|other| other.id == space)?;
    let index = if x > 0. { index.checked_sub(1)? } else { index + 1 };
    spaces.get(index).map(|other| other.id)
}

/// The Space whose page is nearest its place while `from`'s is at `x`, pages `width` wide: it or
/// the one beside.
fn nearest(
    spaces: &[Space],
    form: Option<(u64, u64)>,
    from: u64,
    x: f32,
    width: f32,
) -> Option<u64> {
    spaces.iter().find(|space| space.id == from)?;
    match (x / width).abs() >= 0.5 {
        true => beside(spaces, form, from, x).or(Some(from)),
        false => Some(from),
    }
}

/// Where a page let go at `x`, going at `velocity` a second, heads: a page `width` that way on a
/// flick, else where it is.
fn heading(x: f32, velocity: f32, width: f32) -> f32 {
    if velocity.abs() >= SWIPE_FLICK { velocity.signum() * width } else { x }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui::{
        AppContext, Bounds, Context, Entity, Modifiers, Pixels, ScrollDelta, ScrollEvent,
        TestAppContext, TouchPhase, VisualTestContext, Window, point, px,
    };

    use super::{heading, nearest};
    use crate::shell::{
        Folders, PLATE_INSET, SWIPE_TRACKING, Shell, Space, SpaceForm, WindowButtonGeometry,
        recorder::Recorder,
    };
    use crate::text_input::TextInput;

    fn spaces(ids: &[u64]) -> Vec<Space> {
        let space = |&id| Space {
            id,
            name: format!("Space {id}").into(),
            color: None,
            theme: None,
            icon: Default::default(),
            profile: 0,
            agent: Default::default(),
            pinned: Vec::new(),
            today: Vec::new(),
            splits: Vec::new(),
            folders: Folders::default(),
            shown: 0,
        };
        ids.iter().map(space).collect()
    }

    /// Where fingers let go of a page at `x` going at `velocity` land it, with Space 4's form
    /// made from Space 1, pages 200 wide.
    fn lands(from: u64, x: f32, velocity: f32) -> Option<u64> {
        let spaces = spaces(&[1, 2, 3, 4]);
        nearest(&spaces, Some((4, 1)), from, heading(x, velocity, 200.), 200.)
    }

    /// Back off the form, fingers land on the Space it was made from, not the one before it, past
    /// halfway or on a flick; short of that, and going on past it, they land back on the form.
    #[test]
    fn a_swipe_back_off_the_form_lands_where_it_was_made() {
        assert_eq!(lands(4, 120., 0.), Some(1));
        assert_eq!(lands(4, 20., 300.), Some(1));
        assert_eq!(lands(4, 60., 0.), Some(4));
        assert_eq!(lands(4, -30., -300.), Some(4));
        assert_eq!(lands(4, 120., -300.), Some(4));
    }

    /// Other Spaces page to their neighbours as before, and not past the first and last.
    #[test]
    fn other_spaces_land_beside_them() {
        assert_eq!(lands(2, 120., 0.), Some(1));
        assert_eq!(lands(2, -120., 0.), Some(3));
        assert_eq!(lands(3, -20., -300.), Some(4));
        assert_eq!(lands(1, 120., 0.), Some(1));
    }

    /// A window showing Space `shown` of three, as the core leaves it.
    fn three_spaces(
        cx: &mut TestAppContext,
        shown: u64,
    ) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (id, format!("Space {id}").into(), None, 1, "".into(), "".into(), "".into())
            };
            shell.set_spaces(vec![space(1), space(2), space(3)], cx);
            shell.set_space(shown, cx);
        });
        cx.run_until_parked();
        (shell, host, cx)
    }

    /// Two fingers across the sidebar: down, moved `way` of the page's way in one go, and, if
    /// `lift`, up.
    fn fingers(
        shell: &mut Shell,
        way: f32,
        lift: bool,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        let fingers = |touch_phase, x| ScrollEvent {
            delta: ScrollDelta::Pixels(point(px(x), px(0.))),
            touch_phase,
            ..Default::default()
        };
        use TouchPhase::{Ended, Moved, Started};
        let phases = [(Started, 0.), (Moved, way / SWIPE_TRACKING), (Ended, 0.)];
        for (phase, x) in phases.into_iter().take(if lift { 3 } else { 2 }) {
            shell.scroll(&fingers(phase, x), window, cx);
        }
    }

    /// Two fingers across the sidebar, down, moved `way` of the page's way in one go, and up.
    fn swipe(shell: &mut Shell, way: f32, window: &mut Window, cx: &mut Context<Shell>) {
        fingers(shell, way, true, window, cx);
    }

    /// AppKit's buttons as macOS 26 and later draw them in an app built with an older SDK, as
    /// 0.1.0 was: 12 across on a pitch of 20, where an app built with macOS 26's has 14 on 23.
    const OLDER_SDK: WindowButtonGeometry =
        WindowButtonGeometry { centres: [20., 40., 60.], y: 24., diameter: 12. };

    /// A window of three Spaces on the first, whose host draws its buttons as `OLDER_SDK`.
    fn older_sdk_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        host.window_buttons.set(Some(OLDER_SDK));
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (id, format!("Space {id}").into(), None, 1, "".into(), "".into(), "".into())
            };
            shell.set_spaces(vec![space(1), space(2), space(3)], cx);
            shell.set_space(1, cx);
        });
        cx.run_until_parked();
        (shell, cx)
    }

    /// Fingers going `way` in `steps` across the sidebar, still down. At each step, after
    /// `check`, says whether the window's own buttons show.
    fn fingers_by_steps(
        shell: &mut Shell,
        way: f32,
        steps: usize,
        window: &mut Window,
        cx: &mut Context<Shell>,
        mut check: impl FnMut(&Shell),
    ) {
        let event = |touch_phase, x: f32| ScrollEvent {
            delta: ScrollDelta::Pixels(point(px(x), px(0.))),
            touch_phase,
            ..Default::default()
        };
        shell.scroll(&event(TouchPhase::Started, 0.), window, cx);
        for _ in 0..steps {
            let step = way / steps as f32 / SWIPE_TRACKING;
            shell.scroll(&event(TouchPhase::Moved, step), window, cx);
            check(shell);
        }
    }

    /// The copy the Library's panel draws of the window's buttons, where it is in the window.
    fn panel_copy(shell: &Shell) -> [Bounds<Pixels>; 3] {
        let left = px(shell.library_column_left());
        let moved =
            |frame: Bounds<Pixels>| Bounds { origin: frame.origin + point(left, px(0.)), ..frame };
        shell.window_button_frames().map(moved)
    }

    /// Whether buttons' frames `a` are `b`'s size and row, and across to within `across`.
    fn same(a: [Bounds<Pixels>; 3], b: [Bounds<Pixels>; 3], across: f32) -> bool {
        a.iter().zip(&b).all(|(a, b)| {
            let near = |a: Pixels, b: Pixels, by: f32| (f32::from(a) - f32::from(b)).abs() <= by;
            near(a.left(), b.left(), across) && a.top() == b.top() && a.size == b.size
        })
    }

    /// The copies of the window's buttons take the size and places the system draws its own at,
    /// not Arc's under macOS 26's SDK, so a swipe into the Library can't show them change size
    /// (#1310): the panel's copy is the window's own size all the way in, on them where they
    /// swap, and the sidebar's, which recedes with it, is their size where it takes over.
    #[gpui::test]
    fn the_copies_of_the_windows_buttons_are_its_own_size_into_the_library(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx) = older_sdk_window(cx);
        let own = OLDER_SDK.frames();
        shell.update_in(cx, |shell, window, cx| {
            assert!(shell.window_buttons_shown(), "the window's own aren't up at rest");
            assert_eq!(shell.window_button_frames(), own, "the copies aren't the system's");
            let (mut was_own, mut swapped, mut receded) = (true, false, false);
            let way = 1.2 * shell.library_width();
            fingers_by_steps(shell, way, 600, window, cx, |shell| {
                let own_up = shell.window_buttons_shown();
                let (panel, sidebar) = (shell.library_buttons(), shell.sidebar_buttons());
                assert!(own_up != panel, "drawn twice or not at all");
                if panel {
                    let copy = panel_copy(shell);
                    assert!(copy.iter().all(|frame| frame.size == own[0].size), "{copy:?}");
                }
                if sidebar && was_own {
                    // Where the sidebar's copy takes over, it hasn't begun to recede.
                    assert!(shell.sidebar_scale() > 0.999, "at {}", shell.sidebar_scale());
                    receded = true;
                }
                if own_up && !was_own {
                    // Where the panel stops, to within a pixel's half.
                    assert!(same(panel_copy(shell), own, 0.5), "swapped off the window's own");
                    swapped = true;
                }
                was_own = own_up;
            });
            assert!(receded && swapped, "the swipe didn't swap both ways");
        });
    }

    /// Back out of the Library, the panel's copy takes over on the window's own, at their size,
    /// and stays their size as it goes.
    #[gpui::test]
    fn the_copies_of_the_windows_buttons_are_its_own_size_out_of_the_library(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx) = older_sdk_window(cx);
        let own = OLDER_SDK.frames();
        shell.update(cx, |shell, cx| {
            shell.set_library(true, cx);
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        shell.update_in(cx, |shell, window, cx| {
            assert!(shell.window_buttons_shown(), "the window's own aren't up over the Library");
            let mut swapped = false;
            let (way, steps) = (-shell.library_width(), 600);
            // The window's own go once the panel has left them by a pixel's half: the copy is
            // then on them but for the step that took it there.
            let step = shell.library_width() / steps as f32;
            fingers_by_steps(shell, way, steps, window, cx, |shell| {
                if shell.library_buttons() {
                    let copy = panel_copy(shell);
                    assert!(copy.iter().all(|frame| frame.size == own[0].size), "{copy:?}");
                    if !swapped {
                        assert!(same(copy, own, step + 0.5), "swapped off the window's own");
                        swapped = true;
                    }
                }
            });
            assert!(swapped, "the window's own never went");
        });
    }

    /// Fingers taking the Library back land on the first Space, which it sits left of, though it
    /// was opened from the second.
    #[gpui::test]
    fn a_swipe_back_from_the_library_lands_on_the_first_space(cx: &mut TestAppContext) {
        let (shell, host, cx) = three_spaces(cx, 2);
        shell.update(cx, |shell, cx| {
            shell.set_library(true, cx);
            // At rest, open.
            shell.library_motion = None;
        });
        cx.run_until_parked();
        shell.update_in(cx, |shell, window, cx| {
            swipe(shell, -400., window, cx);
            assert!(!shell.library_open, "the swipe left the Library open");
        });
        assert_eq!(*host.shown.borrow(), [1]);
    }

    /// One long swipe from the second of three Spaces stops on the first, however far the fingers
    /// go, with the Library closed, and the next swipe from there brings the Library out.
    #[gpui::test]
    fn a_swipe_from_the_second_space_stops_on_the_first(cx: &mut TestAppContext) {
        let (shell, host, cx) = three_spaces(cx, 2);
        shell.update_in(cx, |shell, window, cx| {
            let way = shell.width() + 0.8 * shell.library_width();
            swipe(shell, way, window, cx);
            assert!(!shell.library_open, "the swipe went on past the first Space into the Library");
            shell.set_space(1, cx);
            swipe(shell, 0.9 * shell.library_width(), window, cx);
            assert!(shell.library_open, "the next swipe left the Library closed");
        });
        assert_eq!(*host.shown.borrow(), [1]);
    }

    /// The sidebar under the Library fades out whole and the Library fades in over it by what the
    /// sidebar has lost, never a whole sheet over a half-faded sidebar, which would show only cut
    /// along its edge: with fingers part of the way out, and on a click's close as the sidebar
    /// comes back.
    #[gpui::test]
    fn the_library_fades_in_over_the_sidebar_both_ways(cx: &mut TestAppContext) {
        let (shell, _, cx) = three_spaces(cx, 1);
        let crossing = |shell: &Shell, now: Instant| {
            let sidebar = shell.sidebar_opacity_at(now);
            let library = Shell::library_opacity(sidebar);
            sidebar > 0. && sidebar < 1. && (sidebar + library - 1.).abs() < 1e-6
        };
        shell.update_in(cx, |shell, window, cx| {
            fingers(shell, 0.2 * shell.library_width(), false, window, cx);
            assert!(crossing(shell, Instant::now()), "fingers: the sidebar {}", shell.sidebar_opacity());
            fingers(shell, 0.9 * shell.library_width(), true, window, cx);
            assert!(shell.library_open, "the swipe left the Library closed");
            shell.library_motion = None;
            shell.sidebar_fade = None;
            shell.set_library(false, cx);
            // Halfway through the sidebar's fade back in, on a clock of the test's own.
            let start = Instant::now();
            shell.sidebar_fade = shell.sidebar_fade.map(|(from, _)| (from, start));
            let halfway = start + Duration::from_millis(145);
            assert!(crossing(shell, halfway), "a click: the sidebar {}", shell.sidebar_opacity_at(halfway));
        });
    }

    /// Back off the form, the URL field and the favorites are drawn from the first frame, at the
    /// offset of the page of the Space beside, as its rows are, while fingers move it and while it
    /// settles after they lift, not hidden until the form is gone.
    #[gpui::test]
    fn the_url_field_and_favorites_come_back_with_the_rows_off_the_form(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
        // The fourth Space's form, made from the third, in a profile with a favorite.
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (id, format!("Space {id}").into(), None, 1, "".into(), "".into(), "".into())
            };
            shell.set_spaces((1..=4).map(space).collect(), cx);
            shell.set_space(4, cx);
            let name = cx.new(|cx| TextInput::new("Space name", cx));
            shell.form = Some(SpaceForm { space: 4, from: 3, name, tab: None });
            shell.set_favorites(vec![(10, 1)], cx);
        });
        cx.run_until_parked();
        let rest = shell.read_with(cx, |shell, _| shell.drawn_tiles()[&10]);
        let tile = |shell: &Shell| shell.tip_frames.borrow().get(&10).copied();
        assert!(shell.read_with(cx, |shell, _| tile(shell).is_none()), "the form left a tile up");

        // Fingers 0.7 of the way back, down: the Space's page is 0.3 in from the left.
        shell.update_in(cx, |shell, window, cx| {
            let way = 0.7 * shell.width();
            fingers(shell, way, false, window, cx);
        });
        cx.run_until_parked();
        let (tile_at, page) = shell.read_with(cx, |shell, _| {
            let page = shell.pages().into_iter().find(|&(space, _)| space == 3);
            (tile(shell), page.expect("no page of the Space beside").1)
        });
        assert!(page < 0., "the Space's page isn't coming in from the left");
        let tile_at = tile_at.expect("no tile while the form pages back");
        let centre = rest[0] + rest[2] / 2. + page;
        let at = f32::from(tile_at.center().x);
        assert!((at - centre).abs() < 0.5, "the tile isn't with the rows");
        // The field is where the page puts it: a press in its drawn part opens the command bar.
        cx.simulate_click(point(px(100.), px(64.)), Modifiers::none());
        assert_eq!(host.command_bars.take().len(), 1, "the URL field wasn't drawn");

        // Fingers up past halfway: Cancel, and the page settles in with the form's going out.
        shell.update_in(cx, |shell, window, cx| {
            let way = 0.7 * shell.width();
            fingers(shell, way, true, window, cx);
            shell.tip_frames.borrow_mut().clear();
            assert!(shell.form_going.is_some(), "the swipe didn't cancel the form");
            cx.notify();
        });
        cx.run_until_parked();
        let tile_at = shell.read_with(cx, |shell, _| tile(shell));
        let tile_at = tile_at.expect("no tile while the cancelled form goes out");
        let at = f32::from(tile_at.center().x);
        assert!(at < rest[0] + rest[2] / 2., "the tile isn't coming in");
    }

    /// Every frame of a swipe and of a dot click's slide between Spaces of two profiles draws each
    /// Space's URL field and favorites at its page's offset: the shown Space's live ones, and the
    /// page beside's its own. Past the last Space, where the page rubber-bands, they stay put.
    #[gpui::test]
    fn the_url_field_and_favorites_page_with_their_space(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (id, format!("Space {id}").into(), None, id, "".into(), "".into(), "".into())
            };
            shell.set_spaces(vec![space(1), space(2)], cx);
            shell.set_space(1, cx);
            shell.set_favorites(vec![(10, 1), (20, 2)], cx);
        });
        cx.run_until_parked();
        let rest = shell.read_with(cx, |shell, _| shell.drawn_tiles()[&10][0]);
        let favorite = |space: u64| space * 10;
        // Draws a frame and says how many pages it drew, and for each, the offsets it had just
        // before and after the frame (a slide's runs on the clock), and where its tile and its URL
        // field beside were drawn.
        type Drawn = Vec<((f32, f32), Option<f32>, Option<f32>)>;
        let frame = |shell: &Entity<Shell>, cx: &mut VisualTestContext| -> (usize, Drawn) {
            let before = shell.update(cx, |shell, cx| {
                shell.tip_frames.borrow_mut().clear();
                shell.beside_frames.borrow_mut().clear();
                cx.notify();
                shell.pages()
            });
            cx.run_until_parked();
            shell.read_with(cx, |shell, _| {
                let left = |frame: Option<&Bounds<Pixels>>| frame.map(|f| f32::from(f.left()));
                let drawn = before.iter().zip(shell.pages()).map(|(&(space, x), (_, after))| {
                    let xs = (x.min(after), x.max(after));
                    if space == shell.space {
                        (xs, left(shell.tip_frames.borrow().get(&favorite(space))), None)
                    } else {
                        let frames = shell.beside_frames.borrow();
                        (xs, left(frames.get(&favorite(space))), left(frames.get(&0)))
                    }
                });
                (before.len(), drawn.collect())
            })
        };
        let at_page = |drawn: Drawn| {
            let near = |at: f32, (low, high): (f32, f32)| at > low - 0.5 && at < high + 0.5;
            for (xs, tile, field) in drawn {
                let tile = tile.expect("a page drew no tile");
                assert!(near(tile - rest, xs), "a tile at {tile} isn't with its page at {xs:?}");
                if let Some(field) = field {
                    assert!(near(field - PLATE_INSET, xs), "the field beside isn't at {xs:?}");
                }
            }
        };

        // Fingers move the second Space's page in a tenth at a time; on every frame both pages
        // are drawn whole, and a press in the shown URL field where its page put it is its own.
        let width = shell.read_with(cx, |shell, _| shell.width());
        let move_by = |shell: &Entity<Shell>, cx: &mut VisualTestContext, phase, way: f32| {
            let event = ScrollEvent {
                delta: ScrollDelta::Pixels(point(px(way / SWIPE_TRACKING), px(0.))),
                touch_phase: phase,
                ..Default::default()
            };
            shell.update_in(cx, |shell, window, cx| shell.scroll(&event, window, cx));
        };
        move_by(&shell, cx, TouchPhase::Started, 0.);
        for _ in 0..8 {
            move_by(&shell, cx, TouchPhase::Moved, -0.1 * width);
            let (pages, drawn) = frame(&shell, cx);
            assert_eq!(pages, 2, "the second Space's page isn't coming in");
            at_page(drawn);
        }
        // The shown field's part still in the sidebar, at its left.
        let x = shell.read_with(cx, |shell, _| shell.top_offset());
        cx.simulate_click(point(px((x + width) / 2.), px(64.)), Modifiers::none());
        assert_eq!(host.command_bars.take().len(), 1, "the URL field isn't with its page");
        move_by(&shell, cx, TouchPhase::Ended, 0.);
        cx.run_until_parked();

        // A click on the first Space's dot, from the second: its page slides in, frame by frame.
        shell.update(cx, |shell, cx| {
            if let Some(settle) = &mut shell.settle {
                settle.start -= Duration::from_secs(1);
            }
            shell.set_space(2, cx);
            shell.set_space(1, cx);
        });
        let mut frames = 0;
        while shell.read_with(cx, |shell, _| shell.settle.is_some()) {
            let (pages, drawn) = frame(&shell, cx);
            if pages == 2 {
                frames += 1;
                at_page(drawn);
            }
            shell.update(cx, |shell, _| {
                if let Some(settle) = &mut shell.settle {
                    settle.start -= Duration::from_millis(16);
                }
            });
        }
        assert!(frames > 5, "the slide drew {frames} frames with a page beside");

        // Past the last Space, its page rubber-bands under the URL field and favorites.
        shell.update(cx, |shell, cx| shell.set_space(2, cx));
        shell.update(cx, |shell, _| {
            if let Some(settle) = &mut shell.settle {
                settle.start -= Duration::from_secs(1);
            }
        });
        move_by(&shell, cx, TouchPhase::Started, 0.);
        move_by(&shell, cx, TouchPhase::Moved, -0.5 * width);
        let (pages, drawn) = frame(&shell, cx);
        assert_eq!(pages, 1, "a page came in past the last Space");
        assert!(drawn[0].0.1 < 0., "the last Space's page didn't rubber-band");
        let tile = drawn[0].1.expect("no tile past the last Space");
        assert!((tile - rest).abs() < 0.5, "the favorites left their place past the last Space");
    }

    /// A window showing Space 1 of two, back from Space 2 as a click on its dot brings it, its
    /// page sliding in from the left, or, `settled`, with the slide over.
    fn switched_back(
        cx: &mut TestAppContext,
        settled: bool,
    ) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space =
                |id| (id, format!("Space {id}").into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space(1), space(2)], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
            shell.set_space(2, cx);
            shell.set_space(1, cx);
            if settled && let Some(settle) = &mut shell.settle {
                settle.start -= Duration::from_secs(1);
            }
        });
        (shell, host, cx)
    }

    /// Two fingers across the sidebar, `count` events of `dx`, held still past the moves a flick
    /// is read from, then lifted: the spaces check's slow, short swipe.
    fn held_swipe(shell: &Entity<Shell>, cx: &mut VisualTestContext, dx: f32, count: usize) {
        let event = |touch_phase, dx| ScrollEvent {
            position: point(px(110.), px(500.)),
            delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
            touch_phase,
            ..Default::default()
        };
        shell.update_in(cx, |shell, window, cx| {
            for step in 0..count {
                let phase = if step == 0 { TouchPhase::Started } else { TouchPhase::Moved };
                shell.scroll(&event(phase, dx), window, cx);
            }
            let held = Instant::now() - Duration::from_millis(1500);
            for (at, _) in &mut shell.swipe.as_mut().expect("a swipe").samples {
                *at = (*at).min(held);
            }
            shell.scroll(&event(TouchPhase::Ended, 0.), window, cx);
        });
    }

    /// Fingers catch a page still sliding in from a switch where it is, so a short swipe back the
    /// way it came, let go there, lands on the Space it left. Once the page has settled, the same
    /// swipe is short of halfway and stays.
    #[gpui::test]
    fn a_short_swipe_lands_from_where_it_caught_the_page(cx: &mut TestAppContext) {
        let (shell, host, window) = switched_back(cx, false);
        held_swipe(&shell, window, -10., 10);
        assert_eq!(*host.shown.borrow(), [2]);

        let (shell, host, window) = switched_back(cx, true);
        held_swipe(&shell, window, -10., 10);
        assert!(host.shown.borrow().is_empty());
    }
}
