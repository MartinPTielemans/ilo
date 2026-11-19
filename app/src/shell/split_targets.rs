//! Add Split View's targets (Dia, docs/ux-spec.md › Split view › Drag targets): a tab dragged out
//! of the sidebar over the page becomes a picture of a window, and one target slides in, as Arc's
//! one region, at the card's side nearest the pointer, its top and bottom too as Arc's stacks them.
//! Let go over it, the tab joins the page shown in a split view on that side. A link dragged over
//! the page brings the same target, and let go on it it opens in a pane there.
//! GPUI can't draw over the page's native view, so the app draws them in a window of its own over
//! the browser window, which the mouse goes through (`Browser::show_drop_targets`).

use web_time::Instant;

use gpui::{
    AnyElement, Bounds, Context, FontWeight, Hsla, PathBuilder, Pixels, Point, Window, canvas, div,
    point, prelude::*, px, rgb, size,
};

use super::{FAVICON_DRAWN, Place, Shell, TITLE_GAP};
use crate::palette::{Palette, TARGET_ACCENT, TARGET_PLATE};

/// Each target's width and height as a fraction of the card's, and how far it is in from the
/// card's side: as it slides in, grown, and under the tab, where it leans TARGET_LEAN of the way
/// toward the pointer.
const TARGET_REST: (f32, f32, f32) = (0.132, 0.383, 37.5);
const TARGET_GROWN: (f32, f32, f32) = (0.215, 0.611, 30.);
const TARGET_UNDER: (f32, f32) = (0.241, 0.69);
const TARGET_LEAN: f32 = 0.43;
/// The top and bottom targets' height and width as a fraction of the card's, and their inset, the
/// sides' turned, but only as wide as the room between the side targets grown (ours, as Arc's
/// aren't measured).
const STACK_REST: (f32, f32, f32) = (0.132, 0.28, 37.5);
const STACK_GROWN: (f32, f32, f32) = (0.215, 0.45, 30.);
const STACK_UNDER: (f32, f32) = (0.241, 0.51);
/// The targets slide in, then grow, and the one under the tab grows again, each on an ease-out
/// cubic with no overshoot.
const TARGET_SLIDE: f32 = 0.11;
const TARGET_GROW: f32 = 0.12;
/// A target's dashed line, TARGET_DASH_INSET inside its edge: its dashes, as long as its gaps, and
/// its width, away from the tab and under it.
use crate::tokens::shell::split_targets::TARGET_DASH_INSET;
use crate::tokens::shell::split_targets::TARGET_RADIUS;
const TARGET_DASH: (f32, f32) = (6., 1.75);
const TARGET_DASH_UNDER: (f32, f32) = (7., 2.);
use crate::tokens::shell::split_targets::TARGET_DASHES;
use crate::tokens::shell::split_targets::TARGET_EDGE;
use crate::tokens::shell::split_targets::TARGET_TEXT;
/// Arc 1.170 draws the window's content at about 0.92 of its size while a tab or a link is over the
/// page, about its centre, and back after. How long it takes isn't measured: ours, the targets'
/// slide and grow, on their ease-out.
const CONTENT_SCALE: f32 = 0.92;
const CONTENT_SCALE_TIME: f32 = TARGET_SLIDE + TARGET_GROW;
/// Dia's targets came once the pointer was 26 pt into the page from its top strip. Ours: the same
/// past the card's left edge, until Arc's is measured.
const TARGET_TRIGGER: f32 = 26.;
/// The picture of a window the tab becomes over the page, centred on the pointer (Dia), and ours:
/// its corners, rim, traffic lights and the page in it.
const PICTURE: (f32, f32) = (209., 109.5);
use crate::elevation::Level;
use crate::tokens::shell::split_targets::HALF_GLYPH_RADIUS;
use crate::tokens::shell::split_targets::PICTURE_INSET;
use crate::tokens::shell::split_targets::PICTURE_LIGHT;
use crate::tokens::shell::split_targets::PICTURE_LIGHTS;
use crate::tokens::shell::split_targets::PICTURE_RADIUS;
use crate::tokens::shell::split_targets::PICTURE_TOP;
use crate::tokens::text;

/// The pages' scale on its way to `to`, from `from` at `since`.
#[derive(Clone, Copy)]
pub(super) struct ContentScale {
    from: f32,
    to: f32,
    since: Instant,
}

impl Default for ContentScale {
    fn default() -> Self {
        Self {
            from: 1.,
            to: 1.,
            since: Instant::now(),
        }
    }
}

impl ContentScale {
    pub(super) fn now(&self) -> f32 {
        let progress = self.since.elapsed().as_secs_f32() / CONTENT_SCALE_TIME;
        if progress >= 1. {
            self.to
        } else {
            self.from + (self.to - self.from) * ease_out(progress)
        }
    }
}

/// `bounds` drawn at `scale` of their size about `centre`.
pub(super) fn scaled_about(
    bounds: Bounds<Pixels>,
    centre: Point<Pixels>,
    scale: f32,
) -> Bounds<Pixels> {
    let at = |edge: Pixels, mid: Pixels| mid + (edge - mid) * scale;
    Bounds::new(
        point(at(bounds.left(), centre.x), at(bounds.top(), centre.y)),
        size(bounds.size.width * scale, bounds.size.height * scale),
    )
}

