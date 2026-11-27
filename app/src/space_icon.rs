//! A Space's icon as the core keeps it: an emoji, or `glyph:` and the name of one of the icon
//! picker's glyphs, or nothing for the dot (docs/ux-spec.md › Spaces › Icon).

/// What a Space keeps before a glyph's name.
const GLYPH: &str = "glyph:";

/// The icon picker's glyphs in Arc's order, eight to a row: the name a Space keeps for each, and
/// its Lucide outline (NOTICE), or ours on Lucide's grid where it has none. The names are ours,
/// so a glyph can be redrawn without changing what Spaces keep.
pub const GLYPHS: [(&str, &str); 56] = [
    ("star", "icons/star.svg"),
    ("bookmark", "icons/bookmark.svg"),
    ("heart", "icons/heart.svg"),
    ("flag", "icons/flag.svg"),
    ("bolt", "icons/zap.svg"),
    ("triangle", "icons/triangle.svg"),
    ("asterisk", "icons/asterisk.svg"),
    ("bell", "icons/bell.svg"),
    ("lightbulb", "icons/lightbulb.svg"),
    ("shapes", "icons/shapes.svg"),
    ("squares", "icons/grid-2x2.svg"),
    ("dots", "icons/grip.svg"),
    ("layers", "icons/layers.svg"),
    ("database", "icons/database.svg"),
    ("trays", "icons/gallery-vertical-end.svg"),
    ("two-squares", "icons/copy.svg"),
    ("folder", "icons/folder.svg"),
    ("tray", "icons/inbox.svg"),
    ("calendar", "icons/calendar-days.svg"),
    ("envelope", "icons/mail.svg"),
    ("checkbox", "icons/square-check.svg"),
    ("page", "icons/file.svg"),
    ("book", "icons/book-open.svg"),
    ("chat", "icons/message-circle-more.svg"),
    ("people", "icons/users.svg"),
    ("terminal", "icons/square-terminal.svg"),
    ("tools", "icons/wrench.svg"),
    ("square", "icons/square.svg"),
    ("egg", "icons/egg.svg"),
    ("circle", "icons/circle.svg"),
    ("moon", "icons/moon.svg"),
    ("sun", "icons/sun.svg"),
    ("planet", "icons/planet.svg"),
    ("leaf", "icons/leaf.svg"),
    ("cloud", "icons/cloud.svg"),
    ("paw", "icons/paw-print.svg"),
    ("basket", "icons/shopping-basket.svg"),
    ("gift", "icons/gift.svg"),
    ("bed", "icons/bed.svg"),
    ("fork-knife", "icons/utensils.svg"),
    ("dumbbell", "icons/dumbbell.svg"),
    ("airplane", "icons/plane.svg"),
    ("music", "icons/eighth-note.svg"),
    ("palette", "icons/palette.svg"),
    ("video", "icons/video.svg"),
    ("bandage", "icons/bandage.svg"),
    ("code", "icons/code.svg"),
    ("baseball", "icons/baseball.svg"),
    ("cloud-outline", "icons/cloudy.svg"),
    ("map", "icons/map.svg"),
    ("campfire", "icons/flame.svg"),
    ("pizza", "icons/pizza.svg"),
    ("skull", "icons/skull.svg"),
    ("receipt", "icons/receipt.svg"),
    ("thumbs-up", "icons/thumbs-up.svg"),
    ("train", "icons/train-front.svg"),
];

/// A Space's icon, read from what the core keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpaceIcon<'a> {
    /// The dot, for no icon, or a glyph this build doesn't have, such as one a later build added.
    Dot,
    Emoji(&'a str),
    /// The glyph at this index in `GLYPHS`.
    Glyph(usize),
}

impl<'a> SpaceIcon<'a> {
    pub fn parse(icon: &'a str) -> Self {
        match icon.strip_prefix(GLYPH) {
            Some(name) => GLYPHS
                .iter()
                .position(|(each, _)| *each == name)
                .map_or(SpaceIcon::Dot, SpaceIcon::Glyph),
            None if icon.is_empty() => SpaceIcon::Dot,
            None => SpaceIcon::Emoji(icon),
        }
    }

    /// The glyph's drawing, if it's a glyph.
    pub fn glyph_path(self) -> Option<&'static str> {
        match self {
            SpaceIcon::Glyph(index) => Some(GLYPHS[index].1),
            _ => None,
        }
    }
}

/// The first letter of a Space's `name`, uppercased, which marks a Space with no glyph, or with
/// an emoji, which marks are no longer drawn as (#4400). What the Space keeps is left as it is.
pub fn letter(name: &str) -> String {
    (name.trim().chars().next())
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default()
}

/// A Space's mark, drawn `size` across in `ink`: its glyph, else its name's first letter in the
/// UI font. Every place that marks a Space draws it so (#4400).
pub fn mark(icon: &str, name: &str, size: f32, ink: gpui::Hsla) -> gpui::AnyElement {
    use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, px, svg};
    match SpaceIcon::parse(icon).glyph_path() {
        Some(path) => svg()
            .path(path)
            .size(px(size))
            .flex_none()
            .text_color(ink)
            .into_any_element(),
        None => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(size * crate::tokens::shell::SPACE_LETTER))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(ink)
            .child(letter(name))
            .into_any_element(),
    }
}

/// Whether a Space's or a tab's `icon`, as the core keeps it, is one the sidebar draws: the one
/// test the picker's trash, Remove Icon and the name's place go by, so none offers to remove what
/// isn't there. A glyph this build doesn't know draws the dot, so it counts as none.
pub fn has_icon(icon: &str) -> bool {
    SpaceIcon::parse(icon) != SpaceIcon::Dot
}

/// What a Space keeps for the glyph at `index` in `GLYPHS`.
pub fn glyph(index: usize) -> String {
    format!("{GLYPH}{}", GLYPHS[index].0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_round_trips_through_what_a_space_keeps() {
        for index in 0..GLYPHS.len() {
            assert_eq!(SpaceIcon::parse(&glyph(index)), SpaceIcon::Glyph(index));
        }
        assert_eq!(glyph(4), "glyph:bolt");
    }

    #[test]
    fn an_unknown_glyph_draws_the_dot() {
        assert_eq!(SpaceIcon::parse("glyph:unicorn"), SpaceIcon::Dot);
        assert_eq!(SpaceIcon::parse("glyph:"), SpaceIcon::Dot);
        assert_eq!(SpaceIcon::parse(""), SpaceIcon::Dot);
    }

    #[test]
    fn only_what_draws_is_an_icon() {
        assert!(has_icon("🦊"));
        assert!(has_icon("glyph:moon"));
        assert!(!has_icon(""));
        assert!(!has_icon("glyph:unicorn"));
    }

    #[test]
    fn anything_else_is_an_emoji() {
        assert_eq!(SpaceIcon::parse("🦊"), SpaceIcon::Emoji("🦊"));
    }

    #[test]
    fn every_name_is_kept_once() {
        let mut names: Vec<_> = GLYPHS.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), GLYPHS.len());
    }
}
