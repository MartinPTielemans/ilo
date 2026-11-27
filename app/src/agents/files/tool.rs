//! read_file (#4367), the one file tool brought forward from road B's slice 4: a thread's
//! assistant reads a text file, looks at a picture, or reads a PDF's pages, each judged by the
//! path floor and road B's table as a read. What's read is what the held handle holds, judged by
//! its first bytes, never its name. A PDF's pages are the engine's to read, in a sandboxed process
//! of their own (bc_pdf_read); the rest is read here, off the UI thread.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::give::{self, Reach};
use super::read::{self, Kind, Picture, TEXT_ROOM, Unread};
pub use super::read::{
    LINES, MOST_PAGES, MOST_PICTURES, PDF_BROKEN, PDF_LOCKED, PDF_READ, PDF_TIMEOUT, Pdf, PdfRead,
    pdf_answer,
};
use super::words::{self, Step, Unopened};
use super::{Listed, Op, Refused, shown_name};
use crate::product::PRODUCT;

/// The most bytes of a text file read_file reads.
pub const MOST_TEXT_BYTES: u64 = 1 << 20;

/// What read_file was asked: the file, and which of its lines or pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    pub path: PathBuf,
    /// The first line, from 1.
    pub offset: usize,
    pub limit: usize,
    /// The first page, from 0, and how many.
    pub first: usize,
    pub count: usize,
}

/// read_file's arguments, a relative path taken from the thread's first folder: what it asks,
/// or what the assistant is told of a call it got wrong. The path is the assistant's string,
/// shown to no one: only what it resolves to is judged or named.
pub fn asked(arguments: &Value, folders: &[Listed]) -> Result<Asked, String> {
    let Some(path) = arguments.get("path").and_then(Value::as_str) else {
        return Err("read_file needs a path.".into());
    };
    let path = PathBuf::from(path);
    let path = match path.is_absolute() {
        true => path,
        false => match folders.first() {
            Some(folder) => folder.root().join(path),
            None => {
                return Err(
                    "Not done: this task has no folder, so give the file's full path.".into(),
                );
            }
        },
    };
    let number = |name: &str, default: usize| match arguments.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => (value.as_u64())
            .filter(|&value| value >= 1)
            .map(|value| value as usize)
            .ok_or(format!("{name} takes a whole number from 1.")),
    };
    let (offset, limit) = (number("offset", 1)?, number("limit", LINES)?.min(LINES));
    let (first, count) = match arguments.get("pages") {
        None | Some(Value::Null) => (0, MOST_PAGES),
        Some(pages) => pages_asked(pages)?,
    };
    Ok(Asked {
        path,
        offset,
        limit,
        first,
        count,
    })
}

/// A `pages` of "31-60", "31" or 31: its first page from 0, and how many, at most `MOST_PAGES`.
fn pages_asked(pages: &Value) -> Result<(usize, usize), String> {
    let wrong =
        || format!("pages takes a page or a range of at most {MOST_PAGES}, such as \"31-60\".");
    let text = match pages {
        Value::Number(page) => page.to_string(),
        Value::String(text) => text.trim().to_string(),
        _ => return Err(wrong()),
    };
    let page = |part: &str| part.trim().parse::<usize>().ok().filter(|&page| page >= 1);
    let (from, to) = match text.split_once(['-', '\u{2013}']) {
        Some((from, to)) => (page(from).ok_or_else(wrong)?, page(to).ok_or_else(wrong)?),
        None => {
            let only = page(&text).ok_or_else(wrong)?;
            (only, only)
        }
    };
    match to >= from {
        true => Ok((from - 1, (to - from + 1).min(MOST_PAGES))),
        false => Err(wrong()),
    }
}

/// What a read came to, off the UI thread: the file's name, as its step names it, and what was
/// read of it, or why not.
#[derive(Debug)]
pub struct Read {
    pub name: String,
    pub got: Result<Got, NotRead>,
}

#[derive(Debug)]
pub enum Got {
    /// The answer's text, its lines numbered.
    Text(String),
    Picture(Picture),
    /// A PDF's bytes, for the engine's reader.
    Pdf(Vec<u8>),
}

/// Why a file wasn't read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotRead {
    Refused(Refused),
    Unread(Unread),
    /// A HEIC picture, which nothing memory-safe decodes yet.
    Heif,
    /// Not text, a PDF or a picture read_file reads.
    Other,
    /// A text file over `MOST_TEXT_BYTES`.
    TooLarge,
    /// The system wouldn't give its bytes.
    Failed,
}

