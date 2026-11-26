//! What first run's card draws of the guided view (first-run-v9 §5): the browsers found, the one
//! picked read at once, its kinds with their counts, and an import's kinds as each is written. The
//! card draws it; the view keeps the state, so Settings' import is untouched by it.

use std::sync::Arc;

use gpui::{Context, RenderImage, SharedString};

use super::read::{self, Kind, Tally};
use super::{ImportView, Step};

/// What the card shows of the guided view.
pub(crate) struct Bring {
    /// Each browser found, the one used last first: its name and its app icon, if the system gave
    /// one.
    pub(crate) browsers: Vec<(String, Option<Arc<RenderImage>>)>,
    /// The browser picked, which flies to the card's slot.
    pub(crate) picked: Option<usize>,
    pub(crate) state: Bringing,
}

/// Where the browser picked is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Bringing {
    /// Its files are being read.
    Reading,
    /// What it has, a chip a kind with something in it, each with whether it comes.
    Kinds(Vec<Chip>),
    /// macOS keeps its files from the app until it has Full Disk Access.
    NoAccess,
    /// Its files couldn't be read.
    Unread,
    /// An import of it under way, each kind it brings with whether it's written; `stoppable` while
    /// Cancel can still leave everything out, stalled or not, `done` once all of it is in, and
    /// `stalled` with the view's own sentence while a profile it waits for hasn't opened.
    Running {
        chips: Vec<Chip>,
        stoppable: bool,
        done: bool,
        stalled: Option<SharedString>,
    },
}

/// A kind a browser has: how many, whether it comes, and whether an import has written it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Chip {
    pub(crate) kind: Kind,
    pub(crate) count: usize,
    pub(crate) on: bool,
    pub(crate) written: bool,
}

/// How many of `kind` a browser read has: its own count for each kind, extensions as those listed.
fn count(tally: &Tally, kind: Kind) -> usize {
    match kind {
        Kind::Spaces => tally.spaces,
        Kind::Favorites => tally.favorites,
        Kind::Bookmarks => tally.bookmarks,
        Kind::OpenTabs => tally.open_tabs,
        Kind::ReadingList => tally.reading_list,
        Kind::History => tally.pages,
        Kind::Addresses => tally.addresses,
        Kind::Extensions => tally.extensions,
    }
}

impl ImportView {
    /// Reads the browser used last at once, as the card's step shows its kinds as it comes.
    pub(crate) fn bring_first(&mut self, cx: &mut Context<Self>) {
        if self.guided && matches!(self.step, Step::List) && !self.sources.is_empty() {
            self.pick(0, cx);
        }
    }

    /// The card's Import can be pressed: a browser read, with a kind it has on.
    pub(crate) fn bring_ready(&self) -> bool {
        let Step::Pick {
            source,
            picked,
            read: Some(read),
            ..
        } = &self.step
        else {
            return false;
        };
        self.unread != Some(*source) && (picked.iter()).any(|&kind| count(&read.tally, kind) > 0)
    }

    /// Where the card's chips are drawn is logged as the view's own were, for check.sh to click.
    pub(crate) fn card_drawn(&self) -> crate::settings_rows::Drawn {
        self.drawn_at.clone()
    }

    /// An import is waiting on a profile that hasn't opened, which the view's Try Again asks for again.
    pub(crate) fn bring_stalled(&self) -> bool {
        matches!(&self.step, Step::Running { run, .. } if run.stalled.is_some())
    }

    /// The card offers its Import and Skip: no import is under way or over.
    pub(crate) fn bring_offers(&self) -> bool {
        matches!(
            self.step,
            Step::List | Step::Pick { .. } | Step::NoAccess { .. }
        )
    }

    /// Picks the browser `source` for the card, reading it, unless it's picked already.
    pub(crate) fn bring_from(&mut self, source: usize, cx: &mut Context<Self>) {
        let picked = match &self.step {
            Step::Pick { source, .. } | Step::NoAccess { source, .. } => Some(*source),
            _ => None,
        };
        if source < self.sources.len() && (picked != Some(source) || self.unread.is_some()) {
            self.pick(source, cx);
        }
    }

