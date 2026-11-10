//! Boosts (#695, docs/ux-spec.md › Boosts): the CSS the user gives a site, and Zap, which hides
//! what they click there. A profile's Boosts are plain files in its folder, one a site,
//! `Boosts/<site>.css`, so the user can keep them in git, edit them in any editor and see each
//! save at once. The sites they turned off are a list next to the folder, not in it, so a Boost
//! copied from another computer comes on. Settings' switch turns them all off.
//!
//! The app keeps them here and hands the core each site's CSS as it changes (Change), which the
//! core shows on the site's pages as they load and at once on those open. A site is its host
//! without "www.", as the core reads it. Agents get no path to any of it: what a Boost changes is
//! the user's own view of their pages (principle 3, decision 8).
//!
//! A Boost's script, `Boosts/<site>.js` (#2297), runs in its site's pages only while its bytes are
//! the ones the user last turned on in the browser's own dialog, which the core keeps a record of.
//! The app reads the file, gives the core its bytes while the Boost is on (ScriptChange), and
//! shows the state the core answers. It keeps a copy of the text last turned on, in the profile
//! beside the record and outside the Boosts folder, so the dialog can show what changed since.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use gpui::SharedString;

/// The folder in a profile's folder that holds its Boosts.
pub const FOLDER: &str = "Boosts";
/// The sites whose Boost the user turned off, one a line, beside the folder.
pub const OFF_FILE: &str = "Boosts off";
/// What ends a rule Zap wrote, which Undo takes back and the card counts.
const ZAPPED: &str = "/* Zap */";

/// The Boost key of `url`: its host without "www.", for an http(s) page, as the core's
/// Boosts::SiteOf. None for another page, or a host that can't name a file, such as an IPv6
/// address.
pub fn site_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host.split(':').next()?.to_ascii_lowercase();
    let host = match host.strip_prefix("www.") {
        Some(rest) if !rest.is_empty() => rest.to_owned(),
        _ => host,
    };
    let fits = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-';
    let named = !host.is_empty() && !host.starts_with('.') && host.bytes().all(fits);
    named.then_some(host)
}

/// `css` with a rule that hides what `selector` finds, as a click with Zap asks.
pub fn add_zap(css: &str, selector: &str) -> String {
    let mut css = css.to_owned();
    if !css.is_empty() && !css.ends_with('\n') {
        css.push('\n');
    }
    css.push_str(&format!(
        "{selector} {{ display: none !important; }} {ZAPPED}\n"
    ));
    css
}

/// `css` without the last rule Zap wrote, or None if it has none.
pub fn undo_zap(css: &str) -> Option<String> {
    let lines: Vec<&str> = css.lines().collect();
    let last = lines
        .iter()
        .rposition(|line| line.trim_end().ends_with(ZAPPED))?;
    let kept: Vec<&str> = (lines.iter().enumerate())
        .filter(|(index, _)| *index != last)
        .map(|(_, line)| *line)
        .collect();
    Some(match kept.is_empty() {
        true => String::new(),
        false => kept.join("\n") + "\n",
    })
}

/// What a Boost does, for its card and its row in the Library.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// The things Zap hides.
    pub zaps: usize,
    /// The user's own lines of CSS, without blank lines and lines that are only a comment.
    pub lines: usize,
}

impl Summary {
    pub fn of(css: &str) -> Summary {
        let mut summary = Summary::default();
        for line in css.lines().map(str::trim) {
            if line.ends_with(ZAPPED) {
                summary.zaps += 1;
            } else if !line.is_empty() && !(line.starts_with("/*") && line.ends_with("*/")) {
                summary.lines += 1;
            }
        }
        summary
    }

    /// Says it in a sentence: "Hides 3 things and adds 12 lines of your CSS."
    pub fn describe(self) -> String {
        let plural = |count: usize, one: &str, many: &str| match count {
            1 => format!("1 {one}"),
            _ => format!("{count} {many}"),
        };
        let hides = plural(self.zaps, "thing", "things");
        let lines = plural(self.lines, "line", "lines");
        match (self.zaps, self.lines) {
            (0, 0) => "Empty. Zap what you don't want to see, or add your own CSS.".into(),
            (_, 0) => format!("Hides {hides}."),
            (0, _) => format!("Adds {lines} of your CSS."),
            _ => format!("Hides {hides} and adds {lines} of your CSS."),
        }
    }
}

