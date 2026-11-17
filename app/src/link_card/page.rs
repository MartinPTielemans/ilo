//! What a page says about itself, read from the head the core fetched (bc_tab_link_card): its title,
//! description and picture, as a link card shows them. Pure, so it runs off the main thread and its
//! tests run without the engine. Nothing in it trusts the page: every string is decoded, cleaned of
//! controls and direction marks, collapsed to one line and capped, and the picture's address is
//! resolved and kept only if it's http(s).

use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use url::Url;

/// The most of each the card keeps, in characters; the card clamps them to lines as well.
pub const TITLE_MAX: usize = 300;
pub const DESCRIPTION_MAX: usize = 500;
/// The longest picture address the card asks, in bytes.
pub const PICTURE_URL_MAX: usize = 2048;
/// How far into the bytes a <meta> charset is looked for, as the HTML standard's prescan does.
const PRESCAN: usize = 1024;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub title: Option<String>,
    pub description: Option<String>,
    /// An absolute http(s) address, for bc_link_card_picture.
    pub picture: Option<String>,
}

/// Reads |bytes|, the head of the page at |url| whose answer's type was |mime|, with the charset
/// the server named after ";charset=" if it did.
pub fn read(bytes: &[u8], mime: &str, url: &str) -> Page {
    let text = decode(bytes, mime);
    let mut found = Found::default();
    scan(&text, &mut found);
    // As a browser does, a <base> that doesn't resolve leaves the page's own address.
    let base = Url::parse(url).ok().map(|url| {
        found
            .base
            .as_deref()
            .and_then(|base| url.join(clean(base, PICTURE_URL_MAX).as_str()).ok())
            .unwrap_or(url)
    });
    let pick = |a: Option<String>, b: Option<String>, c: Option<String>, max: usize| {
        [a, b, c]
            .into_iter()
            .flatten()
            .map(|text| clean(&text, max))
            .find(|text| !text.is_empty())
    };
    Page {
        title: pick(found.og_title, found.twitter_title, found.title, TITLE_MAX),
        description: pick(
            found.og_description,
            found.twitter_description,
            found.description,
            DESCRIPTION_MAX,
        ),
        picture: [found.og_image, found.twitter_image]
            .into_iter()
            .flatten()
            .find_map(|address| picture(&address, base.as_ref()?)),
    }
}

/// The page's text: its byte order mark first, then the charset the server named, then its own
/// <meta>'s, then UTF-8 if it's valid, else Windows-1252, as a browser falls back to for the web's
/// unlabelled pages.
fn decode(bytes: &[u8], mime: &str) -> String {
    if let Some((encoding, _)) = Encoding::for_bom(bytes) {
        return encoding.decode_with_bom_removal(bytes).0.into_owned();
    }
    let named = mime
        .split(';')
        .skip(1)
        .filter_map(|part| {
            part.trim()
                .strip_prefix("charset=")
                .or_else(|| part.trim().strip_prefix("CHARSET="))
        })
        .find_map(|label| Encoding::for_label(label.trim_matches('"').as_bytes()));
    let encoding = named
        .or_else(|| meta_charset(&bytes[..bytes.len().min(PRESCAN)]))
        .map(Encoding::output_encoding)
        .unwrap_or_else(|| {
            if std::str::from_utf8(bytes).is_ok() {
                UTF_8
            } else {
                WINDOWS_1252
            }
        });
    encoding.decode_without_bom_handling(bytes).0.into_owned()
}

/// A <meta charset> or <meta http-equiv content="…; charset=…"> in |bytes|, read as ASCII, as the
/// labels are.
fn meta_charset(bytes: &[u8]) -> Option<&'static Encoding> {
    let text = String::from_utf8_lossy(bytes);
    let mut found = None;
    each_tag(&text, |name, attributes| {
        if found.is_some() || name != "meta" {
            return;
        }
        if let Some(label) = attribute(attributes, "charset") {
            found = Encoding::for_label(label.trim().as_bytes());
        } else if let Some(content) = attribute(attributes, "content") {
            let lower = content.to_ascii_lowercase();
            if let Some(at) = lower.find("charset=") {
                let label = lower[at + "charset=".len()..]
                    .trim_start_matches(['"', '\''])
                    .split(|c: char| c == ';' || c == '"' || c == '\'' || c.is_whitespace())
                    .next()
                    .unwrap_or("");
                found = Encoding::for_label(label.as_bytes());
            }
        }
    });
    found
}