    /// Leaves `kind` out of what comes from the browser picked, or brings it back.
    pub(crate) fn toggle_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        let (Step::Pick { picked, .. } | Step::NoAccess { picked, .. }) = &mut self.step else {
            return;
        };
        let on = picked.contains(&kind);
        match on {
            true => picked.retain(|each| *each != kind),
            false => picked.push(kind),
        }
        // check.sh reads this line.
        log!("browser: import chip {} {}", kind.label(), !on);
        cx.notify();
    }

    /// Stops the import under way while it can still leave everything out, back at the browser's
    /// kinds as they were picked.
    pub(crate) fn stop_bringing(&mut self, cx: &mut Context<Self>) {
        self.cancel(cx);
        if let Step::Cancelled { source, picked, .. } = &self.step {
            let (source, picked) = (*source, picked.clone());
            self.bringing = None;
            self.kept = Some((source, picked));
            self.pick(source, cx);
        }
    }

    /// What the card draws, asking the system for the browsers' icons at `pixels` square.
    pub(crate) fn card_state(&mut self, pixels: u32) -> Bring {
        // A look without a window (0) keeps the icons it has.
        if pixels != 0 && self.card_icons.0 != pixels {
            let icons = (self.sources.iter())
                .map(|source| self.host.app_icon(source, pixels))
                .collect();
            self.card_icons = (pixels, icons);
        }
        let browsers = (self.sources.iter().enumerate())
            .map(|(at, source)| (source.name(), self.card_icons.1.get(at).cloned().flatten()))
            .collect();
        let (picked, state) = match &self.step {
            Step::Pick { source, .. } if self.unread == Some(*source) => {
                (Some(*source), Bringing::Unread)
            }
            Step::Pick {
                source,
                picked,
                read: Some(read),
                ..
            } => {
                let chips = (read::kinds(&self.sources[*source]).iter())
                    .map(|&kind| Chip {
                        kind,
                        count: count(&read.tally, kind),
                        on: picked.contains(&kind),
                        written: false,
                    })
                    .filter(|chip| chip.count > 0)
                    .collect();
                (Some(*source), Bringing::Kinds(chips))
            }
            Step::Pick { source, .. } => (Some(*source), Bringing::Reading),
            Step::NoAccess { source, .. } => (Some(*source), Bringing::NoAccess),
            Step::Running { source, run, .. } => {
                let written = |kind: Kind| {
                    run.placed
                        && match kind {
                            Kind::History => run.history_left == 0,
                            Kind::Addresses => run.addresses_left == 0,
                            _ => true,
                        }
                };
                let chips = self.chips_brought(|kind| written(kind));
                let stoppable = !run.placed;
                (
                    *source,
                    Bringing::Running {
                        chips,
                        stoppable,
                        done: false,
                        stalled: run.stalled.clone(),
                    },
                )
            }
            Step::Done(_) if self.bringing.is_some() => {
                let source = self.bringing.as_ref().map(|(source, _)| *source);
                let chips = self.chips_brought(|_| true);
                (
                    source,
                    Bringing::Running {
                        chips,
                        stoppable: false,
                        done: true,
                        stalled: None,
                    },
                )
            }
            _ => (None, Bringing::Reading),
        };
        Bring {
            browsers,
            picked,
            state,
        }
    }

    /// The chips of the import begun from the card, each written as `written` says.
    fn chips_brought(&self, written: impl Fn(Kind) -> bool) -> Vec<Chip> {
        let Some((_, chips)) = &self.bringing else {
            return Vec::new();
        };
        (chips.iter())
            .map(|chip| Chip {
                written: chip.on && written(chip.kind),
                ..*chip
            })
            .collect()
    }

    /// Keeps, as the card's import begins, what it brings, for the chips to stay as it runs.
    pub(super) fn keep_bringing(&mut self) {
        let Step::Pick {
            source,
            picked,
            read: Some(read),
            ..
        } = &self.step
        else {
            return;
        };
        if !self.guided {
            return;
        }
        let chips = (read::kinds(&self.sources[*source]).iter())
            .map(|&kind| Chip {
                kind,
                count: count(&read.tally, kind),
                on: picked.contains(&kind),
                written: false,
            })
            .filter(|chip| chip.count > 0)
            .collect();
        self.bringing = Some((*source, chips));
    }
}
