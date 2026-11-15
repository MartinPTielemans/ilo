//! Two fingers or a wheel across the sidebar, paging between Spaces and out to the
//! Library (docs/ux-spec.md › Spaces › Switching), and the list's scroll edge.

use std::collections::VecDeque;
use web_time::Instant;

use gpui::{
    Context, DispatchPhase, Point, ScrollDelta, ScrollEvent, TouchPhase, Window, canvas, div,
    point, prelude::*, px,
};

use super::{
    Command, ROW_PITCH, SCROLL_EDGE, SCROLL_EDGE_FADE, SWIPE_FLICK, SWIPE_HISTORY, SWIPE_TRACKING,
    Settle, Shell, Space, Swipe, WEB, WHEEL_BURST, WHEEL_LINE, WHEEL_REST, fade_at,
    library::library_band,
    rubber_band,
    wheel_gestures::{GAP, LONG},
};

/// Points the fingers go on the web before which way they went says whether they swipe across
/// or scroll along.
const DECIDE: f32 = 4.;

impl Shell {
    /// The Spaces' pages the sidebar draws, and where from its left edge: the shown Space's, and
    /// while a swipe or a settle moves one, it and the page beside it.
    pub(super) fn pages(&self) -> Vec<(u64, f32)> {
        let swipe = self.swipe.as_ref().filter(|swipe| {
            let paging = swipe.across == Some(true) && swipe.cards.is_none();
            paging && self.library_swiped(swipe).is_none()
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
            frames
                .borrow_mut()
                .push(started.elapsed().as_secs_f32() * 1000.)
        };
        (self.pages().len() > 1).then(|| canvas(|_, _, _| {}, paint).absolute().size_0())
    }

    /// Logs the frames drawn while the pages moved, as they settle. check.sh reads the line.
    pub(super) fn log_paging_frames(&self) {
        let mut frames = std::mem::take(&mut *self.paging_frames.borrow_mut());
        frames.sort_by(f32::total_cmp);
        let median = frames.get(frames.len().saturating_sub(1) / 2);
        if let (Some(median), Some(max)) = (median, frames.last()) {
            log!(
                "browser: paged frames {}, median {median:.2} ms, max {max:.2} ms",
                frames.len()
            );
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
        let index = self
            .spaces
            .iter()
            .position(|space| space.id == swipe.from)
            .unwrap_or(0);
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
        first && !self.sidebar_hidden && !self.page_fullscreen && !self.one_space()
    }

    /// How far out a swipe has taken the Library (`library_out`), while it moves the Library.
    pub(super) fn library_swiped(&self, swipe: &Swipe) -> Option<f32> {
        if swipe.across != Some(true) || swipe.cards.is_some() {
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

    /// The preview's fingers (PREVIEW_SWIPE): `x` across the sidebar since the last, down with
    /// `phase`, through the gesture's own code, so a recording shows what fingers do.
    pub fn preview_fingers(
        &mut self,
        x: f32,
        phase: TouchPhase,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let event = ScrollEvent {
            position: gpui::point(px(self.column_x() + self.width() / 2.), px(300.)),
            delta: ScrollDelta::Pixels(gpui::point(px(x), px(0.))),
            touch_phase: phase,
            ..Default::default()
        };
        self.scroll(&event, window, cx);
        cx.notify();
    }

    /// Fingers or a wheel over the sidebar. Across it they page between Spaces, and along it they
    /// scroll the list. Over the Library's Spaces cards they scroll them across, if they don't
    /// fit. Returns whether paging, or the cards, took the event.
    fn scroll(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.scroll_at(event, Instant::now(), window, cx)
    }

    /// The same, with the event come at `now`.
    fn scroll_at(
        &mut self,
        event: &ScrollEvent,
        now: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // The form pages back to the Space it was made from, but not while another Space shows.
        let form_hidden = self.form.is_some() && self.form().is_none();
        if form_hidden || self.renaming.is_some() || self.drag.is_some() {
            return false;
        }
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => delta.map(f32::from),
            ScrollDelta::Lines(lines) => {
                return self.wheel(lines, f32::from(event.position.x), window, cx);
            }
        };
        match event.touch_phase {
            TouchPhase::Started => {
                self.swiped = false;
                let cards = self.fingers_down(f32::from(event.position.x));
                self.swipe = Some(Swipe {
                    from: self.space,
                    library: self.library_open || self.library_motion.is_some(),
                    across: None,
                    way: 0.,
                    samples: VecDeque::new(),
                    landing: None,
                    ready: false,
                    turned: None,
                    cards,
                });
            }
            TouchPhase::Ended | TouchPhase::Cancelled => return self.release(now, window, cx),
            TouchPhase::Moved => {}
        }
        let Some(mut swipe) = self.swipe.take() else {
            // The momentum after fingers lift from the cards carries them on.
            if self.coast_cards(delta.x, cx) {
                return true;
            }
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
            // Over the Spaces cards, fingers scroll them, unless the cards are at the end they
            // push past, where the swipe is the Library's (Arc 1.171).
            swipe.cards = swipe
                .cards
                .filter(|_| across)
                .and_then(|_| self.catch_cards(delta.x));
            if swipe.cards.is_some() {
                // check.sh waits for this line.
                log!("browser: swipe cards");
            }
            // Fingers catch a page, or the Library, that's still settling where it is.
            if across && swipe.library {
                swipe.way = (self.library_out() - 1.) * self.library_width();
                self.library_motion = None;
            } else if across
                && let Some(settle) = self.settle.take()
                && let Some(x) = settle.offset()
            {
                (swipe.from, swipe.way) = (settle.space, x);
            }
            // Where letting go would take the pages before they move.
            if across {
                swipe.landing = self.letting_go(&swipe, 0.);
            }
        }
        let across = swipe.across == Some(true);
        if across && let Some(cards) = swipe.cards {
            swipe.cards = Some(self.move_cards(cards, delta.x, cx));
        } else if across {
            let past_end = |shell: &Self, swipe: &Swipe| {
                shell.library_swiped(swipe).is_some_and(|out| out > 1.)
            };
            let was_past = past_end(self, &swipe);
            swipe.way += SWIPE_TRACKING * delta.x;
            if !was_past && past_end(self, &swipe) {
                // check.sh waits for this line.
                log!("browser: library past its end");
            }
            let x = self.swiped_to(&swipe);
            swipe.samples.push_back((now, x));
            while swipe
                .samples
                .front()
                .is_some_and(|&(at, _)| now - at > SWIPE_HISTORY)
            {
                swipe.samples.pop_front();
            }
            self.ready_new_space(&mut swipe, cx);
            if self.library_swiped(&swipe).is_some() && self.library.is_none() {
                self.show_section(self.section, cx);
            }
            // The trackpad clicks as where letting go would take the pages changes, by the rule
            // the release goes by, so it's felt while the fingers are down: as a swipe takes the
            // next Space, or the Library, and if it turns back (#1075). How fast they go is known
            // from two moves; until then where it was stands.
            if let Some(velocity) = velocity(&swipe.samples, now) {
                let landing = self.letting_go(&swipe, velocity);
                if landing != swipe.landing {
                    swipe.landing = landing;
                    self.browser.detent(cx);
                }
            }
            cx.notify();
        }
        self.swipe = Some(swipe);
        across
    }

    /// The fingers lifted. The pages settle on the next Space in the direction of a flick, else
    /// on the one nearest its place, keeping the release's speed if it's heading there. Off the
    /// form, that's Cancel, its page going out as the Space it was made from settles.
    fn release(&mut self, now: Instant, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(swipe) = self.swipe.take().filter(|swipe| swipe.across == Some(true)) else {
            return false;
        };
        self.swiped = true;
        if let Some(cards) = swipe.cards {
            self.release_cards(cards, cx);
            return true;
        }
        let x = self.swiped_to(&swipe);
        let velocity = velocity(&swipe.samples, now).unwrap_or(0.);
        if let Some(out) = self.library_swiped(&swipe) {
            let open = library_opens(out, velocity);
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
        let from = if space == swipe.from {
            x
        } else {
            x - toward.signum() * self.width()
        };
        let velocity = if velocity * from < 0. { velocity } else { 0. };
        let cancel = space != swipe.from && self.form().is_some();
        let beside = cancel.then_some(swipe.from);
        // The fingers' speed carries on, unless Reduce Motion has the pages land where they go.
        let start = crate::motion::start_at(cx.background_executor().now(), cx);
        self.settle = Some(Settle {
            space,
            beside,
            from,
            velocity,
            start,
            clock: cx.background_executor().clone(),
            next_door: false,
        });
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

    /// Where letting go now, going at `velocity`, would take the pages, as `release` decides: the
    /// Library, or a Space, the first when the Library shuts. The pull past the last Space has its
    /// own detent.
    fn letting_go(&self, swipe: &Swipe, velocity: f32) -> Option<Landing> {
        if let Some(out) = self.library_swiped(swipe) {
            if library_opens(out, velocity) {
                return Some(Landing::Library);
            }
            return self.spaces.first().map(|first| Landing::Space(first.id));
        }
        let toward = heading(self.swiped_to(swipe), velocity, self.width());
        self.nearest(swipe.from, toward).map(Landing::Space)
    }

    /// A browser's wheel event at `at`, as the fingers landing, moving and lifting that AppKit's
    /// phases tell (`wheel_gestures`): a page swipe commits its Space as they lift, and the rest of
    /// its momentum is swallowed. The fingers lift too once the events stop.
    fn web_scroll(
        &mut self,
        event: &ScrollEvent,
        at: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.note("wheel", "");
        let ScrollDelta::Pixels(delta) = event.delta else {
            return self.scroll_at(event, at, window, cx);
        };
        // The way the swipe goes once that's known, so a hand's drift along a swipe across never
        // reads as the fingers turning; until then, the way of the larger delta.
        let across = self.swipe.as_ref().and_then(|swipe| swipe.across);
        let way = match across {
            Some(true) => delta.x,
            Some(false) => delta.y,
            None if delta.x.abs() >= delta.y.abs() => delta.x,
            None => delta.y,
        };
        let cut = self.web_wheel.event(f32::from(way), at);
        let phase = |touch_phase, delta| ScrollEvent {
            touch_phase,
            delta,
            ..event.clone()
        };
        let still = ScrollDelta::Pixels(Point::default());
        if let Some(why) = cut.lifted {
            self.note("lift", why);
            self.web_undecided = None;
            self.web_lift(&phase(TouchPhase::Ended, still), at, window, cx);
        }
        if let Some(why) = cut.landed {
            self.note("land", why);
            self.web_undecided = Some(Point::default());
            self.scroll_at(&phase(TouchPhase::Started, still), at, window, cx);
        }
        // The fingers' first DECIDE points go on as one move, so the way they went, not one
        // event a browser rounded or summed, tells across from along, as AppKit starts a scroll
        // only once they've gone a few points.
        let moved = match self.web_undecided.take() {
            Some(way) => {
                let way = way + delta;
                if way.x.abs().max(way.y.abs()) < px(DECIDE) {
                    self.web_undecided = Some(way);
                    None
                } else {
                    Some(ScrollDelta::Pixels(way))
                }
            }
            None => Some(event.delta),
        };
        let deciding = self
            .swipe
            .as_ref()
            .is_none_or(|swipe| swipe.across.is_none());
        // Until the lift is found, momentum moves the pages as fingers would, on past the Space they
        // take. Only fingers going faster than they have yet take them past it: one swipe takes one
        // Space, and past it fingers only stretch the band. A slower event goes as far as that
        // Space and no further, or at 60 Hz the one that crosses it, two summed, ran on past it.
        let fastest = self.web_wheel.fastest();
        let took = moved.is_none_or(|delta| {
            let delta = match (delta, self.swipe.as_ref()) {
                (ScrollDelta::Pixels(delta), Some(swipe)) if !fastest && delta.x != px(0.) => {
                    let x = self.short_of_next(swipe, f32::from(delta.x));
                    if x == 0. {
                        return true;
                    }
                    ScrollDelta::Pixels(point(px(x), delta.y))
                }
                (delta, _) => delta,
            };
            self.scroll_at(&phase(TouchPhase::Moved, delta), at, window, cx)
        });
        if deciding && let Some(across) = self.swipe.as_ref().and_then(|swipe| swipe.across) {
            self.note("axis", if across { "across" } else { "along" });
        }
        let lift = phase(TouchPhase::Ended, still);
        self.web_wheel_quiet = Some(cx.spawn_in(window, async move |shell, cx| {
            // Slowed fingers lift after GAP, and others after LONG.
            for wait in [GAP, LONG - GAP] {
                cx.background_executor().timer(wait).await;
                let lift = lift.clone();
                let quiet = |shell: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
                    shell.web_quiet(&lift, cx.background_executor().now(), window, cx)
                };
                shell.update_in(cx, quiet).ok();
            }
        }));
        took
    }

    /// How much of `dx` moves `swipe`'s pages no further than the Space beside the one it began
    /// on: all of it short of that Space or back toward it, as far as it of one that crosses it,
    /// and none further past it.
    fn short_of_next(&self, swipe: &Swipe, dx: f32) -> f32 {
        let across = swipe.across == Some(true) && swipe.cards.is_none() && !swipe.library;
        let width = self.width();
        let way = swipe.way + SWIPE_TRACKING * dx;
        if !across || way.abs() <= width || way.abs() < swipe.way.abs() {
            return dx;
        }
        let room = (width.copysign(way) - swipe.way) / SWIPE_TRACKING;
        if room * dx > 0. { room } else { 0. }
    }

    /// No wheel event since the last, at `now`: fingers still down lift, as `lift`.
    fn web_quiet(
        &mut self,
        lift: &ScrollEvent,
        now: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(why) = self.web_wheel.quiet(now) {
            self.note("lift", why);
            self.web_lift(lift, now, window, cx);
        }
    }

    /// The fingers lifted, as a browser's wheel events tell some events after they did: by then
    /// their momentum has moved the page on, as the app's settle would have, so the settle keeps no
    /// more of their speed than lands it without passing its place. Their full speed still says
    /// which Space it lands on.
    fn web_lift(
        &mut self,
        lift: &ScrollEvent,
        at: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let before = self.settle.as_ref().map(|settle| settle.start);
        let (from, library) = (
            self.swipe.as_ref().map(|swipe| swipe.from),
            self.library_open,
        );
        self.scroll_at(lift, at, window, cx);
        if self.library_open != library {
            self.note("library", if self.library_open { "open" } else { "closed" });
        }
        if let Some(settle) = self
            .settle
            .as_mut()
            .filter(|settle| Some(settle.start) != before)
        {
            settle.spend();
            let space = settle.space;
            let index = |id| self.spaces.iter().position(|space| space.id == id);
            if let Some((to, from)) = index(space).zip(from.and_then(index)) {
                self.note("space", &format!("{:+}", to as i64 - from as i64));
            }
        }
    }

    /// Has the demo's `?trace` told each thing the wheel's events decide of a swipe.
    pub fn note_wheel(&mut self, notes: super::WheelNotes) {
        self.web_notes = Some(notes);
    }

    fn note(&self, kind: &'static str, what: &str) {
        if let Some(notes) = &self.web_notes {
            notes(kind, what);
        }
    }

    /// A wheel's ticks across the sidebar page once per burst, then rest. Past the first Space they
    /// open the Library, and back they close it. Back off the form they cancel it. Over the
    /// Library's Spaces cards at `x`, they scroll them first.
    fn wheel(
        &mut self,
        lines: Point<f32>,
        x: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if lines.x.abs() <= lines.y.abs() {
            return false;
        }
        if self.wheel_cards(lines.x, x, cx) {
            self.wheel = None;
            return true;
        }
        let now = Instant::now();
        if self
            .wheel_paged
            .is_some_and(|paged| now - paged < WHEEL_REST)
        {
            // A browser's momentum goes on long after a burst: it rests until the events stop.
            if WEB {
                self.wheel_paged = Some(now);
            }
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
            self.page_step(sum > 0., "wheel", window, cx);
        }
        true
    }

    /// One step along the row the pages go by, back (to the left) or forward, as a wheel's burst
    /// or a mouse's button takes it: back from the first Space opens the Library, forward leaves
    /// it for the first Space, and back off the form cancels it.
    fn page_step(&mut self, back: bool, via: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_popovers(cx);
        let library = back && (self.library_open || self.library_beside(self.space));
        if library != self.library_open {
            // check.sh waits for this line.
            log!("browser: command view-library via {via}");
            if !library {
                self.show_first_space(cx);
            }
            self.set_library(library, cx);
        } else if !self.library_open
            && let Some(space) =
                self.nearest(self.space, if back { 1. } else { -1. } * self.width())
            && space != self.space
        {
            if self.form().is_some() {
                self.cancel_space(window, cx);
                return;
            }
            // check.sh waits for this line.
            log!("browser: command select-space via {via}");
            self.show_space(space, cx);
        }
    }

    /// A mouse's back or forward button over the UI takes the pages a step, and forward from the
    /// last Space makes a new one, as the pull past it does. A private or little window pages no
    /// Spaces with it.
    pub(super) fn navigate(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.private || self.little.is_some() {
            return;
        }
        if !back && !self.library_open && self.new_space_beside(self.space) {
            self.leave_popovers(cx);
            self.run(Command::NewSpace, "mouse", cx);
            return;
        }
        self.page_step(back, "mouse", window, cx);
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
                            true => {
                                let x = shell.from_side(f32::from(event.position.x));
                                x < shell.card_left() + shell.card_push()
                            }
                            false => bounds.contains(&event.position),
                        };
                        (over || shell.swipe.is_some())
                            && match WEB {
                                true => shell.web_scroll(event, wheel_time(), window, cx),
                                false => shell.scroll(event, window, cx),
                            }
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

    /// Moves the list on along its glide, then fades the edge under the favorites in once the list
    /// is past its top and out back there, and the one at its foot in while rows are hidden under
    /// it, the room after the last row not counting, and out once they're in view (Arc), and
    /// brings the knob as the user scrolls.
    pub(super) fn step_scroll_edge(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let glided = self.step_list_glide(window);
        let most = f32::from(self.list_scroll.max_offset().y).max(0.);
        let scrolled = (-f32::from(self.list_scroll.offset().y)).clamp(0., most);
        // As the list was last laid out: none before its first layout.
        let height = f32::from(self.list_scroll.bounds().size.height);
        let rows_end = self.layout().into_values().fold(0., f32::max) + ROW_PITCH;
        let rows_end = rows_end + self.tiles_room() - self.fixed_head();
        let below = height > 0. && rows_end > scrolled + height + 0.5;
        // The list's bounds are the last frame's: when what it's laid out from changes, as in the
        // frame a resize draws, one more frame decides again once this one has measured it.
        let measured = (window.viewport_size(), self.pages_top(), self.list_bottom());
        if self.list_measured != measured {
            self.list_measured = measured;
            window.request_animation_frame();
        }
        for (edge, shown, name) in [
            // Within half a point of its top, as Windows' wheel ticks of a fraction of a line
            // leave it once summed down and back, the list is at its top.
            (&mut self.top_edge, scrolled > 0.5, "edge"),
            (&mut self.foot_edge, below, "foot edge"),
        ] {
            match edge.step(shown) {
                Some(true) => window.request_animation_frame(),
                // check.sh waits for these lines.
                Some(false) => {
                    log!(
                        "browser: scroll {name} {}",
                        if shown { "shown" } else { "hidden" }
                    )
                }
                None => {}
            }
        }
        self.step_scroller(scrolled, glided, window, cx);
    }

    /// The edge across the sidebar 2 over the scrolled rows' top, under the name row that stays
    /// put (Arc 1.170), or under the URL field while the tiles scroll with the list and the name
    /// row with them, while the rows go under it.
    pub(super) fn scroll_edge_line(&self) -> Option<impl IntoElement + use<>> {
        let top = self.pages_top() + self.fixed_head() - 2.;
        Some(self.edge_line(self.top_edge.at())?.top(px(top)))
    }

    /// The edge across the sidebar 1 under the list's foot while rows are hidden under it (Arc).
    pub(super) fn foot_edge_line(&self) -> Option<impl IntoElement + use<>> {
        let line = self.edge_line(self.foot_edge.at())?;
        Some(line.bottom(px(self.list_bottom() - 2.)))
    }

    fn edge_line(&self, shown: f32) -> Option<gpui::Div> {
        (shown > 0.).then(|| {
            div()
                .absolute()
                .left_0()
                .w(px(self.width()))
                .h(px(1.))
                .bg(self.palette.faint(SCROLL_EDGE))
                .opacity(shown)
        })
    }
}

/// A line at an edge of the list, which fades in while it's shown and out once it isn't.
#[derive(Default)]
pub(super) struct EdgeLine {
    pub(super) shown: bool,
    /// Its fade since it last turned, from how far in it was then.
    fade: Option<(Instant, f32)>,
}

impl EdgeLine {
    /// How far it has faded in.
    fn at(&self) -> f32 {
        let to = if self.shown { 1. } else { 0. };
        self.fade.map_or(to, |(start, from)| {
            fade_at(from, to, start.elapsed().as_secs_f32(), SCROLL_EDGE_FADE)
        })
    }

    /// Turns it to `shown`: Some(true) while it's fading, Some(false) the frame it ends.
    fn step(&mut self, shown: bool) -> Option<bool> {
        if shown != self.shown {
            self.fade = Some((Instant::now(), self.at()));
            self.shown = shown;
        }
        let (start, _) = self.fade?;
        let fading = start.elapsed().as_secs_f32() < SCROLL_EDGE_FADE;
        if !fading {
            self.fade = None;
        }
        Some(fading)
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
    let index = if x > 0. {
        index.checked_sub(1)?
    } else {
        index + 1
    };
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

/// Where letting go would take the pages (`Shell::letting_go`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Landing {
    Library,
    Space(u64),
}

/// When the browser's wheel event being handled came, by its own clock: a page busy drawing hears
/// the trackpad's events late, and several summed into one.
#[cfg(target_family = "wasm")]
fn wheel_time() -> Instant {
    let performance = web_sys::window().and_then(|window| window.performance());
    let age = performance.map_or(0., |performance| {
        performance.now() - gpui_web::wheel_time_stamp()
    });
    // A browser that gives no stamp leaves the event as old as the page.
    let age = if (0. ..1000.).contains(&age) { age } else { 0. };
    let now = Instant::now();
    now.checked_sub(std::time::Duration::from_secs_f64(age / 1000.))
        .unwrap_or(now)
}

/// Off the web, the wheel's events are the app's, AppKit's phases and all (`scroll`).
#[cfg(not(target_family = "wasm"))]
fn wheel_time() -> Instant {
    Instant::now()
}

/// How fast the page went over the last SWIPE_HISTORY before `now`, from `samples` of where it
/// was, in points a second. None from fewer than two.
fn velocity(samples: &VecDeque<(Instant, f32)>, now: Instant) -> Option<f32> {
    let mut recent = samples.iter().filter(|&&(at, _)| now - at <= SWIPE_HISTORY);
    let &(start, from) = recent.next()?;
    let &(end, to) = recent.last()?;
    (end > start).then(|| (to - from) / (end - start).as_secs_f32())
}

/// Whether the Library, `out` of the way open, opens when let go going at `velocity`: on a flick
/// toward it, else past halfway.
fn library_opens(out: f32, velocity: f32) -> bool {
    if velocity.abs() >= SWIPE_FLICK {
        velocity > 0.
    } else {
        out > 0.5
    }
}

/// Where a page let go at `x`, going at `velocity` a second, heads: a page `width` that way on a
/// flick, else where it is.
fn heading(x: f32, velocity: f32, width: f32) -> f32 {
    if velocity.abs() >= SWIPE_FLICK {
        velocity.signum() * width
    } else {
        x
    }
}

#[cfg(test)]
mod model;

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui::{
        AppContext, Bounds, Context, Entity, Modifiers, Pixels, ScrollDelta, ScrollEvent,
        TestAppContext, TouchPhase, VisualTestContext, Window, point, px,
    };

    use super::{heading, nearest};
    use crate::shell::sidebar::mirrored;
    use crate::shell::{
        Folders, PLATE_INSET, SWIPE_TRACKING, Shell, SidebarRight, Space, SpaceForm,
        WindowButtonGeometry, recorder::Recorder,
    };
    use crate::text_input::TextInput;
    use crate::tokens::shell::library::COLUMN_WIDTH;

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
            peeked: Vec::new(),
        };
        ids.iter().map(space).collect()
    }

    /// Where fingers let go of a page at `x` going at `velocity` land it, with Space 4's form
    /// made from Space 1, pages 200 wide.
    fn lands(from: u64, x: f32, velocity: f32) -> Option<u64> {
        let spaces = spaces(&[1, 2, 3, 4]);
        nearest(
            &spaces,
            Some((4, 1)),
            from,
            heading(x, velocity, 200.),
            200.,
        )
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
    const OLDER_SDK: WindowButtonGeometry = WindowButtonGeometry {
        centres: [20., 40., 60.],
        y: 24.,
        diameter: 12.,
    };

    /// A window of three Spaces on the first, whose host draws its buttons as `OLDER_SDK`.
    fn older_sdk_window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        host.window_buttons.set(Some(OLDER_SDK));
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
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
        let moved = |frame: Bounds<Pixels>| Bounds {
            origin: frame.origin + point(left, px(0.)),
            ..frame
        };
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
            assert!(
                shell.window_buttons_shown(),
                "the window's own aren't up at rest"
            );
            assert_eq!(
                shell.window_button_frames(),
                own,
                "the copies aren't the system's"
            );
            let (mut was_own, mut swapped, mut receded) = (true, false, false);
            let way = 1.2 * shell.library_width();
            fingers_by_steps(shell, way, 600, window, cx, |shell| {
                let own_up = shell.window_buttons_shown();
                let (panel, sidebar) = (shell.library_buttons(), shell.sidebar_buttons());
                assert!(own_up != panel, "drawn twice or not at all");
                if panel {
                    let copy = panel_copy(shell);
                    assert!(
                        copy.iter().all(|frame| frame.size == own[0].size),
                        "{copy:?}"
                    );
                }
                if sidebar && was_own {
                    // Where the sidebar's copy takes over, it hasn't begun to recede.
                    assert!(
                        shell.sidebar_scale() > 0.999,
                        "at {}",
                        shell.sidebar_scale()
                    );
                    receded = true;
                }
                if own_up && !was_own {
                    // Where the panel stops, to within a pixel's half.
                    assert!(
                        same(panel_copy(shell), own, 0.5),
                        "swapped off the window's own"
                    );
                    swapped = true;
                }
                was_own = own_up;
            });
            assert!(receded && swapped, "the swipe didn't swap both ways");
        });
    }

