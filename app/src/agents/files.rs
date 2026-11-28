//! What a task's assistant may do with files on this computer, by its level (#4101). One table, so
//! nothing else decides: the wall's rules and each assistant's start are made from it.

use super::tier::Tier;

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

#[cfg(test)]
mod tests {
    use super::*;

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
