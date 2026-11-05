//! The icon picker's Emoji tab: the emoji it lists, in Unicode's order under their groups after the
//! ones used last, the search that narrows them, and the numbers its layout is drawn from. The
//! layout is *interim* until Arc's Emoji tab is captured (docs/ux-spec.md › Spaces › Icon), and
//! these are the numbers a capture changes. The columns, pitch and plates are the Icon tab's.

use std::{collections::HashSet, ops::Range};

use emojis::{Emoji, Group, SkinTone};

use super::COLUMNS;

pub use crate::tokens::icon_picker::emoji::SEARCH_HEIGHT;
pub use crate::tokens::icon_picker::emoji::SEARCH_PADDING;
pub use crate::tokens::icon_picker::emoji::SEARCH_RADIUS;
pub use crate::tokens::icon_picker::emoji::SEARCH_TOP;
/// The list starts 6 under the field and ends 6 over the card's bottom, so its rows stop short of
/// the corners' curve.
pub const LIST_TOP: f32 = SEARCH_TOP + SEARCH_HEIGHT + 6.;
pub use crate::tokens::icon_picker::emoji::EMOJI_SIZE;
pub use crate::tokens::icon_picker::emoji::HEADER_BOTTOM;
pub use crate::tokens::icon_picker::emoji::HEADER_LEFT;
pub use crate::tokens::icon_picker::emoji::HEADER_TEXT;
pub use crate::tokens::icon_picker::emoji::LIST_BOTTOM;
pub use crate::tokens::icon_picker::emoji::TONE_EMOJI;
pub use crate::tokens::icon_picker::emoji::TONE_PLATE;
/// How many emoji the picker remembers as used last.
pub const RECENTS: usize = 16;

/// The skin tones the grid offers, in Unicode's order: one each, none of the mixed ones a pair's
/// two people can have.
pub const TONES: [SkinTone; 6] = [
    SkinTone::Default,
    SkinTone::Light,
    SkinTone::MediumLight,
    SkinTone::Medium,
    SkinTone::MediumDark,
    SkinTone::Dark,
];

/// The hand the tone button shows for `tone`.
pub fn hand(tone: SkinTone) -> &'static str {
    let hand = emojis::get("✋").expect("✋ is in the table");
    hand.with_skin_tone(tone).unwrap_or(hand).as_str()
}

/// The line that keeps the skin tone among the recents, which a build without tones reads as no
/// emoji and leaves out.
const TONE_LINE: &str = "skin tone ";

/// The emoji picked last, newest first, kept as one emoji to a line, and the skin tone the grid
/// shows its emoji in, as its place in `TONES`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recents(Vec<String>, usize);

impl Recents {
    /// Reads what `to_text` wrote, leaving out what the table doesn't know.
    pub fn parse(text: &str) -> Self {
        let lines = text.lines().map(str::trim);
        let tone = (lines.clone())
            .find_map(|line| line.strip_prefix(TONE_LINE)?.parse::<usize>().ok())
            .filter(|&index| index < TONES.len())
            .unwrap_or(0);
        let each = lines.filter(|line| emojis::get(line).is_some());
        Recents(each.take(RECENTS).map(str::to_string).collect(), tone)
    }

    pub fn to_text(&self) -> String {
        let emoji = self.0.iter().map(|emoji| format!("{emoji}\n"));
        let tone = (self.1 != 0).then(|| format!("{TONE_LINE}{}\n", self.1));
        tone.into_iter().chain(emoji).collect()
    }

    pub fn tone(&self) -> SkinTone {
        TONES[self.1]
    }

    pub fn set_tone(&mut self, tone: SkinTone) {
        self.1 = TONES.iter().position(|&each| each == tone).unwrap_or(0);
    }

    /// Puts `emoji` first, once.
    pub fn push(&mut self, emoji: &str) {
        self.0.retain(|each| each != emoji);
        self.0.insert(0, emoji.to_string());
        self.0.truncate(RECENTS);
    }

    fn emojis(&self) -> impl Iterator<Item = &'static Emoji> {
        self.0.iter().filter_map(|emoji| emojis::get(emoji))
    }
}

