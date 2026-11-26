use super::*;

/// A folder of the test's own, empty.
fn root(name: &str) -> PathBuf {
    let made = std::env::temp_dir().join(format!("thread-cards-{}-{name}", std::process::id()));
    _ = std::fs::remove_dir_all(&made);
    std::fs::create_dir_all(&made).unwrap();
    made.join(FOLDER)
}

/// A made-up picture: 3 by 2 pixels, each its own colour.
fn picture() -> Pixels {
    Pixels {
        bgra: (0..24).map(|at| (at * 10) as u8).collect(),
        width: 3,
        height: 2,
    }
}

fn names(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = (std::fs::read_dir(folder).into_iter().flatten().flatten())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// An id is 32 hex digits of its own making, a new one each time; only that shape reads back,
/// and printing one prints none of it.
#[test]
fn a_card_id_is_random_hex_and_prints_nothing() {
    let (one, other) = (CardId::new(), CardId::new());
    assert_ne!(one, other);
    assert_eq!(one.as_str().len(), 32);
    assert_eq!(CardId::parse(one.as_str()), Some(one.clone()));
    for bad in [
        "",
        "../../etc",
        &one.as_str()[..31],
        &one.as_str().to_uppercase(),
        &format!("{}/", &one.as_str()[..31]),
    ] {
        assert_eq!(CardId::parse(bad), None, "{bad}");
    }
    assert!(!format!("{one:?}").contains(one.as_str()));
}

/// A card is a folder named by its id alone, which only its owner can open, holding its line
/// and its picture under the names of this file's own, each only its owner can read. They read
/// back as written.
#[test]
fn a_card_is_its_line_and_picture_in_a_folder_of_its_own() {
    let root = root("written");
    let id = CardId::new();
    let line = "Two seats at 9:05, car 4.";
    assert!(write(
        &root,
        &id,
        Some(line),
        &Picture::Write(picture()),
        None
    ));
    assert_eq!(names(&root), [id.as_str()]);
    let folder = root.join(id.as_str());
    assert_eq!(names(&folder), ["card.png", "line"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&root), 0o700);
        assert_eq!(mode(&folder), 0o700);
        assert_eq!(mode(&folder.join("line")), 0o600);
        assert_eq!(mode(&folder.join("card.png")), 0o600);
    }
    let card = read(&root, &id);
    assert_eq!(card.line.as_deref(), Some(line));
    assert_eq!(card.picture, Some(picture()));
}

/// Written again, a card holds what it was given that time alone: no picture leaves none of the
/// one before, and with neither, nothing is left of it.
#[test]
fn a_card_written_again_keeps_nothing_of_the_one_before() {
    let root = root("again");
    let id = CardId::new();
    write(&root, &id, Some("First."), &Picture::Write(picture()), None);
    assert!(write(&root, &id, Some("Second."), &Picture::Remove, None));
    let card = read(&root, &id);
    assert_eq!(card.line.as_deref(), Some("Second."));
    assert_eq!(card.picture, None);
    assert!(write(&root, &id, None, &Picture::Remove, None));
    assert!(!root.join(id.as_str()).exists());
}

/// A file someone changed reads as nothing: a line too long or with a control character, a
/// picture that isn't a PNG or is too large, and anything that's a link.
#[test]
fn a_changed_card_reads_as_nothing() {
    let root = root("changed");
    let id = CardId::new();
    write(&root, &id, Some("Fine."), &Picture::Write(picture()), None);
    let folder = root.join(id.as_str());
    for line in ["a\u{1b}[2J".to_string(), "x".repeat(5000)] {
        std::fs::write(folder.join("line"), &line).unwrap();
        assert_eq!(read(&root, &id).line, None);
    }
    std::fs::write(folder.join("card.png"), b"not a picture").unwrap();
    assert_eq!(read(&root, &id).picture, None);
    let large = Pixels {
        bgra: vec![0; 4 * 3000],
        width: 3000,
        height: 1,
    };
    write(&root, &id, None, &Picture::Write(large), None);
    assert_eq!(read(&root, &id).picture, None);
    #[cfg(unix)]
    {
        let elsewhere = root.with_file_name("elsewhere");
        std::fs::write(&elsewhere, "Not the card's.").unwrap();
        _ = std::fs::remove_file(folder.join("line"));
        std::os::unix::fs::symlink(&elsewhere, folder.join("line")).unwrap();
        assert_eq!(read(&root, &id).line, None);
        // Written over, the link goes and what it pointed to stays as it was.
        write(&root, &id, Some("Mine."), &Picture::Leave, None);
        assert_eq!(
            std::fs::read_to_string(&elsewhere).unwrap(),
            "Not the card's."
        );
        assert_eq!(read(&root, &id).line.as_deref(), Some("Mine."));
    }
}

