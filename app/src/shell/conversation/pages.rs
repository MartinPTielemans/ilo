//! A stretch's steps by the page they acted on (spec-thread-stage.md, The log): a page's line, its
//! icon and title, with its steps under it. Going to a page has no line of its own, the page's line
//! says it, unless it failed or is under way. What isn't on one page (a thought, the assistant's
//! words, a helper, a step on several tabs) stands alone where it came, between pages.
use gpui::SharedString;

use crate::agents::task::{TraceItem, TraceStep};

/// A stretch's item as the log draws it: alone, by its place in the stretch, or a page.
pub(super) enum Paged<'a> {
    Item(usize, &'a TraceItem),
    Page(Page<'a>),
}

/// A page the stretch acted on, and its steps there in order.
pub(super) struct Page<'a> {
    /// Its first step's, which keeps the user's opening of it.
    pub key: usize,
    /// Its tab, from the first of its steps that names one: going to a page in a new tab names
    /// none, the steps after it do.
    pub tab: Option<u64>,
    /// The site it went to or was on, which its line says while its tab has no title.
    pub site: SharedString,
    pub steps: Vec<&'a TraceStep>,
}

impl Page<'_> {
    /// The steps that have lines under it.
    pub fn shown(&self) -> impl Iterator<Item = &TraceStep> {
        let went = |step: &TraceStep| step.page.as_ref().is_some_and(|page| page.went);
        (self.steps.iter().copied()).filter(move |step| !went(step) || step.failed || step.running)
    }
}

/// `items` by page: a step joins the page before it while it acts on the same tab and didn't go
/// somewhere new.
pub(super) fn paged(items: &[TraceItem]) -> Vec<Paged<'_>> {
    let mut paged: Vec<Paged> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let on = match item {
            TraceItem::Step(step) => step.page.as_ref().map(|on| (step, on)),
            _ => None,
        };
        let Some((step, on)) = on else {
            paged.push(Paged::Item(index, item));
            continue;
        };
        match paged.last_mut() {
            Some(Paged::Page(page))
                if !on.went && (page.tab.is_none() || on.tab.is_none() || page.tab == on.tab) =>
            {
                page.tab = page.tab.or(on.tab);
                page.steps.push(step);
            }
            _ => paged.push(Paged::Page(Page {
                key: step.key,
                tab: on.tab,
                site: on.site.clone(),
                steps: vec![step],
            })),
        }
    }
    paged
}

/// How many strands the work split into, at `at`, the first of a run of helpers' lines, whose
/// line says so over them; None at any other place.
pub(super) fn split_at(paged: &[Paged], at: usize) -> Option<usize> {
    let helper = |each: &Paged| matches!(each, Paged::Item(_, TraceItem::Helper { .. }));
    let first = helper(paged.get(at)?) && (at == 0 || !helper(&paged[at - 1]));
    first.then(|| paged[at..].iter().take_while(|each| helper(each)).count())
}

/// The page the assistant is on, by its key: a working stretch's last page. A stretch that's done
/// is on none, so each of its pages folds to its one line.
pub(super) fn current(paged: &[Paged], running: bool) -> Option<usize> {
    let last = paged.iter().rev().find_map(|each| match each {
        Paged::Page(page) => Some(page.key),
        Paged::Item(..) => None,
    });
    last.filter(|_| running)
}