/// Where a target is on the card, and so where the tab joins the page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    /// Whether the split it makes shows its pages top to bottom.
    fn stacked(self) -> bool {
        matches!(self, Edge::Top | Edge::Bottom)
    }

    /// Whether the tab comes before the page it joins.
    fn first(self) -> bool {
        matches!(self, Edge::Left | Edge::Top)
    }

    fn name(self) -> &'static str {
        match self {
            Edge::Left => "left",
            Edge::Right => "right",
            Edge::Top => "top",
            Edge::Bottom => "bottom",
        }
    }
}

/// A dragged tab over the page, where it can join the page shown in a split view.
pub(super) struct OverPage {
    /// The tab it would join.
    with: u64,
    /// Where the pointer is in the window.
    pointer: Point<Pixels>,
    /// Each target, as one split has one orientation: all four for a page alone, else the two
    /// along its split's. For each, whether the tab is over it, since when, and how far it had
    /// grown toward its size under the tab then.
    targets: Vec<(Edge, bool, Instant, f32)>,
    /// The one target shown, at the card's edge nearest the pointer, as Arc's one region, or the
    /// one under the tab while it's there; and since when, as it slides in from that edge.
    shown: (Edge, Instant),
    /// The dragged tab, where its press showed it in place of the page it would join, which shows
    /// under the targets instead: it shows again once they go without it let go on one.
    back: Option<u64>,
}

impl OverPage {
    fn new(
        with: u64,
        edges: Vec<Edge>,
        card: Bounds<Pixels>,
        pointer: Point<Pixels>,
        back: Option<u64>,
    ) -> Self {
        log!("browser: drop targets for {with}");
        let since = Instant::now();
        let shown = (nearest(&edges, card, pointer), since);
        // check.sh reads this line.
        log!("browser: drop target shown {}", shown.0.name());
        let targets = edges
            .into_iter()
            .map(|edge| (edge, false, since, 0.))
            .collect();
        OverPage {
            with,
            pointer,
            targets,
            shown,
            back,
        }
    }

    /// The dragged tab to show again once the targets go without it let go on one.
    pub(super) fn back(&self) -> Option<u64> {
        self.back
    }

    /// The target the tab is over.
    fn edge(&self) -> Option<Edge> {
        self.targets
            .iter()
            .find(|(_, under, ..)| *under)
            .map(|(edge, ..)| *edge)
    }

    /// The pointer is at `position` over `card`: a target it's in grows toward it, and the one it
    /// was under shrinks once it leaves it, from `now` (`motion::start`).
    fn point(&mut self, card: Bounds<Pixels>, position: Point<Pixels>, now: Instant) {
        let edge = self.edge();
        self.pointer = position;
        // The one under the tab lets it go once the pointer leaves it.
        for target in &mut self.targets {
            let (at, under, since, from) = *target;
            if under && !under_bounds(card, at, position).contains(&position) {
                *target = (at, false, now, grown(true, since, from));
            }
        }
        // It stays shown while the tab is over it, so the pointer passing nearer another edge on
        // its way in doesn't take away the target it's aiming at.
        let edges: Vec<Edge> = self.targets.iter().map(|(at, ..)| *at).collect();
        let shown = self
            .edge()
            .unwrap_or_else(|| nearest(&edges, card, position));
        if shown != self.shown.0 {
            self.shown = (shown, now);
            // check.sh reads this line.
            log!("browser: drop target shown {}", shown.name());
        }
        // The one shown is under the tab once the pointer is in it grown.
        for target in &mut self.targets {
            let (at, under, since, from) = *target;
            if !under && at == shown && target_bounds(card, at, grown_at(at)).contains(&position) {
                *target = (at, true, now, grown(false, since, from));
            }
        }
        if self.edge() != edge {
            // check.sh reads this line.
            log!(
                "browser: drop target {}",
                self.edge().map_or("none", Edge::name)
            );
        }
    }
}

/// A link dragged over the page, from it or from another app, with the targets where it can open
/// in a pane beside the page shown.
pub(super) struct LinkOver {
    url: String,
    over: OverPage,
}

impl Shell {
    /// The tab a dragged `tab` would join in a split view on the page: the one shown, or for the
    /// tab shown alone, the one shown before it (Dia). None where the page has no targets for it:
    /// a favorite, a folder, a split view's row, a tab of the split shown, or a split shown full.
    pub(super) fn split_partner(&self, tab: u64) -> Option<u64> {
        if self.is_folder(tab) || self.panes(tab).len() > 1 {
            return None;
        }
        let shown = self.panes(self.selected);
        if self.selected == 0 || shown.len() >= super::MAX_PANES {
            return None;
        }
        if shown != [tab] {
            return (!shown.contains(&tab)).then_some(self.selected);
        }
        let space = self.shown_space()?;
        let in_space = |other: &u64| space.pinned.contains(other) || space.today.contains(other);
        self.recent
            .iter()
            .copied()
            .find(|&other| other != tab && in_space(&other))
    }

