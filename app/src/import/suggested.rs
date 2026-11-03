//! Favorites a first launch offers from the history an import read (owner, 2026-10-29): the sites
//! visited most, worked out here from the pages read and never sent anywhere.

use super::found::{Page, Tab};

/// How many it offers at most (ours): two rows of four tiles in the sidebar's width.
pub const MOST: usize = 8;

/// A site visited fewer times than this isn't one the user goes back to.
const LEAST_VISITS: u32 = 3;

/// The sites of `history` visited most, `MOST` at most, each once by its root (`https://host/`),
/// the most visits first: but for those visited fewer than LEAST_VISITS times, those among `skip`,
/// such as the favorites here, and the user's own machine and network, which no one favorites.
pub fn favorites(history: &[Page], skip: &[String]) -> Vec<Tab> {
    let skip: Vec<String> = skip
        .iter()
        .filter_map(|url| site(url))
        .map(|(_, site)| site)
        .collect();
    // Each site with its visits, and the root it opens at, in the order first visited.
    let mut sites: Vec<(String, String, u32)> = Vec::new();
    for page in history {
        let Some((root, site)) = site(&page.url) else {
            continue;
        };
        match sites.iter_mut().find(|(_, each, _)| *each == site) {
            Some((_, _, visits)) => *visits += page.visits,
            None => sites.push((root, site, page.visits)),
        }
    }
    sites.retain(|(_, site, visits)| *visits >= LEAST_VISITS && !skip.contains(site));
    sites.sort_by(|a, b| b.2.cmp(&a.2));
    let tabs = sites.into_iter().take(MOST);
    tabs.map(|(url, _, _)| Tab {
        url,
        name: String::new(),
    })
    .collect()
}

/// A site's name as the row offering it says it: its host, without `www.`.
pub fn site_name(url: &str) -> String {
    site(url).map_or_else(|| url.to_string(), |(_, site)| site)
}

/// `url`'s root and its site, its host without `www.`, for an http(s) page on the web, not the
/// user's own machine or network.
fn site(url: &str) -> Option<(String, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host
        .rsplit_once('@')
        .map_or(host, |(_, host)| host)
        .to_ascii_lowercase();
    let name = host.split(':').next()?;
    let numeric = name.chars().all(|c| c.is_ascii_digit() || c == '.');
    let local = !name.contains('.') || name.ends_with(".local") || name.ends_with(".localhost");
    if name.is_empty() || host.starts_with('[') || numeric || local {
        return None;
    }
    let site = host.strip_prefix("www.").unwrap_or(&host).to_string();
    Some((format!("{scheme}://{host}/"), site))
}
