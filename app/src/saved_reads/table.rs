//! The table kind's reader: the tables of a page's HTML, each laid out as a grid, so a cell that
//! spans rows or columns fills each place it covers and never shifts the cells after it. It's a
//! walk over the page's tags with link_card's scanner, not a browser's parse: nothing on the page
//! runs, and what it can't place for certain reads as broken rather than as other rows. A table in
//! a comment, a script, a style, a template, a noscript or a textarea is text there, not a table;
//! a table inside a cell is its own table, not the outer one's rows; a cell or row with no end tag
//! ends where the next one starts, as HTML has it; and rows in a tfoot (totals, as a rule) aren't
//! rows.

use crate::link_card::page::{attribute, entities, find_ascii_ci, tag_end};

/// The most columns a table's grid is laid out to, and the most a cell spans.
const COLUMNS_LAID: usize = 200;
/// The most rows a cell spans; a rowspan of 0 spans the rest of its section.
const ROWS_SPANNED: usize = 1000;
/// The most of a cell's text read before it's cut, in bytes.
const CELL_READ: usize = 4 * super::VALUE_KEPT * 4;
/// The most tables read from a page.
const TABLES_READ: usize = 200;
/// The most rows read from a table.
const ROWS_READ: usize = 5000;

/// A table as a grid: its header's names, a column each, and its rows' cells under them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

/// A cell's text, as one line, and the address of its first link, as the page wrote it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub text: String,
    pub link: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Section {
    Head,
    Body,
    Foot,
}

#[derive(Clone, Debug)]
struct Written {
    text: String,
    link: Option<String>,
    header: bool,
    columns: usize,
    rows: usize,
}

#[derive(Debug)]
struct Row {
    section: Section,
    /// Which row group it's in, counting each thead, tbody and tfoot, so a rowspan ends at its own.
    group: usize,
    cells: Vec<Written>,
}

/// A table the walk is in.
#[derive(Debug)]
struct Open {
    section: Section,
    group: usize,
    rows: Vec<Row>,
    row: Option<Row>,
    cell: Option<Written>,
}

impl Open {
    fn new() -> Open {
        Open {
            section: Section::Body,
            group: 0,
            rows: Vec::new(),
            row: None,
            cell: None,
        }
    }

    fn end_cell(&mut self) {
        if let Some(cell) = self.cell.take() {
            let (section, group) = (self.section, self.group);
            self.row
                .get_or_insert_with(|| Row {
                    section,
                    group,
                    cells: Vec::new(),
                })
                .cells
                .push(cell);
        }
    }

    fn end_row(&mut self) {
        self.end_cell();
        if let Some(row) = self.row.take() {
            if self.rows.len() < ROWS_READ {
                self.rows.push(row);
            }
        }
    }
}

/// The tables in `html`, in the order they start, each laid out as a grid.
pub fn tables(html: &str) -> Vec<Table> {
    let mut open: Vec<(usize, Open)> = Vec::new();
    let mut done: Vec<(usize, Vec<Row>)> = Vec::new();
    let mut started = 0;
    walk(html, |token| match token {
        Token::Text(text) => {
            if let Some((_, table)) = open.last_mut() {
                if let Some(cell) = table.cell.as_mut() {
                    if cell.text.len() < CELL_READ {
                        cell.text.push_str(&entities(text));
                    }
                }
            }
        }
        Token::Start(name, attributes) => match name {
            "table" => {
                if started < TABLES_READ {
                    open.push((started, Open::new()));
                }
                started += 1;
            }
            _ if open.is_empty() => {}
            "thead" | "tbody" | "tfoot" => {
                let table = &mut open.last_mut().unwrap().1;
                table.end_row();
                table.group += 1;
                table.section = match name {
                    "thead" => Section::Head,
                    "tfoot" => Section::Foot,
                    _ => Section::Body,
                };
            }
            "tr" => open.last_mut().unwrap().1.end_row(),
            "td" | "th" => {
                let table = &mut open.last_mut().unwrap().1;
                table.end_cell();
                let span = |name: &str, most: usize, zero: usize| {
                    let value = attribute(attributes, name).unwrap_or_default();
                    let digits: String = value
                        .trim()
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect();
                    match digits.parse::<usize>() {
                        Ok(0) => zero,
                        Ok(span) => span.min(most),
                        Err(_) => 1,
                    }
                };
                table.cell = Some(Written {
                    text: String::new(),
                    link: None,
                    header: name == "th",
                    columns: span("colspan", COLUMNS_LAID, 1),
                    rows: span("rowspan", ROWS_SPANNED, ROWS_SPANNED),
                });
            }
            "a" => {
                let table = &mut open.last_mut().unwrap().1;
                if let Some(cell) = table.cell.as_mut().filter(|cell| cell.link.is_none()) {
                    cell.link = attribute(attributes, "href").map(|href| entities(&href));
                }
            }
            _ if breaks(name) => {
                if let Some(cell) = open.last_mut().unwrap().1.cell.as_mut() {
                    cell.text.push(' ');
                }
            }
            _ => {}
        },
        Token::End(name) => match name {
            "table" => {
                if let Some((at, mut table)) = open.pop() {
                    table.end_row();
                    done.push((at, table.rows));
                }
            }
            _ if open.is_empty() => {}
            "thead" | "tbody" | "tfoot" => {
                let table = &mut open.last_mut().unwrap().1;
                table.end_row();
                table.group += 1;
                table.section = Section::Body;
            }
            "tr" => open.last_mut().unwrap().1.end_row(),
            "td" | "th" => open.last_mut().unwrap().1.end_cell(),
            _ if breaks(name) => {
                if let Some(cell) = open.last_mut().unwrap().1.cell.as_mut() {
                    cell.text.push(' ');
                }
            }
            _ => {}
        },
    });
    // A table the page never closed ends with it, as HTML has it.
    while let Some((at, mut table)) = open.pop() {
        table.end_row();
        done.push((at, table.rows));
    }
    done.sort_by_key(|(at, _)| *at);
    done.into_iter().map(|(_, rows)| laid_out(rows)).collect()
}