    /// The dragged tab is at `position` in the window. Past TARGET_TRIGGER into the card, where it
    /// can join the page shown, the targets slide in and the tab becomes a picture of a window, and
    /// a target it's over grows toward it. Returns whether it's over the page. Back over the
    /// sidebar, it's a row again.
    pub(super) fn drag_over_page(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        let card = self.page_frame(self.viewport).0;
        // A file's row, past the trigger, hands its file to the system's drag to upload it to the
        // page, as a download's row does, in place of the targets, and the row goes back.
        // Past the trigger from the card's edge by the sidebar.
        let into_page = match self.right() {
            true => position.x < card.right() - px(TARGET_TRIGGER),
            false => position.x > card.left() + px(TARGET_TRIGGER),
        };
        let listed = matches!(drag.from, Place::List(..)) && !self.sidebar_hidden;
        let tab = drag.tab;
        if let Some(path) = (into_page && listed)
            .then(|| self.file_of_row(tab))
            .flatten()
            && self.browser.drag_file(&path, cx)
        {
            // check.sh waits for this line.
            log!("browser: tab {tab} file drag");
            if let Some(drag) = &mut self.drag {
                drag.moving = true;
            }
            self.cancel_drag(cx);
            return true;
        }
        let Some(drag) = &self.drag else {
            return false;
        };
        // The sidebar shown while hidden lies over the page, so a row dragged in it is over the
        // page from the start and would show the targets at once.
        let with = match drag.from {
            Place::List(..) if !self.sidebar_hidden => self.split_partner(drag.tab),
            Place::List(..) | Place::Favorite(_) => None,
        };
        let Some(with) = with.filter(|_| into_page) else {
            if let Some(over) = self.drag.as_mut().and_then(|drag| drag.over_page.take()) {
                // check.sh reads this line.
                log!("browser: drop targets gone");
                self.show_back(over.back(), cx);
                cx.notify();
            }
            return false;
        };
        let edges = self.edges_for(with);
        // The page under the targets is the one the tab would join, so where the press showed the
        // tab in its place, that page shows again while they're up (#1467).
        let coming = drag.over_page.is_none();
        let back = (coming && !self.panes(self.tab_asked()).contains(&with)).then_some(tab);
        if back.is_some() {
            self.select(with, cx);
        }
        // Where the pages are drawn now, smaller once the tab is over them.
        let card = self.scaled_card();
        let Some(drag) = &mut self.drag else {
            return false;
        };
        drag.moving = true;
        let over = drag
            .over_page
            .get_or_insert_with(|| OverPage::new(with, edges, card, position, back));
        over.point(card, position, crate::motion::start(cx));
        cx.notify();
        true
    }

