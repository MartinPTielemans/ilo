//! A glyph changing into another on the same thing, such as reload into stop as the tab's page
//! starts loading: the old one fades and shrinks as the new one fades and grows in, as Arc's do.
//! Lucide's check draws itself along its stroke.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Bounds, Hsla, PathBuilder, Transformation, Window, canvas, div, fill, point,
    prelude::*, px, size, svg,
};

use super::{Folders, Page, cubic_bezier, header::reload_glyph, rows::speaker_glyph};

/// What a changing glyph belongs to. Only a change on the same one moves, so a tab switch, which
/// asks for another tab's, shows its glyph at once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Swapped {
    /// The header's reload, stop while the tab's page loads.
    Reload(u64),
    /// The speaker after a tab's favicon, crossed out while it's muted.
    Speaker(u64),
    /// A folder's icon in the sidebar, open or closed.
    Folder(u64),
    /// The URL field's Copy URL, a check for a while after a click.
    CopyUrl,
    /// The arrow before a Space's Clear, while Clear would close its last Today tabs.
    Clear(u64),
}

pub(super) const CHECK: &str = "icons/check.svg";
pub(super) const LINK: &str = "icons/link.svg";
pub(super) const ARROW_DOWN: &str = "icons/arrow-down.svg";
/// No glyph, for one that comes and goes.
pub(super) const NONE: &str = "";
// Both glyphs move from the same moment, linearly, about the glyph's middle: the old one fades
// out and shrinks to 0.6 over 70 ms, and the new one fades in over 40 ms and grows from 0.8 over
// 80 (Arc 1.170 rec at 2×, reload into stop and back).
const SWAP: f32 = 0.08;
const LEAVE: f32 = 0.07;
const COME: f32 = 0.04;
const SHRUNK: f32 = 0.6;
const SMALL: f32 = 0.8;
// The check draws in 250 ms, ease-in-out (ours), while the link leaves as above.
const CHECK_DRAW: f32 = 0.25;
// Lucide's check in its 24 box, from its left end down to its point and up to its right end,
// stroked as the icons are, with round ends and join.
const CHECK_LINE: [(f32, f32); 3] = [(4., 12.), (9., 17.), (20., 6.)];
use crate::tokens::shell::swap::CHECK_BOX;
use crate::tokens::shell::swap::CHECK_STROKE;

struct Swap {
    what: Swapped,
    from: &'static str,
    to: &'static str,
    start: Instant,
}

impl Swap {
    fn elapsed(&self, now: Instant) -> f32 {
        now.saturating_duration_since(self.start).as_secs_f32()
    }
}

/// The glyphs changing now. It's empty but for a moment after a change, so a frame that draws a
/// glyph only looks through an empty list.
#[derive(Default)]
pub(super) struct Swaps(Vec<Swap>);

impl Swaps {
    /// `what`'s glyph changes from `from` to `to`. One changed back before it's done goes back
    /// from where it is.
    pub(super) fn start(
        &mut self,
        what: Swapped,
        from: &'static str,
        to: &'static str,
        now: Instant,
    ) {
        if from == to {
            return;
        }
        let mut start = now;
        if let Some(index) = self.0.iter().position(|swap| swap.what == what) {
            let was = self.0.remove(index);
            if (was.from, was.to) == (to, from) {
                // The glyph coming in leaves from as far as it had come.
                let left = 1. - arrived(was.to, was.elapsed(now));
                let back = left * LEAVE;
                start = now.checked_sub(Duration::from_secs_f32(back)).unwrap_or(now);
            }
        }
        self.0.push(Swap { what, from, to, start });
    }

    /// Starts what a report on `tab`'s page changes: the header's reload or stop, only for the tab
    /// shown, and the tab's speaker, only while it stays shown.
    pub(super) fn page_changed(
        &mut self,
        tab: u64,
        selected: u64,
        before: &Page,
        after: &Page,
        now: Instant,
    ) {
        if tab == selected {
            let (from, to) = (reload_glyph(before.loading), reload_glyph(after.loading));
            self.start(Swapped::Reload(tab), from, to, now);
        }
        let speaker = |page: &Page| page.audible || page.muted;
        if speaker(before) && speaker(after) {
            let (from, to) = (speaker_glyph(before.muted), speaker_glyph(after.muted));
            self.start(Swapped::Speaker(tab), from, to, now);
        }
    }

    /// Starts each folder of the Space shown that opened or closed.
    pub(super) fn folders_changed(&mut self, before: &Folders, after: &Folders, now: Instant) {
        for (&folder, &(_, open)) in &after.folders {
            if let Some(&(_, was)) = before.folders.get(&folder) {
                self.start(Swapped::Folder(folder), folder_glyph(was), folder_glyph(open), now);
            }
        }
    }