/// Tags that end a line of a cell's text, so the words either side don't run together.
fn breaks(name: &str) -> bool {
    matches!(
        name,
        "br" | "p"
            | "div"
            | "li"
            | "ul"
            | "ol"
            | "dl"
            | "dt"
            | "dd"
            | "hr"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}

/// The rows as a grid: a cell covers each place its spans reach, and a row's next cell takes the
/// first place no cell above already covers. A rowspan reaches no further than its section, as
/// in HTML. The header is the first row with a header cell and the rows of header cells right
/// under it, a column's name its cells' words top to bottom; the rows are the body's after it
/// with a data cell, so a header repeated down the table isn't a row.
fn laid_out(rows: Vec<Row>) -> Table {
    let mut grid: Vec<(Section, Vec<Option<Written>>)> = Vec::new();
    let mut above: Vec<Option<Written>> = Vec::new();
    let mut group = None;
    for row in rows {
        if group != Some(row.group) {
            above.clear();
            group = Some(row.group);
        }
        // `above` holds, for each column, the cell spanning down into this row and how many rows
        // it still covers, this one included; `below` the same for the next row.
        let mut laid: Vec<Option<Written>> = Vec::new();
        let mut below: Vec<Option<Written>> = vec![None; above.len()];
        let carry =
            |at: usize, laid: &mut Vec<Option<Written>>, below: &mut Vec<Option<Written>>| {
                let Some(cell) = above[at].clone() else {
                    return false;
                };
                if cell.rows > 1 {
                    below[at] = Some(Written {
                        rows: cell.rows - 1,
                        ..cell.clone()
                    });
                }
                laid_at(laid, at, Some(cell));
                true
            };
        let mut at = 0;
        let mut cells = row.cells.into_iter();
        loop {
            while at < above.len() && carry(at, &mut laid, &mut below) {
                at += 1;
            }
            let Some(cell) = cells.next() else { break };
            for column in at..(at + cell.columns).min(COLUMNS_LAID) {
                laid_at(&mut laid, column, Some(cell.clone()));
                if cell.rows > 1 {
                    if below.len() <= column {
                        below.resize(column + 1, None);
                    }
                    below[column] = Some(Written {
                        rows: cell.rows - 1,
                        ..cell.clone()
                    });
                }
            }
            at = (at + cell.columns).min(COLUMNS_LAID);
        }
        // What spans from above past the row's last cell still fills its place.
        while at < above.len() {
            carry(at, &mut laid, &mut below);
            at += 1;
        }
        above = below;
        grid.push((row.section, laid));
    }
    let header = |cells: &Vec<Option<Written>>| cells.iter().flatten().any(|cell| cell.header);
    let all_header = |cells: &Vec<Option<Written>>| {
        !cells.is_empty() && cells.iter().flatten().all(|cell| cell.header)
    };
    let Some(first) = grid
        .iter()
        .position(|(section, cells)| *section != Section::Foot && header(cells))
    else {
        return Table::default();
    };
    let mut last = first;
    while grid
        .get(last + 1)
        .is_some_and(|(section, cells)| *section != Section::Foot && all_header(cells))
        && all_header(&grid[last].1)
    {
        last += 1;
    }
    let width = grid.iter().map(|(_, cells)| cells.len()).max().unwrap_or(0);
    let mut headers = vec![String::new(); width];
    for (_, cells) in &grid[first..=last] {
        for (column, cell) in cells.iter().enumerate() {
            let Some(cell) = cell.as_ref().filter(|cell| cell.header) else {
                continue;
            };
            let words = cell_text(&cell.text);
            if !words.is_empty() && !headers[column].ends_with(&words) {
                if !headers[column].is_empty() {
                    headers[column].push(' ');
                }
                headers[column].push_str(&words);
            }
        }
    }
    let rows = (grid.into_iter().skip(last + 1))
        .filter(|(section, cells)| {
            *section == Section::Body && cells.iter().flatten().any(|cell| !cell.header)
        })
        .map(|(_, cells)| {
            let mut row: Vec<Cell> = (cells.into_iter())
                .map(|cell| {
                    cell.map_or_else(Cell::default, |cell| Cell {
                        text: cell_text(&cell.text),
                        link: cell.link,
                    })
                })
                .collect();
            row.resize(width, Cell::default());
            row
        })
        .collect();
    Table { headers, rows }
}

fn laid_at(laid: &mut Vec<Option<Written>>, at: usize, cell: Option<Written>) {
    if laid.len() <= at {
        laid.resize(at + 1, None);
    }
    laid[at] = cell;
}

/// A cell's text as one line, without its footnote marks, such as "[1]", "[a]" or "[note 3]".
pub fn cell_text(text: &str) -> String {
    let line = super::text(text, usize::MAX / 8);
    let mut out = String::with_capacity(line.len());
    let mut rest = line.as_str();
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']').map(|close| open + close) else {
            break;
        };
        let inside = &rest[open + 1..close];
        let mark = inside.len() <= 8
            && !inside.is_empty()
            && inside
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == ' ')
            && inside
                .split(' ')
                .next_back()
                .is_some_and(|last| !last.is_empty() && last.len() <= 3);
        out.push_str(&rest[..open]);
        if !mark {
            out.push_str(&rest[open..=close]);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    super::text(&out, super::VALUE_KEPT)
}