#[derive(Default)]
struct Found {
    title: Option<String>,
    description: Option<String>,
    og_title: Option<String>,
    og_description: Option<String>,
    og_image: Option<String>,
    twitter_title: Option<String>,
    twitter_description: Option<String>,
    twitter_image: Option<String>,
    /// The first <base>'s href, which relative addresses resolve against.
    base: Option<String>,
}

fn scan(text: &str, found: &mut Found) {
    let mut title_at = None;
    each_tag(text, |name, attributes| {
        if name == "title" && title_at.is_none() {
            title_at = Some(attributes.as_ptr() as usize - text.as_ptr() as usize);
        }
        if name == "base" && found.base.is_none() {
            found.base = attribute(attributes, "href").map(|href| entities(&href));
        }
        if name != "meta" {
            return;
        }
        let key = attribute(attributes, "property")
            .or_else(|| attribute(attributes, "name"))
            .map(|key| key.trim().to_ascii_lowercase());
        let (Some(key), Some(content)) = (key, attribute(attributes, "content")) else {
            return;
        };
        let slot = match key.as_str() {
            "og:title" => &mut found.og_title,
            "og:description" => &mut found.og_description,
            "og:image" | "og:image:url" | "og:image:secure_url" => &mut found.og_image,
            "twitter:title" => &mut found.twitter_title,
            "twitter:description" => &mut found.twitter_description,
            "twitter:image" | "twitter:image:src" => &mut found.twitter_image,
            "description" => &mut found.description,
            _ => return,
        };
        // The first of each, as the sites that repeat a tag put the main one first.
        if slot.is_none() {
            *slot = Some(entities(&content));
        }
    });
    // The title's text runs from its tag's end to the next "</title", in any case.
    if let Some(at) = title_at {
        let rest = &text[at..];
        let start = rest.find('>').map_or(rest.len(), |end| end + 1);
        let body = &rest[start..];
        let end = find_ascii_ci(body, "</title").unwrap_or(body.len());
        found.title = Some(entities(&body[..end]));
    }
}

/// Calls |each| with every start tag's lowercase name and its attributes' text, skipping comments
/// and the insides of <script>, <style>, <template> and <noscript>, whose text isn't markup.
fn each_tag(text: &str, mut each: impl FnMut(&str, &str)) {
    let mut at = 0;
    while let Some(open) = text[at..].find('<').map(|found| at + found) {
        let rest = &text[open + 1..];
        if rest.starts_with("!--") {
            at = rest[3..]
                .find("-->")
                .map_or(text.len(), |end| open + 1 + 3 + end + 3);
            continue;
        }
        let name_len = rest
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(rest.len());
        if name_len == 0 || !rest.as_bytes()[0].is_ascii_alphabetic() {
            at = open + 1;
            continue;
        }
        let name = rest[..name_len].to_ascii_lowercase();
        // A tag the cap cut before its end is no tag: its last value may be half of one.
        let Some(end) = tag_end(&rest[name_len..]) else {
            return;
        };
        let close = open + 1 + name_len + end;
        let attributes = &text[open + 1 + name_len..close];
        each(&name, attributes);
        at = (close + 1).min(text.len());
        if matches!(
            name.as_str(),
            "script" | "style" | "template" | "noscript" | "textarea"
        ) {
            let end = format!("</{name}");
            at = find_ascii_ci(&text[at..], &end).map_or(text.len(), |found| at + found);
        }
    }
}

/// Where a tag's ">" is in |rest|, past quoted attribute values.
fn tag_end(rest: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in rest.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// The value of |name| in a tag's |attributes|, quoted or not; entities left for the caller.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let bytes = attributes.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'/' | b'>')
        {
            i += 1;
        }
        let key = &attributes[start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = "";
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                let from = i + 1;
                i = from;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                value = &attributes[from..i];
                i += 1;
            } else {
                let from = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                    i += 1;
                }
                value = &attributes[from..i];
            }
        }
        if key.eq_ignore_ascii_case(name) {
            return Some(value.to_string());
        }
        if i == start {
            i += 1;
        }
    }
    None
}

