//! Sort Today Tabs by Site in a window (#2344), by `crate::sort_by_site`'s rule. The core moves the
//! rows, and they slide to their new places on the list's reorder motion. Nothing loads, unloads or
//! closes, and the tab shown stays shown.

use gpui::Context;

use super::{Edit, Shell, What};
use crate::sort_by_site::{moves, sorted};

impl Shell {
    /// The Today rows of `space`, each a split's first tab or a tab alone, with how many tabs it
    /// has, top to bottom.
    fn today_rows(&self, space: u64) -> Vec<(u64, usize)> {
        let Some(entry) = self.spaces.iter().find(|entry| entry.id == space) else {
            return Vec::new();
        };
        let (_, today) = entry.rows();
        (today.into_iter())
            .map(|row| (row, entry.panes(row).len()))
            .collect()
    }

    /// The Today rows of `space` sorted by site, if that would change their order. A split view
    /// joins no site: its pages can be on different sites.
    pub(super) fn sorted_by_site(&self, space: u64) -> Option<Vec<u64>> {
        let rows = self.today_rows(space);
        let sites: Vec<(u64, Option<String>)> = (rows.iter())
            .map(|&(row, tabs)| {
                (
                    row,
                    (tabs == 1)
                        .then(|| self.browser.url_site(&self.page(row).url))
                        .flatten(),
                )
            })
            .collect();
        let order = sorted(&sites);
        (!order.iter().eq(rows.iter().map(|(row, _)| row))).then_some(order)
    }

    /// Puts the Today rows of `space` that `order` names in its order, in the places they hold
    /// among the others, which stay where they are. Returns the order they were in, or None if
    /// nothing moved.
    pub(super) fn order_today(
        &mut self,
        space: u64,
        order: &[u64],
        cx: &mut Context<Self>,
    ) -> Option<Vec<u64>> {
        let rows = self.today_rows(space);
        let named = |row: &u64| order.contains(row);
        let was: Vec<u64> = rows.iter().map(|&(row, _)| row).filter(named).collect();
        let mut kept = order.iter().filter(|row| was.contains(row));
        let target: Vec<u64> = (rows.iter())
            .map(|&(row, _)| match named(&row) {
                true => kept.next().copied().unwrap_or(row),
                false => row,
            })
            .collect();
        let moves = moves(&rows, &target);
        if moves.is_empty() {
            return None;
        }
        for (row, index) in moves {
            self.browser.move_tab(row, false, 0, index, cx);
        }
        Some(was)
    }

    /// Sort Today Tabs by Site, in the Space shown: one step for ⌘Z.
    pub(super) fn sort_by_site(&mut self, cx: &mut Context<Self>) {
        let space = self.space;
        let Some(order) = self.sorted_by_site(space) else {
            return;
        };
        let Some(rows) = self.order_today(space, &order, cx) else {
            return;
        };
        // check.sh waits for this line.
        log!("browser: sorted by site {space}");
        self.keep(What::SORT_BY_SITE, Some(Edit::Order { space, rows }), cx);
    }
}
