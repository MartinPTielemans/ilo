//! A split view's row gives its width to the pane with the keyboard, the others folded to their
//! favicons and marks, so one title reads whole where every pane's was cut short (#2340).
//!
//! Each pane has a weight, 1 for the wide one and 0 for the folded: a segment is as wide as its
//! favicon and marks, then its weight of what's left of the row. Weights move on a fixed curve, so
//! the panes trade widths, and wait while the pointer is on the row, so the segment the user aims
//! at doesn't move from under it. A press on a segment refolds the row at once: that's the aim.

use std::collections::HashMap;
use web_time::Instant;

use gpui::{Context, Window};

use super::{Row, Shell, cubic_bezier};
use crate::motion;
use crate::space_icon::has_icon;
use crate::tokens::shell::{
    FAVICON_SIZE, LOADING_SIZE, SEGMENT_END, SEGMENT_FAVICON_X, SEGMENT_MARK_GAP, SPEAKER_SIZE,
};

// The panes trade widths on CSS's ease, as Arc's tab rows make room; a spring would overshoot and
// show the next pane's favicon past its line.
const SEGMENT_FOLD: f32 = 0.12;

/// A split view's row: the pane it's folding to, the pane it was last told to, and the weights it
/// left and when, while it moves.
#[derive(Default)]
pub(super) struct Fold {
    wide: u64,
    seen: u64,
    from: HashMap<u64, f32>,
    start: Option<Instant>,
}

impl Fold {
    /// Each of `panes`' weight at `now`, summing to 1.
    fn weights(&self, panes: &[u64], now: Instant) -> Vec<f32> {
        let to = |pane: u64| f32::from(u8::from(pane == self.wide));
        let eased = self.start.map_or(1., |start| {
            let elapsed = now.saturating_duration_since(start).as_secs_f32();
            cubic_bezier(0.25, 0.1, 0.25, 1., (elapsed / SEGMENT_FOLD).min(1.))
        });
        let weights: Vec<f32> = panes
            .iter()
            .map(|&pane| {
                let to = to(pane);
                let from = self.from.get(&pane).copied().unwrap_or(to);
                from + (to - from) * eased
            })
            .collect();
        // A pane that left mid-motion took its weight with it.
        let sum: f32 = weights.iter().sum();
        if sum > 0. {
            weights.into_iter().map(|weight| weight / sum).collect()
        } else {
            panes.iter().map(|&pane| to(pane)).collect()
        }
    }

    /// Folds to `wide` from where the row is at `now`.
    fn to(&mut self, wide: u64, panes: &[u64], start: Instant, now: Instant) {
        let weights = self.weights(panes, now);
        self.from = panes.iter().copied().zip(weights).collect();
        self.wide = wide;
        self.start = Some(start);
    }
}

impl Shell {
    /// The pane of `panes` the row folds to, in a Space showing `shown`: that one, else the one
    /// shown last, so the row says which comes back while the split isn't shown.
    pub(super) fn wide_pane(&self, panes: &[u64], shown: u64) -> u64 {
        if panes.contains(&shown) {
            return shown;
        }
        self.recent
            .iter()
            .copied()
            .find(|tab| panes.contains(tab))
            .unwrap_or(panes[0])
    }

    /// How wide `tab`'s segment is folded: its favicon, its speaker and its loading dot.
    pub(super) fn folded_width(&self, tab: u64) -> f32 {
        let page = self.page(tab);
        let speaker = if page.audible || page.muted {
            SEGMENT_MARK_GAP + SPEAKER_SIZE
        } else {
            0.
        };
        let loading = if page.loading {
            SEGMENT_MARK_GAP + LOADING_SIZE
        } else {
            0.
        };
        SEGMENT_FAVICON_X + FAVICON_SIZE + speaker + loading + SEGMENT_END
    }

    /// The weight of each of the row's `panes`, which starts at `first`, as drawn now.
    pub(super) fn fold_weights(&self, first: u64, panes: &[u64]) -> Vec<f32> {
        match self.split_folds.get(&first) {
            Some(fold) => fold.weights(panes, Instant::now()),
            None => self.rest_weights(panes, self.selected),
        }
    }

    /// The weight of each of `panes` at rest, in a Space showing `shown`: for a Space's rows
    /// drawn still, as it slides in or on its card.
    pub(super) fn rest_weights(&self, panes: &[u64], shown: u64) -> Vec<f32> {
        let wide = self.wide_pane(panes, shown);
        panes
            .iter()
            .map(|&pane| f32::from(u8::from(pane == wide)))
            .collect()
    }

