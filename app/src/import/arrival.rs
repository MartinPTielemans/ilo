//! The arrival: while an import runs, each Space it makes, or fills, is a card in its own colours
//! under the profile it goes into (#1673). A card comes as the core makes its Space, its count
//! rolls up as its pinned tabs go in, at most once a roll, and its badge ticks once they're in;
//! until then a dot breathes there. Under it, rows for what the core adds after the Spaces:
//! favorites, history, whose count is the one read, as the core reports only its end, and
//! addresses. Under Reduce Motion nothing moves: cards and counts fade in place, the dot holds and
//! the check fades in whole.
//!
//! Every frame it asks for is for motion under way, which ends once the import's work does.

use std::time::Instant;

use gpui::{
    Div, FontWeight, HighlightStyle, Hsla, Rgba, SharedString, StyledText, div, linear_color_stop,
    linear_gradient, prelude::*, px, rgb, svg,
};

use super::Look;
use super::plan::{Add, Existing, Plan, ProfileRef, Target};
use crate::settings_rows::{end, frame, glyph_plate, group};
use crate::shell::cubic_bezier;
use crate::shell::swap::drawing_check;
use crate::space_icon::SpaceIcon;
use crate::tokens::radius;
use crate::tokens::settings::{DETAIL_LINE, DETAIL_TOP, ROW_X};
use crate::tokens::shell::import::motion::{BREATH, CHECK};
use crate::tokens::shell::import::{
    BADGE, BADGE_INSET, CARD_COUNT, CARD_COUNT_ALPHA, CARD_COUNT_LINE, CARD_GAP, CARD_GLYPH,
    CARD_HEIGHT, CARD_INSET, CARD_NAME, CARD_NAME_LINE, CARD_TINT_BOTTOM, CARD_TINT_BOTTOM_LIGHT,
    CARD_TINT_TOP, CARD_TINT_TOP_LIGHT, CARD_WIDTH, CARD_WIDTH_SETTINGS, PROFILE_GAP, PROFILE_NAME,
    PROFILE_NAME_GAP, PROFILE_NAME_LINE,
};
use crate::tokens::shell::task::motion::SHIMMER;
use crate::tokens::shell::task::motion::{CHIPS, CHIPS_RISE, CHIPS_STAGGER, EASE, ROLL, ROLL_RISE};
use crate::tokens::shell::task::{
    SHIMMER_FROM, SHIMMER_HIGH, SHIMMER_LOW, SHIMMER_REACH, SHIMMER_TO,
};
use crate::tokens::text;

/// A Space the import makes, or fills, as its card shows it.
#[derive(Clone, Debug)]
pub struct Card {
    pub name: String,
    /// As a Space keeps it: an emoji, `glyph:` and one of ours, or empty.
    pub icon: String,
    /// Its theme's main colour, if the import gives it one.
    pub color: Option<u32>,
    /// The profile it goes into, by name, and whether the import makes it.
    pub profile: (String, bool),
    /// Its pinned tabs, and how many are in.
    pub total: usize,
    pub placed: usize,
    /// When it came, as the core made its Space, after the cards before it.
    pub came: Instant,
    /// When its last tab went in.
    pub done: Option<Instant>,
    /// The count shown, and when it last rolled, from what.
    shown: usize,
    rolled: Option<(Instant, usize)>,
}

impl Card {
    /// The count it shows at `now`: how many of its tabs are in, but changing at most once a roll,
    /// so a fast import reads as a few steps, with the roll under way, if one is, from the count
    /// before and how far it's gone.
    pub fn count_at(&mut self, now: Instant) -> (usize, Option<(usize, f32)>) {
        let rolled_long_ago = self
            .rolled
            .is_none_or(|(at, _)| now.duration_since(at).as_secs_f32() >= ROLL);
        if self.placed != self.shown && rolled_long_ago {
            self.rolled = Some((now, self.shown));
            self.shown = self.placed;
        }
        let rolling = self.rolled.and_then(|(at, from)| {
            let t = now.duration_since(at).as_secs_f32() / ROLL;
            (t < 1.).then_some((from, t))
        });
        (self.shown, rolling)
    }

