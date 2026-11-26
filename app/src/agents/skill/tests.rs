//! Every test here writes into a made-up home under the system's temporary folder, never a real
//! one (first run's floor 9): the writer and remover take the home they're given, and the real
//! home's function panics under test.

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

const NAME: &str = crate::agents::cli::LINK;

/// A made-up home, removed when the test ends.
struct Home(PathBuf);

impl Home {
    fn new() -> Home {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "skill-home-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let home = std::env::temp_dir().join(name);
        fs::create_dir_all(&home).unwrap();
        Home(home.canonicalize().unwrap())
    }

    fn at(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }

    fn dir(&self, path: &str) -> &Self {
        fs::create_dir_all(self.at(path)).unwrap();
        self
    }

    fn file(&self, path: &str, text: &str) {
        fs::write(self.at(path), text).unwrap();
    }

    fn click(&self, program: Client, kept: Option<&Kept>) -> Result<Kept, Refused> {
        write(&self.0, program, NAME, &text(NAME), kept)
    }

    /// Every path under the home, sorted, so a test sees all a step left behind.
    fn tree(&self) -> Vec<String> {
        fn walk(root: &Path, at: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                out.push(path.strip_prefix(root).unwrap().display().to_string());
                if path.is_dir() && !path.is_symlink() {
                    walk(root, &path, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.0, &self.0, &mut out);
        out.sort();
        out
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        _ = fs::remove_dir_all(&self.0);
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn a_click_writes_the_file_into_a_real_skills_folder() {
    let home = Home::new();
    home.dir(".claude/skills");
    let kept = home.click(Client::ClaudeCode, None).unwrap();
    let file = home.at(&format!(".claude/skills/{NAME}/SKILL.md"));
    assert_eq!(fs::read_to_string(&file).unwrap(), text(NAME));
    assert!(kept.written.is_some(), "the run knows which file it wrote");
    assert_eq!(kept.sha256, sha256(text(NAME).as_bytes()));
    assert!(!kept.made_skills, "the skills folder was the user's");
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&home.at(&format!(".claude/skills/{NAME}"))), 0o700);
    assert_eq!(
        home.tree(),
        [
            ".claude".to_string(),
            ".claude/skills".into(),
            format!(".claude/skills/{NAME}"),
            format!(".claude/skills/{NAME}/SKILL.md"),
        ],
        "nothing else, no file left beside"
    );
}

#[test]
fn the_skills_folder_is_made_only_where_the_makers_folder_is_real() {
    let home = Home::new();
    home.dir(".agents");
    let kept = home.click(Client::Codex, None).unwrap();
    assert!(kept.made_skills);
    assert_eq!(mode(&home.at(".agents/skills")), 0o700);
    assert!(
        home.at(&format!(".agents/skills/{NAME}/SKILL.md"))
            .is_file()
    );

    // No maker's folder: the product never makes one.
    let home = Home::new();
    assert_eq!(home.click(Client::Codex, None), Err(Refused::NoFolder));
    assert_eq!(home.click(Client::ClaudeCode, None), Err(Refused::NoFolder));
    assert!(home.tree().is_empty());
}

#[test]
fn a_link_anywhere_on_the_way_writes_nothing() {
    // The maker's folder a link to another folder.
    let home = Home::new();
    home.dir("elsewhere/skills");
    symlink(home.at("elsewhere"), home.at(".claude")).unwrap();
    assert_eq!(home.click(Client::ClaudeCode, None), Err(Refused::NoFolder));
    assert_eq!(home.tree(), [".claude", "elsewhere", "elsewhere/skills"]);

    // The skills folder a link.
    let home = Home::new();
    home.dir(".claude").dir("elsewhere");
    symlink(home.at("elsewhere"), home.at(".claude/skills")).unwrap();
    assert_eq!(home.click(Client::ClaudeCode, None), Err(Refused::NoFolder));
    assert_eq!(home.tree(), [".claude", ".claude/skills", "elsewhere"]);

    // The skills name a file.
    let home = Home::new();
    home.dir(".claude");
    home.file(".claude/skills", "");
    assert_eq!(home.click(Client::ClaudeCode, None), Err(Refused::NoFolder));
    assert_eq!(home.tree(), [".claude", ".claude/skills"]);

    // The skill's own folder a link, with no kept entry: not ours.
    let home = Home::new();
    home.dir(".claude/skills").dir("elsewhere");
    symlink(
        home.at("elsewhere"),
        home.at(&format!(".claude/skills/{NAME}")),
    )
    .unwrap();
    assert_eq!(home.click(Client::ClaudeCode, None), Err(Refused::NotOurs));
    assert!(fs::read_dir(home.at("elsewhere")).unwrap().next().is_none());
}

#[test]
fn a_users_own_skill_of_that_name_is_never_written_over() {
    let home = Home::new();
    home.dir(&format!(".claude/skills/{NAME}"));
    home.file(&format!(".claude/skills/{NAME}/SKILL.md"), "mine");
    assert_eq!(home.click(Client::ClaudeCode, None), Err(Refused::NotOurs));
    assert_eq!(
        fs::read_to_string(home.at(&format!(".claude/skills/{NAME}/SKILL.md"))).unwrap(),
        "mine"
    );
}

#[test]
fn a_file_of_ours_swapped_for_a_link_is_left() {
    let home = Home::new();
    home.dir(".claude/skills");
    let kept = home.click(Client::ClaudeCode, None).unwrap();
    let file = home.at(&format!(".claude/skills/{NAME}/SKILL.md"));
    fs::remove_file(&file).unwrap();
    home.file("target", &text(NAME));
    symlink(home.at("target"), &file).unwrap();
    assert_eq!(
        home.click(Client::ClaudeCode, Some(&kept)),
        Err(Refused::Changed)
    );
    assert_eq!(remove(&home.0, &kept), Removed::Changed);
    assert!(file.is_symlink() && home.at("target").is_file());
}

#[test]
fn off_removes_an_unchanged_file_and_what_the_product_made() {
    let home = Home::new();
    home.dir(".agents");
    let kept = home.click(Client::Codex, None).unwrap();
    assert_eq!(remove(&home.0, &kept), Removed::Gone);
    assert_eq!(
        home.tree(),
        [".agents"],
        "the skills folder it made is gone too"
    );

    let home = Home::new();
    home.dir(".claude/skills");
    let kept = home.click(Client::ClaudeCode, None).unwrap();
    assert_eq!(remove(&home.0, &kept), Removed::Gone);
    assert_eq!(
        home.tree(),
        [".claude", ".claude/skills"],
        "the user's skills folder stays"
    );
}

#[test]
fn off_leaves_a_changed_file_and_a_folder_the_user_put_something_in() {
    let home = Home::new();
    home.dir(".agents");
    let kept = home.click(Client::Codex, None).unwrap();
    home.file(&format!(".agents/skills/{NAME}/SKILL.md"), "changed");
    assert_eq!(remove(&home.0, &kept), Removed::Changed);
    assert_eq!(
        fs::read_to_string(home.at(&format!(".agents/skills/{NAME}/SKILL.md"))).unwrap(),
        "changed"
    );

    let home = Home::new();
    home.dir(".agents");
    let kept = home.click(Client::Codex, None).unwrap();
    home.dir(".agents/skills/theirs");
    assert_eq!(remove(&home.0, &kept), Removed::Gone);
    assert_eq!(
        home.tree(),
        [".agents", ".agents/skills", ".agents/skills/theirs"],
        "skills isn't empty, so it stays"
    );
}

#[test]
fn a_second_add_rewrites_only_an_unchanged_file_of_ours() {
    let home = Home::new();
    home.dir(".claude/skills");
    let old = write(&home.0, Client::ClaudeCode, NAME, "old", None).unwrap();
    let file = home.at(&format!(".claude/skills/{NAME}/SKILL.md"));
    let new = home.click(Client::ClaudeCode, Some(&old)).unwrap();
    assert_eq!(new.sha256, sha256(text(NAME).as_bytes()));
    assert_eq!(fs::read_to_string(&file).unwrap(), text(NAME));

    // Changed by the user: left.
    home.file(&format!(".claude/skills/{NAME}/SKILL.md"), "mine");
    assert_eq!(
        write(&home.0, Client::ClaudeCode, NAME, "newer", Some(&new)),
        Err(Refused::Changed)
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), "mine");
}

#[test]
fn a_kept_entry_that_isnt_one_of_ours_is_dropped_and_touches_nothing() {
    let home = Home::new();
    home.dir("profile")
        .dir(&format!(".claude/skills/{NAME}"))
        .dir("x");
    home.file(&format!(".claude/skills/{NAME}/SKILL.md"), &text(NAME));
    home.file("x/SKILL.md", &text(NAME));
    let file = home.at("profile/skills.json");
    let hash = sha256(text(NAME).as_bytes());
    let entry = |program: &str, name: &str, hash: &str| json!({ "program": program, "name": name, "sha256": hash, "made_skills": true });
    let bad = [
        entry("claude-code", "../x", &hash),
        entry("claude-code", "a/b", &hash),
        entry("claude-code", "", &hash),
        entry("claude-code", "..", &hash),
        entry("other", NAME, &hash),
        entry("ilo-command", NAME, &hash),
        entry("claude-code", NAME, "not a hash"),
    ];
    for bad in bad {
        let text = json!({ "skills": [bad] }).to_string();
        home.file("profile/skills.json", &text);
        assert!(read_kept(&file).is_empty(), "{bad}");
    }
    // A file that isn't a list, or larger than any of ours, reads as none.
    home.file("profile/skills.json", "{ not json");
    assert!(read_kept(&file).is_empty());
    let mut long = vec![entry("claude-code", NAME, &hash); 200];
    long.push(entry("codex", NAME, &hash));
    home.file(
        "profile/skills.json",
        &json!({ "skills": long }).to_string(),
    );
    assert!(read_kept(&file).is_empty(), "over the cap");
    // Handed straight to the remover, a name that is a path removes nothing.
    for name in ["../x", "..", "a/b"] {
        let kept = Kept {
            program: Client::ClaudeCode,
            name: name.into(),
            sha256: hash.clone(),
            made_skills: true,
            written: None,
        };
        assert_eq!(remove(&home.0, &kept), Removed::Missing, "{name}");
    }
    assert!(home.at("x/SKILL.md").is_file());
    assert!(
        home.at(&format!(".claude/skills/{NAME}/SKILL.md"))
            .is_file()
    );
}

#[test]
fn a_path_in_the_kept_file_is_never_where_anything_is_touched() {
    let home = Home::new();
    home.dir("profile").dir("elsewhere").dir(".claude/skills");
    home.file("elsewhere/SKILL.md", &text(NAME));
    let file = home.at("profile/skills.json");
    let edited = json!({ "skills": [{
        "program": "claude-code",
        "name": NAME,
        "path": home.at("elsewhere/SKILL.md"),
        "sha256": sha256(text(NAME).as_bytes()),
        "made_skills": true,
    }] });
    home.file("profile/skills.json", &edited.to_string());
    let kept = read_kept(&file);
    assert_eq!(kept.len(), 1);
    assert_eq!(
        remove_all(&home.0, &kept),
        [(Client::ClaudeCode, Removed::Missing)]
    );
    assert!(
        home.at("elsewhere/SKILL.md").is_file(),
        "the path was never read"
    );
    assert!(
        home.at(".claude/skills").is_dir(),
        "nor skills, which the product didn't make"
    );
}

#[test]
fn a_file_put_in_place_of_the_one_written_is_left() {
    let home = Home::new();
    home.dir(".claude/skills");
    let kept = home.click(Client::ClaudeCode, None).unwrap();
    // The same words in a new file: not the one this run wrote. Made beside it and renamed over
    // it, since a file made after a delete may be given the freed inode's number.
    let file = home.at(&format!(".claude/skills/{NAME}/SKILL.md"));
    home.file(&format!(".claude/skills/{NAME}/new"), &text(NAME));
    fs::rename(home.at(&format!(".claude/skills/{NAME}/new")), &file).unwrap();
    assert_eq!(remove(&home.0, &kept), Removed::Changed);
    assert!(file.is_file());
}

#[test]
fn nothing_is_written_where_others_may_write() {
    for (folder, mode) in [(".claude", 0o757), (".claude/skills", 0o777)] {
        let home = Home::new();
        home.dir(".claude/skills");
        fs::set_permissions(home.at(folder), fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(
            home.click(Client::ClaudeCode, None),
            Err(Refused::NoFolder),
            "{folder}"
        );
        assert_eq!(home.tree(), [".claude", ".claude/skills"], "{folder}");
    }
    // The skill's own folder, ours, since opened to all: not written into again.
    let home = Home::new();
    home.dir(".claude/skills");
    let kept = home.click(Client::ClaudeCode, None).unwrap();
    let folder = home.at(&format!(".claude/skills/{NAME}"));
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(
        home.click(Client::ClaudeCode, Some(&kept)),
        Err(Refused::NotOurs)
    );
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
    // A group may write.
    let home = Home::new();
    home.dir(".claude/skills");
    fs::set_permissions(home.at(".claude/skills"), fs::Permissions::from_mode(0o770)).unwrap();
    assert!(home.click(Client::ClaudeCode, None).is_ok());
}

#[test]
fn a_list_that_cant_be_kept_takes_away_what_was_just_written() {
    let home = Home::new();
    home.dir(".claude/skills").dir(".agents");
    let after = add(&home.0, &[Client::ClaudeCode, Client::Codex], &[]);
    assert_eq!(after.len(), 2);
    let kept = kept_or_undone(
        after,
        |_| Err(std::io::ErrorKind::Other.into()),
        |fresh| {
            remove_all(&home.0, fresh);
        },
    );
    assert!(kept.is_empty());
    assert_eq!(
        home.tree(),
        [".agents", ".claude", ".claude/skills"],
        "the file, its folder and the skills folder made are gone"
    );

    // Kept, what was written stays.
    let after = add(&home.0, &[Client::ClaudeCode], &[]);
    let kept = kept_or_undone(after, |_| Ok(()), |_| panic!("nothing to undo"));
    assert_eq!(kept.len(), 1);
    assert!(
        home.at(&format!(".claude/skills/{NAME}/SKILL.md"))
            .is_file()
    );
}

#[test]
fn a_kept_entry_under_another_name_isnt_ours_for_this_one() {
    let home = Home::new();
    home.dir(&format!(".claude/skills/{NAME}"));
    let other = Kept {
        program: Client::ClaudeCode,
        name: "another".into(),
        sha256: sha256(b"x"),
        made_skills: false,
        written: None,
    };
    assert_eq!(
        home.click(Client::ClaudeCode, Some(&other)),
        Err(Refused::NotOurs)
    );
}

#[test]
fn only_claude_code_and_codex_get_a_file() {
    let home = Home::new();
    home.dir(".claude/skills").dir(".agents/skills");
    for program in [Client::Other, Client::Command] {
        assert_eq!(home.click(program, None), Err(Refused::NoFolder));
    }
    assert_eq!(
        home.tree(),
        [".agents", ".agents/skills", ".claude", ".claude/skills"]
    );
}

#[test]
fn a_name_that_is_a_path_writes_nothing() {
    let home = Home::new();
    home.dir(".claude/skills");
    for name in ["", ".", "..", "../x", "a/b", "Upper"] {
        let written = write(&home.0, Client::ClaudeCode, name, "x", None);
        assert_eq!(written, Err(Refused::Failed), "{name:?}");
    }
    assert!(super::valid(super::name()), "the command's own name is one");
    assert_eq!(home.tree(), [".claude", ".claude/skills"]);
}

#[test]
fn the_text_names_the_command_and_holds_no_secret_path_or_level() {
    let text = text(NAME);
    assert!(
        text.starts_with(&format!("---\nname: {NAME}\ndescription: ")),
        "frontmatter on line one"
    );
    assert!(!text.contains('{') && !text.contains('}'));
    for word in [
        "/Users",
        "/home",
        "~",
        "token",
        "Full access",
        "Work on pages",
        "Look only",
    ] {
        assert!(!text.contains(word), "{word}");
    }
    let hex_run = text
        .split(|c: char| !c.is_ascii_hexdigit())
        .any(|run| run.len() >= 32);
    assert!(!hex_run, "nothing like a token");
    let lines = [
        format!("{NAME} help"),
        format!("{NAME} help <command>"),
        format!("{NAME} done"),
        "--as".into(),
    ];
    for line in lines {
        assert!(text.contains(&line), "{line}");
    }
}

#[test]
fn the_kept_file_holds_what_was_written_for_the_user_alone() {
    let home = Home::new();
    let file = home.at("profile/skills.json");
    home.dir("profile");
    let kept = vec![Kept {
        program: Client::Codex,
        name: NAME.into(),
        sha256: sha256(b"x"),
        made_skills: true,
        written: None,
    }];
    write_kept(&file, &kept).unwrap();
    assert_eq!(mode(&file), 0o600);
    assert_eq!(read_kept(&file), kept);
    assert_eq!(home.tree(), ["profile", "profile/skills.json"]);
    write_kept(&file, &[]).unwrap();
    assert!(!file.exists());
}

#[test]
#[should_panic(expected = "floor 9")]
fn no_test_reaches_the_real_home() {
    _ = real_home();
}

#[test]
fn add_writes_for_the_programs_found_and_clears_a_skill_under_an_old_name() {
    let home = Home::new();
    home.dir(".claude/skills").dir(".agents");
    // A skill of ours from before the command took its name now.
    let old = write(&home.0, Client::ClaudeCode, "old-name", "old", None).unwrap();
    let kept = add(&home.0, &[Client::ClaudeCode], &[old]);
    assert_eq!(kept.len(), 1);
    assert_eq!(
        (kept[0].program, kept[0].name.as_str()),
        (Client::ClaudeCode, name())
    );
    assert!(!home.at(".claude/skills/old-name").exists());
    assert!(!home.at(".agents/skills").exists(), "Codex wasn't found");
    assert_eq!(
        remove_all(&home.0, &kept),
        [(Client::ClaudeCode, Removed::Gone)]
    );
    assert_eq!(home.tree(), [".agents", ".claude", ".claude/skills"]);
}
#[test]
fn a_home_unset_empty_or_relative_is_none_and_nothing_is_written() {
    assert_eq!(home_of(None), None, "unset");
    assert_eq!(home_of(Some(String::new())), None, "empty");
    assert_eq!(home_of(Some("made-up/home".into())), None, "relative");
    assert_eq!(
        home_of(Some("/made-up".into())),
        Some(PathBuf::from("/made-up"))
    );
    // Handed one anyway, the writer, remover and look start nowhere, here or anywhere else.
    let kept = Kept {
        program: Client::ClaudeCode,
        name: NAME.into(),
        sha256: sha256(text(NAME).as_bytes()),
        made_skills: true,
        written: None,
    };
    for home in ["", "skill-relative-home"] {
        let home = Path::new(home);
        assert_eq!(
            write(home, Client::ClaudeCode, NAME, &text(NAME), None),
            Err(Refused::Home),
            "{home:?}"
        );
        assert_eq!(remove(home, &kept), Removed::Missing, "{home:?}");
        assert_eq!(look(home, &kept), Place::Missing, "{home:?}");
        assert_eq!(at(home, Client::ClaudeCode, NAME), None, "{home:?}");
    }
    assert!(!Path::new("skill-relative-home").exists());
}

#[test]
fn the_look_says_where_each_file_is_and_changes_nothing() {
    let home = Home::new();
    home.dir(".claude/skills");
    let kept = home.click(Client::ClaudeCode, None).unwrap();
    let before = home.tree();
    assert_eq!(look(&home.0, &kept), Place::Ours);
    home.file(&format!(".claude/skills/{NAME}/SKILL.md"), "mine");
    assert_eq!(look(&home.0, &kept), Place::Changed);
    assert_eq!(home.tree(), before, "read only");
    assert_eq!(remove(&home.0, &kept), Removed::Changed);
    assert_eq!(
        look(&home.0, &kept),
        Place::Changed,
        "left as the user had it"
    );
    fs::remove_file(home.at(&format!(".claude/skills/{NAME}/SKILL.md"))).unwrap();
    assert_eq!(look(&home.0, &kept), Place::Missing);
    // Past a link the remover wouldn't follow, the look finds nothing either.
    let home = Home::new();
    home.dir(&format!("elsewhere/skills/{NAME}")).dir(".claude");
    home.file(&format!("elsewhere/skills/{NAME}/SKILL.md"), &text(NAME));
    symlink(home.at("elsewhere/skills"), home.at(".claude/skills")).unwrap();
    let mut linked = kept.clone();
    linked.written = None;
    assert_eq!(look(&home.0, &linked), Place::Missing);
}

#[test]
fn the_reveal_is_made_from_the_home_program_and_checked_name() {
    let home = Path::new("/made-up");
    assert_eq!(
        at(home, Client::Codex, NAME),
        Some(PathBuf::from(format!(
            "/made-up/.agents/skills/{NAME}/SKILL.md"
        )))
    );
    for name in ["", "..", "../x", "a/b"] {
        assert_eq!(at(home, Client::ClaudeCode, name), None, "{name}");
    }
    assert_eq!(at(home, Client::Other, NAME), None);
}