/// What the turn-on dialog asks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asking {
    /// A script never turned on here: its whole text.
    New,
    /// A changed script, with what changed since the copy turned on.
    Changed,
    /// A changed script with no copy of the text turned on to compare: its whole text.
    ChangedWhole,
}

/// The dialog that turns on `site`'s script, which only the user's click on Turn On Script…
/// opens (#2297). It names the site and says what a script can do, and shows the script whole, or
/// what changed since the version last turned on. Turn On is never what ↩ does, and it takes the
/// dialog's moment on screen before a click counts, as a device's request does: a key or a click
/// meant for the page can't turn on code that acts as the user.
pub fn script_question(
    site: &str,
    code: crate::dialog::Code,
    asking: Asking,
) -> crate::dialog::Question {
    let message = match asking {
        Asking::New => format!("Turn on the script for {site}?"),
        Asking::Changed | Asking::ChangedWhole => {
            format!("Turn the changed script for {site} back on?")
        }
    };
    let detail = match asking {
        Asking::New => format!(
            "It runs on {site}'s pages as you: it can read and change them, use your login \
             there, and send what it reads to other sites. Turn it on only if you know what it \
             does."
        ),
        Asking::Changed => format!(
            "Its file changed outside ilo, so it's off. Here's what changed since you turned it \
             on. It runs on {site}'s pages as you, and can send what it reads to other sites."
        ),
        Asking::ChangedWhole => format!(
            "Its file changed outside ilo, so it's off. Here's all of it, as ilo has no copy of \
             the version you turned on. It runs on {site}'s pages as you, and can send what it \
             reads to other sites."
        ),
    };
    crate::dialog::Question {
        icon: None,
        message: message.into(),
        detail: detail.into(),
        list: Vec::new(),
        action: "Turn On".into(),
        cancel: "Cancel".into(),
        tone: crate::dialog::Tone::Accent,
        also: None,
        fields: Vec::new(),
        check: None,
        on_return: crate::dialog::Return::Nothing,
        code: Some(code),
    }
}

/// `text`'s lines as the file has them: split at each newline only, so a carriage return stays
/// in its line and shows, and with no empty line after a last newline.
fn file_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// A new script's whole text, for its dialog.
pub fn script_text(site: &str, script: &str) -> crate::dialog::Code {
    use crate::dialog::{CodeLine, LineKind};
    let lines: Vec<CodeLine> = (file_lines(script).into_iter().enumerate())
        .map(|(index, line)| CodeLine {
            kind: LineKind::Same,
            number: Some(index + 1),
            text: shown_line(line).into(),
        })
        .collect();
    let count = match lines.len() {
        1 => "1 line".to_string(),
        count => format!("{count} lines"),
    };
    crate::dialog::Code {
        caption: format!("{FOLDER}/{site}.js · {count}").into(),
        lines,
    }
}