    /// On the right, fingers going right still bring the Library out from the first Space, and
    /// what they move goes with them: the sidebar's Space pages go right, out of the window, and
    /// the Library follows them in from behind the card, its right edge on the sidebar's left all
    /// the way, shown only from the card's edge. Past where it opens nothing goes further, and the
    /// window's own buttons come up where the panel's copy has stopped on them, mirrored (#2375).
    #[gpui::test]
    fn on_the_right_the_library_follows_the_space_pages_in_from_the_page_side(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx) = older_sdk_window(cx);
        cx.update(|_, cx| {
            cx.set_global(SidebarRight(true));
            cx.refresh_windows();
        });
        cx.run_until_parked();
        shell.update_in(cx, |shell, window, cx| {
            assert!(shell.right(), "the sidebar isn't on the right");
            let across = shell.viewport.width;
            let own = OLDER_SDK
                .frames()
                .map(|frame| mirrored(true, across, frame));
            let (mut went, mut swapped) = (0., false);
            let way = 1.2 * shell.library_width();
            fingers_by_steps(shell, way, 600, window, cx, |shell| {
                let shift = shell.sidebar_shift();
                assert!(
                    shift >= went,
                    "the sidebar went left as the fingers went right"
                );
                went = shift;
                let library_right = shell.library_x() + shell.library_width();
                let sidebar_left = shell.column_x() + shift;
                assert!(
                    (library_right - sidebar_left).abs() < 0.01,
                    "the Library's edge at {library_right}, the sidebar's at {sidebar_left}"
                );
                let (card, _) = shell.card_frames(shell.viewport);
                let card_right = f32::from(card.right());
                assert!(
                    (shell.library_shown_from() - card_right).abs() < 0.01,
                    "shown from {}, the card's edge at {card_right}",
                    shell.library_shown_from()
                );
                if shell.window_buttons_shown() && !swapped {
                    let copy = panel_copy_right(shell);
                    assert!(
                        same(copy, own, 0.5),
                        "swapped off the window's own: {copy:?}"
                    );
                    swapped = true;
                }
            });
            assert!(swapped, "the window's own never came up");
            // Past where it opens, nothing has gone on.
            let (library, card) = (shell.library_x(), shell.card_frames(shell.viewport).0);
            let rest = f32::from(across) - shell.library_width();
            assert_eq!(library, rest);
            assert_eq!(f32::from(card.right()), rest);
            assert_eq!(shell.sidebar_shift(), shell.card_left());
        });
    }

    /// The copy the Library's panel draws of the window's buttons on the right, where it is in the
    /// window: mirrored across its column, at the panel's right.
    fn panel_copy_right(shell: &Shell) -> [Bounds<Pixels>; 3] {
        let column = shell.library_x() + shell.library_width() - COLUMN_WIDTH;
        shell.window_button_frames().map(|frame| {
            let frame = mirrored(true, px(COLUMN_WIDTH), frame);
            Bounds {
                origin: frame.origin + point(px(column), px(0.)),
                ..frame
            }
        })
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
            assert!(
                shell.window_buttons_shown(),
                "the window's own aren't up over the Library"
            );
            let mut swapped = false;
            let (way, steps) = (-shell.library_width(), 600);
            // The window's own go once the panel has left them by a pixel's half: the copy is
            // then on them but for the step that took it there.
            let step = shell.library_width() / steps as f32;
            fingers_by_steps(shell, way, steps, window, cx, |shell| {
                if shell.library_buttons() {
                    let copy = panel_copy(shell);
                    assert!(
                        copy.iter().all(|frame| frame.size == own[0].size),
                        "{copy:?}"
                    );
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
            assert!(
                !shell.library_open,
                "the swipe went on past the first Space into the Library"
            );
            shell.set_space(1, cx);
            swipe(shell, 0.9 * shell.library_width(), window, cx);
            assert!(shell.library_open, "the next swipe left the Library closed");
        });
        assert_eq!(*host.shown.borrow(), [1]);
    }

    /// On either side, one long swipe right from the second Space stops on the first and never
    /// carries on into the Library, however far the fingers go, and the Library stays shut while
    /// they're still down; the next swipe from the first brings it out (#2375).
    #[gpui::test]
    fn on_either_side_a_swipe_from_the_second_space_stops_on_the_first(cx: &mut TestAppContext) {
        for right in [false, true] {
            let (shell, host, cx) = three_spaces(cx, 2);
            cx.update(|_, cx| {
                cx.set_global(SidebarRight(right));
                cx.refresh_windows();
            });
            cx.run_until_parked();
            shell.update_in(cx, |shell, window, cx| {
                assert_eq!(shell.right(), right);
                let way = shell.width() + 2. * shell.library_width();
                fingers(shell, way, false, window, cx);
                assert_eq!(
                    shell.library_out(),
                    0.,
                    "the Library came out, right {right}"
                );
                let lift = ScrollEvent {
                    delta: ScrollDelta::Pixels(point(px(0.), px(0.))),
                    touch_phase: TouchPhase::Ended,
                    ..Default::default()
                };
                shell.scroll(&lift, window, cx);
                assert!(
                    !shell.library_open,
                    "the swipe opened the Library, right {right}"
                );
                // The pages land on the first Space.
                shell.set_space(1, cx);
                shell.settle = None;
                swipe(shell, 0.9 * shell.library_width(), window, cx);
                assert!(
                    shell.library_open,
                    "the next swipe left it shut, right {right}"
                );
            });
            assert_eq!(*host.shown.borrow(), [1], "right {right}");
        }
    }

    /// A flick, fingers lifted while they're still going, goes on to the next Space though they
    /// went less than halfway, and lifted the same way back it goes back. check.sh's swipes go
    /// past halfway, as a frame of the hosted runners' that takes longer than SWIPE_HISTORY can
    /// fall between the last move and the lift there, which reads as fingers held still (#1869).
    #[gpui::test]
    fn a_flick_short_of_halfway_goes_on_to_the_next_space(cx: &mut TestAppContext) {
        let (shell, host, cx) = three_spaces(cx, 1);
        shell.update_in(cx, |shell, window, cx| {
            let fingers = |touch_phase, x| ScrollEvent {
                delta: ScrollDelta::Pixels(point(px(x), px(0.))),
                touch_phase,
                ..Default::default()
            };
            let flick =
                |shell: &mut Shell, step: f32, window: &mut Window, cx: &mut Context<Shell>| {
                    // A quarter of the way, in ten moves.
                    let step = step * 0.25 * shell.width() / SWIPE_TRACKING / 10.;
                    shell.scroll(&fingers(TouchPhase::Started, step), window, cx);
                    for _ in 0..9 {
                        shell.scroll(&fingers(TouchPhase::Moved, step), window, cx);
                    }
                    shell.scroll(&fingers(TouchPhase::Ended, 0.), window, cx);
                };
            flick(shell, -1., window, cx);
            shell.set_space(2, cx);
            flick(shell, 1., window, cx);
        });
        assert_eq!(*host.shown.borrow(), [2, 1]);
    }

    /// Two fingers as Direct Manipulation reports a precision touchpad's on Windows: the touch
    /// comes with the first move, the lift ends the scroll, and the inertia after it goes on
    /// across. The swipe stops at the next Space, the inertia carrying it no further and scrolling
    /// nothing, and the next swipe goes on from there.
    #[gpui::test]
    fn a_touchpads_inertia_stops_at_the_next_space(cx: &mut TestAppContext) {
        let (shell, host, cx) = three_spaces(cx, 1);
        shell.update_in(cx, |shell, window, cx| {
            let fingers = |touch_phase, x| ScrollEvent {
                delta: ScrollDelta::Pixels(point(px(x), px(0.))),
                touch_phase,
                ..Default::default()
            };
            let step = -0.15 * shell.width() / SWIPE_TRACKING;
            for space in [1, 2] {
                shell.set_space(space, cx);
                shell.scroll(&fingers(TouchPhase::Started, step), window, cx);
                for _ in 0..3 {
                    shell.scroll(&fingers(TouchPhase::Moved, step), window, cx);
                }
                shell.scroll(&fingers(TouchPhase::Ended, 0.), window, cx);
                for _ in 0..30 {
                    let took = shell.scroll(&fingers(TouchPhase::Moved, step), window, cx);
                    assert!(took, "the inertia from Space {space} scrolled the list");
                }
            }
        });
        assert_eq!(*host.shown.borrow(), [2, 3]);
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
            assert!(
                crossing(shell, Instant::now()),
                "fingers: the sidebar {}",
                shell.sidebar_opacity()
            );
            fingers(shell, 0.9 * shell.library_width(), true, window, cx);
            assert!(shell.library_open, "the swipe left the Library closed");
            shell.library_motion = None;
            shell.sidebar_fade = None;
            shell.set_library(false, cx);
            // Halfway through the sidebar's fade back in, on a clock of the test's own.
            let start = Instant::now();
            shell.sidebar_fade = shell.sidebar_fade.map(|(from, _)| (from, start));
            let halfway = start + Duration::from_millis(145);
            assert!(
                crossing(shell, halfway),
                "a click: the sidebar {}",
                shell.sidebar_opacity_at(halfway)
            );
        });
    }

    /// Under Reduce Motion the Library doesn't push the page aside or the sidebar back: it's open
    /// from its first frame and closed from its first, while the sidebar under it still fades
    /// (#1380).
    #[gpui::test]
    fn under_reduce_motion_the_library_comes_and_goes_at_once_and_the_sidebar_fades(
        cx: &mut TestAppContext,
    ) {
        let (shell, _, cx) = three_spaces(cx, 1);
        shell.update(cx, |shell, cx| {
            shell.set_library(true, cx);
            assert!(
                shell.library_out() < 0.5,
                "the Library pushed out at once with motion on"
            );
            shell.close_library_now();
            cx.set_reduce_motion(true);
            shell.set_library(true, cx);
            assert_eq!((shell.library_out(), shell.sidebar_scale()), (1., 1.));
            // The sidebar has faded out under it.
            shell.sidebar_fade = shell
                .sidebar_fade
                .map(|(from, start)| (from, start - Duration::from_secs(1)));
            cx.notify();
        });
        cx.run_until_parked();
        shell.update(cx, |shell, cx| {
            shell.set_library(false, cx);
            assert_eq!((shell.library_out(), shell.sidebar_scale()), (0., 1.));
            assert!(
                shell.sidebar_opacity() < 1.,
                "the sidebar came back without its fade"
            );
            cx.notify();
        });
        // A frame, which ends the card's motion but not the sidebar's fade.
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| {
            assert!(
                shell.sidebar_fade.is_some(),
                "the sidebar's fade ended with the card's motion"
            );
        });
    }

    /// Under Reduce Motion the pages still follow the fingers, and once they lift they're where
    /// they land, without a spring (#1380).
    #[gpui::test]
    fn under_reduce_motion_a_swipe_follows_the_fingers_and_lands_without_a_spring(
        cx: &mut TestAppContext,
    ) {
        let (shell, host, cx) = three_spaces(cx, 2);
        shell.update_in(cx, |shell, window, cx| {
            cx.set_reduce_motion(true);
            fingers(shell, 0.3 * shell.width(), false, window, cx);
            let pages = shell.pages();
            assert!(
                pages[0].1 > 0. && pages.len() == 2,
                "the pages didn't follow the fingers: {pages:?}"
            );
            fingers(shell, 0.6 * shell.width(), true, window, cx);
            assert!(shell.settle.is_some(), "the pages didn't settle");
            assert_eq!(shell.pages(), [(1, 0.)]);
        });
        assert_eq!(*host.shown.borrow(), [1]);
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
            shell.set_spaces((1..=4).map(space).collect(), cx);
            shell.set_space(4, cx);
            let name = cx.new(|cx| TextInput::new("Space name", cx));
            shell.form = Some(SpaceForm {
                space: 4,
                from: 3,
                name,
                tab: None,
            });
            shell.set_favorites(vec![(10, 1)], cx);
        });
        cx.run_until_parked();
        let rest = shell.read_with(cx, |shell, _| shell.drawn_tiles()[&10]);
        let tile = |shell: &Shell| shell.tip_frames.borrow().get(&10).copied();
        assert!(
            shell.read_with(cx, |shell, _| tile(shell).is_none()),
            "the form left a tile up"
        );

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
        assert_eq!(
            host.command_bars.take().len(),
            1,
            "the URL field wasn't drawn"
        );

        // Fingers up past halfway: Cancel, and the page settles in with the form's going out.
        shell.update_in(cx, |shell, window, cx| {
            let way = 0.7 * shell.width();
            fingers(shell, way, true, window, cx);
            shell.tip_frames.borrow_mut().clear();
            assert!(
                shell.form_going.is_some(),
                "the swipe didn't cancel the form"
            );
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
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    id,
                    "".into(),
                    "".into(),
                    "".into(),
                )
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
                let drawn = before
                    .iter()
                    .zip(shell.pages())
                    .map(|(&(space, x), (_, after))| {
                        let xs = (x.min(after), x.max(after));
                        if space == shell.space {
                            (
                                xs,
                                left(shell.tip_frames.borrow().get(&favorite(space))),
                                None,
                            )
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
                assert!(
                    near(tile - rest, xs),
                    "a tile at {tile} isn't with its page at {xs:?}"
                );
                if let Some(field) = field {
                    assert!(
                        near(field - PLATE_INSET, xs),
                        "the field beside isn't at {xs:?}"
                    );
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
        assert_eq!(
            host.command_bars.take().len(),
            1,
            "the URL field isn't with its page"
        );
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
        assert!(
            frames > 5,
            "the slide drew {frames} frames with a page beside"
        );

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
        assert!(
            drawn[0].0.1 < 0.,
            "the last Space's page didn't rubber-band"
        );
        let tile = drawn[0].1.expect("no tile past the last Space");
        assert!(
            (tile - rest).abs() < 0.5,
            "the favorites left their place past the last Space"
        );
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
            let space = |id| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
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
                let phase = if step == 0 {
                    TouchPhase::Started
                } else {
                    TouchPhase::Moved
                };
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

    /// Past the last Space, VoiceOver hears the New Space circle come out, and when letting go
    /// makes the Space, as the trackpad's click says it.
    #[gpui::test]
    fn voiceover_hears_when_letting_go_makes_a_space(cx: &mut TestAppContext) {
        use gpui::accesskit::{Live, Role};
        let (shell, _, cx) = three_spaces(cx, 3);
        cx.simulate_a11y_activation();
        let circle = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            let tree = cx.a11y_tree().expect("GPUI built a tree");
            let found = tree
                .nodes
                .into_iter()
                .find(|(_, node)| node.role() == Role::Status && node.label() == Some("New Space"));
            found.map(|(_, node)| (node.value().map(str::to_owned), node.live()))
        };
        assert_eq!(circle(cx), None, "a circle before the fingers");
        shell.update_in(cx, |shell, window, cx| {
            fingers(shell, -8., false, window, cx)
        });
        let pulling = Some((
            Some("Pull further to make one".to_owned()),
            Some(Live::Polite),
        ));
        assert_eq!(circle(cx), pulling);
        // On, without letting go, against the page's rubber band.
        shell.update_in(cx, |shell, window, cx| {
            let on = -2. * shell.width() / SWIPE_TRACKING;
            let moved = ScrollEvent {
                delta: ScrollDelta::Pixels(point(px(on), px(0.))),
                touch_phase: TouchPhase::Moved,
                ..Default::default()
            };
            shell.scroll(&moved, window, cx);
        });
        let ready = Some((Some("Let go to make it".to_owned()), Some(Live::Polite)));
        assert_eq!(circle(cx), ready);
    }

    /// Fingers across the sidebar, `dx` this event, 2 ms after the last on the test's `clock`, as
    /// a trackpad sends them: the clock is the test's, not the runner's, as the fingers' speed
    /// decides the clicks (#2446). Returns how many detents the trackpad has been asked for.
    fn finger(
        shell: &mut Shell,
        host: &Recorder,
        clock: &mut Instant,
        touch_phase: TouchPhase,
        dx: f32,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> usize {
        *clock += Duration::from_millis(2);
        let event = ScrollEvent {
            delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
            touch_phase,
            ..Default::default()
        };
        shell.scroll_at(&event, *clock, window, cx);
        host.detents.get()
    }

    /// A haptic is felt only with a finger on the trackpad, so a swipe's detent comes while the
    /// fingers are down, as soon as letting go would take the next Space, even short of halfway,
    /// where most swipes are let go. Nothing clicks at the lift or in the momentum after it
    /// (#1075).
    #[gpui::test]
    fn a_swipe_clicks_as_it_takes_the_next_space_with_the_fingers_down(cx: &mut TestAppContext) {
        use TouchPhase::{Ended, Moved, Started};
        let (shell, host, cx) = three_spaces(cx, 2);
        shell.update_in(cx, |shell, window, cx| {
            let mut clock = Instant::now();
            let step = -0.05 * shell.width() / SWIPE_TRACKING;
            let first = finger(shell, &host, &mut clock, Started, step, window, cx);
            assert_eq!(first, 0, "a click before the fingers had gone two moves");
            assert_eq!(
                finger(shell, &host, &mut clock, Moved, step, window, cx),
                1,
                "no click going"
            );
            for _ in 0..2 {
                assert_eq!(
                    finger(shell, &host, &mut clock, Moved, step, window, cx),
                    1,
                    "a click going on"
                );
            }
            assert_eq!(
                finger(shell, &host, &mut clock, Ended, 0., window, cx),
                1,
                "a click at the lift"
            );
            for _ in 0..10 {
                let momentum = finger(shell, &host, &mut clock, Moved, step, window, cx);
                assert_eq!(momentum, 1, "a click in the momentum");
            }
        });
        assert_eq!(*host.shown.borrow(), [3]);
    }

    /// From the first Space, the fingers click as letting go would bring out the Library, and
    /// again as they turn back and letting go would leave it shut.
    #[gpui::test]
    fn a_swipe_clicks_as_it_takes_the_library_and_as_it_turns_back(cx: &mut TestAppContext) {
        use TouchPhase::{Ended, Moved, Started};
        let (shell, host, cx) = three_spaces(cx, 1);
        shell.update_in(cx, |shell, window, cx| {
            let mut clock = Instant::now();
            let step = 0.05 * shell.library_width() / SWIPE_TRACKING;
            assert_eq!(
                finger(shell, &host, &mut clock, Started, step, window, cx),
                0
            );
            assert_eq!(
                finger(shell, &host, &mut clock, Moved, step, window, cx),
                1,
                "no click to the Library"
            );
            assert_eq!(finger(shell, &host, &mut clock, Moved, step, window, cx), 1);
            // The moves out are past the ones the fingers' speed is read from.
            let before = clock - Duration::from_millis(500);
            for (at, _) in &mut shell.swipe.as_mut().expect("a swipe").samples {
                *at = (*at).min(before);
            }
            assert_eq!(
                finger(shell, &host, &mut clock, Moved, -step / 2., window, cx),
                1
            );
            let back = finger(shell, &host, &mut clock, Moved, -step / 2., window, cx);
            assert_eq!(back, 2, "no click turning back");
            assert_eq!(
                finger(shell, &host, &mut clock, Ended, 0., window, cx),
                2,
                "a click at the lift"
            );
            assert!(!shell.library_open, "the Library came out turned back from");
        });
    }

    /// Past the last Space, a swipe has no Space to take, and the trackpad clicks once, as the pull
    /// readies a new Space, with the fingers down.
    #[gpui::test]
    fn a_swipe_past_the_last_space_clicks_as_the_pull_readies(cx: &mut TestAppContext) {
        use TouchPhase::{Moved, Started};
        let (shell, host, cx) = three_spaces(cx, 3);
        shell.update_in(cx, |shell, window, cx| {
            let mut clock = Instant::now();
            assert_eq!(
                finger(shell, &host, &mut clock, Started, -20., window, cx),
                0
            );
            for _ in 0..30 {
                let detents = finger(shell, &host, &mut clock, Moved, -20., window, cx);
                let ready = shell.swipe.as_ref().expect("a swipe").ready;
                assert_eq!(
                    detents, ready as usize,
                    "a click short of the pull's or past it"
                );
            }
            let readied = shell.swipe.as_ref().is_some_and(|swipe| swipe.ready);
            assert!(readied, "the pull never readied");
        });
    }
}
