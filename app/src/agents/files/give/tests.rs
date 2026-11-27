use std::os::unix::fs::{PermissionsExt, symlink};

use super::*;
use crate::agents::files::Row;

/// A made-up home with a granted folder, ilo's copies of attached files, a file outside both and
/// a key on the never list, in a tree of the test's own. On the Mac it's under the user's caches,
/// as the temporary folder is under /private, which the never list holds.
struct Home {
    base: PathBuf,
    never: Never,
}

impl Home {
    fn new(name: &str) -> Home {
        let root = match cfg!(target_os = "macos") {
            true => PathBuf::from(std::env::var_os("HOME").unwrap()).join("Library/Caches"),
            false => std::env::temp_dir(),
        };
        let base = root.join(format!("ilo-give-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let base = std::fs::canonicalize(&base).unwrap();
        let home = base.join("home");
        for (path, text) in [
            ("home/Taxes/annual statement.pdf", "%PDF-1.7 the statement"),
            ("home/Taxes/payslip.pdf", "%PDF-1.7 a payslip"),
            ("home/Elsewhere/receipt.jpg", "a receipt"),
            ("home/.ssh/id_ed25519", "a key"),
            ("attached/scan.png", "a scan"),
        ] {
            let path = base.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let never = Never::of(home, vec![base.join("home/profile")], None);
        Home { base, never }
    }

    fn at(&self, path: &str) -> PathBuf {
        self.base.join(path)
    }

    fn give(&self, tier: Tier, paths: &[&str], many: bool) -> Result<Vec<PathBuf>, NotGiven> {
        let folders = [Listed::for_test(self.at("home/Taxes"), Row::Change)];
        let never_ids = NeverIds::of(&self.never.by_identity());
        let attached = self.at("attached");
        let reach = Reach {
            tier,
            folders: &folders,
            attached: Some(&attached),
            never: &self.never,
            never_ids: &never_ids,
        };
        let paths: Vec<PathBuf> = paths.iter().map(|path| self.at(path)).collect();
        give(&paths, many, &reach, &self.uploads())
    }

    fn uploads(&self) -> PathBuf {
        self.at("profile-cache/uploads/thread")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        _ = std::fs::remove_dir_all(&self.base);
    }
}

fn refused(index: usize, why: Refused) -> Result<Vec<PathBuf>, NotGiven> {
    Err(NotGiven::File { index, why })
}

/// A file outside the set the thread was given is refused: at Look only, as no upload is; at Work
/// on pages, which asks before it, as outside its folders until the ask exists; and a file on the never list at
/// every level, Full access too. Nothing is copied for any of them.
#[test]
fn a_file_outside_the_given_set_is_refused() {
    let home = Home::new("outside");
    let outside = "home/Elsewhere/receipt.jpg";
    assert_eq!(
        home.give(Tier::Look, &[outside], false),
        refused(0, Refused::Needs(Tier::Work))
    );
    assert_eq!(
        home.give(Tier::Work, &[outside], false),
        refused(0, Refused::Guarded)
    );
    for tier in Tier::ALL {
        assert_eq!(
            home.give(tier, &["home/.ssh/id_ed25519"], false)
                .map(|_| ()),
            match tier {
                Tier::Look => Err(NotGiven::File {
                    index: 0,
                    why: Refused::Needs(Tier::Work)
                }),
                _ => Err(NotGiven::File {
                    index: 0,
                    why: Refused::Never
                }),
            }
        );
    }
    assert!(!home.uploads().exists());
    // Full access gives what's outside its folders, but never the never list.
    assert!(home.give(Tier::Full, &[outside], false).is_ok());
}

/// From Work on pages up, a file in its folders and an attached file are given, as the app's own
/// copies with the same bytes and name, never the files themselves.
#[test]
fn a_file_in_its_folders_and_an_attached_file_are_given_as_copies() {
    let home = Home::new("given");
    let paths = ["home/Taxes/annual statement.pdf", "attached/scan.png"];
    let given = home.give(Tier::Work, &paths, true).unwrap();
    assert_eq!(given.len(), 2);
    for (copy, original) in given.iter().zip(paths) {
        let original = home.at(original);
        assert!(copy.starts_with(home.uploads()), "{copy:?}");
        assert_eq!(copy.file_name(), original.file_name());
        assert_eq!(
            std::fs::read(copy).unwrap(),
            std::fs::read(&original).unwrap()
        );
        assert_ne!(copy, &original);
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(copy), 0o600);
        assert_eq!(mode(copy.parent().unwrap()), 0o700);
    }
}

/// What the page reads, minutes later as its form is sent, is the file as it was checked: a change
/// to the file, or another put under its name, after the upload reaches nothing of the page's.
#[test]
fn the_copy_holds_the_bytes_as_of_the_check() {
    let home = Home::new("later");
    let statement = home.at("home/Taxes/annual statement.pdf");
    let given = home
        .give(Tier::Work, &["home/Taxes/annual statement.pdf"], false)
        .unwrap();
    std::fs::write(&statement, "something else").unwrap();
    assert_eq!(
        std::fs::read_to_string(&given[0]).unwrap(),
        "%PDF-1.7 the statement"
    );
    std::fs::remove_file(&statement).unwrap();
    symlink(home.at("home/.ssh/id_ed25519"), &statement).unwrap();
    assert_eq!(
        std::fs::read_to_string(&given[0]).unwrap(),
        "%PDF-1.7 the statement"
    );
}

/// A link in a granted folder is judged where it lands, so one out to the never list or outside
/// its folders is refused as that place is.
#[test]
fn a_link_out_of_a_granted_folder_is_judged_where_it_lands() {
    let home = Home::new("link");
    symlink(
        home.at("home/.ssh/id_ed25519"),
        home.at("home/Taxes/key.pdf"),
    )
    .unwrap();
    symlink(
        home.at("home/Elsewhere/receipt.jpg"),
        home.at("home/Taxes/receipt.jpg"),
    )
    .unwrap();
    assert_eq!(
        home.give(Tier::Full, &["home/Taxes/key.pdf"], false),
        refused(0, Refused::Never)
    );
    assert_eq!(
        home.give(Tier::Work, &["home/Taxes/receipt.jpg"], false),
        refused(0, Refused::Guarded)
    );
}

/// A field that takes one file gets one; none is no upload; and one refused file among several
/// puts none of them, as nothing is copied before all are checked.
#[test]
fn the_field_gets_all_the_files_or_none() {
    let home = Home::new("count");
    let two = ["home/Taxes/payslip.pdf", "attached/scan.png"];
    assert_eq!(home.give(Tier::Work, &two, false), Err(NotGiven::Many));
    assert_eq!(home.give(Tier::Work, &[], true), Err(NotGiven::None));
    let one_bad = ["home/Taxes/payslip.pdf", "home/Elsewhere/receipt.jpg"];
    assert_eq!(
        home.give(Tier::Work, &one_bad, true),
        refused(1, Refused::Guarded)
    );
    assert!(!home.uploads().exists());
}

/// A folder never goes in, as the page would get read access under it, nor a name with nothing.
#[test]
fn a_folder_or_a_missing_file_is_refused() {
    let home = Home::new("folder");
    assert_eq!(
        home.give(Tier::Full, &["home/Taxes"], false),
        refused(0, Refused::NotOpened)
    );
    assert_eq!(
        home.give(Tier::Work, &["home/Taxes/gone.pdf"], false),
        refused(0, Refused::Gone)
    );
}

/// Where a path is is judged before whether anything's there, so a missing file outside the
/// thread's folders is refused as any file there is, never said to be missing.
#[test]
fn a_missing_file_outside_its_folders_is_refused_as_any_file_there() {
    let home = Home::new("missing");
    for (path, tier, why) in [
        ("home/Elsewhere/gone.jpg", Tier::Work, Refused::Guarded),
        ("home/Gone/receipt.jpg", Tier::Work, Refused::Guarded),
        ("home/.ssh/gone", Tier::Full, Refused::Never),
        ("home/Elsewhere/gone.jpg", Tier::Full, Refused::Gone),
    ] {
        assert_eq!(home.give(tier, &[path], false), refused(0, why), "{path}");
    }
}

/// The copies go with the thread: closed, deleted, or a private window's at its close.
#[test]
fn a_threads_uploads_go_when_its_forgotten() {
    let home = Home::new("forget");
    let given = home
        .give(Tier::Work, &["home/Taxes/payslip.pdf"], false)
        .unwrap();
    assert!(given[0].exists());
    forget(&home.uploads());
    assert!(!home.uploads().exists());
    assert!(
        home.at("home/Taxes/payslip.pdf").exists(),
        "the file itself stays"
    );
    // Forgetting twice, or a thread that never uploaded, is nothing.
    forget(&home.uploads());
}

/// The upload's lines name each file by its name alone and the site, as the table writes them.
#[test]
fn an_uploads_lines_are_the_tables() {
    use words::*;
    let site = "tax.example";
    let one = ["annual statement.pdf"];
    let two = ["payslip.pdf", "scan.png"];
    assert_eq!(
        uploading(&one, site, false),
        "Uploading \u{201c}annual statement.pdf\u{201d} to tax.example"
    );
    assert_eq!(
        uploading(&one, site, true),
        "Uploaded \u{201c}annual statement.pdf\u{201d} to tax.example"
    );
    assert_eq!(
        uploading(&two, site, false),
        "Uploading 2 files to tax.example"
    );
    assert_eq!(
        uploading(&two, site, true),
        "Uploaded 2 files to tax.example"
    );
    assert_eq!(
        uploaded("scan.png", site),
        "Uploaded \u{201c}scan.png\u{201d} to tax.example"
    );
    let not = |files: &[&str], why| not_uploaded(files, site, why).unwrap();
    assert_eq!(
        not(&two, NotUploaded::Given(NotGiven::Many)),
        "Couldn't upload 2 files to tax.example: the field takes one file"
    );
    assert_eq!(
        not(&one, NotUploaded::FieldGone),
        "Couldn't upload \u{201c}annual statement.pdf\u{201d} to tax.example: the page changed first"
    );
    assert_eq!(
        not(&two, NotUploaded::FieldGone),
        "Couldn't upload 2 files to tax.example: the page changed first"
    );
    let file = |index, why| NotUploaded::Given(NotGiven::File { index, why });
    assert_eq!(
        not(&two, file(1, Refused::Gone)),
        "Couldn't upload to tax.example: \u{201c}scan.png\u{201d} isn't there"
    );
    assert_eq!(
        not(&one, file(0, Refused::Needs(Tier::Work))),
        "Couldn't upload to tax.example: \u{201c}annual statement.pdf\u{201d} needs Work on pages"
    );
    assert_eq!(
        not(&one, file(0, Refused::Needs(Tier::Full))),
        "Couldn't upload to tax.example: \u{201c}annual statement.pdf\u{201d} needs Full access"
    );
    // A refusal with no clause of the upload's own is the step's failed line.
    assert_eq!(
        not(&one, file(0, Refused::Changed)),
        "Couldn't upload \u{201c}annual statement.pdf\u{201d} to tax.example"
    );
    assert_eq!(
        not(&two, file(1, Refused::Changed)),
        "Couldn't upload 2 files to tax.example"
    );
    assert_eq!(
        not(&one, file(0, Refused::Cloud)),
        "Couldn't upload to tax.example: \u{201c}annual statement.pdf\u{201d} isn't downloaded"
    );
    let product = crate::product::PRODUCT;
    assert_eq!(
        not(&two, file(1, Refused::Never)),
        format!(
            "Couldn't upload to tax.example: \u{201c}scan.png\u{201d} is in a place {product} keeps from threads"
        )
    );
    let outside = file(0, Refused::Guarded);
    assert_eq!(
        not(&one, outside),
        "Couldn't upload to tax.example: \u{201c}annual statement.pdf\u{201d} isn't in a folder you've added"
    );
    assert!(adds_folder(outside));
    assert!(!adds_folder(file(0, Refused::Never)));
    assert_eq!(
        not(&one, file(0, Refused::NotOpened)),
        format!(
            "Couldn't upload to tax.example: \u{201c}annual statement.pdf\u{201d} isn't a file {product} can open"
        )
    );
    let copy = |index, full| NotUploaded::Given(NotGiven::Copy { index, full });
    assert_eq!(
        not(&two, copy(1, false)),
        "Couldn't upload to tax.example: \u{201c}scan.png\u{201d} couldn't be copied"
    );
    assert_eq!(
        not(&two, copy(0, true)),
        "Couldn't upload to tax.example: the disk is full"
    );
    // No files is no line: only the assistant is told.
    assert_eq!(
        not_uploaded(&[], site, NotUploaded::Given(NotGiven::None)),
        None
    );
    // No line names a path.
    for why in [
        file(0, Refused::Never),
        outside,
        NotUploaded::FieldGone,
        copy(0, true),
    ] {
        assert!(!not(&one, why).contains('/'));
    }
}

/// ilo keeps its copies of a thread's attached files in its own folder, which is on the never
/// list: they're still given as attached, and nothing else there is.
#[test]
fn attached_copies_in_ilos_own_folder_are_given() {
    let home = Home::new("own-folder");
    let copies = home.at("home/profile/Tasks/thread files");
    std::fs::create_dir_all(&copies).unwrap();
    std::fs::write(copies.join("scan.png"), "a scan").unwrap();
    std::fs::write(home.at("home/profile/Cookies"), "cookies").unwrap();
    let never_ids = NeverIds::of(&home.never.by_identity());
    let reach = Reach {
        tier: Tier::Work,
        folders: &[],
        attached: Some(&copies),
        never: &home.never,
        never_ids: &never_ids,
    };
    let given = give(&[copies.join("scan.png")], false, &reach, &home.uploads()).unwrap();
    assert_eq!(std::fs::read_to_string(&given[0]).unwrap(), "a scan");
    let cookies = give(
        &[home.at("home/profile/Cookies")],
        false,
        &reach,
        &home.uploads(),
    );
    assert_eq!(cookies, refused(0, Refused::Never));
}