enum Token<'a> {
    Start(&'a str, &'a str),
    End(&'a str),
    Text(&'a str),
}

/// Calls `each` with the page's start tags (a lower-case name and its attributes' text), end
/// tags and text between them, in order. Comments, doctypes and processing instructions are
/// passed over, and what's inside a script, style, template, noscript or textarea isn't markup.
fn walk(html: &str, mut each: impl FnMut(Token<'_>)) {
    let mut at = 0;
    let mut name_buffer = String::new();
    while at < html.len() {
        let Some(open) = html[at..].find('<').map(|found| at + found) else {
            each(Token::Text(&html[at..]));
            return;
        };
        if open > at {
            each(Token::Text(&html[at..open]));
        }
        let rest = &html[open + 1..];
        if let Some(comment) = rest.strip_prefix("!--") {
            at = comment
                .find("-->")
                .map_or(html.len(), |end| open + 4 + end + 3);
            continue;
        }
        if rest.starts_with('!') || rest.starts_with('?') {
            at = rest.find('>').map_or(html.len(), |end| open + 1 + end + 1);
            continue;
        }
        let (closing, body) = match rest.strip_prefix('/') {
            Some(body) => (true, body),
            None => (false, rest),
        };
        let name_len = body
            .find(|c: char| !c.is_ascii_alphanumeric())
            .unwrap_or(body.len());
        if name_len == 0 || !body.as_bytes()[0].is_ascii_alphabetic() {
            // A "<" that starts no tag is text, as HTML reads it.
            each(Token::Text("<"));
            at = open + 1;
            continue;
        }
        let name_at = open + 1 + usize::from(closing);
        let Some(end) = tag_end(&body[name_len..]) else {
            return;
        };
        let close = name_at + name_len + end;
        name_buffer.clear();
        name_buffer.push_str(&html[name_at..name_at + name_len]);
        name_buffer.make_ascii_lowercase();
        let name = name_buffer.as_str();
        at = (close + 1).min(html.len());
        if closing {
            each(Token::End(name));
            continue;
        }
        each(Token::Start(name, &html[name_at + name_len..close]));
        if matches!(
            name,
            "script" | "style" | "template" | "noscript" | "textarea" | "title" | "xmp"
        ) {
            let end = format!("</{name}");
            at = find_ascii_ci(&html[at..], &end).map_or(html.len(), |found| at + found);
        }
    }
}