    /// Shows `what`'s glyph at once, as on a thing just drawn.
    pub(super) fn end(&mut self, what: Swapped) {
        self.0.retain(|swap| swap.what != what);
    }

    /// Whether `what`'s glyph is changing.
    pub(super) fn changing(&self, what: Swapped) -> bool {
        self.0.iter().any(|swap| swap.what == what)
    }

    /// How far `what`'s glyph has come toward `path`, ease-in-out over the whole change, for the
    /// room a glyph that comes and goes makes as it does. None while it isn't changing.
    pub(super) fn toward(&self, what: Swapped, path: &'static str) -> Option<f32> {
        let swap = self.0.iter().find(|swap| swap.what == what)?;
        let way = ease(swap.elapsed(Instant::now()) / lasts(swap.to));
        Some(if swap.to == path { way } else { 1. - way })
    }

    /// Lets the changes that are done go, and asks for the next frame while one isn't.
    pub(super) fn step(&mut self, window: &mut Window) {
        if self.0.is_empty() {
            return;
        }
        let now = Instant::now();
        self.0.retain(|swap| swap.elapsed(now) < lasts(swap.to));
        if !self.0.is_empty() {
            window.request_animation_frame();
        }
    }

    /// `what`'s glyph, `path`, `side` square in `color`, coming in over the one before while it
    /// changes.
    pub(super) fn glyph(
        &self,
        what: Swapped,
        path: &'static str,
        side: f32,
        color: Hsla,
    ) -> AnyElement {
        let Some(swap) = self.0.iter().find(|swap| swap.what == what && swap.to == path) else {
            return match path {
                NONE => div().flex_none().size(px(side)).into_any_element(),
                _ => svg().path(path).size(px(side)).text_color(color).into_any_element(),
            };
        };
        let elapsed = swap.elapsed(Instant::now());
        let glyph = |path: &'static str, opacity: f32, scale: f32| {
            if path == NONE {
                return div().into_any_element();
            }
            svg()
                .absolute()
                .size_full()
                .path(path)
                .text_color(color.opacity(opacity))
                .with_transformation(Transformation::scale(size(scale, scale)))
                .into_any_element()
        };
        let coming = match path {
            CHECK => drawing_check(ease(elapsed / CHECK_DRAW), color).into_any_element(),
            _ => glyph(path, coming(elapsed), SMALL + (1. - SMALL) * grown(elapsed)),
        };
        let gone = leaving(elapsed);
        div()
            .relative()
            .flex_none()
            .size(px(side))
            .child(glyph(swap.from, 1. - gone, 1. - (1. - SHRUNK) * gone))
            .child(coming)
            .into_any_element()
    }
}

/// A folder's icon, open or closed.
pub(super) fn folder_glyph(open: bool) -> &'static str {
    if open { "icons/folder-open.svg" } else { "icons/folder.svg" }
}

/// How long a change to `to` takes.
fn lasts(to: &'static str) -> f32 {
    match to {
        CHECK => CHECK_DRAW,
        _ => SWAP,
    }
}

/// How far the old glyph has gone, `elapsed` seconds into its change, 0 to 1.
fn leaving(elapsed: f32) -> f32 {
    (elapsed / LEAVE).clamp(0., 1.)
}

/// How far the new glyph has faded in, 0 to 1.
fn coming(elapsed: f32) -> f32 {
    (elapsed / COME).clamp(0., 1.)
}

/// How far the new glyph has grown, 0 to 1.
fn grown(elapsed: f32) -> f32 {
    (elapsed / SWAP).clamp(0., 1.)
}

/// How far a change to `to` has come, `elapsed` seconds in, 0 to 1: the check drawn, or another
/// glyph faded in.
fn arrived(to: &'static str, elapsed: f32) -> f32 {
    match to {
        CHECK => ease(elapsed / CHECK_DRAW),
        _ => coming(elapsed),
    }
}

/// Ease-in-out, exactly 0 and 1 at its ends, which the curve's search only comes near.
fn ease(t: f32) -> f32 {
    match t {
        ..=0. => 0.,
        1.0.. => 1.,
        _ => cubic_bezier(0.42, 0., 0.58, 1., t),
    }
}

/// The points of Lucide's check drawn `way` of its length along, from its left end, in its 24 box.
fn check_line(way: f32) -> Vec<(f32, f32)> {
    let length = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| (bx - ax).hypot(by - ay);
    let total: f32 = CHECK_LINE.windows(2).map(|pair| length(pair[0], pair[1])).sum();
    let mut left = way.clamp(0., 1.) * total;
    let mut line = vec![CHECK_LINE[0]];
    for pair in CHECK_LINE.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let part = length(a, b);
        if left >= part {
            line.push(b);
            left -= part;
            continue;
        }
        if left > 0. {
            let t = left / part;
            line.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
        break;
    }
    line
}