    /// Draws the pages smaller while a tab or a link is over them, and at their size again once it
    /// isn't, a step each frame, telling the core each step. Under Reduce Motion they keep their
    /// size.
    pub(super) fn step_content_scale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scale_content(cx) {
            window.request_animation_frame();
        }
    }

    /// The step of `step_content_scale`: returns whether the pages are still on their way.
    fn scale_content(&mut self, cx: &mut Context<Self>) -> bool {
        let over = (self.drag.as_ref()).is_some_and(|drag| drag.over_page.is_some())
            || self.link_over.is_some();
        let to = if over {
            crate::motion::scale(CONTENT_SCALE, cx)
        } else {
            1.
        };
        if to != self.content_scale.to {
            self.content_scale = ContentScale {
                from: self.content_scale.now(),
                to,
                since: crate::motion::start(cx),
            };
        }
        let scale = self.content_scale.now();
        if scale != self.content_scale_told {
            self.content_scale_told = scale;
            self.browser.set_page_scale(scale, cx);
            if scale == to {
                // check.sh waits for this line.
                log!("browser: page scale {scale}");
            }
        }
        scale != to
    }

    /// The card as the pages are drawn now: smaller under a tab or a link over them.
    pub(super) fn scaled_card(&self) -> Bounds<Pixels> {
        let card = self.page_frame(self.viewport).0;
        scaled_about(card, card.center(), self.content_scale.now())
    }

    /// The targets a tab or a link can join `with` at: all four for a page alone, else the two
    /// along its split's orientation.
    fn edges_for(&self, with: u64) -> Vec<Edge> {
        match (self.panes(with).len() > 1, self.stacked(with)) {
            (false, _) => vec![Edge::Left, Edge::Right, Edge::Top, Edge::Bottom],
            (true, false) => vec![Edge::Left, Edge::Right],
            (true, true) => vec![Edge::Top, Edge::Bottom],
        }
    }

    /// Where `tab`'s page is in the window, if it's one of the pages shown.
    fn shown_pane(&self, tab: u64) -> Option<Bounds<Pixels>> {
        let panes = self.panes(self.selected);
        let (frame, _) = self.page_frame(self.viewport);
        let (stacked, shares) = (self.stacked(self.selected), self.shares(self.selected));
        let frames = super::pane_frames(frame, panes.len(), stacked, &shares);
        panes
            .iter()
            .position(|&pane| pane == tab)
            .map(|at| frames[at])
    }

    /// A link to `url` is dragged over `tab`'s page, at `point` in it. Over a page shown that has
    /// room for another, the target slides in as for a tab, and grows once the pointer is in it.
    /// Returns whether the pointer is on one, where the drag is the app's and the page doesn't see
    /// it.
    pub fn link_over_page(
        &mut self,
        tab: u64,
        point: Point<Pixels>,
        url: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let room = self.selected != 0 && self.panes(self.selected).len() < super::MAX_PANES;
        let busy = self.drag.is_some() || self.adding_pane.is_some() || self.page_fullscreen;
        let Some(page) = self.shown_pane(tab).filter(|_| room && !busy) else {
            self.link_left_page(cx);
            return false;
        };
        let position = page.origin + point;
        let card = self.scaled_card();
        let with = self.selected;
        if self
            .link_over
            .as_ref()
            .is_none_or(|link| link.over.with != with)
        {
            let over = OverPage::new(with, self.edges_for(with), card, position, None);
            self.link_over = Some(LinkOver {
                url: url.to_string(),
                over,
            });
        }
        let Some(link) = &mut self.link_over else {
            return false;
        };
        link.url = url.to_string();
        link.over.point(card, position, crate::motion::start(cx));
        cx.notify();
        link.over.edge().is_some()
    }

    /// The link dragged over the page left it, or the drag ended: the targets go.
    pub fn link_left_page(&mut self, cx: &mut Context<Self>) {
        if self.link_over.take().is_some() {
            // check.sh reads this line.
            log!("browser: drop targets gone");
            cx.notify();
        }
    }

    /// The link was let go at `point` in `tab`'s page. On a target, it opens in a new tab joining
    /// the page shown on that side. Returns whether it opened, else the page has the drop.
    pub fn link_dropped(&mut self, tab: u64, point: Point<Pixels>, cx: &mut Context<Self>) -> bool {
        let url = self
            .link_over
            .as_ref()
            .map(|link| link.url.clone())
            .unwrap_or_default();
        let taken = self.link_over_page(tab, point, &url, cx);
        let Some(link) = self.link_over.take() else {
            return false;
        };
        cx.notify();
        let Some(edge) = link.over.edge().filter(|_| taken) else {
            log!("browser: drop targets gone");
            return false;
        };
        // check.sh waits for this line.
        log!("browser: link dropped {}", edge.name());
        let alone = self.panes(link.over.with).len() == 1;
        // A page alone has every target, and the top and bottom ones stack its new split.
        let stacked = alone && edge.stacked();
        self.browser
            .open_split_at(&link.url, link.over.with, edge.first(), stacked, cx);
        true
    }

    /// The pointer let go of a tab over the page: over a target, it joins the page shown in a split
    /// view on that side and shows there, and anywhere else it opens in a window of its own, as big
    /// as this one and centred on the pointer, where the picture of a window was (Dia). A private or
    /// Little ilo window's tab, which no other window takes, goes back to its row. Returns whether
    /// it was over the page.
    pub(super) fn drop_over_page(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        let Some(over) = &drag.over_page else {
            return false;
        };
        let (tab, with, edge, back) = (drag.tab, over.with, over.edge(), over.back());
        let pointer = point(f32::from(over.pointer.x), f32::from(over.pointer.y));
        let alone = self.panes(with).len() == 1;
        let own = self.private || self.little.is_some();
        // The release that ends the drag isn't a click.
        self.dragged = true;
        self.settle_drag(cx);
        match edge {
            // A page alone has every target, and the top and bottom ones stack its new split.
            Some(edge) => self.join_split(tab, with, edge.first(), alone && edge.stacked(), cx),
            None if own => self.show_back(back, cx),
            None => {
                let at = self.browser.screen_point(pointer);
                let size = self.viewport;
                let middle = point(f32::from(size.width) / 2., f32::from(size.height) / 2.);
                // check.sh waits for this line.
                log!("browser: tab {tab} let go off the targets");
                self.browser.tab_to_new_window(tab, at, middle, cx);
            }
        }
        cx.notify();
        true
    }

    /// Shows the dragged tab `back` again, as its press did, once the targets that showed the page
    /// it would join have gone without it let go on one.
    pub(super) fn show_back(&mut self, back: Option<u64>, cx: &mut Context<Self>) {
        if let Some(tab) = back {
            self.select(tab, cx);
            self.browser.focus_page(cx);
        }
    }

    /// What the window over the page draws while a dragged tab is over it: the targets, clipped to
    /// the card, and the tab as a picture of a window under the pointer.
    pub fn drop_targets(&self, window: &mut Window) -> AnyElement {
        // A tab's, or a link's, whose picture the system draws under the pointer.
        let dragged =
            (self.drag.as_ref()).and_then(|drag| Some((drag.over_page.as_ref()?, drag.tab)));
        let link = self.link_over.as_ref().map(|link| (&link.over, 0));
        let Some((over, tab)) = dragged.or(link) else {
            return div().into_any_element();
        };
        let card = self.scaled_card();
        let palette = self.palette;
        let fill = self.window_fill().blend(palette.plate(TARGET_PLATE));
        let t = over.shown.1.elapsed().as_secs_f32();
        let growing = over
            .targets
            .iter()
            .any(|(_, _, since, _)| since.elapsed().as_secs_f32() < TARGET_GROW);
        if growing || t < TARGET_SLIDE + TARGET_GROW {
            window.request_animation_frame();
        }
        // Only the one shown: one the pointer left for another edge goes at once.
        let targets = over
            .targets
            .iter()
            .filter(|(edge, ..)| *edge == over.shown.0);
        let targets = targets.map(|&(edge, under, since, from)| {
            let way = grown(under, since, from);
            let under = under_bounds(card, edge, over.pointer);
            let bounds = mix_bounds(arriving(card, edge, t), under, way);
            target(bounds, card, edge, way, fill, palette)
        });
        let page = div()
            .absolute()
            .left(card.left())
            .top(card.top())
            .w(card.size.width)
            .h(card.size.height)
            .rounded(px(self.card_radius()))
            .overflow_hidden()
            .children(targets);
        let picture = (tab != 0).then(|| {
            let title = self
                .tabs
                .get(&tab)
                .map(|tab| tab.title.clone())
                .unwrap_or_default();
            picture(
                over.pointer,
                self.window_fill(),
                self.palette,
                self.tab_icon(tab),
                title,
            )
        });
        div()
            .size_full()
            .relative()
            .font_family(crate::face::UI)
            .child(page)
            .children(picture)
            .into_any_element()
    }
}