/// What changed from `before`, the version last turned on, to `after`, with this many unchanged
/// lines around each change, for the changed script's dialog.
pub fn script_diff(site: &str, before: &str, after: &str) -> crate::dialog::Code {
    use crate::dialog::{CodeLine, LineKind};
    const CONTEXT: usize = 2;
    // Past this many cells, the lines between the common ends show as all removed, then all
    // added: still what changed, without a table the size of two 256 KB files' lines.
    const MOST_CELLS: usize = 4_000_000;
    let old = file_lines(before);
    let new = file_lines(after);
    let head = (old.iter().zip(&new)).take_while(|(a, b)| a == b).count();
    let tail = (old[head..].iter().rev().zip(new[head..].iter().rev()))
        .take_while(|(a, b)| a == b)
        .count();
    let (old_mid, new_mid) = (&old[head..old.len() - tail], &new[head..new.len() - tail]);
    let mut all: Vec<(LineKind, Option<usize>, &str)> = Vec::new();
    for (index, line) in new[..head].iter().enumerate() {
        all.push((LineKind::Same, Some(index + 1), line));
    }
    if old_mid.len().saturating_mul(new_mid.len()) <= MOST_CELLS {
        // The longest common subsequence of lines, from the ends.
        let width = new_mid.len() + 1;
        let mut common = vec![0u32; (old_mid.len() + 1) * width];
        for i in (0..old_mid.len()).rev() {
            for j in (0..new_mid.len()).rev() {
                common[i * width + j] = match old_mid[i] == new_mid[j] {
                    true => common[(i + 1) * width + j + 1] + 1,
                    false => common[(i + 1) * width + j].max(common[i * width + j + 1]),
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < old_mid.len() || j < new_mid.len() {
            if i < old_mid.len() && j < new_mid.len() && old_mid[i] == new_mid[j] {
                all.push((LineKind::Same, Some(head + j + 1), new_mid[j]));
                (i, j) = (i + 1, j + 1);
            } else if i < old_mid.len()
                && (j == new_mid.len() || common[(i + 1) * width + j] >= common[i * width + j + 1])
            {
                // What went before what came, as a diff reads.
                all.push((LineKind::Removed, None, old_mid[i]));
                i += 1;
            } else {
                all.push((LineKind::Added, Some(head + j + 1), new_mid[j]));
                j += 1;
            }
        }
    } else {
        all.extend(old_mid.iter().map(|line| (LineKind::Removed, None, *line)));
        all.extend(
            (new_mid.iter().enumerate())
                .map(|(index, line)| (LineKind::Added, Some(head + index + 1), *line)),
        );
    }
    for (index, line) in new[new.len() - tail..].iter().enumerate() {
        all.push((LineKind::Same, Some(new.len() - tail + index + 1), line));
    }
    let near = |index: usize| {
        let from = index.saturating_sub(CONTEXT);
        let to = (index + CONTEXT + 1).min(all.len());
        all[from..to].iter().any(|line| line.0 != LineKind::Same)
    };
    let mut lines = Vec::new();
    let mut skipped = 0;
    for (index, &(kind, number, text)) in all.iter().enumerate() {
        if near(index) {
            if skipped > 0 {
                lines.push(CodeLine {
                    kind: LineKind::Gap,
                    number: None,
                    text: unchanged(skipped).into(),
                });
                skipped = 0;
            }
            lines.push(CodeLine {
                kind,
                number,
                text: shown_line(text).into(),
            });
        } else {
            skipped += 1;
        }
    }
    if skipped > 0 {
        lines.push(CodeLine {
            kind: LineKind::Gap,
            number: None,
            text: unchanged(skipped).into(),
        });
    }
    let count = |kind| all.iter().filter(|line| line.0 == kind).count();
    let (added, removed) = (count(LineKind::Added), count(LineKind::Removed));
    // Bytes that differ in no line, such as a last newline, leave no line to show.
    if added + removed == 0 {
        return script_text(site, after);
    }
    let lines_of = |count: usize| match count {
        1 => "1 line".to_string(),
        _ => format!("{count} lines"),
    };
    crate::dialog::Code {
        caption: format!(
            "{FOLDER}/{site}.js · {} added, {} removed",
            lines_of(added),
            lines_of(removed)
        )
        .into(),
        lines,
    }
}

fn unchanged(count: usize) -> String {
    match count {
        1 => "1 line unchanged".into(),
        _ => format!("{count} lines unchanged"),
    }
}

/// A line of a script as the dialog shows it: tabs as spaces, and a control or direction
/// character as its code, so nothing in the file hides what the line does.
fn shown_line(line: &str) -> String {
    let mut shown = String::new();
    for c in line.chars() {
        match c {
            '\t' => shown.push_str("    "),
            c if c.is_control()
                || matches!(
                    c,
                    '\u{00ad}'
                        | '\u{061c}'
                        | '\u{180e}'
                        | '\u{200b}'..='\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2060}'..='\u{2069}'
                        | '\u{feff}'
                ) =>
            {
                shown.push_str(&format!("\\u{{{:x}}}", c as u32))
            }
            c => shown.push(c),
        }
    }
    shown
}

/// A site's CSS as the core should show it now: empty to show none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub profile: u64,
    pub site: String,
    pub css: String,
}

/// A Boost, as the card and the Library show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Boost {
    pub site: SharedString,
    pub summary: Summary,
    /// The user hasn't turned it off on its site.
    pub on: bool,
    /// Its script, `Boosts/<site>.js`, where it has one (#2297).
    pub script: Option<Script>,
}

/// The largest script that runs, in bytes, as the core's BC_BOOST_SCRIPT_MAX_BYTES: one past it is
/// off, and the states say so in this figure.
pub const SCRIPT_LIMIT: usize = 256 * 1024;
/// The folder in a profile's folder, beside FOLDER and never in it, with the copy of each script
/// as the user last turned it on, for the dialog's diff. Nothing copies or syncs it with the Boosts,
/// and the dialog trusts a copy only while the core says it's the text turned on.
pub const TURNED_ON: &str = "Boost scripts turned on";

/// What a Boost's script does, as the core's record says (#2297).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    /// Its bytes are the ones the user turned on, and it runs.
    On,
    /// It was never turned on here, so it's off until the user turns it on.
    New,
    /// Its file changed since the user turned it on, so it's off until they turn it on again.
    Changed,
    /// It's over SCRIPT_LIMIT, so it's off whatever the user says.
    TooLarge,
}