/// The check, drawn `way` of the way along its stroke.
fn drawing_check(way: f32, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let line = check_line(way);
            if line.len() < 2 {
                return;
            }
            let scale = f32::from(bounds.size.width) / CHECK_BOX;
            let at = |(x, y): (f32, f32)| bounds.origin + point(px(x * scale), px(y * scale));
            let width = px(CHECK_STROKE * scale);
            // A stretch at a time and a dot at each point make Lucide's round ends and join, which
            // GPUI's stroke doesn't draw.
            for pair in line.windows(2) {
                let mut path = PathBuilder::stroke(width);
                path.move_to(at(pair[0]));
                path.line_to(at(pair[1]));
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
            for &each in &line {
                let dot = Bounds::centered_at(at(each), size(width, width));
                window.paint_quad(fill(dot, color).corner_radii(width / 2.));
            }
        },
    )
    .absolute()
    .size_full()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        COME, LEAVE, Page, SWAP, Swapped, Swaps, check_line, coming, grown, leaving,
    };

    fn near(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    /// The check starts at its left end, reaches its point 5/16 of its length along, as the
    /// stroke down is 5√2 of its 16√2, then goes up toward the right end, which it reaches at 1.
    #[test]
    fn the_check_draws_from_its_left_end_down_then_up() {
        assert_eq!(check_line(0.), vec![(4., 12.)]);
        let down = check_line(5. / 32.);
        assert_eq!(down.len(), 2);
        assert!(near(down[1], (6.5, 14.5)), "half way down it's at {:?}", down[1]);
        let point = check_line(5. / 16.);
        assert!(near(*point.last().unwrap(), (9., 17.)), "{point:?}");
        let up = check_line(5. / 16. + 11. / 32.);
        assert_eq!(up.len(), 3);
        assert!(near(up[2], (14.5, 11.5)), "half way up it's at {:?}", up[2]);
        assert_eq!(check_line(1.), vec![(4., 12.), (9., 17.), (20., 6.)]);
        assert_eq!(check_line(2.), check_line(1.));
    }

    /// Both glyphs move from the start: the new one has faded in before the old one has gone, and
    /// has grown to full size by the end, as Arc's reload and stop do.
    #[test]
    fn the_old_glyph_leaves_as_the_new_one_comes() {
        assert_eq!((leaving(0.), coming(0.), grown(0.)), (0., 0., 0.));
        let early = SWAP / 8.;
        assert!(leaving(early) > 0. && coming(early) > 0. && grown(early) > 0.);
        assert_eq!(coming(COME), 1.);
        assert!(leaving(COME) < 1. && grown(COME) < 1.);
        assert_eq!((leaving(LEAVE), grown(SWAP)), (1., 1.));
    }

    /// A page starting to load turns the reload of the tab shown into stop, but not a tab behind
    /// it, so switching to that tab shows its glyph at once. A mute changes its speaker's glyph.
    #[test]
    fn only_a_change_on_the_tab_shown_moves_its_reload() {
        let now = Instant::now();
        let idle = Page::default();
        let loading = Page { loading: true, ..Page::default() };
        let mut swaps = Swaps::default();
        swaps.page_changed(2, 1, &idle, &loading, now);
        assert!(!swaps.changing(Swapped::Reload(2)));
        swaps.page_changed(1, 1, &idle, &loading, now);
        assert!(swaps.changing(Swapped::Reload(1)));
        assert!(!swaps.changing(Swapped::Reload(2)));
        swaps.page_changed(1, 1, &loading, &loading, now);
        assert!(swaps.changing(Swapped::Reload(1)), "a report that changes nothing keeps it");

        let audible = Page { audible: true, ..Page::default() };
        let muted = Page { audible: true, muted: true, ..Page::default() };
        swaps.page_changed(3, 1, &idle, &muted, now);
        assert!(!swaps.changing(Swapped::Speaker(3)), "a speaker that just came shows at once");
        swaps.page_changed(3, 1, &audible, &muted, now);
        assert!(swaps.changing(Swapped::Speaker(3)));
    }

    /// Changed back mid-way, a glyph goes back from where it was: the one coming in leaves from
    /// as far as it had come.
    #[test]
    fn a_change_undone_goes_back_from_where_it_is() {
        let start = Instant::now();
        let mut swaps = Swaps::default();
        swaps.start(Swapped::CopyUrl, "a", "b", start);
        let back = start + Duration::from_secs_f32(COME / 2.);
        let came = coming(COME / 2.);
        swaps.start(Swapped::CopyUrl, "b", "a", back);
        let swap = &swaps.0[0];
        assert_eq!((swap.from, swap.to), ("b", "a"));
        let gone = leaving(swap.elapsed(back));
        assert!((1. - gone - came).abs() < 1e-3, "it came {came} and is {} there", 1. - gone);
    }
}