/// The emoji the system can draw. The table knows the newest Unicode's, which an older system's
/// font draws as a missing glyph, or as its parts when it's a sequence, so the grid offers only
/// these. Only the emoji the grid lists are asked about, once, and a skin tone's when the grid
/// first shows it.
pub struct Drawable(HashSet<&'static str>, HashSet<u8>);

impl Drawable {
    /// Asks `draws` about each emoji the grid lists.
    pub fn new(draws: impl Fn(&str) -> bool) -> Self {
        let base = emojis::iter().filter(|emoji| is_base(emoji));
        let drawn = base
            .map(Emoji::as_str)
            .filter(|emoji| draws(emoji))
            .collect();
        Drawable(drawn, HashSet::from([SkinTone::Default as u8]))
    }

    /// Asks `draws` about each emoji in `tone`, the first time.
    pub fn ask_tone(&mut self, tone: SkinTone, draws: impl Fn(&str) -> bool) {
        if !self.1.insert(tone as u8) {
            return;
        }
        let toned = emojis::iter()
            .filter(|emoji| is_base(emoji))
            .filter_map(|emoji| {
                emoji
                    .with_skin_tone(tone)
                    .filter(|toned| toned.skin_tone() == Some(tone))
            });
        self.0
            .extend(toned.map(Emoji::as_str).filter(|emoji| draws(emoji)));
    }

    fn has(&self, emoji: &Emoji) -> bool {
        self.0.contains(emoji.as_str())
    }
}

/// One row of the list: a header, or up to `COLUMNS` emoji, as a range of the grid's items.
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Header(&'static str),
    Cells(Range<usize>),
}

/// What the Emoji tab lists for a search: its emoji in order, and the rows they fall in.
#[derive(Default)]
pub struct Grid {
    pub items: Vec<&'static Emoji>,
    pub rows: Vec<Row>,
}

impl Grid {
    /// The recently used, then every group in Unicode's order, for no search; else the emoji whose
    /// name or shortcode has each word searched for, with no headers. Only `drawable` emoji: a
    /// recent one a newer system saved stays in `recents`, and shows again where that system is.
    pub fn new(query: &str, recents: &Recents, drawable: &Drawable) -> Self {
        let mut grid = Grid {
            items: Vec::new(),
            rows: Vec::new(),
        };
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let tone = recents.tone();
        let toned = |emoji: &'static Emoji| in_tone(emoji, tone, drawable);
        if words.is_empty() {
            grid.section(
                "Recently Used",
                recents.emojis().filter(|emoji| drawable.has(emoji)),
            );
            for group in Group::iter() {
                let each = group
                    .emojis()
                    .filter(|emoji| is_base(emoji) && drawable.has(emoji));
                grid.section(group_name(group), each.map(toned));
            }
        } else {
            let found = emojis::iter()
                .filter(|emoji| is_base(emoji) && drawable.has(emoji) && matches(emoji, &words));
            grid.section("", found.map(toned));
        }
        grid
    }

    fn section(&mut self, title: &'static str, emojis: impl Iterator<Item = &'static Emoji>) {
        let start = self.items.len();
        self.items.extend(emojis);
        if self.items.len() == start {
            return;
        }
        if !title.is_empty() {
            self.rows.push(Row::Header(title));
        }
        for row in (start..self.items.len()).step_by(COLUMNS) {
            self.rows
                .push(Row::Cells(row..(row + COLUMNS).min(self.items.len())));
        }
    }

    /// The first item that is `emoji`.
    pub fn position(&self, emoji: &str) -> Option<usize> {
        self.items.iter().position(|each| each.as_str() == emoji)
    }

    /// The row `item` is in.
    pub fn row_of(&self, item: usize) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| matches!(row, Row::Cells(cells) if cells.contains(&item)))
    }

    /// The item a step from `from` lands on: along the items for `across`, which goes on to the
    /// next row at a row's end, or down and up for `down`, keeping the column as far as the row
    /// goes. `from` none steps onto the first item. Stays where it is at either end.
    pub fn step(&self, from: Option<usize>, across: isize, down: isize) -> Option<usize> {
        let Some(from) = from else {
            return (!self.items.is_empty()).then_some(0);
        };
        let last = self.items.len().checked_sub(1)?;
        if across != 0 {
            return Some(from.saturating_add_signed(across).min(last));
        }
        let row = self.row_of(from)?;
        let Row::Cells(here) = &self.rows[row] else {
            return Some(from);
        };
        let column = from - here.start;
        let mut rows: Box<dyn Iterator<Item = &Row>> = if down > 0 {
            Box::new(self.rows[row + 1..].iter())
        } else {
            Box::new(self.rows[..row].iter().rev())
        };
        match rows.find_map(|row| match row {
            Row::Cells(cells) => Some(cells),
            Row::Header(_) => None,
        }) {
            Some(there) => Some((there.start + column).min(there.end - 1)),
            None => Some(from),
        }
    }
}