impl Script {
    /// Why it's off, in the same words in the card, Site Controls and the Library, or None while
    /// it runs.
    pub fn off_reason(self) -> Option<&'static str> {
        match self {
            Script::On => None,
            Script::New => Some("New script, off"),
            Script::Changed => Some("Script changed, off"),
            Script::TooLarge => Some("Script over 256 KB, off"),
        }
    }

    /// Whether Turn On Script… can turn it on: one too large can't run whatever the user says.
    pub fn can_turn_on(self) -> bool {
        matches!(self, Script::New | Script::Changed)
    }
}

/// A site's script as the core should have it now: its bytes while the Boost is on, none to run
/// none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptChange {
    pub profile: u64,
    pub site: String,
    pub script: Option<Vec<u8>>,
}

/// A site's script file as read.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ScriptFile {
    Bytes(Vec<u8>),
    /// Over SCRIPT_LIMIT, so it isn't read at all.
    TooLarge,
}

/// Each loaded profile's Boosts, and whether Settings has them all off.
#[derive(Default)]
pub struct Boosts {
    profiles: HashMap<u64, Kept>,
    all_off: bool,
}

thread_local! {
    static BOOSTS: RefCell<Boosts> = RefCell::new(Boosts::default());
}

/// The UI thread's Boosts. Nothing that calls out, such as to the core, runs inside `change`.
pub fn with<R>(change: impl FnOnce(&mut Boosts) -> R) -> R {
    BOOSTS.with_borrow_mut(change)
}

#[derive(Default)]
struct Kept {
    /// The profile's own folder, which holds FOLDER and OFF_FILE.
    folder: PathBuf,
    sites: BTreeMap<String, String>,
    off: BTreeSet<String>,
    /// What the core was last given, by site, to give it only what changed.
    shown: BTreeMap<String, String>,
    /// Each site's script file, where it has one.
    scripts: BTreeMap<String, ScriptFile>,
    /// What the core was last given of each site's script, as `shown` is of its CSS.
    given: BTreeMap<String, Vec<u8>>,
    /// The state the core answered for each script given.
    states: BTreeMap<String, Script>,
}

impl Kept {
    fn boosts(&self) -> PathBuf {
        self.folder.join(FOLDER)
    }

    fn path(&self, site: &str) -> PathBuf {
        self.boosts().join(format!("{site}.css"))
    }

    fn script_path(&self, site: &str) -> PathBuf {
        self.boosts().join(format!("{site}.js"))
    }

    fn copy_path(&self, site: &str) -> PathBuf {
        self.folder.join(TURNED_ON).join(format!("{site}.js"))
    }

    /// The state of `site`'s script, where it has one: too large as read, else as the core said,
    /// and new until it has.
    fn script(&self, site: &str) -> Option<Script> {
        match self.scripts.get(site)? {
            ScriptFile::TooLarge => Some(Script::TooLarge),
            ScriptFile::Bytes(_) => Some(self.states.get(site).copied().unwrap_or(Script::New)),
        }
    }

    fn read(&mut self) {
        self.sites = read_folder(&self.boosts());
        self.scripts = read_scripts(&self.boosts());
        let off = std::fs::read_to_string(self.folder.join(OFF_FILE)).unwrap_or_default();
        self.off = off.lines().filter_map(site_of_line).collect();
    }