/// Removing one card leaves the others; Remove All leaves nothing.
#[test]
fn remove_takes_one_card_and_remove_all_takes_every_one() {
    let root = root("removed");
    let (one, other) = (CardId::new(), CardId::new());
    write(&root, &one, Some("One."), &Picture::Remove, None);
    write(
        &root,
        &other,
        Some("Other."),
        &Picture::Write(picture()),
        None,
    );
    assert!(remove(&root, &one));
    assert_eq!(names(&root), [other.as_str()]);
    remove_all(&root);
    assert!(!root.exists());
}

/// At launch, a card no kept record holds goes, and one a record holds stays. Anything not named
/// as a card is left alone; with no card left, neither is the folder.
#[test]
fn the_sweep_takes_a_card_with_no_record_and_leaves_one_with_a_record() {
    let root = root("swept");
    let (recorded, left) = (CardId::new(), CardId::new());
    write(
        &root,
        &recorded,
        Some("Kept."),
        &Picture::Write(picture()),
        None,
    );
    write(
        &root,
        &left,
        Some("Left."),
        &Picture::Write(picture()),
        None,
    );
    sweep(&root, std::slice::from_ref(&recorded));
    assert_eq!(names(&root), [recorded.as_str()]);
    assert_eq!(read(&root, &recorded).line.as_deref(), Some("Kept."));
    sweep(&root, &[]);
    assert!(!root.exists());
}

/// Asked one after another, the jobs run in that order, each finding what the one before left: a
/// card Remove All took is written again by a later Done; one taken out and written again holds
/// only what came after; a sweep takes only what no record holds, a card written just before it
/// too. Asked again once the queue is empty, it runs again.
#[gpui::test]
fn the_queue_runs_its_jobs_in_the_order_asked(cx: &mut gpui::TestAppContext) {
    let root = root("queue");
    let queue = Queue::default();
    let executor = cx.executor();
    let write = |id: &CardId, line: &str, picture: Picture| Job::Write {
        root: root.clone(),
        id: id.clone(),
        line: Some(line.to_string()),
        picture,
        log: None,
    };
    let (first, second, third) = (CardId::new(), CardId::new(), CardId::new());
    queue.ask(
        write(&second, "Gone.", Picture::Write(picture())),
        &executor,
    );
    queue.ask(Job::RemoveAll(root.clone()), &executor);
    queue.ask(
        write(&first, "Before.", Picture::Write(picture())),
        &executor,
    );
    queue.ask(Job::Remove(root.clone(), vec![first.clone()]), &executor);
    queue.ask(write(&first, "After.", Picture::Leave), &executor);
    queue.ask(write(&second, "Back.", Picture::Leave), &executor);
    queue.ask(write(&third, "Swept.", Picture::Leave), &executor);
    let kept = vec![first.clone(), second.clone()];
    queue.ask(Job::Sweep(root.clone(), kept), &executor);
    cx.run_until_parked();

    let card = |id: &CardId| read(&root, id);
    assert_eq!(card(&first).line.as_deref(), Some("After."));
    assert_eq!(card(&first).picture, None);
    assert_eq!(card(&second).line.as_deref(), Some("Back."));
    assert_eq!(card(&second).picture, None);
    assert_eq!(card(&third), Card::default());
    assert!(!root.join(third.as_str()).exists());

    queue.ask(Job::Remove(root.clone(), vec![first.clone()]), &executor);
    cx.run_until_parked();
    assert_eq!(card(&first), Card::default());
    assert_eq!(card(&second).line.as_deref(), Some("Back."));
}

/// A card's log is a file of its folder's own, only its owner can read, and reads back as the
/// records written. One the app didn't write, or over a log's most, reads as none, and the rest
/// of the card stays. A write with no log takes the one there out, as one over the most at the
/// thread's Done is never written.
#[test]
fn a_cards_log_is_a_file_of_its_own_read_back_whole_or_not_at_all() {
    use crate::agents::kept::log::{self, Record};
    let root = root("log");
    let id = CardId::new();
    let records = vec![
        Record::You("Two seats".into()),
        Record::Said("Done.".into()),
    ];
    let text = log::write(&records).unwrap();
    assert!(write(
        &root,
        &id,
        Some("Line."),
        &Picture::Remove,
        Some(&text)
    ));
    let folder = root.join(id.as_str());
    assert_eq!(names(&folder), ["line", "log"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(folder.join("log"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert_eq!(read_log(&root, &id), Some(records));

    std::fs::write(folder.join("log"), "button\tPay\n").unwrap();
    assert_eq!(read_log(&root, &id), None);
    assert_eq!(read(&root, &id).line.as_deref(), Some("Line."));
    std::fs::write(folder.join("log"), "x".repeat(log::MOST + 1)).unwrap();
    assert_eq!(read_log(&root, &id), None);

    assert!(write(&root, &id, Some("Line."), &Picture::Remove, None));
    assert_eq!(names(&folder), ["line"]);
    // A log alone is a card's.
    assert!(write(&root, &id, None, &Picture::Remove, Some(&text)));
    assert_eq!(names(&folder), ["log"]);
    assert!(remove(&root, &id));
    assert!(!folder.exists());
}
