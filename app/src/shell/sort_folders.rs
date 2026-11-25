//! Sort Tabs into Folders (#4252) in a window: which of a Space's tabs a sort hands over, and the
//! checked answer put in place as one step for ⌘Z. The run, and what the broom shows, are the
//! browser's (`crate::broom`); the shell lists what it's asked for and moves what it's told.

use gpui::{Context, SharedString};

use super::{Edit, Shell, What};
use crate::broom::{self, Folder, Tab};

impl Shell {
    /// The tabs a sort of `space` would hand over, in the list's order: its today's tabs outside a
    /// folder and a split view, at a web page, which the agent's own Space has none of. Allocates
    /// nothing, as the broom's answer reads it on ⌘T.
    fn sortable(&self, space: u64) -> impl Iterator<Item = u64> + '_ {
        let entry = (self.spaces.iter()).find(|entry| entry.id == space && entry.agent.is_empty());
        (entry.into_iter())
            .flat_map(|entry| entry.today.iter().map(move |&tab| (entry, tab)))
            .filter(|(entry, tab)| !entry.splits.iter().any(|split| split.tabs.contains(tab)))
            .filter(|(entry, tab)| !entry.folders.parents.contains_key(tab))
            .filter(|(_, tab)| (self.tabs.get(tab)).is_some_and(|each| web(&each.page.url)))
            .map(|(_, tab)| tab)
    }

    /// Whether `space` is one a sort could be of: a Space the window knows, and the user's own.
    pub fn broom_space(&self, space: u64) -> bool {
        (self.spaces.iter()).any(|entry| entry.id == space && entry.agent.is_empty())
    }

    /// Whether a sort of `space` has the `FEWEST` tabs it needs, looking no further than the last
    /// of them.
    pub fn broom_sorts(&self, space: u64) -> bool {
        self.sortable(space).nth(broom::FEWEST - 1).is_some()
    }

    /// Whether `tab`'s page counts for a sort, as `broom_sorts` reads it: the broom's answer
    /// changes only when this flips.
    pub fn broom_counts(&self, tab: u64) -> bool {
        (self.tabs.get(&tab)).is_some_and(|each| web(&each.page.url))
    }

    fn broom_rows(&self, space: u64) -> Vec<(u64, String)> {
        (self.sortable(space))
            .filter_map(|tab| Some((tab, web_host(&self.tabs.get(&tab)?.page.url)?)))
            .collect()
    }

    /// What a sort of `space` hands over, each tab's title as the sidebar shows it and its host
    /// alone, with their ids: the first `broom::MOST` of them.
    pub fn broom_tabs(&self, space: u64) -> Vec<(u64, Tab)> {
        (self.broom_rows(space).into_iter().take(broom::MOST))
            .map(|(tab, host)| {
                (
                    tab,
                    Tab {
                        title: self.title(tab).to_string(),
                        host,
                    },
                )
            })
            .collect()
    }

    /// The today's folders of `space`, by id and name, in the list's order.
    fn today_folders(&self, space: u64) -> Vec<(u64, SharedString)> {
        let Some(entry) = self.spaces.iter().find(|entry| entry.id == space) else {
            return Vec::new();
        };
        let mut found: Vec<(u64, SharedString)> = Vec::new();
        for tab in entry.rows().1 {
            let Some(&folder) = entry.folders.parents.get(&tab) else {
                continue;
            };
            let name = entry
                .folders
                .folders
                .get(&folder)
                .map(|(name, _)| name.clone());
            if !found.iter().any(|(each, _)| *each == folder) {
                found.push((folder, name.unwrap_or_default()));
            }
        }
        found
    }

    /// Puts a sort's checked `folders` of `handed`, the tabs as they were handed, in `space`: a
    /// folder named as a today's folder there, letter case aside, takes its tabs into the first of
    /// them, and each other one is made in today's list. One step for ⌘Z. Returns how many tabs
    /// moved and how many folders took them, or None if nothing moved: a tab handed isn't one a
    /// sort would hand over now, the answer makes too many folders, or the core makes none.
    pub fn sort_into_folders(
        &mut self,
        space: u64,
        handed: &[u64],
        folders: Vec<Folder>,
        cx: &mut Context<Self>,
    ) -> Option<(usize, usize)> {
        let now: Vec<u64> = self
            .broom_rows(space)
            .into_iter()
            .map(|(tab, _)| tab)
            .collect();
        if !handed.iter().all(|tab| now.contains(tab)) {
            return None;
        }
        let today = self.today_folders(space);
        let names: Vec<&str> = today.iter().map(|(_, name)| name.as_ref()).collect();
        let placed = broom::placed(folders, &names)?;
        let tabs = |folder: &Folder| -> Option<Vec<u64>> {
            folder
                .tabs
                .iter()
                .map(|&at| handed.get(at).copied())
                .collect()
        };
        let (mut new, mut joins) = (Vec::new(), Vec::new());
        for (into, folder) in &placed {
            match into {
                None => new.push((folder.name.clone(), tabs(folder)?)),
                Some(at) => joins.push((today[*at].0, tabs(folder)?)),
            }
        }
        let rows = self
            .today_rows(space)
            .into_iter()
            .map(|(row, _)| row)
            .collect();
        let made = self.browser.make_today_folders(space, &new, &joins, cx)?;
        let moved = (new.iter().map(|(_, tabs)| tabs.len()))
            .chain(joins.iter().map(|(_, tabs)| tabs.len()))
            .sum();
        let joined = joins.into_iter().flat_map(|(_, tabs)| tabs).collect();
        let edit = Edit::Unsort {
            space,
            made,
            joined,
            rows,
        };
        self.keep(What::SORT_TABS, Some(edit), cx);
        Some((moved, placed.len()))
    }
}

/// Whether `url` is a web page's, without reading it further: Chromium gives a scheme in lower case.
fn web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// The host of `url` if it's a web page's, the one part of its address a sort hands over.
fn web_host(url: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    matches!(url.scheme(), "http" | "https")
        .then(|| url.host_str().map(str::to_owned))
        .flatten()
}

#[cfg(test)]
mod tests;