/// How far a target has grown toward its size under the tab: `from` at `since`, going to all the
/// way if `under`, else none.
/// Which of `edges` is nearest `pointer`, each as a fraction of the card's width or height, so its
/// diagonals split it, the first of them on a tie.
fn nearest(edges: &[Edge], card: Bounds<Pixels>, pointer: Point<Pixels>) -> Edge {
    let away = |edge: &Edge| -> f32 {
        let (gap, across) = match edge {
            Edge::Left => (pointer.x - card.left(), card.size.width),
            Edge::Right => (card.right() - pointer.x, card.size.width),
            Edge::Top => (pointer.y - card.top(), card.size.height),
            Edge::Bottom => (card.bottom() - pointer.y, card.size.height),
        };
        gap / across
    };
    edges
        .iter()
        .min_by(|a, b| away(a).total_cmp(&away(b)))
        .copied()
        .unwrap_or(Edge::Right)
}

fn grown(under: bool, since: Instant, from: f32) -> f32 {
    let to = if under { 1. } else { 0. };
    from + (to - from) * ease_out(since.elapsed().as_secs_f32() / TARGET_GROW)
}

fn ease_out(progress: f32) -> f32 {
    1. - (1. - progress.clamp(0., 1.)).powi(3)
}

/// The target at `edge`'s rest and grown sizes: `(across, along, inset)`.
fn rest_at(edge: Edge) -> (f32, f32, f32) {
    if edge.stacked() {
        STACK_REST
    } else {
        TARGET_REST
    }
}

fn grown_at(edge: Edge) -> (f32, f32, f32) {
    if edge.stacked() {
        STACK_GROWN
    } else {
        TARGET_GROWN
    }
}

/// The target at `edge`'s bounds in `card`: `(across, along, inset)`, its size across the card's
/// edge and along it as fractions of the card's, and how far in it is from that edge, centred
/// along it.
fn target_bounds(card: Bounds<Pixels>, edge: Edge, at: (f32, f32, f32)) -> Bounds<Pixels> {
    let (across, along, inset) = at;
    let (width, height) = if edge.stacked() {
        (along, across)
    } else {
        (across, along)
    };
    let size = size(card.size.width * width, card.size.height * height);
    let centred = card.center() - point(size.width / 2., size.height / 2.);
    let origin = match edge {
        Edge::Left => point(card.left() + px(inset), centred.y),
        Edge::Right => point(card.right() - px(inset) - size.width, centred.y),
        Edge::Top => point(centred.x, card.top() + px(inset)),
        Edge::Bottom => point(centred.x, card.bottom() - px(inset) - size.height),
    };
    Bounds::new(origin, size)
}

/// A target under the tab: bigger, its centre leaning across the card toward the pointer.
fn under_bounds(card: Bounds<Pixels>, edge: Edge, pointer: Point<Pixels>) -> Bounds<Pixels> {
    let grown = target_bounds(card, edge, grown_at(edge));
    let (across, along) = if edge.stacked() {
        STACK_UNDER
    } else {
        TARGET_UNDER
    };
    let (width, height) = if edge.stacked() {
        (along, across)
    } else {
        (across, along)
    };
    let size = size(card.size.width * width, card.size.height * height);
    let lean = |from: Pixels, to: Pixels| from + (to - from) * TARGET_LEAN;
    let mut centre = card.center();
    if edge.stacked() {
        centre.y = lean(grown.center().y, pointer.y);
    } else {
        centre.x = lean(grown.center().x, pointer.x);
    }
    Bounds::new(centre - point(size.width / 2., size.height / 2.), size)
}

/// Where a target is `t` seconds after the tab came over the page: sliding in at its first size
/// from outside the card's edge, then growing.
fn arriving(card: Bounds<Pixels>, edge: Edge, t: f32) -> Bounds<Pixels> {
    let rest = target_bounds(card, edge, rest_at(edge));
    if t < TARGET_SLIDE {
        let away = 1. - ease_out(t / TARGET_SLIDE);
        let inset = px(rest_at(edge).2);
        let origin = match edge {
            Edge::Left => rest.origin - point((rest.size.width + inset) * away, px(0.)),
            Edge::Right => rest.origin + point((rest.size.width + inset) * away, px(0.)),
            Edge::Top => rest.origin - point(px(0.), (rest.size.height + inset) * away),
            Edge::Bottom => rest.origin + point(px(0.), (rest.size.height + inset) * away),
        };
        return Bounds::new(origin, rest.size);
    }
    let grown = target_bounds(card, edge, grown_at(edge));
    mix_bounds(rest, grown, ease_out((t - TARGET_SLIDE) / TARGET_GROW))
}

fn mix_bounds(from: Bounds<Pixels>, to: Bounds<Pixels>, way: f32) -> Bounds<Pixels> {
    let mix = |from: Pixels, to: Pixels| from + (to - from) * way;
    Bounds::new(
        point(mix(from.left(), to.left()), mix(from.top(), to.top())),
        size(
            mix(from.size.width, to.size.width),
            mix(from.size.height, to.size.height),
        ),
    )
}

