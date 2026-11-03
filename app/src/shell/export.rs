//! Spaces as Markdown lists of links, which the user saves where they pick: one Space's pinned tabs,
//! folders and Today tabs from its menu (docs/ux-spec.md › Spaces › Renaming and deleting), or
//! every Space's favorites and pinned tabs from Settings (docs/ux-spec.md › Settings).

use gpui::Context;

use super::{Shell, Space};

/// A row as it's written: a link, or a folder and what it holds.
enum Item {
    Link(String, String),
    Folder(String, Vec<Item>),
}

impl Shell {
    /// Asks where to save `space` as Markdown, named for it, and saves it there. The user's alone:
    /// it's a file they keep, so no agent tool asks for it.
    pub(super) fn export_space(&mut self, space: u64, cx: &mut Context<Self>) {
        let Some(each) = self.spaces.iter().find(|each| each.id == space) else {
            return;
        };
        let (markdown, links) = self.space_markdown(each);
        let name = file_name(&each.name);
        self.browser.save_markdown(&name, markdown, links, cx);
    }

    /// Asks where to save every Space of the user's as Markdown, and saves it there, for
    /// Settings' Export…. The user's alone, as Settings is.
    pub fn export_spaces(&mut self, cx: &mut Context<Self>) {
        let (markdown, links) = self.links_markdown();
        self.browser.save_markdown("Spaces.md", markdown, links, cx);
    }

    /// `space` as Markdown, its name a heading over its pinned rows, folders holding theirs nested
    /// under them, then its Today tabs, and how many links it holds.
    fn space_markdown(&self, space: &Space) -> (String, usize) {
        let mut markdown = format!("# {}\n", text(space_name(space)));
        let mut links = 0;
        let today = self.rows_in(space, &space.today, 0);
        for (heading, items) in [
            ("Pinned", self.rows_in(space, &space.pinned, 0)),
            ("Today", today),
        ] {
            if !items.is_empty() {
                markdown.push_str(&format!("\n## {heading}\n\n"));
                links += write(&mut markdown, &items, 0);
            }
        }
        (markdown, links)
    }

    /// Every Space of the user's, top to bottom, as Markdown, and how many links it holds. An
    /// agent's Spaces are the agent's, and Today tabs come and go, so neither is written.
    pub fn links_markdown(&self) -> (String, usize) {
        let mut markdown = String::from("# Spaces\n");
        let mut links = 0;
        for space in self.spaces.iter().filter(|space| space.agent.is_empty()) {
            markdown.push_str(&format!("\n## {}\n", text(space_name(space))));
            let favorites: Vec<Item> = self
                .favorites_of(space)
                .into_iter()
                .filter_map(|tab| self.link(tab))
                .collect();
            let pinned = self.rows_in(space, &space.pinned, 0);
            for (heading, items) in [("Favorites", favorites), ("Pinned", pinned)] {
                if !items.is_empty() {
                    markdown.push_str(&format!("\n### {heading}\n\n"));
                    links += write(&mut markdown, &items, 0);
                }
            }
        }
        (markdown, links)
    }

    /// The rows of `rows` that `folder` holds, or those outside folders for 0, top to bottom, each
    /// tab of a split view a link of its own.
    fn rows_in(&self, space: &Space, rows: &[u64], folder: u64) -> Vec<Item> {
        let later = |row: &u64| {
            space
                .splits
                .iter()
                .any(|split| split.tabs[1..].contains(row))
        };
        let rows = rows
            .iter()
            .filter(|row| !later(row) && space.folder_of(**row) == folder);
        rows.flat_map(|&row| match space.folders.folders.get(&row) {
            Some((name, _)) => {
                vec![Item::Folder(
                    name.to_string(),
                    self.rows_in(space, &space.pinned, row),
                )]
            }
            None => space
                .panes(row)
                .into_iter()
                .filter_map(|tab| self.link(tab))
                .collect(),
        })
        .collect()
    }

    /// `tab` as a link: a pinned tab's or a favorite's to the URL it goes back to, else to its
    /// page's.
    fn link(&self, tab: u64) -> Option<Item> {
        let mut url = self.browser.pinned_url(tab);
        if url.is_empty() {
            url = self.url(tab).to_string();
        }
        (!url.is_empty()).then(|| Item::Link(self.title(tab).to_string(), url))
    }
}

fn space_name(space: &Space) -> &str {
    if space.name.is_empty() {
        "Space"
    } else {
        space.name.as_ref()
    }
}

/// Writes `items` as a list, nested `depth` deep, and returns how many links it wrote.
fn write(markdown: &mut String, items: &[Item], depth: usize) -> usize {
    let indent = "  ".repeat(depth);
    items.iter().fold(0, |links, item| match item {
        Item::Link(title, url) => {
            markdown.push_str(&format!("{indent}- [{}]({})\n", text(title), href(url)));
            links + 1
        }
        Item::Folder(name, held) => {
            markdown.push_str(&format!("{indent}- {}\n", text(name)));
            links + write(markdown, held, depth + 1)
        }
    })
}

/// `name` as Markdown text, which a bracket, a backslash or emphasis's marks would otherwise change.
fn text(name: &str) -> String {
    let mut text = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '\\' | '[' | ']' | '*' | '_' | '`' | '#' | '<' | '>') {
            text.push('\\');
        }
        text.push(if c == '\n' { ' ' } else { c });
    }
    text
}

/// `url` as a link's destination, which a space or a parenthesis would otherwise end.
fn href(url: &str) -> String {
    let mut href = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            ' ' => href.push_str("%20"),
            '(' => href.push_str("%28"),
            ')' => href.push_str("%29"),
            '<' => href.push_str("%3C"),
            '>' => href.push_str("%3E"),
            c => href.push(c),
        }
    }
    href
}

/// The file a Space saves as: its name, without what a file's name can't hold on any of the
/// systems, then .md.
fn file_name(name: &str) -> String {
    let name: String = (name.chars())
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '-'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_start_matches('.');
    format!("{}.md", if name.is_empty() { "Space" } else { name })
}

#[cfg(test)]
mod tests;