    /// Whether it still moves at `now`: coming, rolling, ticking, or waiting with its dot.
    pub fn moving(&self, now: Instant) -> bool {
        let since = |at: Instant| now.saturating_duration_since(at).as_secs_f32();
        since(self.came) < CHIPS
            || self.placed != self.shown
            || self.rolled.is_some_and(|(at, _)| since(at) < ROLL)
            || self.done.is_none_or(|at| since(at) < CHECK)
    }
}

/// A card for each Space `plan` makes or fills, in the order the core makes them, from `came`,
/// each `chips-stagger` after the one before; its profile from `existing`'s, or the plan's new
/// ones.
pub fn cards(plan: &Plan, existing: &Existing, came: Instant) -> Vec<Card> {
    let profile = |profile: ProfileRef| match profile {
        ProfileRef::Existing(id) => (existing.profiles.iter())
            .find(|(each, _)| *each == id)
            .map_or((String::new(), false), |(_, name)| (name.clone(), false)),
        ProfileRef::New(at) => (plan.new_profiles.get(at).cloned().unwrap_or_default(), true),
    };
    let stagger = std::time::Duration::from_secs_f32(CHIPS_STAGGER);
    (plan.spaces.iter().enumerate())
        .map(|(at, space)| {
            let (name, icon, color, profile) = match &space.into {
                Target::New {
                    name,
                    profile: into,
                    icon,
                    theme,
                } => (
                    name.clone(),
                    icon.clone(),
                    theme.as_ref().map(|theme| theme.main_color()),
                    profile(*into),
                ),
                Target::Existing(id) => {
                    let here = existing.spaces.iter().find(|each| each.id == *id);
                    let name = here.map_or(String::new(), |here| here.name.clone());
                    let into = here.map_or(ProfileRef::Existing(0), |here| {
                        ProfileRef::Existing(here.profile)
                    });
                    (name, String::new(), None, profile(into))
                }
            };
            Card {
                name,
                icon,
                color,
                profile,
                total: tabs(&space.pinned),
                placed: 0,
                came: came + stagger * at as u32,
                done: None,
                shown: 0,
                rolled: None,
            }
        })
        .collect()
}

/// How many tabs `adds` pins, in its folders too.
fn tabs(adds: &[Add]) -> usize {
    adds.iter()
        .map(|add| match add {
            Add::Tab(_) => 1,
            Add::Folder { adds, .. } => tabs(adds),
        })
        .sum()
}

/// The motion's ease `t` of the way through.
fn eased(t: f32) -> f32 {
    cubic_bezier(EASE[0], EASE[1], EASE[2], EASE[3], t.clamp(0., 1.))
}

/// The check draw's curve, CSS's ease-in-out (ux-spec, Check draw).
fn ease_in_out(t: f32) -> f32 {
    cubic_bezier(0.42, 0., 0.58, 1., t.clamp(0., 1.))
}

/// A waiting dot's strength `phase` of the way through its breath: from 0.35 up to full half way
/// and back, as a loading tab's status dot's.
fn breath(phase: f32) -> f32 {
    let low = crate::tokens::shell::BREATH_LOW;
    low + (1. - low) * (1. - (std::f32::consts::TAU * phase).cos()) / 2.
}