/// A target at `bounds` in the window, drawn in `card`, `way` of the way to its look under the
/// tab: its fill, edge and dashed line, with a glyph of the half it adds over its title.
fn target(
    bounds: Bounds<Pixels>,
    card: Bounds<Pixels>,
    edge: Edge,
    way: f32,
    fill: Hsla,
    palette: Palette,
) -> impl IntoElement {
    let accent = Hsla::from(palette.accent);
    let fill = fill.blend(accent.opacity(TARGET_ACCENT * way));
    let mix = |from: Hsla, to: Hsla| from.opacity(1. - way).blend(to.opacity(way));
    let text = mix(palette.text(TARGET_TEXT), palette.text(1.));
    let dashes = mix(palette.faint(TARGET_DASHES), accent);
    let dash = TARGET_DASH.0 + (TARGET_DASH_UNDER.0 - TARGET_DASH.0) * way;
    let width = TARGET_DASH.1 + (TARGET_DASH_UNDER.1 - TARGET_DASH.1) * way;
    let line = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let inset = px(TARGET_DASH_INSET);
            let inner = Bounds::new(
                bounds.origin + point(inset, inset),
                size(
                    bounds.size.width - inset * 2.,
                    bounds.size.height - inset * 2.,
                ),
            );
            let mut path = PathBuilder::stroke(px(width)).dash_array(&[px(dash), px(dash)]);
            rounded_rect(&mut path, inner, px(TARGET_RADIUS - TARGET_DASH_INSET));
            if let Ok(path) = path.build() {
                window.paint_path(path, dashes);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();
    div()
        .absolute()
        .left(bounds.left() - card.left())
        .top(bounds.top() - card.top())
        .w(bounds.size.width)
        .h(bounds.size.height)
        .rounded(px(TARGET_RADIUS))
        .bg(fill)
        .border(px(0.5))
        .border_color(palette.faint(TARGET_EDGE))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(TITLE_GAP))
        .child(line)
        .child(half_glyph(edge, text))
        .child(
            div()
                .text_size(px(text::BODY))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(text)
                .child(match edge {
                    Edge::Left => "Add left split",
                    Edge::Right => "Add right split",
                    Edge::Top => "Add top split",
                    Edge::Bottom => "Add bottom split",
                }),
        )
}

/// A rounded rectangle at `bounds` with corners of `radius`, clockwise from its top left.
pub(super) fn rounded_rect(path: &mut PathBuilder, bounds: Bounds<Pixels>, radius: Pixels) {
    let (left, top, right, bottom) = (bounds.left(), bounds.top(), bounds.right(), bounds.bottom());
    let radii = point(radius, radius);
    path.move_to(point(left + radius, top));
    path.line_to(point(right - radius, top));
    path.arc_to(radii, px(0.), false, true, point(right, top + radius));
    path.line_to(point(right, bottom - radius));
    path.arc_to(radii, px(0.), false, true, point(right - radius, bottom));
    path.line_to(point(left + radius, bottom));
    path.arc_to(radii, px(0.), false, true, point(left, bottom - radius));
    path.line_to(point(left, top + radius));
    path.arc_to(radii, px(0.), false, true, point(left + radius, top));
    path.close();
}

/// A ≈ 19 pt rectangle with the half at `edge` filled.
fn half_glyph(edge: Edge, color: Hsla) -> impl IntoElement {
    let half = div().absolute().bg(color);
    let half = match edge {
        Edge::Left => half.top_0().bottom_0().w(px(8.)).left_0(),
        Edge::Right => half.top_0().bottom_0().w(px(8.)).right_0(),
        Edge::Top => half.left_0().right_0().h(px(6.)).top_0(),
        Edge::Bottom => half.left_0().right_0().h(px(6.)).bottom_0(),
    };
    div()
        .relative()
        .w(px(19.))
        .h(px(15.))
        .rounded(px(HALF_GLYPH_RADIUS))
        .border(px(1.5))
        .border_color(color)
        .overflow_hidden()
        .child(half)
}