/// Emoji with no skin tone to pick, or in the one every emoji starts as, which the grid lists in
/// the tone picked.
fn is_base(emoji: &Emoji) -> bool {
    emoji
        .skin_tone()
        .is_none_or(|tone| tone == SkinTone::Default)
}

/// `emoji` in `tone` if it has one and the system draws it, else as it is.
fn in_tone(emoji: &'static Emoji, tone: SkinTone, drawable: &Drawable) -> &'static Emoji {
    (emoji.with_skin_tone(tone))
        .filter(|toned| toned.skin_tone() == Some(tone) && drawable.has(toned))
        .unwrap_or(emoji)
}

/// Whether each of `words` (lowercase) is in the emoji's name or one of its shortcodes.
fn matches(emoji: &Emoji, words: &[String]) -> bool {
    let name = emoji.name().to_lowercase();
    words.iter().all(|word| {
        name.contains(word.as_str())
            || emoji
                .shortcodes()
                .any(|code| code.replace('_', " ").contains(word.as_str()))
    })
}

fn group_name(group: Group) -> &'static str {
    match group {
        Group::SmileysAndEmotion => "Smileys & Emotion",
        Group::PeopleAndBody => "People & Body",
        Group::AnimalsAndNature => "Animals & Nature",
        Group::FoodAndDrink => "Food & Drink",
        Group::TravelAndPlaces => "Travel & Places",
        Group::Activities => "Activities",
        Group::Objects => "Objects",
        Group::Symbols => "Symbols",
        Group::Flags => "Flags",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Drawable {
        Drawable::new(|_| true)
    }

    #[test]
    fn the_groups_come_in_unicodes_order_after_the_recents() {
        let recents = Recents::parse("🦊\n🌱\n");
        let grid = Grid::new("", &recents, &all());
        let headers: Vec<_> = grid
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::Header(title) => Some(*title),
                Row::Cells(_) => None,
            })
            .collect();
        assert_eq!(
            headers[..3],
            ["Recently Used", "Smileys & Emotion", "People & Body"]
        );
        assert_eq!(headers.last(), Some(&"Flags"));
        assert_eq!(grid.items[0].as_str(), "🦊");
        assert_eq!(grid.items[1].as_str(), "🌱");
        // 😀 grinning face, Unicode's first, is the first of the Smileys.
        assert_eq!(grid.items[2].as_str(), "😀");
    }

    #[test]
    fn no_recents_leave_no_header() {
        let grid = Grid::new("", &Recents::default(), &all());
        assert_eq!(grid.rows[0], Row::Header("Smileys & Emotion"));
    }

    #[test]
    fn the_grid_has_no_skin_tones_and_no_components() {
        let grid = Grid::new("", &Recents::default(), &all());
        assert!(grid.items.len() > 1_500, "{}", grid.items.len());
        assert!(grid.position("👍").is_some());
        assert!(grid.position("👍🏽").is_none());
        assert!(
            grid.items
                .iter()
                .all(|emoji| !emoji.name().contains("skin tone"))
        );
    }

    #[test]
    fn a_tone_shows_each_emoji_that_has_one_in_it_and_the_rest_as_they_are() {
        let mut recents = Recents::default();
        recents.set_tone(SkinTone::MediumDark);
        let mut drawable = all();
        drawable.ask_tone(SkinTone::MediumDark, |_| true);
        let toned = Grid::new("", &recents, &drawable);
        assert!(toned.position("👍🏾").is_some());
        assert!(toned.position("👍").is_none());
        assert!(toned.position("🦊").is_some());
        // The same emoji, one for one, so the plate keeps its place.
        assert_eq!(
            toned.items.len(),
            Grid::new("", &Recents::default(), &all()).items.len()
        );
        assert!(
            Grid::new("thumbs", &recents, &drawable)
                .position("👍🏾")
                .is_some()
        );
    }

    #[test]
    fn a_tone_the_system_cant_draw_leaves_the_emoji_as_they_are() {
        let mut recents = Recents::default();
        recents.set_tone(SkinTone::Dark);
        let mut drawable = all();
        drawable.ask_tone(SkinTone::Dark, |emoji| !emoji.starts_with('👍'));
        let grid = Grid::new("", &recents, &drawable);
        assert!(grid.position("👍").is_some());
        assert!(grid.position("👋🏿").is_some());
    }

    #[test]
    fn the_tone_is_kept_with_the_recents_and_an_old_build_skips_it() {
        let mut recents = Recents::parse("🦊\n");
        assert_eq!(recents.to_text(), "🦊\n");
        recents.set_tone(SkinTone::Light);
        assert_eq!(recents.to_text(), "skin tone 1\n🦊\n");
        let read = Recents::parse(&recents.to_text());
        assert_eq!(read.tone(), SkinTone::Light);
        assert_eq!(read, recents);
        // An old build reads only the emoji, as `parse` did before tones.
        let text = recents.to_text();
        let old: Vec<_> = (text.lines())
            .filter(|line| emojis::get(line).is_some())
            .collect();
        assert_eq!(old, ["🦊"]);
        assert_eq!(Recents::parse("skin tone 9\n").tone(), SkinTone::Default);
    }

    #[test]
    fn a_search_matches_names_and_shortcodes() {
        let grid = Grid::new("fox", &Recents::default(), &all());
        assert_eq!(grid.items[0].as_str(), "🦊");
        let grid = Grid::new("  GRINNING  face ", &Recents::default(), &all());
        assert_eq!(grid.items[0].as_str(), "😀");
        // A shortcode that isn't in the name.
        let grid = Grid::new("+1", &Recents::default(), &all());
        assert!(grid.position("👍").is_some());
        assert!(
            Grid::new("zzzzzz", &Recents::default(), &all())
                .items
                .is_empty()
        );
        // Found emoji have no headers.
        assert!(grid.rows.iter().all(|row| matches!(row, Row::Cells(_))));
    }

    #[test]
    fn rows_hold_eight_and_cover_every_item_once() {
        let grid = Grid::new("", &Recents::parse("🦊"), &all());
        let mut next = 0;
        for row in &grid.rows {
            if let Row::Cells(cells) = row {
                assert_eq!(cells.start, next);
                assert!(cells.len() <= COLUMNS);
                next = cells.end;
            }
        }
        assert_eq!(next, grid.items.len());
    }

    #[test]
    fn steps_go_along_rows_and_keep_the_column_up_and_down() {
        let grid = Grid::new("smil", &Recents::default(), &all());
        let last = grid.items.len() - 1;
        assert_eq!(grid.step(None, 1, 0), Some(0));
        assert_eq!(grid.step(Some(0), 1, 0), Some(1));
        assert_eq!(grid.step(Some(0), -1, 0), Some(0));
        assert_eq!(grid.step(Some(last), 1, 0), Some(last));
        assert_eq!(grid.step(Some(3), 0, 1), Some(3 + COLUMNS));
        assert_eq!(grid.step(Some(3 + COLUMNS), 0, -1), Some(3));
        assert_eq!(grid.step(Some(3), 0, -1), Some(3));
        assert_eq!(grid.step(Some(last), 0, 1), Some(last));
        // Down from a column the last row doesn't reach lands on its last item.
        let first_of_last_row = last - last % COLUMNS;
        if first_of_last_row >= COLUMNS && last % COLUMNS < COLUMNS - 1 {
            assert_eq!(grid.step(Some(first_of_last_row - 1), 0, 1), Some(last));
        }
    }

    #[test]
    fn steps_skip_the_headers_between_groups() {
        let grid = Grid::new("", &Recents::default(), &all());
        let people = grid
            .rows
            .iter()
            .position(|row| *row == Row::Header("People & Body"))
            .unwrap();
        let Row::Cells(last_smileys) = &grid.rows[people - 1] else {
            panic!("not a row")
        };
        let Row::Cells(first_people) = &grid.rows[people + 1] else {
            panic!("not a row")
        };
        assert_eq!(
            grid.step(Some(last_smileys.start), 0, 1),
            Some(first_people.start)
        );
        assert_eq!(
            grid.step(Some(first_people.start), 0, -1),
            Some(last_smileys.start)
        );
    }

    #[test]
    fn a_pick_goes_first_once_and_the_list_keeps_sixteen() {
        let mut recents = Recents::default();
        recents.push("🦊");
        recents.push("🌱");
        recents.push("🦊");
        assert_eq!(recents.to_text(), "🦊\n🌱\n");
        for emoji in emojis::iter().take(30) {
            recents.push(emoji.as_str());
        }
        assert_eq!(recents.0.len(), RECENTS);
        assert_eq!(Recents::parse(&recents.to_text()), recents);
    }

    /// A system whose font stops at Unicode 13: it draws no emoji added after, a sequence of them
    /// included, which `host_can_draw_emoji` finds out from the font.
    fn unicode_13(emoji: &str) -> bool {
        emojis::get(emoji).is_some_and(|emoji| emoji.unicode_version().major() <= 13)
    }

    #[test]
    fn the_grid_offers_only_what_the_system_draws() {
        // 🫠 melting face is Unicode 14, and 🐦‍🔥 phoenix a 15.1 sequence of 🐦 and 🔥.
        let (melting, phoenix) = (emojis::get("🫠").unwrap(), emojis::get("🐦‍🔥").unwrap());
        assert!(phoenix.as_str().contains('\u{200d}'));
        let old = Drawable::new(unicode_13);
        let recents = Recents::parse("🫠\n🦊\n🐦‍🔥\n");
        let grid = Grid::new("", &recents, &old);
        assert!(grid.position(melting.as_str()).is_none());
        assert!(grid.position(phoenix.as_str()).is_none());
        assert!(grid.items.iter().all(|emoji| unicode_13(emoji.as_str())));
        // The recents that draw are left, first.
        assert_eq!(grid.items[0].as_str(), "🦊");
        assert_eq!(grid.rows[0], Row::Header("Recently Used"));
        // Searching doesn't find them either, while a newer system does.
        assert!(Grid::new("melting", &recents, &old).items.is_empty());
        assert!(
            Grid::new("melting", &recents, &all())
                .position(melting.as_str())
                .is_some()
        );
        // The whole is still most of the list.
        assert!(grid.items.len() > 1_400, "{}", grid.items.len());
    }

    #[test]
    fn what_the_system_cant_draw_stays_in_the_recents() {
        let mut recents = Recents::parse("🫠\n");
        let old = Drawable::new(unicode_13);
        assert_eq!(
            Grid::new("", &recents, &old).rows[0],
            Row::Header("Smileys & Emotion")
        );
        // Picking another doesn't lose it, so a newer system sharing the profile still lists it.
        recents.push("🦊");
        assert_eq!(recents.to_text(), "🦊\n🫠\n");
        assert_eq!(Grid::new("", &recents, &all()).position("🫠"), Some(1));
    }

    #[test]
    fn the_system_is_asked_once_about_each_emoji_the_grid_lists() {
        let asked = std::cell::Cell::new(0);
        let drawable = Drawable::new(|_| {
            asked.set(asked.get() + 1);
            true
        });
        let listed = Grid::new("", &Recents::default(), &drawable).items.len();
        // Searches and the whole list again ask nothing more.
        Grid::new("fox", &Recents::default(), &drawable);
        Grid::new("", &Recents::default(), &drawable);
        assert_eq!(asked.get(), listed);
    }

    #[test]
    fn recents_leave_out_what_the_table_doesnt_know() {
        assert_eq!(
            Recents::parse("🦊\nabc\n\n日\n"),
            Recents(vec!["🦊".to_string()], 0)
        );
    }
}