/// The cards, a group a profile, each under its name and whether the import makes it, `width`
/// wide: the first launch's on its ground, Settings' in a group of its own.
pub fn draw(cards: &mut [Card], look: &Look, settings: bool, still: bool, now: Instant) -> Div {
    let mut groups: Vec<((String, bool), Vec<Div>)> = Vec::new();
    for card in cards.iter_mut() {
        let drawn = card_view(card, look, settings, still, now);
        match groups
            .iter_mut()
            .find(|(profile, _)| *profile == card.profile)
        {
            Some((_, group)) => group.push(drawn),
            None => groups.push((card.profile.clone(), vec![drawn])),
        }
    }
    let groups = groups.into_iter().map(|((name, new), cards)| {
        let made = if new { "new profile" } else { "existing" };
        let label = format!("{name} · {made}");
        let named = StyledText::new(label).with_highlights([(
            0..name.len(),
            HighlightStyle {
                color: Some(look.ink.into()),
                font_weight: Some(FontWeight::MEDIUM),
                ..Default::default()
            },
        )]);
        div()
            .flex()
            .flex_col()
            .gap(px(PROFILE_NAME_GAP))
            .child(
                div()
                    .text_size(px(PROFILE_NAME))
                    .line_height(px(PROFILE_NAME_LINE))
                    .text_color(look.detail)
                    .child(named),
            )
            .child(div().flex().flex_wrap().gap(px(CARD_GAP)).children(cards))
    });
    let cards = div()
        .flex()
        .flex_wrap()
        .gap(px(PROFILE_GAP))
        .children(groups);
    match settings {
        true => div()
            .p(px(ROW_X))
            .rounded(px(radius::ROW))
            .bg(look.surface)
            .when_some(look.edge, |box_, edge| box_.border_1().border_color(edge))
            .child(cards),
        false => cards,
    }
}

/// How far under its place a card that `came` is drawn at `now`, and how far it has faded in:
/// rising `chips-rise` into place as it fades in, or fading in place under Reduce Motion.
fn pose(came: Instant, now: Instant, still: bool) -> (f32, f32) {
    let t = match now >= came {
        true => eased(now.duration_since(came).as_secs_f32() / CHIPS),
        false => 0.,
    };
    let rise = if still { 0. } else { CHIPS_RISE * (1. - t) };
    (rise, t)
}