fn find_ascii_ci(haystack: &str, needle: &str) -> Option<usize> {
    let (hay, needle) = (haystack.as_bytes(), needle.as_bytes());
    (0..=hay.len().saturating_sub(needle.len())).find(|&i| {
        hay.len() >= needle.len() && hay[i..i + needle.len()].eq_ignore_ascii_case(needle)
    })
}

/// |text| with its character references replaced: every numeric one, and the named ones pages'
/// titles and descriptions use. Any other stays as written.
fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let end = rest[1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
            .map_or(rest.len(), |end| end + 1);
        let name = &rest[1..end];
        let semicolon = rest[end..].starts_with(';');
        let replaced = if let Some(number) = name.strip_prefix('#') {
            let value = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => number.parse().ok(),
            };
            value.map(|value| match char::from_u32(value) {
                Some('\0') | None => '\u{FFFD}',
                Some(c) => c,
            })
        } else if semicolon {
            named(name)
        } else {
            None
        };
        match replaced {
            Some(c) if !name.is_empty() => {
                out.push(c);
                rest = &rest[end + usize::from(semicolon)..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn named(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{A0}',
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "ldquo" => '“',
        "rdquo" => '”',
        "bdquo" => '„',
        "laquo" => '«',
        "raquo" => '»',
        "lsaquo" => '‹',
        "rsaquo" => '›',
        "middot" => '·',
        "bull" => '•',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "deg" => '°',
        "times" => '×',
        "euro" => '€',
        "pound" => '£',
        "yen" => '¥',
        "cent" => '¢',
        "sect" => '§',
        "para" => '¶',
        "iexcl" => '¡',
        "iquest" => '¿',
        "shy" => '\u{AD}',
        "zwj" => '\u{200D}',
        "zwnj" => '\u{200C}',
        "lrm" => '\u{200E}',
        "rlm" => '\u{200F}',
        "Agrave" => 'À',
        "Aacute" => 'Á',
        "Acirc" => 'Â',
        "Atilde" => 'Ã',
        "Auml" => 'Ä',
        "Aring" => 'Å',
        "agrave" => 'à',
        "aacute" => 'á',
        "acirc" => 'â',
        "atilde" => 'ã',
        "auml" => 'ä',
        "aring" => 'å',
        "AElig" => 'Æ',
        "aelig" => 'æ',
        "Ccedil" => 'Ç',
        "ccedil" => 'ç',
        "Egrave" => 'È',
        "Eacute" => 'É',
        "Ecirc" => 'Ê',
        "Euml" => 'Ë',
        "egrave" => 'è',
        "eacute" => 'é',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "Igrave" => 'Ì',
        "Iacute" => 'Í',
        "Icirc" => 'Î',
        "Iuml" => 'Ï',
        "igrave" => 'ì',
        "iacute" => 'í',
        "icirc" => 'î',
        "iuml" => 'ï',
        "Ntilde" => 'Ñ',
        "ntilde" => 'ñ',
        "Ograve" => 'Ò',
        "Oacute" => 'Ó',
        "Ocirc" => 'Ô',
        "Otilde" => 'Õ',
        "Ouml" => 'Ö',
        "Oslash" => 'Ø',
        "ograve" => 'ò',
        "oacute" => 'ó',
        "ocirc" => 'ô',
        "otilde" => 'õ',
        "ouml" => 'ö',
        "oslash" => 'ø',
        "Ugrave" => 'Ù',
        "Uacute" => 'Ú',
        "Ucirc" => 'Û',
        "Uuml" => 'Ü',
        "ugrave" => 'ù',
        "uacute" => 'ú',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "Yacute" => 'Ý',
        "yacute" => 'ý',
        "yuml" => 'ÿ',
        "szlig" => 'ß',
        _ => return None,
    })
}

/// |text| as one line the card can show: controls and direction marks gone, so a page can't
/// reorder the card's text or the address beside it; every run of white space, line breaks
/// included, one space; trimmed, and at most |max| characters.
pub fn clean(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut space = false;
    let mut count = 0;
    for c in text.chars() {
        if c.is_whitespace() || c.is_control() {
            // A control that isn't white space is dropped, as a soft hyphen is, without a space.
            space |= c.is_whitespace();
            continue;
        }
        if matches!(c, '\u{AD}' | '\u{200B}' | '\u{FEFF}' | '\u{061C}' | '\u{200E}' | '\u{200F}'
            | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
        {
            continue;
        }
        if count == max {
            break;
        }
        if space && !out.is_empty() {
            out.push(' ');
            count += 1;
            if count == max {
                break;
            }
        }
        space = false;
        out.push(c);
        count += 1;
    }
    out
}

/// |address| resolved against the page's |url|, if it's http(s) and short enough to ask.
fn picture(address: &str, base: &Url) -> Option<String> {
    let address = clean(address, PICTURE_URL_MAX);
    if address.is_empty() {
        return None;
    }
    let resolved = base.join(&address).ok()?;
    let resolved_text = resolved.as_str();
    (matches!(resolved.scheme(), "http" | "https")
        && resolved.host_str().is_some_and(|host| !host.is_empty())
        && resolved_text.len() <= PICTURE_URL_MAX)
        .then(|| resolved_text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: &str = "https://example.com/post/1";

    #[test]
    fn white_space_and_line_breaks_collapse_to_one_space() {
        let page = read(
            b"<title>a\nb</title><meta name=description content=\"one\r\ntwo\tthree&#10;four&#13;&#10;five   six\n\">",
            "text/html",
            AT,
        );
        assert_eq!(page.title.as_deref(), Some("a b"));
        assert_eq!(
            page.description.as_deref(),
            Some("one two three four five six")
        );
        assert_eq!(
            clean("  \n lead\u{A0}\u{3000}and trail \r\n", 100),
            "lead and trail"
        );
    }

    #[test]
    fn open_graph_wins_over_twitter_and_the_title() {
        let page = read(
            br#"<head><title>Plain</title>
            <meta name="twitter:title" content="Twitter">
            <meta property="og:title" content="Open Graph">
            <meta name="description" content="Plain description">
            <meta name="twitter:image" content="/t.png">
            <meta property="og:image" content="/og.png"></head>"#,
            "text/html",
            AT,
        );
        assert_eq!(page.title.as_deref(), Some("Open Graph"));
        assert_eq!(page.description.as_deref(), Some("Plain description"));
        assert_eq!(page.picture.as_deref(), Some("https://example.com/og.png"));
    }

    #[test]
    fn an_empty_tag_falls_through_to_the_next() {
        let page = read(
            br#"<meta property="og:title" content="  "><title>Real</title>"#,
            "text/html",
            AT,
        );
        assert_eq!(page.title.as_deref(), Some("Real"));
    }

    #[test]
    fn entities_are_decoded_and_unknown_ones_kept() {
        let page = read(
            b"<title>Tom &amp; Jerry &#x2014; &#8220;caf&eacute;&#8221; &bogus; AT&T</title>",
            "text/html",
            AT,
        );
        assert_eq!(
            page.title.as_deref(),
            Some("Tom & Jerry \u{2014} \u{201C}caf\u{E9}\u{201D} &bogus; AT&T")
        );
    }

    #[test]
    fn direction_marks_and_controls_go() {
        let title = "\u{202E}gnp.exe\u{202C} a\u{0007}b\u{200F}c\u{2066}d\u{2069}";
        let page = read(
            format!("<title>{title}</title>").as_bytes(),
            "text/html",
            AT,
        );
        assert_eq!(page.title.as_deref(), Some("gnp.exe abcd"));
    }

    #[test]
    fn each_is_capped() {
        let long = "x".repeat(TITLE_MAX + 50);
        let page = read(format!("<title>{long}</title>").as_bytes(), "text/html", AT);
        assert_eq!(page.title.unwrap().chars().count(), TITLE_MAX);
        let desc = "é".repeat(DESCRIPTION_MAX * 2);
        let page = read(
            format!("<meta name=description content='{desc}'>").as_bytes(),
            "text/html",
            AT,
        );
        assert_eq!(page.description.unwrap().chars().count(), DESCRIPTION_MAX);
        let path = "a".repeat(PICTURE_URL_MAX);
        let page = read(
            format!("<meta property=og:image content='/{path}'>").as_bytes(),
            "text/html",
            AT,
        );
        assert_eq!(page.picture, None);
    }

    #[test]
    fn the_picture_is_resolved_and_only_http_or_https() {
        let at = |content: &str| {
            read(
                format!("<meta property='og:image' content='{content}'>").as_bytes(),
                "text/html",
                AT,
            )
            .picture
        };
        assert_eq!(
            at("i.png").as_deref(),
            Some("https://example.com/post/i.png")
        );
        assert_eq!(
            at("//cdn.example.net/i.png").as_deref(),
            Some("https://cdn.example.net/i.png")
        );
        assert_eq!(
            at("http://example.org/i.jpg").as_deref(),
            Some("http://example.org/i.jpg")
        );
        assert_eq!(at("javascript:alert(1)"), None);
        assert_eq!(at("data:image/png;base64,AAAA"), None);
        assert_eq!(at("file:///etc/passwd"), None);
        assert_eq!(at(""), None);
    }

    #[test]
    fn a_base_moves_where_the_picture_resolves() {
        let at = |head: &str| read(head.as_bytes(), "text/html", AT).picture;
        assert_eq!(
            at("<base href='/media/'><meta property='og:image' content='i.png'>").as_deref(),
            Some("https://example.com/media/i.png")
        );
        assert_eq!(
            at(
                "<base href='https://cdn.example.net/a/'><meta property='og:image' content='i.png'>"
            )
            .as_deref(),
            Some("https://cdn.example.net/a/i.png")
        );
        // Only the first <base> counts, as in a browser.
        assert_eq!(
            at("<base href='/x/'><base href='/y/'><meta property='og:image' content='i.png'>")
                .as_deref(),
            Some("https://example.com/x/i.png")
        );
    }

    #[test]
    fn shift_jis_named_by_the_server() {
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode("<title>日本語のページ</title>");
        let page = read(&bytes, "text/html;charset=Shift_JIS", AT);
        assert_eq!(page.title.as_deref(), Some("日本語のページ"));
    }

    #[test]
    fn euc_jp_named_by_its_meta() {
        let (bytes, _, _) = encoding_rs::EUC_JP
            .encode("<meta http-equiv=\"Content-Type\" content=\"text/html; charset=EUC-JP\"><title>東京の天気</title>");
        let page = read(&bytes, "text/html", AT);
        assert_eq!(page.title.as_deref(), Some("東京の天気"));
        let (bytes, _, _) =
            encoding_rs::SHIFT_JIS.encode("<meta charset=\"shift_jis\"><title>大阪</title>");
        assert_eq!(read(&bytes, "text/html", AT).title.as_deref(), Some("大阪"));
    }

    #[test]
    fn unlabelled_bytes_are_utf8_if_valid_else_windows_1252() {
        assert_eq!(
            read("<title>naïve</title>".as_bytes(), "text/html", AT)
                .title
                .as_deref(),
            Some("naïve")
        );
        assert_eq!(
            read(b"<title>na\xEFve \x93q\x94</title>", "text/html", AT)
                .title
                .as_deref(),
            Some("naïve “q”")
        );
    }

    #[test]
    fn markup_in_scripts_and_comments_is_not_read() {
        let page = read(
            br#"<script>var s = '<meta property="og:title" content="script">';</script>
            <!-- <meta property="og:title" content="comment"> -->
            <meta property="og:title" content="real">"#,
            "text/html",
            AT,
        );
        assert_eq!(page.title.as_deref(), Some("real"));
    }

    #[test]
    fn nothing_found_is_nothing() {
        assert_eq!(read(b"", "text/html", AT), Page::default());
        assert_eq!(
            read(b"<html><head></head>", "text/html", AT),
            Page::default()
        );
        // An unclosed tag at the cap's cut.
        assert_eq!(
            read(
                b"<meta property=\"og:title\" content=\"cut",
                "text/html",
                AT
            )
            .title,
            None
        );
    }
}