/// Reads what `asked` names within `reach`. Off the UI thread.
pub fn read(asked: &Asked, reach: &Reach) -> Read {
    let shown = shown_name(&asked.path);
    let (fd, name) = match give::open(&asked.path, reach) {
        Ok(held) => held,
        Err(why) => {
            return Read {
                name: shown,
                got: Err(NotRead::Refused(why)),
            };
        }
    };
    let name = name.to_string_lossy().into_owned();
    let mut file = std::fs::File::from(fd);
    let size = file.metadata().map(|data| data.len()).unwrap_or(0);
    let mut head = [0; 16];
    let read = (|| {
        let mut took = 0;
        while took < head.len() {
            match file.read(&mut head[took..])? {
                0 => break,
                more => took += more,
            }
        }
        std::io::Result::Ok(took)
    })();
    let Ok(took) = read else {
        return Read {
            name,
            got: Err(NotRead::Failed),
        };
    };
    let rest = |file: &mut std::fs::File, most: u64| {
        let mut bytes = head[..took].to_vec();
        (file.take(most.saturating_sub(took as u64)))
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|_| NotRead::Failed)
    };
    let got = match read::kind_of(&head[..took]) {
        Kind::Pdf => read::pdf_fits(size)
            .map_err(NotRead::Unread)
            .and_then(|()| rest(&mut file, read::MOST_PDF_BYTES))
            .map(Got::Pdf),
        Kind::Image(format) => (rest(&mut file, u64::MAX))
            .and_then(|bytes| read::picture(&bytes, format).map_err(NotRead::Unread))
            .map(Got::Picture),
        Kind::Heif => Err(NotRead::Heif),
        // The head is too short to judge text by: the whole file is.
        Kind::Text | Kind::Other if size > MOST_TEXT_BYTES => Err(NotRead::TooLarge),
        Kind::Text | Kind::Other => {
            (rest(&mut file, MOST_TEXT_BYTES)).and_then(|bytes| match read::kind_of(&bytes) {
                Kind::Text => Ok(Got::Text(lines(
                    std::str::from_utf8(&bytes).unwrap_or_default(),
                    asked,
                ))),
                _ => Err(NotRead::Other),
            })
        }
    };
    Read { name, got }
}

/// A text's lines from `asked.offset`, numbered, up to its limit and an answer's room, with
/// where to read on when there's more.
fn lines(text: &str, asked: &Asked) -> String {
    let all: Vec<&str> = text.lines().collect();
    if all.is_empty() {
        return "(The file is empty.)".into();
    }
    let total = all.len();
    if asked.offset > total {
        return format!(
            "[The file has {total} lines, so none from line {}.]",
            asked.offset
        );
    }
    let mut answer = String::new();
    let mut last = asked.offset - 1;
    for (index, line) in all
        .iter()
        .enumerate()
        .skip(asked.offset - 1)
        .take(asked.limit)
    {
        let numbered = format!("{}\t{line}\n", index + 1);
        if !answer.is_empty() && answer.len() + numbered.len() > TEXT_ROOM {
            break;
        }
        answer.push_str(&numbered);
        last = index + 1;
    }
    if last < total {
        answer.push_str(&format!(
            "[Lines {}\u{2013}{last} of {total}. Read on with offset {}.]",
            asked.offset,
            last + 1
        ));
    }
    answer
}

/// A read's step line in the thread: the done line, naming the pages of a PDF read that isn't
/// the whole of it (PD1), or a failed one with the table's reason where it has one.
pub fn line(name: &str, read: Result<Option<(usize, usize, usize)>, NotRead>) -> String {
    let step = Step::Read(name);
    match read {
        Ok(None) => words::step(step, true),
        Ok(Some(pages)) => words::read_pages(name, pages),
        Err(NotRead::Refused(why)) => words::refused(step, why),
        Err(NotRead::Heif) => words::refused(step, Refused::NotOpened),
        Err(NotRead::Other) => words::refused(step, Refused::NotText),
        Err(NotRead::TooLarge) => words::refused(step, Refused::TooLarge),
        Err(NotRead::Unread(why)) => match unopened(why) {
            Some(why) => words::unopened(step, why),
            None => words::failed(step),
        },
        Err(NotRead::Failed) => words::failed(step),
    }
}

/// The table's reason a PDF or picture wasn't read, for one it has a row for.
fn unopened(why: Unread) -> Option<Unopened> {
    Some(match why {
        Unread::Locked => Unopened::Locked,
        Unread::Damaged => Unopened::Damaged,
        Unread::TimedOut => Unopened::TimedOut,
        Unread::Bytes => Unopened::PdfOver,
        Unread::Pixels => Unopened::PixelsOver,
        Unread::Broken => Unopened::NotPicture,
        Unread::Stopped | Unread::Past { .. } => return None,
    })
}

/// What the assistant is told of a read that wasn't done.
pub fn told(not: NotRead) -> String {
    match not {
        NotRead::Refused(Refused::TooLarge) | NotRead::TooLarge => {
            "Not done: read_file reads text files of at most 1 MB.".into()
        }
        NotRead::Other => words::told(Op::Read, Refused::NotText),
        NotRead::Refused(why) => words::told(Op::Read, why),
        NotRead::Heif => format!(
            "Not done: {PRODUCT} can't open HEIC pictures yet. Ask the user for a PNG or JPEG copy, or go on without it."
        ),
        NotRead::Unread(Unread::Stopped) => "Not done: the read was stopped.".into(),
        NotRead::Unread(Unread::Past { pages, from }) => {
            format!("Not done: this PDF has {pages} pages, so none from page {from}.")
        }
        NotRead::Unread(why) => unopened(why).map(words::unopened_told).unwrap_or_default(),
        NotRead::Failed => "Not done: the file couldn't be read. Try again.".into(),
    }
}

/// The real path of a thread's uploads, under the tasks' folder, which the never list holds.
pub fn uploads(tasks: &Path, agent: &str) -> PathBuf {
    tasks.join("Uploads").join(agent)
}

#[cfg(test)]
mod tests;
