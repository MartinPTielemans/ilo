//! A task's access: how far its agent may act on pages without the user. ilo runs what's inside it
//! and refuses what's above it, never asking, and only the user changes it (#4086). What each one
//! refuses is a row of this table, so nothing else decides.

/// A task's access, lowest first, which their order follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// It opens pages, reads, scrolls and takes pictures, and changes nothing.
    Look,
    /// It types, presses, fills in and sends forms, but not what ilo recognises as paying, placing
    /// an order, sending a message as the user, or a bank's site. Where every task starts.
    #[default]
    Work,
    /// Everything the user could do on a page. Set by the user for one task, never a default and
    /// never remembered.
    Full,
}

impl Tier {
    /// Every tier, lowest first, as its menu lists them.
    pub const ALL: [Tier; 3] = [Tier::Look, Tier::Work, Tier::Full];

    /// Its name, as its chip, its menu and an ilo line say it.
    pub fn label(self) -> &'static str {
        match self {
            Tier::Look => "Look only",
            Tier::Work => "Work on pages",
            Tier::Full => "Full access",
        }
    }

    /// The tier one up, which a refusal offers, or None from the top.
    pub fn up(self) -> Option<Tier> {
        match self {
            Tier::Look => Some(Tier::Work),
            Tier::Work => Some(Tier::Full),
            Tier::Full => None,
        }
    }
}
