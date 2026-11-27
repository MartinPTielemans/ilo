//! Bring your stuff (first-run-v9 §5), on the import view's own state (crate::import, bring.rs):
//! the browsers found in a row, none picked until the user picks one (#4449), which lifts in its
//! cell on the selected plate; a copy of its icon at the flow's left end, a chip for each kind it
//! has with its count, strands from the copy through each chip to the knot, and as an import
//! writes each kind its strand draws. Then the favourites, as the step's second state.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, FontWeight, MouseButton, PathBuilder, Rgba, SharedString, Window, canvas,
    div, point, prelude::*, px, svg,
};

use super::card::{self, Inks};
use super::{Shell, Step, cubic_bezier};
use crate::import::read::Kind;
use crate::import::{Bring, Bringing, Chip, ImportView};
use crate::product::PRODUCT;
use crate::settings_rows::{named_button, named_check};
use crate::tokens::shell::onboarding::card as t;

/// The step's motion, the card's own: the view keeps what is picked and written.
#[derive(Default)]
pub(super) struct BringMotion {
    /// The browser picked last and when, its icon lifting in its cell from then.
    pub(super) lift: Option<(usize, Instant)>,
    /// Each kind written as the card shows it, its strand drawing from then.
    written: Vec<(Kind, Instant)>,
    /// When the card said All here., the import over and every strand drawn.
    pub(super) all_here: Option<Instant>,
    /// More… shows the browsers past the row's.
    more: bool,
    /// The card goes on once All here. has shown, a timer set once an import.
    going: bool,
    /// The last frame drew something moving, so the card asks for another.
    pub(super) moving: bool,
}

/// DRAW, the curve a cord drawing takes.
fn drawing(x: f32) -> f32 {
    let [x1, y1, x2, y2] = t::DRAW;
    cubic_bezier(x1, y1, x2, y2, x.clamp(0., 1.))
}

/// A kind's glyph on its chip.
fn glyph(kind: Kind) -> &'static str {
    match kind {
        Kind::Spaces => "icons/spaces.svg",
        Kind::OpenTabs => "icons/app-window.svg",
        Kind::Favorites => "icons/star.svg",
        Kind::Bookmarks => "icons/bookmark.svg",
        Kind::ReadingList => "icons/glasses.svg",
        Kind::History => "icons/clock.svg",
        Kind::Addresses => "icons/map-pin.svg",
        Kind::Extensions => "icons/puzzle.svg",
    }
}

/// What a chip's count says: extensions are listed, never installed.
fn counted(chip: &Chip) -> String {
    match chip.kind {
        Kind::Extensions => format!("{} listed", chip.count),
        _ => chip.count.to_string(),
    }
}

/// The centre of the `at`th of `count` chips, on the step's centre line.
fn chip_y(at: usize, count: usize) -> f32 {
    let [_, centre, _, _, _, pitch] = t::BRING_CHIP;
    centre + (at as f32 - (count.saturating_sub(1)) as f32 / 2.) * pitch
}

/// The point `along` a cubic from `from` to `to` that leaves and arrives level.
fn strand_at(from: (f32, f32), to: (f32, f32), along: f32) -> (f32, f32) {
    let mid = (from.0 + to.0) / 2.;
    let (p0, p1, p2, p3) = (from, (mid, from.1), (mid, to.1), to);
    let u = 1. - along;
    let x = u * u * u * p0.0
        + 3. * u * u * along * p1.0
        + 3. * u * along * along * p2.0
        + along * along * along * p3.0;
    let y = u * u * u * p0.1
        + 3. * u * u * along * p1.1
        + 3. * u * along * along * p2.1
        + along * along * along * p3.1;
    (x, y)
}

/// A strand from `from` to `to`, `drawn` of it, in `ink`, at the card's `origin`.
fn paint_strand(
    origin: gpui::Point<gpui::Pixels>,
    from: (f32, f32),
    to: (f32, f32),
    drawn: f32,
    ink: Rgba,
    window: &mut Window,
) {
    const SEGMENTS: usize = 32;
    let [width, _, _] = t::BRING_STRAND;
    let last = (drawn.clamp(0., 1.) * SEGMENTS as f32).ceil() as usize;
    if last == 0 {
        return;
    }
    let at = |(x, y): (f32, f32)| point(origin.x + px(x), origin.y + px(y));
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(at(from));
    for each in 1..=last {
        let along = (each as f32 / SEGMENTS as f32).min(drawn);
        path.line_to(at(strand_at(from, to, along)));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, ink);
    }
}

