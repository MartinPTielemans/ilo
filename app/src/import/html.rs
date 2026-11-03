//! A bookmarks HTML file, the Netscape format every browser exports and imports: `<DL>` lists of
//! `<DT>` rows, each a bookmark, `<A HREF>`, or a folder, an `<H3>` with its own `<DL>` after it.
//! Browsers write it alike but not quite, so this reads only those tags, in any case, and passes
//! over the rest, such as `<p>`, a description's `<DD>` and the `ICON` data.

use super::found::{Node, Tab, opens_here};
use super::read::{folder_name, pinned};

/// What every browser's export starts with.
const DOCTYPE: &str = "<!doctype netscape-bookmark-file-1>";

/// The bookmarks in `html` as pinned tabs and folders, as a browser's own come in (read::pinned):
/// the bar's, which the file marks `PERSONAL_TOOLBAR_FOLDER`, loose at the top or in a folder of
/// its name once it's long, then those kept outside any folder, in a folder named after the file's
/// heading, as Firefox's menu is, then the other folders. In a file with no bar, those outside any
/// folder are the bar. Pages that can't open here and folders left empty stay out.
pub fn bookmarks(html: &str) -> Result<Vec<Node>, String> {
    let start = html.trim_start_matches('\u{feff}').trim_start();
    if !start
        .get(..DOCTYPE.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(DOCTYPE))
    {
        return Err("it isn't a bookmarks file a browser exported".into());
    }
    let mut heading = String::new();
    // The folders open, outermost first, the file's own list being the first.
    let mut open: Vec<Folder> = Vec::new();
    // A folder's name, and whether it's the bar, until its list opens.
    let mut named: Option<(String, bool)> = None;
    let mut bar: Option<(String, Vec<Node>)> = None;
    let mut top: Option<Vec<Node>> = None;
    let mut rest = start;
    while let Some(at) = rest.find('<') {
        rest = &rest[at..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some((name, attributes, after)) = tag(rest) else {
            break;
        };
        rest = after;
        match name.as_str() {
            "h1" => heading = text(&mut rest, "h1"),
            "h3" => {
                let bar = attribute(attributes, "personal_toolbar_folder")
                    .is_some_and(|value| value.eq_ignore_ascii_case("true"));
                named = Some((text(&mut rest, "h3"), bar));
            }
            "a" => {
                let url = attribute(attributes, "href").unwrap_or_default();
                let name = text(&mut rest, "a");
                if let Some(folder) = open.last_mut()
                    && opens_here(&url)
                {
                    folder.children.push(Node::Tab(Tab { url, name }));
                }
            }
            "dl" => {
                let (name, bar) = named.take().unwrap_or_default();
                open.push(Folder {
                    name,
                    bar,
                    children: Vec::new(),
                });
            }
            "/dl" => close(&mut open, &mut bar, &mut top),
            _ => {}
        }
    }
    // A file cut short closes what it left open.
    while top.is_none() && !open.is_empty() {
        close(&mut open, &mut bar, &mut top);
    }
    let top = top.unwrap_or_default();
    let heading = match heading.trim() {
        "" => "Bookmarks".to_string(),
        heading => heading.to_string(),
    };
    let (loose, folders): (Vec<Node>, Vec<Node>) = top
        .into_iter()
        .partition(|node| matches!(node, Node::Tab(_)));
    let mut folders: Vec<(String, Vec<Node>)> = (folders.into_iter())
        .filter_map(|node| match node {
            Node::Folder { name, children } => Some((name, children)),
            Node::Tab(_) => None,
        })
        .collect();
    let bar = match bar {
        Some(bar) => {
            folders.insert(0, (heading, loose));
            Some(bar)
        }
        None => Some((heading, loose)),
    };
    Ok(pinned(bar, folders))
}

struct Folder {
    name: String,
    bar: bool,
    children: Vec<Node>,
}

/// Closes the innermost folder open: the bar, among the file's own, is kept apart, an empty one
/// stays out, and the file's own list is the top.
fn close(
    open: &mut Vec<Folder>,
    bar: &mut Option<(String, Vec<Node>)>,
    top: &mut Option<Vec<Node>>,
) {
    let Some(folder) = open.pop() else {
        return;
    };
    let name = folder_name(&folder.name);
    let in_top = open.len() == 1;
    match open.last_mut() {
        None => *top = Some(folder.children),
        Some(_) if folder.bar && in_top && bar.is_none() => *bar = Some((name, folder.children)),
        Some(_) if folder.children.is_empty() => {}
        Some(parent) => parent.children.push(Node::Folder {
            name,
            children: folder.children,
        }),
    }
}

/// The tag `rest` starts with: its name, lowercase with a `/` before a closing one, its attributes,
/// and what follows it. None for a tag the file never ends.
fn tag(rest: &str) -> Option<(String, &str, &str)> {
    let inner = &rest[1..];
    let mut quote = None;
    let end = inner.find(|c: char| {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(open), c) if c == open => quote = None,
            (None, '>') => return true,
            _ => {}
        }
        false
    })?;
    let body = &inner[..end];
    let split = body
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(body.len());
    Some((
        body[..split].to_ascii_lowercase(),
        &body[split..],
        &inner[end + 1..],
    ))
}

/// The value of the attribute `name` among `attributes`, any case, quoted or not.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let mut rest = attributes;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return None;
        }
        let end = rest
            .find(|c: char| c == '=' || c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let key = &rest[..end];
        rest = rest[end..].trim_start();
        let Some(value) = rest.strip_prefix('=') else {
            // An attribute with no value, such as Safari's FOLDED.
            continue;
        };
        let value = value.trim_start();
        let (value, after) = match value.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let inner = &value[1..];
                let end = inner.find(quote).unwrap_or(inner.len());
                (&inner[..end], inner.get(end + 1..).unwrap_or(""))
            }
            _ => {
                let end = value
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(value.len());
                (&value[..end], &value[end..])
            }
        };
        if key.eq_ignore_ascii_case(name) {
            return Some(unescape(value).trim().to_string());
        }
        rest = after;
    }
}

/// The text up to the tag that closes `name`, which `rest` moves past; to the end of the file if
/// it's never closed.
fn text(rest: &mut &str, name: &str) -> String {
    let closing = format!("</{name}");
    let lower = rest.to_ascii_lowercase();
    let end = lower.find(&closing).unwrap_or(rest.len());
    let said = unescape(&rest[..end]);
    *rest = &rest[end..];
    said.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` with HTML's character references read.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let reference = rest[1..]
            .find(';')
            .filter(|end| *end <= 10)
            .map(|end| &rest[1..end + 1]);
        let read = reference.and_then(|reference| match reference {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => {
                let number = reference.strip_prefix('#')?;
                let code = match number.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => number.parse().ok()?,
                };
                char::from_u32(code)
            }
        });
        match (read, reference) {
            (Some(read), Some(reference)) => {
                out.push(read);
                rest = &rest[reference.len() + 2..];
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
