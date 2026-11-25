//! Transparency, one per Space: how much of what's behind the window comes through its glass while
//! the Space is shown, from solid at 0 to 1, the clearest at which text still holds its floor over a
//! white window behind, which is today's More. It's kept in the Space's theme text, which the core
//! keeps and syncs with the Space, as `transparency=0.400`, so a Space with no colour keeps a text
//! with that key alone. A build before this one reads the text as it always has: its theme reader
//! skips a key it doesn't know, and a text without the main colour's place is no colour, as before.
//!
//! Today's level, the app's one, carries into every Space once, at the first Spaces a window is
//! given after the launch has read the level: while no Space keeps a value, each takes the level's
//! place on the bar. A Space keeping one is the mark that it's done, so nothing more is read at
//! launch than the level and the Spaces, which it reads already. The level's own file is left as
//! it is, so a build before this one reads the same level after the move as before it. What it
//! can't keep: an older build that changes a Space's colour writes the text without the value, and
//! the Space then comes back at the start value, or at today's level again if every Space did.

use crate::tokens::space_transparency::{LESS as SPACE_LESS, START as SPACE_START};
use crate::transparency::Transparency;

/// The key in a Space's theme text.
const KEY: &str = "transparency";

/// The value a Space comes with, and where Reset puts it: the look of today's Standard.
pub const START: f32 = SPACE_START;

/// Where `level` sits on the bar, so a Space carried over at it looks as the app did.
pub fn of_level(level: Transparency) -> f32 {
    match level {
        Transparency::Less => SPACE_LESS,
        Transparency::Standard => SPACE_START,
        Transparency::More => 1.,
    }
}

/// The value a Space's theme `text` keeps, or None where it keeps none.
pub fn read(text: &str) -> Option<f32> {
    text.split_whitespace()
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == KEY)
        .and_then(|(_, value)| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0., 1.))
}

/// `text` without its value, as the theme is logged.
pub fn without(text: &str) -> String {
    let pairs = text.split_whitespace();
    let kept = pairs.filter(|pair| pair.split_once('=').is_none_or(|(key, _)| key != KEY));
    kept.collect::<Vec<_>>().join(" ")
}

/// `text` keeping `value` in place of any it kept, or none.
pub fn with(text: &str, value: Option<f32>) -> String {
    let mut text = without(text);
    if let Some(value) = value {
        if !text.is_empty() {
            text.push(' ');
        }
        text += &format!("{KEY}={:.3}", value.clamp(0., 1.));
    }
    text
}

/// The window's black over its material in dark, or its white in light, at `value` on the bar:
/// solid at 0, then through today's three levels where they sit, so a carried Space is unchanged.
pub fn tint(value: f32, dark: bool) -> f32 {
    let stops = [
        (0., 1.),
        (SPACE_LESS, Transparency::Less.tint(dark)),
        (SPACE_START, Transparency::Standard.tint(dark)),
        (1., Transparency::More.tint(dark)),
    ];
    let value = value.clamp(0., 1.);
    let [.., (_, clearest)] = stops;
    let mut pairs = stops.windows(2);
    pairs
        .find(|pair| value <= pair[1].0)
        .map_or(clearest, |pair| {
            let [(from, at_from), (to, at_to)] = [pair[0], pair[1]];
            at_from + (at_to - at_from) * (value - from) / (to - from)
        })
}

/// The texts `spaces` (each its id and theme text) take as today's `level` carries into them: every
/// one, while none keeps a value; once one does, the carry is done and there's nothing.
pub fn carry_in(level: Transparency, spaces: &[(u64, String)]) -> Vec<(u64, String)> {
    if spaces.iter().any(|(_, text)| read(text).is_some()) {
        return Vec::new();
    }
    let value = Some(of_level(level));
    let carried = spaces.iter().map(|(id, text)| (*id, with(text, value)));
    carried.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn a_value_is_kept_beside_the_theme_and_read_back() {
        let theme = Theme::at(0.3, -0.5).to_text();
        let kept = with(&theme, Some(0.6));
        assert_eq!(read(&kept), Some(0.6));
        assert_eq!(without(&kept), theme);
        assert_eq!(
            with(&kept, Some(0.25)),
            format!("{theme} transparency=0.250")
        );
        assert_eq!(with(&kept, None), theme);
        assert_eq!(with("", Some(0.4)), "transparency=0.400");
        assert_eq!(read(&theme), None);
        assert_eq!(read("transparency=7"), Some(1.));
        assert_eq!(read("transparency=clear"), None);
    }

    /// What a build before this one reads: the theme it read before, and no colour where there was
    /// none, as its reader is today's without this module.
    #[test]
    fn an_older_build_reads_each_space_as_before() {
        let theme = Theme::at(0.3, -0.5);
        assert_eq!(
            Theme::parse(&with(&theme.to_text(), Some(0.6))).map(|theme| theme.to_text()),
            Some(theme.to_text())
        );
        assert!(Theme::parse(&with("", Some(0.6))).is_none());
    }

    #[test]
    fn the_bar_meets_todays_levels_where_they_sit() {
        for dark in [true, false] {
            for level in Transparency::ALL {
                let at = tint(of_level(level), dark);
                assert!((at - level.tint(dark)).abs() < 1e-6, "{level:?} {dark}");
            }
            assert_eq!(tint(0., dark), 1., "solid at 0");
            assert_eq!(tint(-1., dark), 1.);
            assert_eq!(
                tint(2., dark),
                Transparency::More.tint(dark),
                "never clearer"
            );
            let steps: Vec<f32> = (0..=100).map(|n| tint(n as f32 / 100., dark)).collect();
            assert!(
                steps.windows(2).all(|pair| pair[1] <= pair[0]),
                "clearer all the way"
            );
        }
        assert_eq!(START, of_level(Transparency::Standard));
    }

    /// Today's level reaches every Space, and once one keeps a value, as each does after it, the
    /// carry is done: the next launch, with the Spaces as the core kept them, carries nothing.
    #[test]
    fn todays_level_carries_into_every_space_once() {
        let colored = Theme::at(0.3, -0.5).to_text();
        let spaces = vec![(1, colored.clone()), (2, String::new())];
        let carried = carry_in(Transparency::More, &spaces);
        assert_eq!(
            carried,
            [
                (1, format!("{colored} transparency=1.000")),
                (2, "transparency=1.000".to_string()),
            ]
        );
        assert!(carry_in(Transparency::More, &carried).is_empty(), "once");
        let one = vec![(1, colored), (2, with("", Some(0.15)))];
        assert!(
            carry_in(Transparency::Less, &one).is_empty(),
            "a value of its own"
        );
    }

    #[test]
    fn standard_carries_as_the_start_value() {
        let carried = carry_in(Transparency::Standard, &[(4, String::new())]);
        assert_eq!(carried, [(4, with("", Some(START)))]);
    }
}
