//! A sortable list (#4452, #4440): the card or row held follows the pointer along its axis, the
//! ones it passes spring one cell aside, and the open slot is where it lands. The model is the
//! same along x, for the Library's Spaces, and along y, for the Space button's menu: everything
//! here is a distance along the list's axis from the list's start, and the host says which axis
//! that is by what it measures.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::Instant;

/// The gap among the cells but the `from`th that `at` is over, with the first cell starting at
/// `first`: how many of the others' middles are before it.
pub fn gap_at(cells: &[f32], first: f32, from: usize, at: f32) -> usize {
    let mut start = first;
    let mut gap = 0;
    for (index, &cell) in cells.iter().enumerate() {
        if index != from && start + cell / 2. < at {
            gap += 1;
        }
        start += cell;
    }
    gap
}

/// Where the held one's middle is with the pointer at `pointer`, held `grab` from its start and
/// `size` long.
pub fn held_middle(pointer: f32, grab: f32, size: f32) -> f32 {
    pointer - grab + size / 2.
}

/// How far after its place the `from`th cell is drawn with its middle at `middle`, for cells
/// `pitch` long from `first`.
pub fn held_offset(middle: f32, first: f32, pitch: f32, from: usize) -> f32 {
    middle - (first + pitch * from as f32 + pitch / 2.)
}

/// How far each of the others goes aside, by its cell, for the `from`th to be in the open slot
/// `gap` among them: one cell for each it has passed.
pub fn shifts_for(count: usize, from: usize, gap: usize, pitch: f32) -> Vec<(usize, f32)> {
    let others = (0..count).filter(|&at| at != from);
    others
        .enumerate()
        .filter_map(|(other, at)| {
            let now = if other < gap { other } else { other + 1 };
            (at != now).then(|| (at, (now as f32 - at as f32) * pitch))
        })
        .collect()
}

/// How far the held one goes from its place to land in the open slot `gap`.
pub fn slot_shift(from: usize, gap: usize, pitch: f32) -> f32 {
    (gap as f32 - from as f32) * pitch
}

/// How far along a spring is `t` seconds in, or None once it's there.
pub type Ease = fn(f32) -> Option<f32>;

/// Where each of the cells has been sent from its place, which its spring goes on to, by what
/// names it. A cell's drawn offset is where it's been sent and the way its spring still has to go,
/// so a new send starts from where it's drawn: a spring in the middle of its way is interrupted
/// without a jump.
pub struct Sent<K> {
    pub to: HashMap<K, f32>,
    /// How far after its place each cell's spring started, and when.
    pub slides: HashMap<K, (f32, Instant)>,
}

impl<K> Default for Sent<K> {
    fn default() -> Self {
        Sent {
            to: HashMap::new(),
            slides: HashMap::new(),
        }
    }
}

impl<K: Copy + Eq + Hash> Sent<K> {
    /// How far after its place `key`'s spring is.
    pub fn slide(&self, key: K, ease: Ease) -> f32 {
        self.slides.get(&key).map_or(0., |&(from, start)| {
            ease(start.elapsed().as_secs_f32()).map_or(0., |done| from * (1. - done))
        })
    }

    /// How far after its place `key` is drawn.
    pub fn drawn(&self, key: K, ease: Ease) -> f32 {
        self.to.get(&key).copied().unwrap_or(0.) + self.slide(key, ease)
    }

    /// Sends each of `keys` to its shift in `to`, none for the ones left out, from where it's drawn,
    /// or for `held`, from where the pointer was holding it. `now` is when the springs start.
    pub fn send(
        &mut self,
        keys: impl IntoIterator<Item = K>,
        to: HashMap<K, f32>,
        held: Option<(K, f32)>,
        now: Instant,
        ease: Ease,
    ) {
        for key in keys {
            let target = to.get(&key).copied().unwrap_or(0.);
            let drawn = match held {
                Some((held, offset)) if held == key => offset,
                _ => self.drawn(key, ease),
            };
            if drawn != target {
                self.slides.insert(key, (drawn - target, now));
            }
        }
        self.to = to;
    }

    /// Drops the springs that are done. Returns whether any still go, and whether the last of
    /// them just did.
    pub fn step(&mut self, ease: Ease) -> (bool, bool) {
        let was = !self.slides.is_empty();
        self.slides
            .retain(|_, (_, start)| ease(start.elapsed().as_secs_f32()).is_some());
        (!self.slides.is_empty(), was && self.slides.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::{gap_at, held_offset, shifts_for, slot_shift};

    /// Three cells 34 long from 57, so their middles are at 74, 108 and 142.
    const CELLS: [f32; 3] = [34.; 3];

    /// The gap is how many of the others' middles the held one's middle is past; its own cell is
    /// the gap it came from.
    #[test]
    fn the_gap_counts_the_others_the_middle_is_past() {
        assert_eq!(gap_at(&CELLS, 57., 1, 0.), 0);
        assert_eq!(gap_at(&CELLS, 57., 1, 80.), 1);
        assert_eq!(gap_at(&CELLS, 57., 1, 108.), 1);
        assert_eq!(gap_at(&CELLS, 57., 1, 150.), 2);
        assert_eq!(gap_at(&CELLS, 57., 0, 120.), 1);
        assert_eq!(gap_at(&CELLS, 57., 0, 216.), 2);
    }

    /// Only the cells passed move, one cell each, toward where the held one was.
    #[test]
    fn the_others_move_one_cell_for_each_the_held_one_passed() {
        assert!(shifts_for(3, 0, 0, 34.).is_empty());
        assert_eq!(shifts_for(3, 0, 1, 34.), [(1, -34.)]);
        assert_eq!(shifts_for(3, 0, 2, 34.), [(1, -34.), (2, -34.)]);
        assert_eq!(shifts_for(3, 2, 0, 34.), [(0, 34.), (1, 34.)]);
        assert_eq!(shifts_for(3, 1, 1, 34.), []);
        assert_eq!(slot_shift(0, 2, 34.), 68.);
        assert_eq!(held_offset(142., 57., 34., 1), 34.);
    }
}
