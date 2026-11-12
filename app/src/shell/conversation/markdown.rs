//! The Markdown an agent writes, as its conversation shows it: paragraphs, bullet and numbered
//! lists, bold, italic, inline code and links, a heading as a bold paragraph. The rest shows as
//! written. A line break in a paragraph stays one, as a chat's does.

use std::ops::Range;

use gpui::SharedString;

/// A block of a message.
#[derive(Debug, PartialEq)]
pub(super) enum Block {
    Paragraph(Inline),
    /// A list's item: how deep it nests, its marker, "•" or "2.", and its text.
    Item {
        depth: usize,
        marker: SharedString,
        text: Inline,
    },
}

/// A block's text, with the stretches of it that are styled and those that link.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Inline {
    pub text: String,
    /// In order, apart, and never plain.
    pub styled: Vec<(Range<usize>, Style)>,
    pub links: Vec<(Range<usize>, SharedString)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub link: bool,
}

/// How far a list in a list is in, at most, in levels.
const DEPTH: usize = 3;

/// `source`'s blocks: paragraphs apart at blank lines, and list items, each from its marker to the
/// next, with the indented lines under it.
pub(super) fn blocks(source: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut item: Option<(usize, SharedString, Vec<&str>)> = None;
    for line in source.lines() {
        let text = line.trim_start();
        let indent = line.len() - text.len();
        let text = text.trim_end();
        let rule = text.len() >= 3
            && (text.chars().all(|char| char == '-') || text.chars().all(|char| char == '*'));
        if text.is_empty() || rule {
            flush(&mut blocks, &mut paragraph, &mut item);
        } else if let Some((marker, rest)) = list_marker(text) {
            flush(&mut blocks, &mut paragraph, &mut item);
            item = Some(((indent / 2).min(DEPTH), marker, vec![rest]));
        } else if let Some(heading) = heading(text) {
            flush(&mut blocks, &mut paragraph, &mut item);
            blocks.push(Block::Paragraph(inline(
                heading,
                Style {
                    bold: true,
                    ..Style::default()
                },
            )));
        } else if let Some((_, _, lines)) = item.as_mut().filter(|_| indent > 0) {
            lines.push(text);
        } else {
            // A line at the margin after a list is a paragraph after it, as an agent's question
            // under its options is.
            flush(&mut blocks, &mut Vec::new(), &mut item);
            paragraph.push(text);
        }
    }
    flush(&mut blocks, &mut paragraph, &mut item);
    blocks
}

/// The paragraph or the item gathered so far, as its block.
fn flush<'a>(
    blocks: &mut Vec<Block>,
    paragraph: &mut Vec<&'a str>,
    item: &mut Option<(usize, SharedString, Vec<&'a str>)>,
) {
    if !paragraph.is_empty() {
        blocks.push(Block::Paragraph(inline(
            &paragraph.join("\n"),
            Style::default(),
        )));
        paragraph.clear();
    }
    if let Some((depth, marker, lines)) = item.take() {
        blocks.push(Block::Item {
            depth,
            marker,
            text: inline(&lines.join("\n"), Style::default()),
        });
    }
}

/// A list item's marker as it shows, and its text: "- ", "* ", "+ " or "• " is a bullet, and
/// "3. " or "3) " a number.
fn list_marker(line: &str) -> Option<(SharedString, &str)> {
    for bullet in ["- ", "* ", "+ ", "• "] {
        if let Some(rest) = line.strip_prefix(bullet) {
            return Some(("•".into(), rest.trim_start()));
        }
    }
    let digits = line.find(|char: char| !char.is_ascii_digit())?;
    let rest = line[digits..]
        .strip_prefix(". ")
        .or_else(|| line[digits..].strip_prefix(") "))?;
    (1..=9)
        .contains(&digits)
        .then(|| (format!("{}.", &line[..digits]).into(), rest.trim_start()))
}

/// A heading's text, without its #s.
fn heading(line: &str) -> Option<&str> {
    let hashes = line.find(|char: char| char != '#')?;
    let rest = line[hashes..].strip_prefix(' ')?;
    (1..=6)
        .contains(&hashes)
        .then(|| rest.trim().trim_end_matches('#').trim_end())
}

