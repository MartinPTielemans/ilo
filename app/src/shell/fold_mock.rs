//! Mock only (#2340, not for merge): a split view's row giving the pane with the keyboard its title
//! and folding the others to their favicons, with PREVIEW_FOLD=1. PREVIEW_SPLIT_FROM=<tab> and
//! PREVIEW_SPLIT_T=<0..1> draw the row that far through focus moving from that tab's pane, on a
//! fixed curve, for the motion mock.

use std::sync::OnceLock;

use gpui::{Context, Div, div, prelude::*, px};

use super::{
    DIVIDER, FAVICON_SIZE, LOADING_SIZE, PLATE_HEIGHT, Row, SEGMENT_END, SEGMENT_FAVICON_X,
    SEGMENT_LINE, SEGMENT_TITLE_GAP, SLEEPING_FAVICON, SPEAKER_SIZE, Shell, cubic_bezier, title,
};

/// Between a folded segment's favicon and each mark after it.
const MARK_GAP: f32 = 4.;

struct Fold {
    on: bool,
    from: Option<u64>,
    t: f32,
    drop: Option<u64>,
}

fn fold() -> &'static Fold {
    static FOLD: OnceLock<Fold> = OnceLock::new();
    FOLD.get_or_init(|| {
        let var = |name| std::env::var(name).ok();
        Fold {
            on: var("PREVIEW_FOLD").is_some(),
            from: var("PREVIEW_SPLIT_FROM").and_then(|tab| tab.parse().ok()),
            t: var("PREVIEW_SPLIT_T")
                .and_then(|t| t.parse().ok())
                .unwrap_or(1.),
            drop: var("PREVIEW_SPLIT_DROP").and_then(|tab| tab.parse().ok()),
        }
    })
}

pub(super) fn on() -> bool {
    fold().on
}

/// PREVIEW_SPLIT_DROP=<tab> rests that tab on the split's row, on its right, as a drag would.
pub(super) fn drop(drop: Option<(u64, bool)>) -> Option<(u64, bool)> {
    drop.or(fold().drop.map(|tab| (tab, false)))
}

impl Shell {
    /// PREVIEW_SPLIT_HOVER=<tab> puts the pointer on that tab's row.
    pub fn mock_hover(&mut self, tab: u64) {
        self.hovered = Some(Row::Tab(tab));
    }

    /// How wide `tab`'s segment is folded: its favicon 7 after the line and 5 before the next, as a
    /// segment's, and room for its speaker and its loading dot after the favicon while it has them.
    pub(super) fn folded_width(&self, tab: u64) -> f32 {
        let page = self.page(tab);
        let speaker = (page.audible || page.muted) as u8 as f32 * (MARK_GAP + SPEAKER_SIZE);
        let loading = page.loading as u8 as f32 * (MARK_GAP + LOADING_SIZE);
        SEGMENT_FAVICON_X + FAVICON_SIZE + speaker + loading + SEGMENT_END
    }

    /// The tab whose segment is wide: the one with the keyboard, else the one that had it last,
    /// else the one a drag would drop.
    pub(super) fn wide_pane(&self, panes: &[u64], dropped: Option<u64>) -> u64 {
        if let Some(tab) = dropped {
            return tab;
        }
        match panes.contains(&self.selected) {
            true => self.selected,
            false => self.row_tab(panes[0]),
        }
    }

    /// Each segment's width, in the order of `tabs`, `total` between them, while focus moves to
    /// `wide`: none for the wide one at rest, which takes what's left.
    pub(super) fn fold_widths(&self, tabs: &[u64], wide: u64, total: f32) -> Vec<Option<f32>> {
        let rest = |wide: u64| -> Vec<f32> {
            let folded: f32 = tabs
                .iter()
                .filter(|&&tab| tab != wide)
                .map(|&tab| self.folded_width(tab))
                .sum();
            tabs.iter()
                .map(|&tab| match tab == wide {
                    true => total - folded,
                    false => self.folded_width(tab),
                })
                .collect()
        };
        let Some(from) = fold().from.filter(|from| tabs.contains(from)) else {
            return tabs
                .iter()
                .map(|&tab| (tab != wide).then(|| self.folded_width(tab)))
                .collect();
        };
        // CSS's ease, as Row motion's 0.13 s spring is in about 115–120 ms, on a fixed curve as a
        // key or click moves it (ux-principles › Under the fingers, physics).
        let t = cubic_bezier(0.25, 0.1, 0.25, 1., fold().t.clamp(0., 1.));
        rest(from)
            .into_iter()
            .zip(rest(wide))
            .map(|(old, new)| Some(old + (new - old) * t))
            .collect()
    }

    /// How much of its title `tab`'s segment shows: all while it's wide, none folded, and on the
    /// way between them as focus moves.
    fn title_shown(&self, tab: u64, wide: u64) -> f32 {
        let t = cubic_bezier(0.25, 0.1, 0.25, 1., fold().t.clamp(0., 1.));
        let at = |wide_tab: u64| if tab == wide_tab { 1. } else { 0. };
        match fold().from {
            Some(from) => at(from) + (at(wide) - at(from)) * t,
            None => at(wide),
        }
    }

    /// A segment of the folding row: its favicon, its speaker and loading dot, and its title while
    /// it's wide, after a line unless it's the `first`.
    pub(super) fn fold_segment(
        &self,
        tab: u64,
        wide: u64,
        width: Option<f32>,
        first: bool,
        focused: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let page = self.page(tab);
        let shown = self.title_shown(tab, wide);
        let icon = div()
            .flex_none()
            .size(px(FAVICON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .when(page.unloaded, |icon| icon.opacity(SLEEPING_FAVICON))
            .child(self.tab_icon(tab));
        let line = div()
            .absolute()
            .left_0()
            .top(px((PLATE_HEIGHT - SEGMENT_LINE) / 2.))
            .w(px(1.))
            .h(px(SEGMENT_LINE))
            .bg(self.palette.faint(DIVIDER));
        let speaker = self
            .speaker(tab, cx)
            .map(|speaker| speaker.ml(px(MARK_GAP)).mr(px(0.)));
        let segment = div()
            .relative()
            .h_full()
            .flex()
            .items_center()
            .overflow_hidden()
            .pl(px(SEGMENT_FAVICON_X))
            .pr(px(SEGMENT_END))
            .when(!first, |segment| segment.child(line))
            .child(icon)
            .children(speaker);
        let segment = match width {
            Some(width) => segment.flex_none().w(px(width)),
            None => segment.flex_1().min_w_0(),
        };
        let segment = match shown > 0. {
            true => segment.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .ml(px(SEGMENT_TITLE_GAP))
                    .opacity(shown)
                    .child(title(
                        &self.palette,
                        self.title(tab),
                        focused || page.developer_mode,
                    )),
            ),
            false => segment,
        };
        segment.children(self.status_dot(tab))
    }
}