/// One card: its colours from top to bottom, its glyph and badge, and its name over its count,
/// rising into place as it comes.
fn card_view(card: &mut Card, look: &Look, settings: bool, still: bool, now: Instant) -> Div {
    let since = |at: Instant| now.saturating_duration_since(at).as_secs_f32();
    let (rise, came) = pose(card.came, now, still);
    let width = if settings {
        CARD_WIDTH_SETTINGS
    } else {
        CARD_WIDTH
    };
    let base: Rgba = card.color.map_or(look.detail, rgb);
    let (top, bottom) = match look.light() {
        true => (CARD_TINT_TOP_LIGHT, CARD_TINT_BOTTOM_LIGHT),
        false => (CARD_TINT_TOP, CARD_TINT_BOTTOM),
    };
    let tint = |by: f32| -> Hsla { look.surface.blend(base.opacity(by)).into() };
    let glyph = match SpaceIcon::parse(&card.icon) {
        SpaceIcon::Emoji(emoji) => div()
            .text_size(px(CARD_GLYPH - 2.))
            .line_height(px(CARD_GLYPH))
            .child(emoji.to_string())
            .into_any_element(),
        icon => match icon.glyph_path() {
            Some(path) => svg()
                .path(path)
                .size(px(CARD_GLYPH))
                .text_color(look.ink)
                .into_any_element(),
            None => div().size(px(CARD_GLYPH)).into_any_element(),
        },
    };
    let (count, rolling) = card.count_at(now);
    let said = |count: usize| match card.done {
        _ if count == 0 && card.done.is_none() => "—".to_string(),
        _ => format!("{count} pinned"),
    };
    let count_line = |said: String| {
        div()
            .text_size(px(CARD_COUNT))
            .line_height(px(CARD_COUNT_LINE))
            .text_color(look.ink.opacity(CARD_COUNT_ALPHA))
            .child(said)
    };
    // The old count leaves upward as the new one comes from under it, or crossfades in place.
    let counted = match rolling {
        Some((from, t)) => {
            let t = eased(t);
            let lift = if still { 0. } else { ROLL_RISE };
            div()
                .relative()
                .h(px(CARD_COUNT_LINE))
                .child(
                    count_line(said(from))
                        .absolute()
                        .top(px(-lift * t))
                        .opacity(1. - t),
                )
                .child(
                    count_line(said(count))
                        .absolute()
                        .top(px(lift * (1. - t)))
                        .opacity(t),
                )
        }
        None => count_line(said(count)),
    };
    let badge = match card.done {
        Some(at) => {
            let way = ease_in_out(since(at) / CHECK);
            let tick = match still {
                true => drawing_check(1., gpui::white().into()).into_any_element(),
                false => drawing_check(way, gpui::white().into()).into_any_element(),
            };
            div()
                .size(px(BADGE))
                .rounded_full()
                .bg(look.accent)
                .relative()
                .when(still, |badge| badge.opacity(way))
                .child(div().absolute().inset(px(BADGE * 0.2)).child(tick))
        }
        None => {
            let strength = match still {
                true => 1.,
                false => breath((since(card.came) / BREATH).fract()),
            };
            div()
                .size(px(BADGE))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(BADGE * 0.4))
                        .rounded_full()
                        .bg(look.ink.opacity(strength)),
                )
        }
    };
    div()
        .relative()
        .top(px(rise))
        .opacity(came)
        .w(px(width))
        .h(px(CARD_HEIGHT))
        .flex_none()
        .rounded(px(radius::ROW))
        .bg(linear_gradient(
            180.,
            linear_color_stop(tint(top), 0.),
            linear_color_stop(tint(bottom), 1.),
        ))
        .border_1()
        .border_color(look.line)
        .child(
            div()
                .absolute()
                .top(px(CARD_INSET))
                .left(px(CARD_INSET))
                .child(glyph),
        )
        .child(
            div()
                .absolute()
                .top(px(BADGE_INSET))
                .right(px(BADGE_INSET))
                .child(badge),
        )
        .child(
            div()
                .absolute()
                .left(px(CARD_INSET))
                .right(px(CARD_INSET))
                .bottom(px(CARD_INSET))
                .child(
                    div()
                        .truncate()
                        .text_size(px(CARD_NAME))
                        .line_height(px(CARD_NAME_LINE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(look.ink)
                        .child(card.name.clone()),
                )
                .child(counted),
        )
}

/// Where one of what the core adds after the Spaces stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    /// Not yet: the Spaces come first.
    Waiting,
    /// The core is adding them, from what was read: "Adding 12,408 pages…".
    Adding(SharedString),
    /// In, with what came: "11,960".
    In(SharedString),
}

/// A row for what comes after the Spaces: its glyph and name, the line that says where it goes,
/// and where it stands at its end, the words shimmering while the core adds them.
pub fn stage_row(
    look: &Look,
    glyph: &'static str,
    name: &'static str,
    line: &'static str,
    stage: &Stage,
    still: bool,
    now: Instant,
    since: Instant,
) -> Div {
    let quiet = look.detail;
    let value = match stage {
        Stage::Waiting => div()
            .flex()
            .items_center()
            .gap(px(ROW_X / 2.))
            .text_color(quiet)
            .child("Not yet")
            .child(div().size(px(BADGE * 0.4)).rounded_full().bg(quiet)),
        Stage::Adding(said) => {
            let phase = (now.saturating_duration_since(since).as_secs_f32() / SHIMMER).fract();
            let words = shimmering(said, look, phase, still);
            div()
                .flex()
                .items_center()
                .gap(px(ROW_X / 2.))
                .text_color(quiet)
                .child(words)
                .child(div().size(px(BADGE * 0.4)).rounded_full().bg(look.ink))
        }
        Stage::In(said) => div()
            .flex()
            .items_center()
            .gap(px(ROW_X / 2.))
            .child(said.clone())
            .child(
                div()
                    .size(px(BADGE))
                    .rounded_full()
                    .bg(look.accent)
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .inset(px(BADGE * 0.2))
                            .child(drawing_check(1., gpui::white().into())),
                    ),
            ),
    };
    let line = div()
        .mt(px(DETAIL_TOP))
        .text_size(px(text::SMALL))
        .line_height(px(DETAIL_LINE))
        .text_color(look.detail)
        .child(line);
    frame()
        .child(glyph_plate(look, glyph, true))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(name))
                .child(line),
        )
        .child(end([value.into_any_element()]))
}

