//! Sort Today Tabs by Site (#2344): each site's Today tabs come together at its first tab's place,
//! in the order they were in, and the rest stay where they are. The shell's command and the agents'
//! tool both sort by this, with the engine's sites (bc_url_site), so a user and an agent get the
//! same list.

/// A site as hosts without the engine make one up, the preview's and the tests': the last two
/// labels of an http(s) URL's host. The engine's is Chromium's registry (bc_url_site), which knows
/// that example.co.uk and alice.github.io are sites of their own.
pub fn made_up_site(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let host = rest.split(['/', '?', '#', ':']).next()?;
    let labels: Vec<&str> = host.split('.').collect();
    let site = labels[labels.len().saturating_sub(2)..].join(".");
    (!site.is_empty()).then_some(site)
}

/// `rows` in their new order: a row is its id and its site, None for one that keeps its place,
/// such as a split view or a page that isn't on the web.
pub fn sorted(rows: &[(u64, Option<String>)]) -> Vec<u64> {
    let mut placed: Vec<&str> = Vec::new();
    let mut order = Vec::with_capacity(rows.len());
    for (row, site) in rows {
        let Some(site) = site.as_deref() else {
            order.push(*row);
            continue;
        };
        if placed.contains(&site) {
            continue;
        }
        placed.push(site);
        let same = rows
            .iter()
            .filter(|(_, each)| each.as_deref() == Some(site));
        order.extend(same.map(|(row, _)| *row));
    }
    order
}

/// The moves that put `rows`, each a row's id and how many tabs it has, in `order`: each the row
/// and where it goes among Today's tabs, counted without it, as the core moves a tab with its split
/// (bc_tab_move). Rows already in place don't move.
pub fn moves(rows: &[(u64, usize)], order: &[u64]) -> Vec<(u64, usize)> {
    let mut now = rows.to_vec();
    let mut moves = Vec::new();
    for (place, &row) in order.iter().enumerate() {
        let Some(at) = now.iter().position(|&(each, _)| each == row) else {
            continue;
        };
        if at != place {
            let moved = now.remove(at);
            now.insert(place, moved);
            moves.push((row, now[..place].iter().map(|&(_, tabs)| tabs).sum()));
        }
    }
    moves
}

#[cfg(test)]
mod tests {
    use super::{moves, sorted};

    fn rows(sites: &[&str]) -> Vec<(u64, Option<String>)> {
        (sites.iter().enumerate())
            .map(|(at, site)| (at as u64, (!site.is_empty()).then(|| site.to_string())))
            .collect()
    }

    /// Each site gathers at its first tab's place, its tabs in the order they were in; a site with
    /// one tab, a split and a page off the web ("") stay where they fall.
    #[test]
    fn each_site_gathers_at_its_first_tabs_place() {
        let list = rows(&["a", "b", "a", "c", "", "b", "a", "d"]);
        assert_eq!(sorted(&list), [0, 2, 6, 1, 5, 3, 4, 7]);
    }

    #[test]
    fn sites_already_together_stay_as_they_are() {
        for sites in [
            &["a", "a", "b", "c", "c"][..],
            &["a", "b", "c"],
            &["", "a", "a", ""],
            &[],
        ] {
            let list = rows(sites);
            let order: Vec<u64> = list.iter().map(|(row, _)| *row).collect();
            assert_eq!(sorted(&list), order, "{sites:?}");
        }
        // A second sort changes nothing.
        let list = rows(&["a", "b", "a", "b"]);
        let once = sorted(&list);
        let again: Vec<_> = (once.iter())
            .map(|&row| list[row as usize].clone())
            .collect();
        assert_eq!(sorted(&again), once);
    }

    /// The moves, made as the core makes them, give the order, and a row in place doesn't move.
    #[test]
    fn the_moves_give_the_order() {
        // Row 30 is a split of two tabs.
        let rows = [(10, 1), (20, 1), (30, 2), (40, 1), (50, 1)];
        for order in [
            vec![10, 40, 20, 30, 50],
            vec![50, 40, 30, 20, 10],
            vec![10, 20, 30, 40, 50],
            vec![30, 10, 20, 40, 50],
        ] {
            let mut tabs: Vec<u64> = (rows.iter())
                .flat_map(|&(row, count)| (0..count as u64).map(move |pane| row + pane))
                .collect();
            for (row, index) in moves(&rows, &order) {
                let count = rows.iter().find(|(each, _)| *each == row).unwrap().1 as u64;
                let panes: Vec<u64> = (row..row + count).collect();
                tabs.retain(|tab| !panes.contains(tab));
                tabs.splice(index..index, panes);
            }
            let got: Vec<u64> = tabs.into_iter().filter(|tab| tab % 10 == 0).collect();
            assert_eq!(got, order);
        }
        assert!(moves(&rows, &[10, 20, 30, 40, 50]).is_empty());
        assert_eq!(moves(&rows, &[10, 40, 20, 30, 50]), [(40, 1)]);
    }
}
