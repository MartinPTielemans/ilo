//! How often and how lately the user took something, which the command bar puts first among equal
//! matches: a command picked (#1395), a tab switched to (#1396). Only the times, kept on this
//! computer: never synced, sent or put in a crash report (the owner, 2026-11-03).

/// The uses kept of each thing, newest first, as Arc keeps a tab's last 10 switches.
pub const TIMES: usize = 10;
/// The most things kept. Past it, the one that counts least goes (ours).
pub const CAP: usize = 256;
/// A use counts half as much a week later (ours: Arc's weights weren't recovered).
pub const HALF_LIFE: u64 = 7 * 24 * 60 * 60;

/// Each thing used, with the times it was, in seconds since 1970, newest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Usage<K> {
    uses: Vec<(K, Vec<u64>)>,
}

impl<K> Default for Usage<K> {
    fn default() -> Self {
        Usage { uses: Vec::new() }
    }
}

impl<K: PartialEq> Usage<K> {
    /// `key` was used at `now`.
    pub fn used(&mut self, key: K, now: u64) {
        if let Some((_, times)) = self.uses.iter_mut().find(|(each, _)| *each == key) {
            times.insert(0, now);
            times.truncate(TIMES);
            return;
        }
        if self.uses.len() >= CAP {
            let score = |times: &[u64]| decayed(times, now);
            let least = (0..self.uses.len())
                .min_by(|&a, &b| score(&self.uses[a].1).total_cmp(&score(&self.uses[b].1)));
            if let Some(least) = least {
                self.uses.swap_remove(least);
            }
        }
        self.uses.push((key, vec![now]));
    }

    /// How much `key`'s uses count at `now`: one for each, halving every HALF_LIFE.
    pub fn score<Q: PartialEq + ?Sized>(&self, key: &Q, now: u64) -> f32
    where
        K: std::borrow::Borrow<Q>,
    {
        (self.uses.iter())
            .find(|(each, _)| each.borrow() == key)
            .map_or(0.0, |(_, times)| decayed(times, now))
    }

    /// Forgets the uses from `begin` to `end`, in seconds since 1970, as history cleared for a
    /// time takes them, and the things with no use left.
    pub fn forget_between(&mut self, begin: u64, end: u64) {
        for (_, times) in &mut self.uses {
            times.retain(|time| !(begin..end).contains(time));
        }
        self.uses.retain(|(_, times)| !times.is_empty());
    }

    /// Keeps only the things `keep` says to, as a closed tab's switches go with it.
    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        self.uses.retain(|(key, _)| keep(key));
    }

    pub fn len(&self) -> usize {
        self.uses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.uses.is_empty()
    }
}

impl Usage<String> {
    /// Reads the file: a line a thing, its name, a tab, then its times, newest first, by commas.
    pub fn parse(text: &str) -> Self {
        let mut usage = Usage::default();
        for (key, times) in text.lines().filter_map(|line| line.split_once('\t')) {
            let mut times: Vec<u64> = times.split(',').filter_map(|t| t.parse().ok()).collect();
            times.truncate(TIMES);
            if !key.is_empty() && !times.is_empty() && usage.uses.len() < CAP {
                usage.uses.push((key.to_string(), times));
            }
        }
        usage
    }

    pub fn to_text(&self) -> String {
        let line = |(key, times): &(String, Vec<u64>)| {
            let times: Vec<String> = times.iter().map(u64::to_string).collect();
            format!("{key}\t{}\n", times.join(","))
        };
        self.uses.iter().map(line).collect()
    }
}

fn decayed(times: &[u64], now: u64) -> f32 {
    let half_lives = |time: u64| now.saturating_sub(time) as f32 / HALF_LIFE as f32;
    times
        .iter()
        .map(|&time| 0.5f32.powf(half_lives(time)))
        .sum()
}

/// Now, in seconds since 1970.
pub fn now() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 24 * 60 * 60;

    #[test]
    fn a_use_counts_half_as_much_a_week_later() {
        let mut usage = Usage::default();
        usage.used("New Window", 0);
        assert_eq!(usage.score(&"New Window", 0), 1.0);
        assert_eq!(usage.score(&"New Window", 7 * DAY), 0.5);
        assert_eq!(usage.score(&"New Folder…", 0), 0.0);
        // Three uses a month ago count less than one today.
        let mut usage = Usage::default();
        for _ in 0..3 {
            usage.used("old", 0);
        }
        usage.used("new", 30 * DAY);
        assert!(usage.score(&"new", 30 * DAY) > usage.score(&"old", 30 * DAY));
    }

    #[test]
    fn only_the_last_ten_uses_are_kept() {
        let mut usage = Usage::default();
        for time in 0..15 {
            usage.used(1u64, time);
        }
        assert!(usage.score(&1, 15) < 10.0);
        assert!(usage.score(&1, 15) > 9.9);
    }

    #[test]
    fn past_the_cap_the_least_used_goes() {
        let mut usage = Usage::default();
        usage.used(0u64, 0);
        usage.used(0u64, 0);
        for key in 1..CAP as u64 {
            usage.used(key, DAY);
        }
        assert_eq!(usage.len(), CAP);
        usage.used(CAP as u64, 2 * DAY);
        assert_eq!(usage.len(), CAP);
        // Thing 0's two uses a day older still count more than one of the others'.
        assert!(usage.score(&0, 2 * DAY) > 1.0);
        assert_eq!(usage.score(&(CAP as u64), 2 * DAY), 1.0);
        assert_eq!(usage.score(&1, 2 * DAY), 0.0);
    }

    #[test]
    fn a_span_of_time_forgets_the_uses_in_it() {
        let mut usage = Usage::default();
        usage.used("kept", 1);
        usage.used("both", 2);
        usage.used("both", 6);
        usage.used("gone", 5);
        usage.forget_between(4, 8);
        assert_eq!(usage.len(), 2);
        assert_eq!(usage.score(&"both", 2), 1.0);
        assert_eq!(usage.score(&"gone", 5), 0.0);
        assert_eq!(usage.score(&"kept", 1), 1.0);
    }

    #[test]
    fn the_file_keeps_names_and_times_only() {
        let mut usage = Usage::default();
        usage.used("New Window".to_string(), 5);
        usage.used("New Window".to_string(), 9);
        usage.used("Focus Work".to_string(), 7);
        let text = usage.to_text();
        assert_eq!(text, "New Window\t9,5\nFocus Work\t7\n");
        assert_eq!(Usage::parse(&text), usage);
        assert_eq!(Usage::parse("\t1\nbroken\nNo times\t\n"), Usage::default());
    }
}
