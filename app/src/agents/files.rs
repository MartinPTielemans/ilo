//! What a task's assistant may do with files on this computer, by its level (#4101). One table, so
//! nothing else decides: the wall's rules and each assistant's start are made from it.

use std::path::PathBuf;

use super::tier::Tier;
use super::wall::Granted;

/// Where a file is: one the user attached, which the assistant gets as ilo's copy, or in the folder
/// the user granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    Attached,
    Folder,
}

impl Place {
    pub const ALL: [Place; 2] = [Place::Attached, Place::Folder];
}

/// What is done to a file or a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Read,
    Search,
    /// A new file, folder or link, a second name for a file in the folder included.
    Make,
    /// A file's contents, written over in whole or in part.
    Change,
    Delete,
    /// A name taken to another place or another name.
    Move,
    /// One file moved over another, which loses the other's name.
    Replace,
}

impl Op {
    pub const ALL: [Op; 7] = [
        Op::Read,
        Op::Search,
        Op::Make,
        Op::Change,
        Op::Delete,
        Op::Move,
        Op::Replace,
    ];
}

/// Whether a task at `tier` may do `op` to a file in `place`: Look only reads, Work on pages also
/// makes and changes, Full access also deletes and moves, and attached files are only read.
pub fn allows(tier: Tier, place: Place, op: Op) -> bool {
    match (place, op) {
        (_, Op::Read | Op::Search) => true,
        (Place::Attached, _) => false,
        (Place::Folder, Op::Make | Op::Change) => tier >= Tier::Work,
        (Place::Folder, Op::Delete | Op::Move | Op::Replace) => tier == Tier::Full,
    }
}

/// The files the user gave a task: a folder they granted, ilo's copies of the files they attached,
/// both or neither. A task with any has its assistant run inside a wall (wall.rs).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Given {
    pub folder: Option<Granted>,
    /// Where ilo's copies of the attached files are.
    pub attached: Option<PathBuf>,
}

impl std::fmt::Debug for Given {
    /// Whether it has each and never where, as what holds one may reach a log (#3656).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Given")
            .field("folder", &self.folder.is_some())
            .field("attached", &self.attached.is_some())
            .finish()
    }
}

impl Given {
    pub fn any(&self) -> bool {
        self.folder.is_some() || self.attached.is_some()
    }

    /// Whether a task with these files is allowed the same at `from` and at `to`: a wall made
    /// for one is then the wall for the other. True of a task with none, and of one whose files
    /// are attached copies alone, which every level only reads.
    pub fn same_rules(&self, from: Tier, to: Tier) -> bool {
        let has = |place| match place {
            Place::Attached => self.attached.is_some(),
            Place::Folder => self.folder.is_some(),
        };
        (Place::ALL.into_iter().filter(|&place| has(place))).all(|place| {
            Op::ALL
                .iter()
                .all(|&op| allows(from, place, op) == allows(to, place, op))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_changes_the_rules_only_of_a_task_with_a_folder() {
        let folder = Given {
            folder: Some(Granted::for_test()),
            attached: Some(PathBuf::from("/nonexistent/attached")),
        };
        let attached = Given {
            folder: None,
            ..folder.clone()
        };
        for from in Tier::ALL {
            for to in Tier::ALL {
                assert!(Given::default().same_rules(from, to));
                assert!(attached.same_rules(from, to));
                assert_eq!(folder.same_rules(from, to), from == to, "{from:?} {to:?}");
            }
        }
    }

    #[test]
    fn files_printed_name_no_place() {
        let given = Given {
            folder: Some(Granted::for_test()),
            attached: Some(PathBuf::from("/nonexistent/Private Letters")),
        };
        let printed = format!("{given:?}");
        assert_eq!(printed, "Given { folder: true, attached: true }");
    }

    #[test]
    fn every_level_reads_and_searches_both_places() {
        for tier in Tier::ALL {
            for place in Place::ALL {
                assert!(allows(tier, place, Op::Read) && allows(tier, place, Op::Search));
            }
        }
    }

    #[test]
    fn attached_files_are_only_read_at_every_level() {
        for tier in Tier::ALL {
            for op in [Op::Make, Op::Change, Op::Delete, Op::Move, Op::Replace] {
                assert!(!allows(tier, Place::Attached, op), "{tier:?} {op:?}");
            }
        }
    }

    #[test]
    fn each_level_adds_to_the_one_below_in_the_folder() {
        let folder = |tier| {
            Op::ALL
                .into_iter()
                .filter(move |&op| allows(tier, Place::Folder, op))
        };
        let look: Vec<Op> = folder(Tier::Look).collect();
        let work: Vec<Op> = folder(Tier::Work).collect();
        let full: Vec<Op> = folder(Tier::Full).collect();
        assert_eq!(look, [Op::Read, Op::Search]);
        assert_eq!(work, [Op::Read, Op::Search, Op::Make, Op::Change]);
        assert_eq!(full, Op::ALL);
    }
}