impl Shell {
    /// The import view the card holds.
    pub(super) fn bring_view(&self) -> Option<gpui::Entity<ImportView>> {
        self.importing
            .as_ref()
            .map(|importing| importing.view.clone())
    }

    /// What the import view has for the card, its icons at the slot's size on `window`.
    pub(super) fn bring_state(&self, window: &Window, cx: &mut Context<Self>) -> Option<Bring> {
        let [_, _, slot] = t::BRING_SLOT;
        let pixels = (slot * window.scale_factor()).round() as u32;
        let view = self.bring_view()?;
        Some(view.update(cx, |view, _| view.card_state(pixels)))
    }

    /// The step as this frame draws it: what the view has, paced as the card shows it.
    pub(super) fn bring_now(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<Bring> {
        let bring = self.bring_state(window, cx)?;
        self.bring_follow(&bring, cx);
        let moving = !crate::motion::still(cx) && self.bring_moving(Some(&bring));
        if let Some(onboarding) = self.onboarding.as_mut() {
            onboarding.bring.moving = moving;
        }
        Some(bring)
    }

    /// The card's motion follows what the view says: a new pick lifts, each kind written takes its
    /// time at least the step's gap after the one before, and once all are drawn, All here.
    fn bring_pace(&mut self, bring: &Bring) {
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        let now = onboarding.clock.now();
        let motion = &mut onboarding.bring;
        if let Some(picked) = bring.picked
            && motion.lift.map(|(at, _)| at) != Some(picked)
        {
            motion.lift = Some((picked, now));
        }
        let Bringing::Running { chips, done, .. } = &bring.state else {
            motion.written.clear();
            motion.all_here = None;
            motion.going = false;
            return;
        };
        let [draw, later, gap] = t::BRING_DRAW;
        for chip in chips.iter().filter(|chip| chip.written) {
            if motion.written.iter().any(|(kind, _)| *kind == chip.kind) {
                continue;
            }
            let after = (motion.written.last())
                .map(|(_, at)| *at + Duration::from_secs_f32(gap))
                .filter(|after| *after > now);
            motion.written.push((chip.kind, after.unwrap_or(now)));
        }
        if *done && motion.all_here.is_none() {
            let last = motion.written.last().map_or(now, |(_, at)| *at);
            let drawn = last + Duration::from_secs_f32(later + draw);
            motion.all_here = Some(drawn.max(now));
        }
    }

    /// The import view said something: the card paces it as a frame would.
    pub(super) fn bring_heard(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.bring_view() else {
            return;
        };
        let bring = view.update(cx, |view, _| view.card_state(0));
        self.bring_follow(&bring, cx);
    }

    /// Paces what the view says, and once All here. is set, goes on after it has shown, to the
    /// favourites where the import offers some.
    fn bring_follow(&mut self, bring: &Bring, cx: &mut Context<Self>) {
        self.bring_pace(bring);
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        let Some(all_here) = onboarding
            .bring
            .all_here
            .filter(|_| !onboarding.bring.going)
        else {
            return;
        };
        onboarding.bring.going = true;
        let now = onboarding.clock.now();
        let wait = all_here.saturating_duration_since(now) + Duration::from_secs_f32(t::BRING_ON);
        cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(wait).await;
            shell
                .update(cx, |shell, cx| {
                    if shell.onboarding_on(Step::Import) {
                        shell.onboarding_next(cx);
                    }
                })
                .ok();
        })
        .detach();
    }

    /// The step's heading and its line: the kind being written while an import runs, then All
    /// here.
    pub(super) fn bring_words(&self, bring: Option<&Bring>) -> Option<SharedString> {
        let onboarding = self.onboarding.as_ref()?;
        let Bringing::Running { chips, .. } = &bring?.state else {
            return None;
        };
        let now = onboarding.clock.now();
        let motion = &onboarding.bring;
        if motion.all_here.is_some_and(|at| at <= now) {
            return Some("All here.".into());
        }
        let shown = (motion.written.iter().rev()).find(|(_, at)| *at <= now);
        let kind = match shown {
            Some((kind, _)) => Some(*kind),
            None => chips.iter().find(|chip| chip.on).map(|chip| chip.kind),
        }?;
        Some(format!("{}…", kind.label()).into())
    }

    /// Imports the chips on, from outside this update, as the import reads the Shell (#2428).
    pub(super) fn bring_import(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.bring_view() {
            cx.defer(move |cx| view.update(cx, |view, cx| view.import(cx)));
        }
    }

    /// Try Again: asks a stalled import's profiles again, as the view's own button does, or reads
    /// again the browser that couldn't be read.
    pub(super) fn bring_try_again(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.bring_view() else {
            return;
        };
        if view.read(cx).bring_stalled() {
            log!("browser: onboarding import tries again");
            return view.update(cx, |view, cx| view.press(cx));
        }
        let picked = view.update(cx, |view, _| view.card_state(0)).picked;
        if let Some(picked) = picked {
            self.bring_pick(picked, cx);
        }
    }

    /// Whether the step is drawing: a pick's lift, a strand or the knot's gap.
    pub(super) fn bring_moving(&self, bring: Option<&Bring>) -> bool {
        let Some(onboarding) = self.onboarding.as_ref() else {
            return false;
        };
        let now = onboarding.clock.now();
        let motion = &onboarding.bring;
        let [rise, _] = t::BRING_LIFT;
        let lifting = (motion.lift)
            .is_some_and(|(_, at)| now.saturating_duration_since(at).as_secs_f32() < rise);
        // A stalled import's knot stands still until it's asked again.
        let running = matches!(
            bring.map(|b| &b.state),
            Some(Bringing::Running { stalled: None, .. })
        );
        let [_, close] = t::BRING_LAP;
        let closing = (motion.all_here)
            .is_none_or(|at| now.saturating_duration_since(at).as_secs_f32() < close + t::BRING_ON);
        lifting || (running && closing)
    }

    /// The step's controls over the whole card: the row of browsers, the picked one's copy, the
    /// chips and their strands, and the knot; the line asking for a pick before one; or what keeps
    /// the browser from being read.
    pub(super) fn bring_controls(
        &mut self,
        bring: Option<&Bring>,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(bring) = bring else {
            return div().into_any_element();
        };
        let still = crate::motion::still(cx);
        let now = self.onboarding.as_ref().map(|o| o.clock.now());
        let Some(now) = now else {
            return div().into_any_element();
        };
        let running = matches!(bring.state, Bringing::Running { .. });
        div()
            .absolute()
            .inset_0()
            .child(self.bring_browsers(bring, running, still, now, inks, cx))
            .children(self.bring_copy(bring, inks))
            .children(self.bring_kinds(bring, still, now, inks, cx))
            .into_any_element()
    }

    /// The browsers found in a row, the rest past five behind More…. The one picked keeps its icon
    /// in its cell, lifted on the one look's selected plate (#4401), so the row never has a gap.
    fn bring_browsers(
        &self,
        bring: &Bring,
        running: bool,
        still: bool,
        now: Instant,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let [top, size, cell] = t::BRING_ROW;
        let [name_size, name_line, name_gap] = t::BRING_NAME;
        let shown = t::BRING_SHOWN as usize;
        let more = self.onboarding.as_ref().is_some_and(|o| o.bring.more);
        let past = bring.browsers.len() > shown;
        let cells = bring.browsers.len().min(shown) + usize::from(past);
        let left = (t::WIDTH - cells as f32 * cell) / 2.;
        let a11y = self.a11y;
        let [reach, corners] = t::BRING_PICKED;
        let [rise, lift] = t::BRING_LIFT;
        let lifted = (self.onboarding.as_ref())
            .and_then(|o| o.bring.lift)
            .filter(|(at, _)| bring.picked == Some(*at))
            .map_or(0., |(_, since)| match still {
                true => lift,
                false => {
                    lift * super::arrive(now.saturating_duration_since(since).as_secs_f32() / rise)
                }
            });
        // check.sh clicks a browser where this says it is.
        let logged = (self.bring_view()).map(|view| view.read(cx).card_drawn());
        let plate = |name: &str, icon: Option<std::sync::Arc<gpui::RenderImage>>| match icon {
            Some(icon) => gpui::img(icon)
                .size(px(size))
                .flex_none()
                .into_any_element(),
            None => letter_plate(name, size, inks).into_any_element(),
        };
        let row = (bring.browsers.iter().enumerate().take(shown)).map(|(at, (name, icon))| {
            let picked = bring.picked == Some(at);
            let spoken = name.clone();
            div()
                .id(("onboarding-browser", at))
                .when(a11y, |cell| named_check(cell, spoken, picked))
                .relative()
                .w(px(cell))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(name_gap))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .when(!running, |cell| {
                    cell.on_click(cx.listener(move |shell, _, _, cx| shell.bring_pick(at, cx)))
                })
                .when(picked, |cell| {
                    cell.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(-reach))
                            .bottom(px(-reach))
                            .rounded(px(corners))
                            .bg(inks.selected()),
                    )
                })
                .child(
                    div()
                        .relative()
                        .top(px(if picked { -lifted } else { 0. }))
                        .size(px(size))
                        .rounded(px(size * t::BRING_PLATE[0]))
                        .child(plate(name, icon.clone()))
                        .children(self.place(Step::Import, icon_place(at))),
                )
                .child(
                    div()
                        .max_w(px(cell))
                        .truncate()
                        .text_size(px(name_size))
                        .line_height(px(name_line))
                        .text_color(inks.text_2())
                        .child(name.clone()),
                )
                .children(self.place(Step::Import, browser_place(at)))
                .children(logged.clone().map(|logged| {
                    crate::settings_rows::drawn(format!("import-browser {name}"), logged)
                        .absolute()
                        .inset_0()
                }))
                .into_any_element()
        });
        let mut cells: Vec<AnyElement> = row.collect();
        if past {
            let rest: Vec<(usize, String)> = (bring.browsers.iter().enumerate().skip(shown))
                .map(|(at, (name, _))| (at, name.clone()))
                .collect();
            let menu = more.then(|| self.bring_more(rest, inks, cx));
            cells.push(
                div()
                    .id("onboarding-more")
                    .relative()
                    .w(px(cell))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(name_gap))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .when(a11y, |more| named_button(more, "More…"))
                    .when(!running, |more| {
                        more.on_click(cx.listener(|shell, _, _, cx| {
                            if let Some(onboarding) = shell.onboarding.as_mut() {
                                onboarding.bring.more = !onboarding.bring.more;
                            }
                            cx.notify();
                        }))
                    })
                    .child(
                        div()
                            .size(px(size))
                            .rounded(px(size * t::BRING_PLATE[0]))
                            .bg(inks.button())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                svg()
                                    .path("icons/ellipsis.svg")
                                    .size(px(size * t::BRING_PLATE[2]))
                                    .text_color(inks.text(1.)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(name_size))
                            .line_height(px(name_line))
                            .text_color(inks.text_2())
                            .child("More…"),
                    )
                    .children(menu)
                    .into_any_element(),
            );
        }
        card::at(left, top)
            .flex()
            .opacity(if running { t::BRING_OFF } else { 1. })
            .children(cells)
    }

    /// More…'s menu: the browsers past the row's, each a pick.
    fn bring_more(
        &self,
        rest: Vec<(usize, String)>,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let [_, size, cell] = t::BRING_ROW;
        let [name_size, name_line, _] = t::BRING_NAME;
        let rows = rest.into_iter().map(|(at, name)| {
            div()
                .id(("onboarding-more-browser", at))
                .px(px(t::BARE_INSET * 4.))
                .py(px(t::BARE_INSET * 2.))
                .rounded(px(t::BARE_INSET * 3.))
                .text_size(px(name_size + 1.))
                .line_height(px(name_line + 2.))
                .text_color(inks.text(1.))
                .hover(move |row| row.bg(inks.button()))
                .when(self.a11y, |row| named_button(row, name.clone()))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    if let Some(onboarding) = shell.onboarding.as_mut() {
                        onboarding.bring.more = false;
                    }
                    shell.bring_pick(at, cx)
                }))
                .child(name)
        });
        div()
            .absolute()
            .top(px(size + t::BARE_INSET * 2.))
            .right(px(cell / 2. - size / 2.))
            .min_w(px(cell * 2.))
            .p(px(t::BARE_INSET * 2.))
            .rounded(px(t::BARE_INSET * 5.))
            .bg(inks.fill(false))
            .border_1()
            .border_color(inks.rim())
            .children(rows)
            .into_any_element()
    }

    /// Picks the browser `at` in the card, which reads it.
    pub(super) fn bring_pick(&mut self, at: usize, cx: &mut Context<Self>) {
        if let Some(view) = self.bring_view() {
            view.update(cx, |view, cx| view.bring_from(at, cx));
        }
        cx.notify();
    }

    /// A copy of the picked browser's icon at the flow's left end, where its strands start: the
    /// row keeps its own (#4449), and another pick swaps it with the counts.
    fn bring_copy(&self, bring: &Bring, inks: Inks) -> Option<AnyElement> {
        let (name, image) = bring.browsers.get(bring.picked?)?;
        let [slot_x, slot_y, slot] = t::BRING_SLOT;
        let plate = match image {
            Some(image) => gpui::img(image.clone()).size(px(slot)).into_any_element(),
            None => letter_plate(name, slot, inks).into_any_element(),
        };
        let step = self.onboarding.as_ref().map_or(Step::Import, |o| o.step);
        Some(
            card::at(slot_x, slot_y)
                .size(px(slot))
                .rounded(px(slot * t::BRING_PLATE[0]))
                .child(plate)
                .children(self.place(step, "copy"))
                .into_any_element(),
        )
    }

    /// Before a pick, in the flow's place: the one line that asks for one.
    fn bring_unpicked(&self, inks: Inks) -> AnyElement {
        let [_, centre, ..] = t::BRING_CHIP;
        let [_, size, height] = t::LINE;
        card::at(0., centre - height / 2.)
            .w(px(t::WIDTH))
            .flex()
            .justify_center()
            .child(
                div()
                    .relative()
                    .text_size(px(size))
                    .line_height(px(height))
                    .text_color(inks.text(t::TEXT_3))
                    .child(PICK_A_BROWSER)
                    .children(self.place(Step::Import, "pick-a-browser")),
            )
            .into_any_element()
    }

    /// The chips in their column, the strands through them and the knot, or what keeps the browser
    /// from being read.
    fn bring_kinds(
        &self,
        bring: &Bring,
        still: bool,
        now: Instant,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        if bring.picked.is_none() {
            return vec![self.bring_unpicked(inks)];
        }
        if let Some(said) = bring_held(bring) {
            return vec![self.bring_again(said, inks, cx)];
        }
        let chips = match &bring.state {
            Bringing::Kinds(chips) => chips.clone(),
            Bringing::Running { chips, .. } => chips.clone(),
            Bringing::Reading | Bringing::Unread => Vec::new(),
            Bringing::NoAccess => return vec![self.bring_notice(&picked_name(bring), inks, cx)],
        };
        let running = matches!(bring.state, Bringing::Running { .. });
        let motion = self.onboarding.as_ref().map(|o| &o.bring);
        // How long since `kind`'s strand began drawing, once it has.
        let written_at = |kind: Kind| {
            (motion?.written.iter())
                .find(|(each, at)| *each == kind && *at <= now)
                .map(|(_, at)| now.saturating_duration_since(*at).as_secs_f32())
        };
        let [cx_mid, centre, width, height, radius, _] = t::BRING_CHIP;
        let [glyph_size, text_size] = t::BRING_CHIP_TEXT;
        let [knot_x, knot_y, knot] = t::BRING_KNOT;
        let [_, slot_y, slot] = t::BRING_SLOT;
        let [slot_x, _, _] = t::BRING_SLOT;
        let [_, ghost, off] = t::BRING_STRAND;
        let [draw, later, _] = t::BRING_DRAW;
        let count = chips.len();
        let left = cx_mid - width / 2.;
        let icon_end = (slot_x + slot, slot_y + slot / 2.);
        let knot_start = (knot_x - knot / 2., knot_y);
        let _ = centre;
        // Each strand: icon → chip, then chip → knot, each with its ghost and how far it's drawn.
        let strands: Vec<((f32, f32), (f32, f32), f32, f32)> = (chips.iter().enumerate())
            .flat_map(|(at, chip)| {
                let y = chip_y(at, count);
                let base = if chip.on { ghost } else { off };
                let since = written_at(chip.kind);
                let first =
                    since.map_or(0., |since| if still { 1. } else { drawing(since / draw) });
                let second = since.map_or(0., |since| {
                    if still {
                        1.
                    } else {
                        drawing((since - later) / draw)
                    }
                });
                let reaches = chip.kind != Kind::Extensions;
                let mut both = vec![(icon_end, (left, y), base, first)];
                both.push((
                    (left + width, y),
                    knot_start,
                    base,
                    if reaches { second } else { 0. },
                ));
                both
            })
            .collect();
        let ink = inks.text(1.);
        let strands = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                for (from, to, base, drawn) in &strands {
                    paint_strand(
                        bounds.origin,
                        *from,
                        *to,
                        1.,
                        Rgba { a: *base, ..ink },
                        window,
                    );
                    paint_strand(bounds.origin, *from, *to, *drawn, ink, window);
                }
            },
        )
        .absolute()
        .inset_0();
        let a11y = self.a11y;
        let logged = (self.bring_view()).map(|view| view.read(cx).card_drawn());
        let chip_cells = chips.iter().enumerate().map(|(at, chip)| {
            let y = chip_y(at, count);
            let shown = written_at(chip.kind).is_some();
            let kind = chip.kind;
            let label = kind.label();
            let on = chip.on;
            card::at(left, y - height / 2.)
                .child(
                    div()
                        .id(("onboarding-chip", at))
                        .when(a11y, |chip| named_check(chip, label, on))
                        .w(px(width))
                        .h(px(height))
                        .px(px(height * t::BRING_CHIP_INSET[0]))
                        .flex()
                        .items_center()
                        .gap(px(height * t::BRING_CHIP_INSET[1]))
                        .rounded(px(radius))
                        .bg(inks.group())
                        .opacity(if on { 1. } else { t::BRING_OFF })
                        .text_size(px(text_size))
                        .text_color(inks.text(1.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .when(!running, |chip| {
                            chip.on_click(cx.listener(move |shell, _, _, cx| {
                                if let Some(view) = shell.bring_view() {
                                    view.update(cx, |view, cx| view.toggle_kind(kind, cx));
                                }
                                cx.notify();
                            }))
                        })
                        .child(
                            svg()
                                .path(glyph(kind))
                                .size(px(glyph_size))
                                .flex_none()
                                .text_color(inks.text(1.)),
                        )
                        .child(div().flex_1().child(label))
                        .child(
                            div()
                                .text_color(if shown || !running {
                                    inks.text(if running { 1. } else { t::TEXT_2 })
                                } else {
                                    inks.text_2()
                                })
                                .font_weight(FontWeight::MEDIUM)
                                .child(counted(chip)),
                        )
                        .children(self.place(Step::Import, chip_place(at)))
                        // check.sh clicks a chip where this says it is.
                        .children(logged.clone().map(|logged| {
                            crate::settings_rows::drawn(format!("import-chip {label}"), logged)
                        })),
                )
                .into_any_element()
        });
        let working = running
            && self
                .onboarding
                .as_ref()
                .is_some_and(|o| o.bring.all_here.is_none_or(|at| at > now));
        let knot =
            card::at(knot_x - knot / 2., knot_y - knot / 2.).child(match working && !still {
                true => {
                    let [lap, _] = t::BRING_LAP;
                    let since = self
                        .onboarding
                        .as_ref()
                        .map_or(0., |o| o.secs_since(o.since));
                    card::knot(knot, (since / lap).fract(), inks.text(1.))
                }
                false => card::whole_knot(knot, inks.text(1.)),
            });
        let mut drawn = vec![strands.into_any_element()];
        drawn.extend(chip_cells);
        drawn.push(knot.into_any_element());
        drawn
    }

    /// Safari without Full Disk Access: the notice in Settings' form beside the icon, its way to
    /// System Settings, read again as the app comes back.
    fn bring_notice(&self, name: &str, inks: Inks, cx: &mut Context<Self>) -> AnyElement {
        let [left, width] = t::BRING_NOTICE;
        let [_, slot_y, slot] = t::BRING_SLOT;
        let [height, ..] = t::ROW;
        let open = card::quiet(
            "onboarding-full-disk-access",
            "Open System Settings".into(),
            inks,
        )
        .when(self.a11y, |open| named_button(open, "Open System Settings"))
        .on_click(cx.listener(|shell, _, _, cx| {
            if let Some(view) = shell.bring_view() {
                view.update(cx, |view, cx| view.press(cx));
            }
        }))
        .relative()
        .children(self.place(Step::Import, "full-disk-access"));
        let words = format!("{name} needs Full Disk Access in System Settings.");
        card::at(left + slot + t::FOOT_GAP * 2., slot_y + slot / 2. - height)
            .w(px(width - slot - t::FOOT_GAP * 2.))
            .child(card::rows(
                vec![
                    card::row(
                        format!("Let {PRODUCT} read {name}").into(),
                        words.into(),
                        Some(open.into_any_element()),
                        inks,
                    )
                    .into_any_element(),
                ],
                width - slot - t::FOOT_GAP * 2.,
                inks,
            ))
            .into_any_element()
    }

    /// What keeps the import from going on, `said`, with Try Again: a browser that couldn't be
    /// read, which reads it again, another still to be picked; or a run whose profile hasn't
    /// opened, which asks it again.
    fn bring_again(&self, said: String, inks: Inks, cx: &mut Context<Self>) -> AnyElement {
        let [_, centre, ..] = t::BRING_CHIP;
        let [_, size, height] = t::LINE;
        let again = card::quiet("onboarding-try-again", "Try Again".into(), inks)
            .when(self.a11y, |again| named_button(again, "Try Again"))
            .on_click(cx.listener(|shell, _, _, cx| shell.bring_try_again(cx)))
            .relative()
            .children(self.place(Step::Import, "try-again"));
        card::at(0., centre - height)
            .w(px(t::WIDTH))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(t::FOOT_GAP))
            .child(
                div()
                    .text_size(px(size))
                    .line_height(px(height))
                    .text_color(inks.text_2())
                    .child(said),
            )
            .child(again)
            .into_any_element()
    }

    /// The favourites, the step's second state: up to eight tiles, a click leaving one out, with
    /// the picked icon's copy and the knot staying either side.
    pub(super) fn bring_tiles(
        &mut self,
        bring: Option<&Bring>,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(view) = self.bring_view() else {
            return div().into_any_element();
        };
        let (offered, ticked) = view.read_with(cx, |view, _| {
            let offered: Vec<crate::import::found::Tab> =
                view.suggested().map_or(Vec::new(), <[_]>::to_vec);
            (offered, view.ticked())
        });
        let [width, height, radius, gap] = t::TILE;
        let per_row = t::TILE_ROW as usize;
        let [name_size, name_line] = t::TILE_NAME;
        let [cx_mid, centre, ..] = t::BRING_CHIP;
        let [knot_x, knot_y, knot] = t::BRING_KNOT;
        let tiles: Vec<_> = offered.into_iter().take(per_row * 2).collect();
        let rows = tiles.len().div_ceil(per_row).max(1);
        let in_row = tiles.len().min(per_row).max(1);
        let block_w = in_row as f32 * width + (in_row - 1) as f32 * gap;
        let block_h = rows as f32 * height + (rows - 1) as f32 * gap;
        let (left, top) = (cx_mid - block_w / 2., centre - block_h / 2.);
        let a11y = self.a11y;
        let cells = tiles.iter().enumerate().map(|(at, tab)| {
            let on = ticked.iter().any(|each| each.url == tab.url);
            let name = crate::import::suggested::site_name(&tab.url);
            let url = tab.url.clone();
            let (column, row) = (at % per_row, at / per_row);
            let letter: String = name
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect())
                .unwrap_or_default();
            card::at(
                left + column as f32 * (width + gap),
                top + row as f32 * (height + gap),
            )
            .child(
                div()
                    .id(("onboarding-favorite", at))
                    .when(a11y, |tile| named_check(tile, name.clone(), on))
                    .relative()
                    .w(px(width))
                    .h(px(height))
                    .rounded(px(radius))
                    .bg(if on { inks.button() } else { inks.group() })
                    .opacity(if on { 1. } else { t::BRING_OFF })
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(t::BARE_INSET * 3.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        if let Some(view) = shell.bring_view() {
                            view.update(cx, |view, cx| view.toggle_favorite(&url, cx));
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(t::TILE_PLATE))
                            .rounded(px(t::TILE_PLATE * t::TILE_PLATE_CORNER))
                            .bg(inks.group())
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(name_size + 2.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(inks.text(1.))
                            .child(letter),
                    )
                    .child(
                        div()
                            .max_w(px(width - t::BARE_INSET * 4.))
                            .truncate()
                            .text_size(px(name_size))
                            .line_height(px(name_line))
                            .text_color(inks.text(1.))
                            .child(name.clone()),
                    )
                    .when(on, |tile| {
                        tile.child(
                            div()
                                .absolute()
                                .top(px(t::BARE_INSET * 3.))
                                .right(px(t::BARE_INSET * 3.))
                                .child(card::check_disc(inks).size(px(t::TILE_DISC))),
                        )
                    })
                    .children(self.place(Step::Favorites, tile_place(at))),
            )
        });
        let landed = bring.and_then(|bring| self.bring_copy(bring, inks));
        div()
            .absolute()
            .inset_0()
            .children(landed)
            .children(cells)
            .child(
                card::at(knot_x - knot / 2., knot_y - knot / 2.)
                    .child(card::whole_knot(knot, inks.text(1.))),
            )
            .into_any_element()
    }
}

