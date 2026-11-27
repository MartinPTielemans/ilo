//! A stretch's steps by the page they acted on, for the line over a run of helpers and the kept
//! log's page records. The trace draws its steps flat at one indent, with no page's line over
//! them. What isn't on one page (a thought, the assistant's words, a helper, a step on several
//! tabs) stands alone where it came.
use gpui::SharedString;

use crate::agents::task::{TraceItem, TraceStep};

/// A stretch's item as the log draws it: alone, by its place in the stretch, or a page.
pub(crate) enum Paged<'a> {
    Item(usize, &'a TraceItem),
    Page(Page<'a>),
}

/// A page the stretch acted on, and its steps there in order.
pub(crate) struct Page<'a> {
    /// Its tab, from the first of its steps that names one: going to a page in a new tab names
    /// none, the steps after it do.
    pub tab: Option<u64>,
    /// The site it went to or was on, which its line says while it has no title.
    pub site: SharedString,
    /// The title the page had as its last step to see one ran, or empty: its line keeps the page
    /// it was, though its tab has gone on to another.
    pub title: SharedString,
    pub steps: Vec<&'a TraceStep>,
}

impl Page<'_> {
    /// The steps the kept log keeps under the page's record: going to it is the page's record,
    /// unless it failed or is under way.
    pub fn shown(&self) -> impl Iterator<Item = &TraceStep> {
        let went = |step: &TraceStep| step.page.as_ref().is_some_and(|page| page.went);
        (self.steps.iter().copied()).filter(move |step| !went(step) || step.failed || step.running)
    }
}

/// `items` by page: a step joins the page before it while it acts on the same tab and didn't go
/// somewhere new.
pub(crate) fn paged(items: &[TraceItem]) -> Vec<Paged<'_>> {
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
                if !on.title.is_empty() {
                    page.title = on.title.clone();
                }
                page.steps.push(step);
            }
            _ => paged.push(Paged::Page(Page {
                tab: on.tab,
                site: on.site.clone(),
                title: on.title.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::task::{OnPage, Opens};

    fn step(key: usize, page: Option<(Option<u64>, &str, bool)>) -> TraceItem {
        TraceItem::Step(TraceStep {
            kind: Default::default(),
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
                title: "".into(),
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

    /// The kept log keeps going to a page as the page's record alone, unless it failed or is
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
            began: None,
            ended: None,
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
}
