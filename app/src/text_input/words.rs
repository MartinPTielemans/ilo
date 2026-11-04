//! Where a field's words start and end, as a Mac text field finds them for ⌥← ⌥→ ⌥⌫ ⌥⌦ and a
//! double-click: UAX #29's word segments, each a word if it holds a letter, number, mark or
//! symbol. So `example.com` and `can't` are one word, `=` and an emoji are words, and spaces and
//! punctuation such as `/`, `:`, `?` and `。` aren't. The tests hold AppKit's own answers, from
//! NSAttributedString's `nextWord(from:forward:)` and `doubleClick(at:)` (#1385). AppKit splits
//! Japanese and Chinese by a dictionary, which UAX #29 doesn't, so here kanji and hiragana go a
//! character at a time and a katakana run is one word.

use std::ops::Range;

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};
use unicode_segmentation::UnicodeSegmentation;

fn is_word(segment: &str) -> bool {
    segment.chars().any(|char| {
        matches!(
            char.general_category_group(),
            GeneralCategoryGroup::Letter
                | GeneralCategoryGroup::Number
                | GeneralCategoryGroup::Mark
                | GeneralCategoryGroup::Symbol
        )
    })
}

/// `text`'s word segments, as byte ranges, with whether each is a word.
fn segments(text: &str) -> impl DoubleEndedIterator<Item = (Range<usize>, bool)> + '_ {
    let segments = text.split_word_bound_indices();
    segments.map(|(at, segment)| (at..at + segment.len(), is_word(segment)))
}

fn words(text: &str) -> impl DoubleEndedIterator<Item = Range<usize>> + '_ {
    segments(text).filter_map(|(range, word)| word.then_some(range))
}

/// Where ⌥← goes from `offset`: the start of the word it's in or of the one before it, else the
/// text's start.
pub(super) fn start_before(text: &str, offset: usize) -> usize {
    words(text)
        .rev()
        .map(|word| word.start)
        .find(|&start| start < offset)
        .unwrap_or(0)
}

/// Where ⌥→ goes from `offset`, and Ctrl+→ on Linux: the end of the word it's in or of the
/// one after it, else the text's end.
pub(super) fn end_after(text: &str, offset: usize) -> usize {
    words(text)
        .map(|word| word.end)
        .find(|&end| end > offset)
        .unwrap_or(text.len())
}

/// Where Ctrl+→ goes from `offset` on Windows: the start of the next word, else the text's end.
pub(super) fn next_start(text: &str, offset: usize) -> usize {
    words(text)
        .map(|word| word.start)
        .find(|&start| start > offset)
        .unwrap_or(text.len())
}