    fn save_off(&self) {
        let path = self.folder.join(OFF_FILE);
        let saved = match self.off.is_empty() {
            true => std::fs::remove_file(&path).or_else(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(error),
            }),
            false => std::fs::write(
                &path,
                self.off
                    .iter()
                    .map(|site| site.clone() + "\n")
                    .collect::<String>(),
            ),
        };
        if let Err(error) = saved {
            crate::log!("couldn't save the Boosts turned off: {error}");
        }
    }

    fn write(&mut self, site: &str, css: &str) -> bool {
        let written = std::fs::create_dir_all(self.boosts())
            .and_then(|_| std::fs::write(self.path(site), css));
        match written {
            Ok(()) => {
                self.sites.insert(site.to_owned(), css.to_owned());
                true
            }
            Err(error) => {
                crate::log!("couldn't save the Boost for {site}: {error}");
                false
            }
        }
    }
}

/// A new Boost's start: a line saying what its file is.
fn first_line(site: &str) -> String {
    format!("/* {site}: your CSS for its pages, shown as you save. */\n\n")
}

/// A line of OFF_FILE, if it names a site.
fn site_of_line(line: &str) -> Option<String> {
    let line = line.trim();
    site_of(&format!("https://{line}/")).filter(|site| site == line)
}

/// The scripts in `folder`, by site: each `<site>.js` that names one, unread if it's too large.
fn read_scripts(folder: &Path) -> BTreeMap<String, ScriptFile> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return BTreeMap::new();
    };
    (entries.flatten())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let site = site_of_line(name.strip_suffix(".js")?)?;
            if entry.metadata().ok()?.len() > SCRIPT_LIMIT as u64 {
                return Some((site, ScriptFile::TooLarge));
            }
            let bytes = std::fs::read(entry.path()).ok()?;
            // It may have grown since its size was read.
            Some(match bytes.len() > SCRIPT_LIMIT {
                true => (site, ScriptFile::TooLarge),
                false => (site, ScriptFile::Bytes(bytes)),
            })
        })
        .collect()
}

/// The Boosts in `folder`, by site: each `<site>.css` that names one.
pub fn read_folder(folder: &Path) -> BTreeMap<String, String> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return BTreeMap::new();
    };
    (entries.flatten())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let site = site_of_line(name.strip_suffix(".css")?)?;
            Some((site, std::fs::read_to_string(entry.path()).ok()?))
        })
        .collect()
}

impl Boosts {
    /// Whether `profile`'s Boosts are read.
    pub fn loaded(&self, profile: u64) -> bool {
        self.profiles.contains_key(&profile)
    }

    /// Reads `profile`'s Boosts from its folder, `folder`, and says what the core should show.
    pub fn load(&mut self, profile: u64, folder: PathBuf) -> Vec<Change> {
        let kept = self.profiles.entry(profile).or_default();
        kept.folder = folder;
        kept.read();
        self.changes(profile)
    }