    /// Moves each split view's row in the Space shown to the pane it folds to, unless the pointer
    /// is on it, and runs frames while one moves.
    pub(super) fn step_split_folds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(space) = self.shown_space() else {
            return;
        };
        // A named split is one plate with no segments to fold.
        let rows: Vec<Vec<u64>> = space
            .splits
            .iter()
            .filter(|split| split.tabs.len() > 1 && split.name.is_empty())
            .filter(|split| !has_icon(&split.icon))
            .map(|split| split.tabs.clone())
            .collect();
        let now = Instant::now();
        let mut moving = false;
        for panes in &rows {
            let first = panes[0];
            let wide = self.wide_pane(panes, self.selected);
            let aimed = self.pointer_row() == Some(Row::Tab(first));
            let fold = self.split_folds.entry(first).or_insert_with(|| Fold {
                wide,
                seen: wide,
                ..Fold::default()
            });
            // Only a change moves it, so a press's fold isn't undone before the core shows its pane.
            if fold.seen != wide && !aimed {
                fold.seen = wide;
                if fold.wide != wide {
                    fold.to(wide, panes, motion::start_at(now, cx), now);
                }
            }
            match fold.start {
                Some(start)
                    if now.saturating_duration_since(start).as_secs_f32() < SEGMENT_FOLD =>
                {
                    moving = true
                }
                _ => fold.start = None,
            }
        }
        self.split_folds
            .retain(|first, _| rows.iter().any(|panes| panes[0] == *first));
        if moving {
            window.request_animation_frame();
        }
    }

    /// A press on `tab`'s segment folds its row to it now, the pointer on it or not.
    pub(super) fn refold(&mut self, tab: u64, cx: &mut Context<Self>) {
        let panes = self.panes(tab);
        let Some(&first) = panes.first() else {
            return;
        };
        let now = Instant::now();
        let start = motion::start_at(now, cx);
        let shown = self.wide_pane(&panes, self.selected);
        let fold = self.split_folds.entry(first).or_insert_with(|| Fold {
            wide: shown,
            seen: shown,
            ..Fold::default()
        });
        if fold.wide != tab {
            fold.to(tab, &panes, start, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn weights_trade_on_the_curve_and_always_sum_to_one() {
        let now = Instant::now();
        let mut fold = Fold {
            wide: 1,
            ..Fold::default()
        };
        assert_eq!(fold.weights(&[1, 2, 3], now), vec![1., 0., 0.]);
        fold.to(2, &[1, 2, 3], now, now);
        let half = fold.weights(&[1, 2, 3], now + Duration::from_millis(60));
        assert!(half[0] > 0. && half[0] < 1. && half[1] > 0. && half[1] < 1.);
        assert_eq!(half[2], 0.);
        assert!((half.iter().sum::<f32>() - 1.).abs() < 1e-5);
        let end = fold.weights(&[1, 2, 3], now + Duration::from_millis(200));
        assert_eq!(end, vec![0., 1., 0.]);
    }

    #[test]
    fn an_interrupted_fold_turns_from_where_it_was() {
        let now = Instant::now();
        let mut fold = Fold {
            wide: 1,
            ..Fold::default()
        };
        fold.to(2, &[1, 2, 3], now, now);
        let at = now + Duration::from_millis(60);
        let was = fold.weights(&[1, 2, 3], at);
        fold.to(3, &[1, 2, 3], at, at);
        let turned = fold.weights(&[1, 2, 3], at);
        for (was, turned) in was.iter().zip(&turned) {
            assert!((was - turned).abs() < 1e-5, "{was} jumped to {turned}");
        }
        assert_eq!(
            fold.weights(&[1, 2, 3], at + Duration::from_millis(200)),
            vec![0., 0., 1.]
        );
    }

    #[test]
    fn a_pane_gone_mid_fold_leaves_the_rest_summing_to_one() {
        let now = Instant::now();
        let mut fold = Fold {
            wide: 1,
            ..Fold::default()
        };
        fold.to(2, &[1, 2, 3], now, now);
        let weights = fold.weights(&[2, 3], now + Duration::from_millis(30));
        assert!((weights.iter().sum::<f32>() - 1.).abs() < 1e-5);
    }
}