/// What a double-click at `offset` selects: the word there, or the run of spaces or the mark of
/// punctuation, and past the text's end, its last. With `trailing_spaces`, as on Windows, a word
/// takes the spaces after it too.
pub(super) fn at(text: &str, offset: usize, trailing_spaces: bool) -> Range<usize> {
    let mut segments = segments(text).peekable();
    while let Some((range, word)) = segments.next() {
        if range.end <= offset && range.end < text.len() {
            continue;
        }
        let spaces = segments
            .peek()
            .filter(|(next, _)| text[next.clone()].trim().is_empty());
        return match spaces {
            Some((spaces, _)) if word && trailing_spaces => range.start..spaces.end,
            _ => range,
        };
    }
    0..0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where repeated moves go from one end, as `nextWord(from:forward:)` answered for each.
    fn walk(text: &str, step: fn(&str, usize) -> usize, from: usize) -> Vec<usize> {
        let mut at = from;
        let mut stops = Vec::new();
        loop {
            let next = step(text, at);
            if next == at {
                return stops;
            }
            stops.push(next);
            at = next;
        }
    }

    fn back(text: &str) -> Vec<usize> {
        walk(text, start_before, text.len())
    }

    fn forward(text: &str) -> Vec<usize> {
        walk(text, end_after, 0)
    }

    /// AppKit's own stops, in UTF-16 units, which these texts share with bytes.
    #[test]
    fn option_arrows_stop_where_a_mac_fields_do() {
        assert_eq!(forward("example.com/some/path"), [11, 16, 21]);
        assert_eq!(back("example.com/some/path"), [17, 12, 0]);
        assert_eq!(forward("hello world  foo"), [5, 11, 16]);
        assert_eq!(back("hello world  foo"), [13, 6, 0]);
        assert_eq!(
            (forward("can't stop"), back("can't stop")),
            (vec![5, 10], vec![6, 0])
        );
        assert_eq!(
            (forward("3.14 pi"), back("3.14 pi")),
            (vec![4, 7], vec![5, 0])
        );
        assert_eq!(
            (forward("foo_bar-baz"), back("foo_bar-baz")),
            (vec![7, 11], vec![8, 0])
        );
        assert_eq!(
            forward("https://www.example.com/a?b=c"),
            [5, 23, 25, 27, 28, 29]
        );
        assert_eq!(
            back("https://www.example.com/a?b=c"),
            [28, 27, 26, 24, 8, 0]
        );
        assert_eq!(forward("a+b $5 x"), [1, 2, 3, 5, 6, 8]);
        assert_eq!((forward(" x "), back(" x ")), (vec![2, 3], vec![1, 0]));
        assert_eq!(
            (forward("e.g. this"), back("e.g. this")),
            (vec![3, 9], vec![5, 0])
        );
    }

    /// Bytes, where AppKit counts UTF-16: "über" is 5 bytes and 4 units, the thumb with its tone
    /// 8 and 4, and "テキスト" 12 and 4.
    #[test]
    fn a_word_is_whole_characters_in_any_script() {
        assert_eq!(
            (forward("über café"), back("über café")),
            (vec![5, 11], vec![6, 0])
        );
        assert_eq!(forward("👍🏽 ok"), [8, 11]);
        assert_eq!(forward("テキスト「本」"), [12, 18, 21]);
    }

    #[test]
    fn ctrl_right_on_windows_goes_to_the_next_words_start() {
        assert_eq!(walk("hello world  foo", next_start, 0), [6, 13, 16]);
        assert_eq!(walk("example.com/some/path", next_start, 0), [12, 17, 21]);
    }

    /// What `doubleClick(at:)` printed for each offset, as `start+length`.
    fn double_clicks(text: &str) -> String {
        let each = (0..text.len()).map(|offset| at(text, offset, false));
        each.map(|word| format!("{}+{}", word.start, word.len()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_double_click_selects_the_word_the_spaces_or_the_mark_under_it() {
        let clicks = "0+5 0+5 0+5 0+5 0+5 5+1 6+5 6+5 6+5 6+5 6+5 11+2 11+2 13+3 13+3 13+3";
        assert_eq!(double_clicks("hello world  foo"), clicks);
        assert_eq!(
            double_clicks("e.g. this"),
            "0+3 0+3 0+3 3+1 4+1 5+4 5+4 5+4 5+4"
        );
        assert_eq!(
            double_clicks("foo_bar-baz"),
            "0+7 0+7 0+7 0+7 0+7 0+7 0+7 7+1 8+3 8+3 8+3"
        );
        assert_eq!(at("https://www.example.com", 5, false), 5..6);
        assert_eq!(at("https://www.example.com", 9, false), 8..23);
        assert_eq!(at("hello world  foo", 16, false), 13..16);
        assert_eq!(at("", 0, false), 0..0);
    }

    #[test]
    fn a_double_click_on_windows_takes_the_spaces_after_a_word() {
        assert_eq!(at("hello world  foo", 0, true), 0..6);
        assert_eq!(at("hello world  foo", 7, true), 6..13);
        assert_eq!(at("hello world  foo", 11, true), 11..13);
        assert_eq!(at("hello world  foo", 14, true), 13..16);
        assert_eq!(at("a/b", 0, true), 0..1);
    }
}