    /// Reads `profile`'s Boosts again, as its folder changed, and says what changed.
    pub fn reload(&mut self, profile: u64) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        kept.read();
        self.changes(profile)
    }

    /// The folder `profile`'s Boosts are in, once read.
    pub fn folder(&self, profile: u64) -> Option<PathBuf> {
        Some(self.profiles.get(&profile)?.boosts())
    }

    pub fn all_off(&self) -> bool {
        self.all_off
    }

    /// Settings' switch: every Boost off, or back as each site has it.
    pub fn set_all_off(&mut self, off: bool) -> Vec<Change> {
        self.all_off = off;
        let profiles: Vec<u64> = self.profiles.keys().copied().collect();
        profiles
            .into_iter()
            .flat_map(|profile| self.changes(profile))
            .collect()
    }

    /// `site`'s Boost in `profile`, if it has one.
    pub fn get(&self, profile: u64, site: &str) -> Option<Boost> {
        let kept = self.profiles.get(&profile)?;
        let css = kept.sites.get(site);
        if css.is_none() && !kept.scripts.contains_key(site) {
            return None;
        }
        Some(Boost {
            site: site.to_owned().into(),
            summary: Summary::of(css.map_or("", String::as_str)),
            on: !kept.off.contains(site),
            script: kept.script(site),
        })
    }

    /// What the core should have of each script that it doesn't yet: a site's bytes while its
    /// Boost is on, and none for one turned off, too large, or there no more. The core answers
    /// each with its state, for `set_script_state`.
    pub fn script_changes(&mut self, profile: u64) -> Vec<ScriptChange> {
        let all_off = self.all_off;
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let sites: BTreeSet<String> =
            (kept.scripts.keys().chain(kept.given.keys()).cloned()).collect();
        let mut changes = Vec::new();
        for site in sites {
            let script = match kept.scripts.get(&site) {
                Some(ScriptFile::Bytes(bytes)) if !all_off && !kept.off.contains(&site) => {
                    Some(bytes.clone())
                }
                _ => None,
            };
            if kept.given.get(&site) == script.as_ref() {
                continue;
            }
            match &script {
                Some(bytes) => kept.given.insert(site.clone(), bytes.clone()),
                None => {
                    kept.states.remove(&site);
                    kept.given.remove(&site)
                }
            };
            changes.push(ScriptChange {
                profile,
                site,
                script,
            });
        }
        changes
    }

    /// `script_changes` for every profile read.
    pub fn all_script_changes(&mut self) -> Vec<ScriptChange> {
        let profiles: Vec<u64> = self.profiles.keys().copied().collect();
        (profiles.into_iter())
            .flat_map(|profile| self.script_changes(profile))
            .collect()
    }

    /// Every script the core has of `profile`, to ask its state again, as once the profile has
    /// loaded: the core says new until it has.
    pub fn given_scripts(&self, profile: u64) -> Vec<ScriptChange> {
        let Some(kept) = self.profiles.get(&profile) else {
            return Vec::new();
        };
        (kept.given.iter())
            .map(|(site, bytes)| ScriptChange {
                profile,
                site: site.clone(),
                script: Some(bytes.clone()),
            })
            .collect()
    }

    /// The core's answer for a script it was given.
    pub fn set_script_state(&mut self, profile: u64, site: &str, state: Script) {
        if let Some(kept) = self.profiles.get_mut(&profile)
            && kept.given.contains_key(site)
        {
            kept.states.insert(site.to_owned(), state);
        }
    }

    /// The bytes of `site`'s script as read, for the turn-on dialog, unless it's too large.
    pub fn script_bytes(&self, profile: u64, site: &str) -> Option<Vec<u8>> {
        match self.profiles.get(&profile)?.scripts.get(site)? {
            ScriptFile::Bytes(bytes) => Some(bytes.clone()),
            ScriptFile::TooLarge => None,
        }
    }

    /// The copy of `site`'s script as the user last turned it on, if there's one, which the dialog
    /// shows only while the core says it's that text.
    pub fn turned_on_copy(&self, profile: u64, site: &str) -> Option<Vec<u8>> {
        let path = self.profiles.get(&profile)?.copy_path(site);
        std::fs::read(path)
            .ok()
            .filter(|bytes| bytes.len() <= SCRIPT_LIMIT)
    }

    /// The user turned on `bytes` as `site`'s script, which the core now has and runs: keeps the
    /// copy for the next diff.
    pub fn turned_on(&mut self, profile: u64, site: &str, bytes: &[u8]) {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return;
        };
        let path = kept.copy_path(site);
        let written = (path.parent())
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&path, bytes));
        if let Err(error) = written {
            crate::log!("couldn't keep the script turned on for {site}: {error}");
        }
        kept.given.insert(site.to_owned(), bytes.to_owned());
        kept.states.insert(site.to_owned(), Script::On);
    }

    /// The user turned `site`'s script off, which the core forgot: it's new again, and its copy
    /// goes.
    pub fn turned_off(&mut self, profile: u64, site: &str) {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return;
        };
        if let Err(error) = std::fs::remove_file(kept.copy_path(site))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::log!("couldn't remove the script turned on for {site}: {error}");
        }
        if kept.given.contains_key(site) {
            kept.states.insert(site.to_owned(), Script::New);
        }
    }

    /// `profile`'s Boosts, by site.
    pub fn list(&self, profile: u64) -> Vec<Boost> {
        let Some(kept) = self.profiles.get(&profile) else {
            return Vec::new();
        };
        let sites: BTreeSet<&String> = kept.sites.keys().chain(kept.scripts.keys()).collect();
        (sites.into_iter())
            .filter_map(|site| self.get(profile, site))
            .collect()
    }

    /// Turns `site`'s Boost on or off there.
    pub fn set_on(&mut self, profile: u64, site: &str, on: bool) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let changed = match on {
            true => kept.off.remove(site),
            false => kept.off.insert(site.to_owned()),
        };
        if changed {
            kept.save_off();
        }
        self.changes(profile)
    }

    /// Adds a rule hiding what `selector` finds to `site`'s Boost, starting one if it has none,
    /// and turns it on: what the user just zapped goes.
    pub fn zap(&mut self, profile: u64, site: &str, selector: &str) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let css = add_zap(kept.sites.get(site).map_or("", String::as_str), selector);
        if kept.write(site, &css) && kept.off.remove(site) {
            kept.save_off();
        }
        self.changes(profile)
    }

    /// Takes back the last rule Zap added to `site`'s Boost.
    pub fn undo_zap(&mut self, profile: u64, site: &str) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        if let Some(css) = kept.sites.get(site).and_then(|css| undo_zap(css)) {
            kept.write(site, &css);
        }
        self.changes(profile)
    }

    /// The editor's edits for `site`'s Boost, the default where it has none.
    pub fn style(&self, profile: u64, site: &str) -> crate::boost_style::Style {
        let css = (self.profiles.get(&profile))
            .and_then(|kept| kept.sites.get(site))
            .map_or("", String::as_str);
        crate::boost_style::Style::of(css)
    }

    /// Rewrites the editor's block in `site`'s Boost as `style`, starting the Boost if it has
    /// none, and turns it on, as the user sees each edit. While `kept` is false, as a drag goes,
    /// the page shows it but the file waits for the drag's end.
    pub fn set_style(
        &mut self,
        profile: u64,
        site: &str,
        style: &crate::boost_style::Style,
        kept: bool,
    ) -> Vec<Change> {
        let Some(kept_boosts) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let before = kept_boosts.sites.get(site).cloned();
        let css = style.into_file(&before.unwrap_or_else(|| first_line(site)));
        let written = match kept {
            true => kept_boosts.write(site, &css),
            false => {
                kept_boosts.sites.insert(site.to_owned(), css);
                true
            }
        };
        if written && kept_boosts.off.remove(site) {
            kept_boosts.save_off();
        }
        self.changes(profile)
    }

    /// Deletes `site`'s Boost: its file, and its place in the list of those turned off.
    pub fn delete(&mut self, profile: u64, site: &str) -> Vec<Change> {
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        if let Err(error) = std::fs::remove_file(kept.path(site))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            crate::log!("couldn't delete the Boost for {site}: {error}");
            return Vec::new();
        }
        kept.sites.remove(site);
        // Its script goes with it, and the copy of the text turned on: the caller has the core
        // forget the record.
        for path in [kept.script_path(site), kept.copy_path(site)] {
            if let Err(error) = std::fs::remove_file(path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                crate::log!("couldn't delete the Boost script for {site}: {error}");
            }
        }
        kept.scripts.remove(site);
        kept.states.remove(site);
        if kept.off.remove(site) {
            kept.save_off();
        }
        self.changes(profile)
    }

    /// `site`'s file, for the user's editor, started with a line saying what it is if the site has
    /// no Boost yet.
    pub fn file(&mut self, profile: u64, site: &str) -> Option<PathBuf> {
        let kept = self.profiles.get_mut(&profile)?;
        if !kept.sites.contains_key(site) {
            if !kept.write(site, &first_line(site)) {
                return None;
            }
        }
        Some(kept.path(site))
    }

    /// What the core should show that it doesn't yet: each site's CSS, or none for a site turned
    /// off, there no more, or with Settings' switch off.
    fn changes(&mut self, profile: u64) -> Vec<Change> {
        let all_off = self.all_off;
        let Some(kept) = self.profiles.get_mut(&profile) else {
            return Vec::new();
        };
        let sites: BTreeSet<String> =
            (kept.sites.keys().chain(kept.shown.keys()).cloned()).collect();
        let mut changes = Vec::new();
        for site in sites {
            let css = match kept.sites.get(&site) {
                Some(css) if !all_off && !kept.off.contains(&site) => css.clone(),
                _ => String::new(),
            };
            if kept.shown.get(&site).map_or("", String::as_str) == css {
                continue;
            }
            match css.is_empty() {
                true => kept.shown.remove(&site),
                false => kept.shown.insert(site.clone(), css.clone()),
            };
            changes.push(Change { profile, site, css });
        }
        changes
    }
}