/// The dragged tab over the page: a picture of a window centred on `pointer`, its page showing the
/// tab's favicon and title.
fn picture(
    pointer: Point<Pixels>,
    fill: Hsla,
    palette: Palette,
    icon: AnyElement,
    title: gpui::SharedString,
) -> impl IntoElement {
    let (width, height) = PICTURE;
    let lights = PICTURE_LIGHTS.map(|color| {
        div()
            .size(px(PICTURE_LIGHT))
            .rounded(px(PICTURE_LIGHT / 2.))
            .bg(rgb(color))
    });
    let page = div()
        .absolute()
        .left(px(PICTURE_INSET))
        .right(px(PICTURE_INSET))
        .top(px(PICTURE_TOP))
        .bottom(px(PICTURE_INSET))
        .rounded(px(PICTURE_RADIUS - PICTURE_INSET))
        .bg(fill.blend(palette.plate(TARGET_PLATE)))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(TITLE_GAP))
        .px(px(FAVICON_DRAWN))
        .child(div().flex_none().child(icon))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(text::SMALL))
                .text_color(palette.text(TARGET_TEXT))
                .child(title),
        );
    div()
        .absolute()
        .left(pointer.x - px(width / 2.))
        .top(pointer.y - px(height / 2.))
        .w(px(width))
        .h(px(height))
        .rounded(px(PICTURE_RADIUS))
        .bg(fill)
        .border(px(0.5))
        .border_color(Level::Popover.rim_in(&palette))
        .shadow(Level::Popover.shadow())
        .child(
            div()
                .absolute()
                .left(px(8.))
                .top(px(6.))
                .flex()
                .gap(px(4.))
                .children(lights),
        )
        .child(page)
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use gpui::{Bounds, Entity, Pixels, Point, TestAppContext, point, px, size};

    use super::{Edge, grown_at, target_bounds};
    use crate::shell::drag::detents::{X, drag_to, press, shell_with};
    use crate::shell::{Folders, Shell, Split, recorder::Recorder};

    /// A Space whose Today holds 10, 11 and 12, with 10 shown alone in a window 1200 by 800.
    fn shown_alone(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, Bounds<Pixels>) {
        let (shell, host) = shell_with(cx, &[], &[10, 11, 12], Folders::default());
        let card = shell.update(cx, |shell, cx| {
            shell.set_selected(10, cx);
            shell.viewport = size(px(1200.), px(800.));
            shell.page_frame(shell.viewport).0
        });
        host.take();
        (shell, host, card)
    }

    /// `at` in the window, as a point in the page's view.
    fn in_page(card: Bounds<Pixels>, at: Point<Pixels>) -> Point<Pixels> {
        at - card.origin
    }

    /// A link dragged over a page alone finds a target at each of the card's edges, which take the
    /// drag from the page, and let go on one it opens in a pane on that side, the top and bottom
    /// ones stacking the split, as a tab let go there does.
    #[gpui::test]
    fn a_link_let_go_on_a_target_opens_in_a_pane_there(cx: &mut TestAppContext) {
        let url = "https://example.org/";
        for (edge, left, stacked) in [
            (Edge::Left, true, false),
            (Edge::Top, true, true),
            (Edge::Bottom, false, true),
        ] {
            let (shell, host, card) = shown_alone(cx);
            let point = in_page(card, target_bounds(card, edge, grown_at(edge)).center());
            let edge = edge.name();
            let taken = shell.update(cx, |shell, cx| shell.link_over_page(10, point, url, cx));
            assert!(taken, "the {edge} target didn't take the link");
            assert!(
                shell.update(cx, |shell, cx| shell.link_dropped(10, point, cx)),
                "{edge}"
            );
            assert_eq!(
                host.take(),
                [format!("open split {url} 10 {left} {stacked}")],
                "{edge}"
            );
            assert!(shell.read_with(cx, |shell, _| shell.link_over.is_none()));
        }
    }

    /// Off the targets, the page keeps the drag and the drop, and the targets go as the link leaves
    /// the page. A split with no room for another page shows none.
    #[gpui::test]
    fn a_link_off_the_targets_is_the_pages(cx: &mut TestAppContext) {
        let (shell, host, card) = shown_alone(cx);
        let middle = in_page(card, card.center());
        let url = "https://example.org/";
        assert!(!shell.update(cx, |shell, cx| shell.link_over_page(10, middle, url, cx)));
        assert!(
            shell.read_with(cx, |shell, _| shell.link_over.is_some()),
            "no targets shown"
        );
        assert!(!shell.update(cx, |shell, cx| shell.link_dropped(10, middle, cx)));
        assert!(host.take().is_empty());
        shell.update(cx, |shell, cx| {
            shell.link_over_page(10, middle, url, cx);
            shell.link_left_page(cx);
        });
        assert!(shell.read_with(cx, |shell, _| shell.link_over.is_none()));

        shell.update(cx, |shell, cx| {
            let split = Split {
                tabs: vec![10, 11, 12],
                ..Default::default()
            };
            shell.set_tabs(
                1,
                Vec::new(),
                vec![10, 11, 12],
                vec![split],
                Folders::default(),
                cx,
            );
        });
        let left = point(px(30.), middle.y);
        assert!(!shell.update(cx, |shell, cx| shell.link_over_page(10, left, url, cx)));
        assert!(
            shell.read_with(cx, |shell, _| shell.link_over.is_none()),
            "a full split has none"
        );
    }

    /// One target shows, Arc's one region, at the card's edge nearest the pointer as a fraction
    /// of its width or height, and the one under the tab stays shown while it's there, though the
    /// pointer comes nearer another edge.
    #[gpui::test]
    fn one_target_shows_at_the_edge_nearest_the_pointer(cx: &mut TestAppContext) {
        let (shell, _host, card) = shown_alone(cx);
        let url = "https://example.org/";
        let at = |x: f32, y: f32| {
            let point = point(
                card.left() + card.size.width * x,
                card.top() + card.size.height * y,
            );
            in_page(card, point)
        };
        let mut shown = |x: f32, y: f32| {
            shell.update(cx, |shell, cx| {
                shell.link_over_page(10, at(x, y), url, cx);
                let over = &shell.link_over.as_ref().unwrap().over;
                (over.shown.0.name(), over.edge().map(Edge::name))
            })
        };
        // Off the targets grown, which reach about a quarter of the way in.
        assert_eq!(shown(0.3, 0.5), ("left", None));
        assert_eq!(shown(0.5, 0.3), ("top", None));
        assert_eq!(shown(0.6, 0.7), ("bottom", None));
        assert_eq!(shown(0.7, 0.5), ("right", None));
        let left = target_bounds(card, Edge::Left, grown_at(Edge::Left)).center();
        let left = ((left.x - card.left()) / card.size.width, 0.5);
        assert_eq!(shown(left.0, left.1), ("left", Some("left")));
        assert_eq!(
            shown(0.2, 0.17),
            ("left", Some("left")),
            "nearer the top, but under the left target"
        );
        assert_eq!(
            shown(0.5, 0.17),
            ("top", Some("top")),
            "out of the left target, into the top one"
        );
    }

    /// A tab's row pressed, which shows it at once, and the drag it becomes: `reported` once the
    /// core has said it's shown. Returns where the row was pressed, down its middle.
    fn press_to_drag(
        shell: &Entity<Shell>,
        cx: &mut TestAppContext,
        tab: u64,
        reported: bool,
    ) -> f32 {
        shell.update(cx, |shell, cx| shell.show_row(tab, cx));
        if reported {
            shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
        }
        let y = press(shell, cx, tab);
        drag_to(shell, cx, X, y + 10.);
        y
    }

    /// A tab its press showed in place of the page alone, dragged out over the page: the page it
    /// would join shows under the targets, whether or not the core has shown the tab yet, and the
    /// tab shows again once they go back over the sidebar. Let go on one, it joins the page shown
    /// under them (#1467); let go off them, it opens in a window of its own centred on the pointer,
    /// and the page under the targets stays.
    #[gpui::test]
    fn the_page_under_the_targets_is_the_one_the_tab_joins(cx: &mut TestAppContext) {
        let (shell, host, card) = shown_alone(cx);
        let right = target_bounds(card, Edge::Right, grown_at(Edge::Right)).center();
        let to = |cx: &mut TestAppContext, at: Point<Pixels>| {
            drag_to(&shell, cx, f32::from(at.x), f32::from(at.y))
        };
        let y = press_to_drag(&shell, cx, 11, true);
        host.take();
        to(cx, right);
        assert_eq!(
            host.take(),
            ["select 10"],
            "the page it joins didn't show under the targets"
        );
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        drag_to(&shell, cx, X, y);
        assert_eq!(
            host.take(),
            ["select 11", "focus page"],
            "back over the sidebar"
        );
        shell.update(cx, |shell, cx| shell.set_selected(11, cx));
        to(cx, right);
        assert_eq!(host.take(), ["select 10"]);
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        host.take_edits();
        shell.update(cx, |shell, cx| shell.drop_row(cx));
        assert_eq!(host.take_edits(), ["split 11 10 false"]);
        assert!(
            host.take().is_empty(),
            "the tab let go on a target showed alone"
        );

        // Over the page before the core has shown the tab pressed, and let go off the targets.
        let (shell, host, card) = shown_alone(cx);
        press_to_drag(&shell, cx, 12, false);
        host.take();
        let middle = card.center();
        drag_to(&shell, cx, f32::from(middle.x), f32::from(middle.y));
        assert_eq!(
            host.take(),
            ["select 10"],
            "the press's switch under way showed the tab"
        );
        shell.update(cx, |shell, cx| shell.drop_row(cx));
        // The recorder's window is at 100, 50 on the screen, and 1200 by 800.
        let (x, y) = (f32::from(middle.x) + 100., f32::from(middle.y) + 50.);
        assert_eq!(
            host.take(),
            [format!("new window for 12 at {x} {y}, held at 600 400")],
            "let go off the targets"
        );
        assert!(host.take_edits().is_empty());
        // Escape over them puts the tab back too.
        press_to_drag(&shell, cx, 11, true);
        drag_to(&shell, cx, f32::from(middle.x), f32::from(middle.y));
        host.take();
        assert!(shell.update(cx, |shell, cx| shell.cancel_drag(cx)));
        assert_eq!(
            host.take(),
            ["select 11", "focus page"],
            "escape over the targets"
        );
    }

    /// In a private window, whose tabs no other window takes, a tab let go off the targets goes
    /// back to its row.
    #[gpui::test]
    fn a_private_windows_tab_let_go_off_the_targets_stays(cx: &mut TestAppContext) {
        let (shell, host, card) = shown_alone(cx);
        shell.update(cx, |shell, _| shell.private = true);
        press_to_drag(&shell, cx, 11, true);
        let middle = card.center();
        drag_to(&shell, cx, f32::from(middle.x), f32::from(middle.y));
        host.take();
        shell.update(cx, |shell, cx| shell.drop_row(cx));
        assert_eq!(host.take(), ["select 11", "focus page"]);
    }

    /// A tab dragged over the page draws the pages at Arc's 0.92 of their size about the card's
    /// centre, with the targets on the card drawn so, and back over the sidebar at their own size
    /// again.
    #[gpui::test]
    fn a_tab_over_the_page_draws_the_pages_smaller(cx: &mut TestAppContext) {
        let (shell, host, card) = shown_alone(cx);
        let settled = |cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| {
                shell.scale_content(cx);
                shell.content_scale.since -= Duration::from_secs(1);
                assert!(!shell.scale_content(cx), "the pages are still on their way");
                (
                    *host.page_scales.borrow().last().unwrap(),
                    shell.scaled_card(),
                )
            })
        };
        let y = press_to_drag(&shell, cx, 11, true);
        let middle = card.center();
        drag_to(&shell, cx, f32::from(middle.x), f32::from(middle.y));
        let (scale, drawn) = settled(cx);
        assert_eq!(scale, 0.92);
        assert_eq!(drawn.center(), card.center());
        assert!((drawn.size.width - card.size.width * 0.92).abs() < px(0.01));
        assert!((drawn.size.height - card.size.height * 0.92).abs() < px(0.01));

        drag_to(&shell, cx, X, y);
        let (scale, drawn) = settled(cx);
        assert_eq!(scale, 1., "back over the sidebar");
        assert_eq!(drawn, card);
    }

    /// The tab shown dragged over its own page splits with the one used before it (Dia), which
    /// shows under the targets too.
    #[gpui::test]
    fn the_tab_shown_dragged_over_its_page_shows_the_one_before_it(cx: &mut TestAppContext) {
        let (shell, host, card) = shown_alone(cx);
        shell.update(cx, |shell, cx| {
            shell.set_selected(12, cx);
            shell.set_selected(10, cx);
        });
        press_to_drag(&shell, cx, 10, true);
        host.take();
        let left = target_bounds(card, Edge::Left, grown_at(Edge::Left)).center();
        drag_to(&shell, cx, f32::from(left.x), f32::from(left.y));
        assert_eq!(host.take(), ["select 12"]);
        shell.update(cx, |shell, cx| shell.set_selected(12, cx));
        host.take_edits();
        shell.update(cx, |shell, cx| shell.drop_row(cx));
        assert_eq!(host.take_edits(), ["split 10 12 true"]);
    }
}