/// `source`'s text in `style`, with the bold, italic, code and links it marks.
pub(super) fn inline(source: &str, style: Style) -> Inline {
    let mut inline = Inline::default();
    spans(source, style, &mut inline);
    inline
}

impl Inline {
    fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        if style == Style::default() {
            return;
        }
        match self.styled.last_mut() {
            Some((range, last)) if *last == style && range.end == start => range.end = end,
            _ => self.styled.push((start..end, style)),
        }
    }
}

fn spans(source: &str, style: Style, out: &mut Inline) {
    let (mut plain, mut at) = (0, 0);
    while at < source.len() {
        let rest = &source[at..];
        let before = source[..at].chars().next_back();
        // What starts here, as the text it shows, its style, the address it links to, and its
        // length in the source.
        let found = escaped(rest)
            .map(|(text, length)| (Shown::Text(text), style, None, length))
            .or_else(|| {
                let (text, length) = code(rest)?;
                Some((
                    Shown::Text(text),
                    Style {
                        code: true,
                        ..style
                    },
                    None,
                    length,
                ))
            })
            .or_else(|| {
                let (text, emphasis, length) = emphasis(rest, before)?;
                Some((Shown::Marked(text), emphasis.apply(style), None, length))
            })
            .or_else(|| {
                let (label, url, length) = link(rest).filter(|_| !style.link)?;
                Some((
                    Shown::Marked(label),
                    Style {
                        link: url.is_some(),
                        ..style
                    },
                    url,
                    length,
                ))
            })
            .or_else(|| {
                let (url, length) = bare_link(rest, before).filter(|_| !style.link)?;
                Some((
                    Shown::Text(url),
                    Style {
                        link: true,
                        ..style
                    },
                    Some(url.into()),
                    length,
                ))
            });
        let Some((shown, inner, url, length)) = found else {
            at += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        out.push(&source[plain..at], style);
        let start = out.text.len();
        match shown {
            Shown::Text(text) => out.push(text, inner),
            Shown::Marked(text) => spans(text, inner, out),
        }
        if let Some(url) = url {
            out.links.push((start..out.text.len(), url));
        }
        at += length;
        plain = at;
    }
    out.push(&source[plain..], style);
}

/// What a mark shows: text as it is, or text with marks of its own.
enum Shown<'a> {
    Text(&'a str),
    Marked(&'a str),
}

#[derive(Clone, Copy)]
enum Emphasis {
    Bold,
    Italic,
}

impl Emphasis {
    fn apply(self, style: Style) -> Style {
        match self {
            Emphasis::Bold => Style {
                bold: true,
                ..style
            },
            Emphasis::Italic => Style {
                italic: true,
                ..style
            },
        }
    }
}

/// A backslash's punctuation, as itself: "\*".
fn escaped(rest: &str) -> Option<(&str, usize)> {
    let char = rest
        .strip_prefix('\\')?
        .chars()
        .next()
        .filter(char::is_ascii_punctuation)?;
    Some((&rest[1..1 + char.len_utf8()], 1 + char.len_utf8()))
}

/// Inline code: "`text`".
fn code(rest: &str) -> Option<(&str, usize)> {
    let inner = rest.strip_prefix('`')?;
    let end = inner.find('`').filter(|&end| end > 0)?;
    Some((&inner[..end], end + 2))
}

/// "**bold**", "__bold__", "*italic*", "_italic_" or "***both***": its text, which, and its
/// length. An underscore within a word, as in snake_case, is itself, and a mark needs text against
/// it on the inside.
fn emphasis(rest: &str, before: Option<char>) -> Option<(&str, Emphasis, usize)> {
    let mark = rest
        .chars()
        .next()
        .filter(|char| matches!(char, '*' | '_'))?;
    let run = |text: &str| text.len() - text.trim_start_matches(mark).len();
    let opening = run(rest);
    // Three is bold, with the third an italic's inside it.
    let (width, emphasis) = match opening {
        1 => (1, Emphasis::Italic),
        _ => (2, Emphasis::Bold),
    };
    let word = |char: Option<char>| char.is_some_and(char::is_alphanumeric);
    let inner = &rest[width..];
    let text = inner.trim_start_matches(mark);
    if (mark == '_' && word(before)) || text.is_empty() || text.starts_with(char::is_whitespace) {
        return None;
    }
    let mut from = opening - width;
    while let Some(found) = inner[from..].find(mark) {
        let end = from + found;
        let length = run(&inner[end..]);
        let previous = inner[..end].chars().next_back();
        let next = inner[end + length..].chars().next();
        // A single mark closes alone, or as the last of three; two are a bold's inside it.
        let fits = match width {
            2 => length >= 2,
            _ => length != 2,
        };
        if end > 0
            && fits
            && !previous.is_some_and(char::is_whitespace)
            && !(mark == '_' && word(next))
        {
            let close = end + length - width;
            return Some((&inner[..close], emphasis, width + close + width));
        }
        from = end + length;
    }
    None
}

/// "[label](address)": its label, its address if it's a web page's or a mail's, and its length.
/// Any other address shows its label alone.
fn link(rest: &str) -> Option<(&str, Option<SharedString>, usize)> {
    let inner = rest.strip_prefix('[')?;
    let close = inner.find(']')?;
    let after = inner[close + 1..].strip_prefix('(')?;
    let end = after.find(')')?;
    let address = after[..end]
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>');
    // A title after the address, "[a](b "c")", isn't shown.
    let address = address.split_whitespace().next().unwrap_or_default();
    let web = ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| address.starts_with(scheme));
    Some((
        &inner[..close],
        web.then(|| address.to_string().into()),
        1 + close + 2 + end + 1,
    ))
}

/// A web address written out, "https://maat.pt/en", or in angle brackets: its address and its
/// length. The punctuation that ends a sentence after it isn't part of it.
fn bare_link(rest: &str, before: Option<char>) -> Option<(&str, usize)> {
    let (rest, bracket) = match rest.strip_prefix('<') {
        Some(inner) => (inner, true),
        None => (rest, false),
    };
    if !rest.starts_with("https://") && !rest.starts_with("http://") {
        return None;
    }
    if !bracket && before.is_some_and(|char| char.is_alphanumeric() || char == '/') {
        return None;
    }
    if bracket {
        let end = rest.find('>')?;
        return Some((&rest[..end], end + 2));
    }
    let end = rest
        .find(|char: char| char.is_whitespace() || matches!(char, '<' | '>' | '"'))
        .unwrap_or(rest.len());
    let mut url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', '*', '_']);
    // A closing parenthesis is the address's only when it opened one.
    while url.ends_with(')') && url.matches('(').count() < url.matches(')').count() {
        url = &url[..url.len() - 1];
    }
    Some((url, url.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styled(inline: &Inline) -> Vec<(&str, Style)> {
        (inline.styled.iter())
            .map(|(range, style)| (&inline.text[range.clone()], *style))
            .collect()
    }

    const BOLD: Style = Style {
        bold: true,
        italic: false,
        code: false,
        link: false,
    };
    const ITALIC: Style = Style {
        bold: false,
        italic: true,
        code: false,
        link: false,
    };
    const CODE: Style = Style {
        bold: false,
        italic: false,
        code: true,
        link: false,
    };
    const LINK: Style = Style {
        bold: false,
        italic: false,
        code: false,
        link: true,
    };

    /// An agent's answer, as Claude writes one: paragraphs, a list of options with bold names and
    /// a number list, then its question at the margin under the list.
    #[test]
    fn an_answer_shows_its_paragraphs_and_lists() {
        let answer = "Both are open **Saturday**.\n\n- **Gulbenkian**: 10:00–18:00\n- *MAAT*: 11:00–19:00,\n  closed Tuesdays\n\n1. Book online\n2) Go early\nWhich would you rather visit?";
        let blocks = blocks(answer);
        let texts: Vec<(String, &str)> = (blocks.iter())
            .map(|block| match block {
                Block::Paragraph(text) => (String::new(), text.text.as_str()),
                Block::Item {
                    depth,
                    marker,
                    text,
                } => (format!("{depth}{marker}"), text.text.as_str()),
            })
            .collect();
        assert_eq!(
            texts,
            [
                ("".into(), "Both are open Saturday."),
                ("0•".into(), "Gulbenkian: 10:00–18:00"),
                ("0•".into(), "MAAT: 11:00–19:00,\nclosed Tuesdays"),
                ("01.".into(), "Book online"),
                ("02.".into(), "Go early"),
                ("".into(), "Which would you rather visit?"),
            ]
        );
        let Block::Item { text, .. } = &blocks[1] else {
            panic!()
        };
        assert_eq!(styled(text), [("Gulbenkian", BOLD)]);
        let Block::Item { text, .. } = &blocks[2] else {
            panic!()
        };
        assert_eq!(styled(text), [("MAAT", ITALIC)]);
    }

    #[test]
    fn a_nested_list_and_a_heading() {
        let blocks = blocks("## Opening hours ##\n- Lisbon\n  - Gulbenkian\n---\nThat's all.");
        assert!(
            matches!(&blocks[0], Block::Paragraph(text) if text.text == "Opening hours" && styled(text) == [("Opening hours", BOLD)])
        );
        assert!(matches!(&blocks[1], Block::Item { depth: 0, .. }));
        assert!(
            matches!(&blocks[2], Block::Item { depth: 1, text, .. } if text.text == "Gulbenkian")
        );
        assert!(matches!(&blocks[3], Block::Paragraph(text) if text.text == "That's all."));
    }

    #[test]
    fn bold_italic_and_code_mark_their_text() {
        let text = inline(
            "Use **SPRING10** at *checkout*, or `GIFT-5` — ***both*** if you can.",
            Style::default(),
        );
        assert_eq!(
            text.text,
            "Use SPRING10 at checkout, or GIFT-5 — both if you can."
        );
        let both = Style {
            bold: true,
            italic: true,
            ..Style::default()
        };
        assert_eq!(
            styled(&text),
            [
                ("SPRING10", BOLD),
                ("checkout", ITALIC),
                ("GIFT-5", CODE),
                ("both", both)
            ]
        );
        // Code shows its marks as written.
        let text = inline("Run `a_*b*_c` then __go__.", Style::default());
        assert_eq!(
            (text.text.as_str(), styled(&text)),
            (
                "Run a_*b*_c then go.",
                vec![("a_*b*_c", CODE), ("go", BOLD)]
            )
        );
    }

    /// What isn't a mark shows as written: an underscore in a word, a star with space after it,
    /// a mark never closed, and an escaped one.
    #[test]
    fn what_isnt_a_mark_shows_as_written() {
        for written in [
            "snake_case_name",
            "2 * 3 * 4",
            "a **bold start",
            "price*",
            "it's _open",
        ] {
            let text = inline(written, Style::default());
            assert_eq!(
                (text.text.as_str(), text.styled.len()),
                (written, 0),
                "{written}"
            );
        }
        assert_eq!(
            inline(r"\*not italic\*", Style::default()).text,
            "*not italic*"
        );
    }

    #[test]
    fn links_show_their_label_and_open_their_address() {
        let text = inline(
            "Tickets on [the **museum's** site](https://gulbenkian.pt/tickets \"Tickets\"), or https://maat.pt/en/visit. See [notes](file:///etc/hosts).",
            Style::default(),
        );
        assert_eq!(
            text.text,
            "Tickets on the museum's site, or https://maat.pt/en/visit. See notes."
        );
        let links: Vec<(&str, &str)> = text
            .links
            .iter()
            .map(|(range, url)| (&text.text[range.clone()], url.as_ref()))
            .collect();
        assert_eq!(
            links,
            [
                ("the museum's site", "https://gulbenkian.pt/tickets"),
                ("https://maat.pt/en/visit", "https://maat.pt/en/visit")
            ]
        );
        let bold_link = Style {
            bold: true,
            link: true,
            ..Style::default()
        };
        assert_eq!(
            styled(&text),
            [
                ("the ", LINK),
                ("museum's", bold_link),
                (" site", LINK),
                ("https://maat.pt/en/visit", LINK)
            ]
        );
        let text = inline(
            "(see https://en.wikipedia.org/wiki/Arc_(web_browser)) and <https://maat.pt>",
            Style::default(),
        );
        let urls: Vec<&str> = text.links.iter().map(|(_, url)| url.as_ref()).collect();
        assert_eq!(
            urls,
            [
                "https://en.wikipedia.org/wiki/Arc_(web_browser)",
                "https://maat.pt"
            ]
        );
        assert_eq!(
            text.text,
            "(see https://en.wikipedia.org/wiki/Arc_(web_browser)) and https://maat.pt"
        );
    }
}