/// Calls `changed` on a thread of its own each time a file in `folder` is written, added, renamed
/// or deleted, until the app quits, making the folder if it isn't there. The thread waits on the
/// system's notice, so an idle browser isn't woken (principle 1). Returns whether it watches.
pub fn watch(folder: PathBuf, changed: impl Fn() + Send + 'static) -> bool {
    if let Err(error) = std::fs::create_dir_all(&folder) {
        crate::log!("couldn't make the Boosts folder: {error}");
        return false;
    }
    let started = std::thread::Builder::new()
        .name("boosts".into())
        .spawn(move || watch_on_thread(&folder, changed));
    started.is_ok()
}

/// `watch`, calling `changed` on the UI thread rather than the watcher's.
pub fn watch_in(
    folder: PathBuf,
    cx: &mut gpui::App,
    changed: impl Fn(&mut gpui::App) + 'static,
) -> bool {
    use futures::StreamExt;
    let (told, mut changes) = futures::channel::mpsc::unbounded::<()>();
    if !watch(folder, move || _ = told.unbounded_send(())) {
        return false;
    }
    cx.spawn(async move |cx| {
        while changes.next().await.is_some() {
            cx.update(|cx| changed(cx));
        }
    })
    .detach();
    true
}

#[cfg(target_os = "linux")]
fn watch_on_thread(folder: &Path, changed: impl Fn()) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
        return;
    };
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    if fd < 0 {
        return;
    }
    let mask = libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_MOVED_FROM | libc::IN_DELETE;
    if unsafe { libc::inotify_add_watch(fd, path.as_ptr(), mask) } < 0 {
        unsafe { libc::close(fd) };
        return;
    }
    let mut events = [0u8; 4096];
    loop {
        let read = unsafe { libc::read(fd, events.as_mut_ptr().cast(), events.len()) };
        if read < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if read <= 0 {
            break;
        }
        changed();
    }
    unsafe { libc::close(fd) };
}