/// The pages a stretch visited, once each in the order it first came to them, by tab, or by site
/// for a page whose tab isn't known.
pub(super) fn visited<'a>(paged: &'a [Paged]) -> Vec<&'a Page<'a>> {
    let mut visited: Vec<&Page> = Vec::new();
    for page in paged.iter().filter_map(|each| match each {
        Paged::Page(page) => Some(page),
        Paged::Item(..) => None,
    }) {
        let same = |seen: &&Page| match (seen.tab, page.tab) {
            (Some(seen), Some(tab)) => seen == tab,
            _ => seen.site == page.site,
        };
        if !visited.iter().any(same) {
            visited.push(page);
        }
    }
    visited
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::task::{OnPage, Opens};

    fn step(key: usize, page: Option<(Option<u64>, &str, bool)>) -> TraceItem {
        TraceItem::Step(TraceStep {
            key,
            glyph: "icons/globe.svg",
            title: format!("step {key}").into(),
            description: "".into(),
            running: false,
            failed: false,
            opens: Opens::Nothing,
            under: "".into(),
            page: page.map(|(tab, site, went)| OnPage {
                tab,
                site: site.to_string().into(),
                went,
            }),
            says: "".into(),
        })
    }

    fn keys(paged: &[Paged]) -> Vec<String> {
        (paged.iter())
            .map(|each| match each {
                Paged::Item(index, _) => format!("item {index}"),
                Paged::Page(page) => format!(
                    "page {} [{}]",
                    page.site,
                    (page.steps.iter().map(|step| step.key.to_string()))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            })
            .collect()
    }

    /// Opening a page starts it, the steps on its tab after that are under it, and going somewhere
    /// new starts the next; a thought between stands alone between them.
    #[test]
    fn a_stretchs_steps_go_under_the_page_they_acted_on() {
        let items = [
            step(0, Some((None, "a.example", true))),
            step(1, Some((Some(4), "a.example", false))),
            step(2, Some((Some(4), "a.example", false))),
            step(3, None),
            step(4, Some((Some(4), "b.example", true))),
            step(5, Some((Some(4), "b.example", false))),
            step(6, Some((Some(7), "c.example", false))),
        ];
        assert_eq!(
            keys(&paged(&items)),
            [
                "page a.example [0 1 2]",
                "item 3",
                "page b.example [4 5]",
                "page c.example [6]"
            ]
        );
    }

    /// Going to a page has no line of its own, the page's line says it, unless it failed or is
    /// under way, which the user needs to see.
    #[test]
    fn going_to_a_page_shows_only_when_it_failed_or_is_under_way() {
        let mut items = vec![
            step(0, Some((None, "a.example", true))),
            step(1, Some((Some(4), "a.example", false))),
        ];
        let shown = |items: &[TraceItem]| match &paged(items)[0] {
            Paged::Page(page) => page.shown().map(|step| step.key).collect::<Vec<_>>(),
            Paged::Item(..) => unreachable!(),
        };
        assert_eq!(shown(&items), [1]);
        for failed in [true, false] {
            if let TraceItem::Step(went) = &mut items[0] {
                went.failed = failed;
                went.running = !failed;
            }
            assert_eq!(shown(&items), [0, 1]);
        }
    }

    /// While it works the assistant is on its last page, which stays open; once it's done it's on
    /// none, and every page folds to its line.
    #[test]
    fn the_page_its_on_is_a_working_stretchs_last() {
        let items = [
            step(0, Some((Some(1), "a.example", true))),
            step(1, Some((Some(1), "b.example", true))),
            step(2, None),
        ];
        let paged = paged(&items);
        assert_eq!(current(&paged, true), Some(1));
        assert_eq!(current(&paged, false), None);
    }

    /// The split's line stands over the first of a run of helpers and counts that run alone.
    #[test]
    fn the_split_counts_the_helpers_under_it() {
        let helper = |key| TraceItem::Helper {
            step: match step(key, None) {
                TraceItem::Step(step) => step,
                _ => unreachable!(),
            },
            name: "".into(),
            tab: None,
        };
        let items = [
            step(0, None),
            helper(1),
            helper(2),
            helper(3),
            step(4, None),
            helper(5),
        ];
        let paged = paged(&items);
        let splits: Vec<_> = (0..paged.len()).map(|at| split_at(&paged, at)).collect();
        assert_eq!(splits, [None, Some(3), None, None, None, Some(1)]);
    }

    /// The done fold shows each page once, the first time it came to it.
    #[test]
    fn a_page_visited_twice_shows_once_in_the_fold() {
        let items = [
            step(0, Some((Some(1), "a.example", true))),
            step(1, Some((Some(2), "b.example", true))),
            step(2, Some((Some(1), "a.example", true))),
            step(3, Some((None, "c.example", true))),
        ];
        let paged = paged(&items);
        let sites: Vec<_> = (visited(&paged).iter())
            .map(|page| page.site.to_string())
            .collect();
        assert_eq!(sites, ["a.example", "b.example", "c.example"]);
    }
}