/// `said` with a highlight sweeping along it, a character at a time, as a task's status while it
/// works; still, under Reduce Motion.
fn shimmering(said: &SharedString, look: &Look, phase: f32, still: bool) -> StyledText {
    if still {
        return StyledText::new(said.clone());
    }
    let width = said.chars().count().max(1) as f32;
    let middle = width * (SHIMMER_FROM + (SHIMMER_TO - SHIMMER_FROM) * phase);
    let highlights = said.char_indices().enumerate().map(|(at, (byte, c))| {
        let near = (1. - (at as f32 - middle).abs() / (SHIMMER_REACH * width)).max(0.);
        let alpha = SHIMMER_LOW + (SHIMMER_HIGH - SHIMMER_LOW) * near;
        (
            byte..byte + c.len_utf8(),
            HighlightStyle {
                color: Some(look.ink.opacity(alpha).into()),
                ..Default::default()
            },
        )
    });
    StyledText::new(said.clone()).with_highlights(highlights.collect::<Vec<_>>())
}

/// The rows after the cards: favorites, history and addresses, each that the import brings.
pub fn stages(look: &Look, rows: Vec<Div>) -> Option<Div> {
    (!rows.is_empty()).then(|| group(look, rows))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Card, ROLL, pose};

    fn card(placed: usize, total: usize, now: Instant) -> Card {
        Card {
            name: "Home".into(),
            icon: String::new(),
            color: None,
            profile: ("Default".into(), false),
            total,
            placed,
            came: now,
            done: None,
            shown: 0,
            rolled: None,
        }
    }

    /// A count changes at most once a roll: tabs going in faster show as a few steps, each the
    /// count then, and the last one shows once the roll before it is over.
    #[test]
    fn a_count_changes_at_most_once_a_roll() {
        let start = Instant::now();
        let at = |seconds: f32| start + Duration::from_secs_f32(seconds);
        let mut card = card(2, 9, start);
        assert_eq!(card.count_at(at(0.)).0, 2);
        card.placed = 5;
        assert_eq!(
            card.count_at(at(ROLL / 2.)).0,
            2,
            "it rolled again mid-roll"
        );
        card.placed = 9;
        assert_eq!(card.count_at(at(ROLL * 0.9)).0, 2);
        let (shown, rolling) = card.count_at(at(ROLL));
        assert_eq!(shown, 9, "the count skipped to where the tabs are");
        assert_eq!(rolling.map(|(from, _)| from), Some(2));
        assert_eq!(card.count_at(at(ROLL * 3.)), (9, None));
    }

    /// A card rises into place as it fades in; under Reduce Motion it fades in place, and its
    /// count, its check and its dot don't move either.
    #[test]
    fn under_reduce_motion_no_card_moves() {
        let start = Instant::now();
        let (rise, faded) = pose(start, start, false);
        assert!(rise > 3.9 && faded < 1e-3, "{rise} {faded}");
        let (rise, faded) = pose(start, start, true);
        assert!(
            rise == 0. && faded < 1e-3,
            "it moved under Reduce Motion: {rise} {faded}"
        );
        let later = start + Duration::from_secs(1);
        assert_eq!(pose(start, later, false), (0., 1.));
        assert_eq!(pose(start, later, true), (0., 1.));
    }
}