/// kqueue tells of a change to a file it holds open, and of a folder's names, not its files'
/// contents, so each wait holds the folder and every file in it, opened again after each change.
#[cfg(target_os = "macos")]
fn watch_on_thread(folder: &Path, changed: impl Fn()) {
    use std::os::unix::ffi::OsStrExt;
    let queue = unsafe { libc::kqueue() };
    if queue < 0 {
        return;
    }
    let open = |path: &Path| {
        let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) };
        (fd >= 0).then_some(fd)
    };
    loop {
        let files = (std::fs::read_dir(folder).into_iter().flatten().flatten())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "css" || extension == "js")
            });
        let fds: Vec<i32> = std::iter::once(folder.to_path_buf())
            .chain(files)
            .filter_map(|path| open(&path))
            .collect();
        if fds.is_empty() {
            break;
        }
        let watches: Vec<libc::kevent> = (fds.iter())
            .map(|&fd| libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_VNODE,
                flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: libc::NOTE_WRITE
                    | libc::NOTE_EXTEND
                    | libc::NOTE_DELETE
                    | libc::NOTE_RENAME
                    | libc::NOTE_ATTRIB,
                data: 0,
                udata: std::ptr::null_mut(),
            })
            .collect();
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let got = unsafe {
            libc::kevent(
                queue,
                watches.as_ptr(),
                watches.len() as i32,
                &mut event,
                1,
                std::ptr::null(),
            )
        };
        // Closing a file takes its watch out of the queue.
        for fd in fds {
            unsafe { libc::close(fd) };
        }
        if got < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            break;
        }
        if got > 0 {
            changed();
        }
    }
    unsafe { libc::close(queue) };
}

#[cfg(target_os = "windows")]
fn watch_on_thread(folder: &Path, changed: impl Fn()) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FindCloseChangeNotification,
        FindFirstChangeNotificationW, FindNextChangeNotification,
    };
    use windows_sys::Win32::System::Threading::{INFINITE, WaitForSingleObject};
    let path: Vec<u16> = folder.as_os_str().encode_wide().chain([0]).collect();
    let filter = FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_LAST_WRITE;
    let handle = unsafe { FindFirstChangeNotificationW(path.as_ptr(), 0, filter) };
    if handle == INVALID_HANDLE_VALUE {
        return;
    }
    while unsafe { WaitForSingleObject(handle, INFINITE) } == WAIT_OBJECT_0 {
        changed();
        if unsafe { FindNextChangeNotification(handle) } == 0 {
            break;
        }
    }
    unsafe { FindCloseChangeNotification(handle) };
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn watch_on_thread(_: &Path, _: impl Fn()) {}

#[cfg(test)]
mod tests;