/// A plate with a name's first letter, for a browser whose icon the system didn't give.
fn letter_plate(name: &str, size: f32, inks: Inks) -> gpui::Div {
    let letter: String = (name.chars().next())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    div()
        .size(px(size))
        .rounded(px(size * t::BRING_PLATE[0]))
        .bg(inks.button())
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * t::BRING_PLATE[1]))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(inks.text(1.))
        .child(letter)
}

/// The line in the flow's place until a browser is picked (#4449).
pub(super) const PICK_A_BROWSER: &str = "Pick a browser to see what comes over.";

/// What a test calls a browser's icon in its cell.
fn icon_place(at: usize) -> &'static str {
    ["icon 0", "icon 1", "icon 2", "icon 3", "icon 4"]
        .get(at)
        .copied()
        .unwrap_or("icon")
}

/// What a test calls a browser's cell, a chip and a tile.
fn browser_place(at: usize) -> &'static str {
    [
        "browser 0",
        "browser 1",
        "browser 2",
        "browser 3",
        "browser 4",
    ]
    .get(at)
    .copied()
    .unwrap_or("browser")
}

fn chip_place(at: usize) -> &'static str {
    [
        "chip 0", "chip 1", "chip 2", "chip 3", "chip 4", "chip 5", "chip 6", "chip 7",
    ]
    .get(at)
    .copied()
    .unwrap_or("chip")
}

fn tile_place(at: usize) -> &'static str {
    [
        "tile 0", "tile 1", "tile 2", "tile 3", "tile 4", "tile 5", "tile 6", "tile 7",
    ]
    .get(at)
    .copied()
    .unwrap_or("tile")
}

/// The browser picked, by name.
fn picked_name(bring: &Bring) -> String {
    (bring.picked)
        .and_then(|at| bring.browsers.get(at))
        .map_or(String::new(), |(name, _)| name.clone())
}

/// What keeps the import from going on, in the failure's place: a browser that couldn't be read,
/// or a run whose profile is taking too long (FR20; v9 draws no stall).
pub(super) fn bring_held(bring: &Bring) -> Option<String> {
    let name = picked_name(bring);
    match &bring.state {
        Bringing::Running {
            stalled: Some(_), ..
        } => Some(format!("{name} is taking too long.")),
        Bringing::Unread => Some(format!("Couldn't read {name}.")),
        _ => None,
    }
}
