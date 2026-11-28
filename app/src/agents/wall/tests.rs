use super::*;

/// A made-up profile and home of a test's own under the system's temp folder, gone with it:
/// never the user's.
struct Made(PathBuf);

impl Made {
    fn new(name: &str) -> Made {
        let root = std::env::temp_dir().join(format!("ilo-put-up-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&root);
        for folder in [
            "Tasks/task",
            "home/.codex",
            "home/.claude",
            "bin",
            "ilo",
            "conf",
        ] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
        }
        std::fs::write(root.join("bin/codex"), "").unwrap();
        std::fs::write(root.join("ilo/mcp-bridge"), "").unwrap();
        Made(std::fs::canonicalize(&root).unwrap())
    }

    fn at(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }

    /// The places made under its `Tasks/.tmp`, marks and folders.
    fn places(&self) -> usize {
        std::fs::read_dir(self.at("Tasks/.tmp")).map_or(0, |entries| entries.count())
    }
}

impl Drop for Made {
    fn drop(&mut self) {
        _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What build is handed for a run of `made`'s by `cli`, keeping its conversation or not. `odd`
/// replaces one of the paths, `"data"` or `"task_dir"`, by the one given.
fn built_as(
    made: &Made,
    cli: Cli,
    keeps: bool,
    odd: Option<(&str, &Path)>,
) -> Result<Wall, WallRefused> {
    let given = |name: &str, default: PathBuf| match odd {
        Some((odd, path)) if odd == name => path.to_path_buf(),
        _ => default,
    };
    let cli_home = match cli {
        Cli::Codex => made.at("home/.codex"),
        Cli::Claude => made.at("home/.claude"),
    };
    let claude_config = (cli == Cli::Claude).then(|| made.at("conf/.claude.json"));
    build(&WallFor {
        cli,
        task_dir: &given("task_dir", made.at("Tasks/task")),
        config: &made.at("Tasks/task.json"),
        program: &made.at("bin/codex"),
        data: &given("data", made.at("profile")),
        home: &made.at("home"),
        socket: &made.at("Agents.sock"),
        bridge: Some(&made.at("ilo/mcp-bridge")),
        places: &made.at("Tasks/.tmp"),
        cli_home: &cli_home,
        claude_config: claude_config.as_deref(),
        keeps,
    })
}

fn built(made: &Made) -> Result<Wall, WallRefused> {
    built_as(made, Cli::Claude, false, None)
}

/// A path the checks compare that can't be resolved refuses the box, never compared as given:
/// here ilo's data two folders below one that is there.
#[test]
fn a_path_the_checks_compare_that_cant_be_resolved_isnt_boxed() {
    let made = Made::new("unresolved");
    let deep = made.at("missing/profile");
    let refused = built_as(&made, Cli::Claude, false, Some(("data", &deep))).unwrap_err();
    assert_eq!(refused, WallRefused::Failed(NOT_RESOLVED.into()));
    assert_eq!(made.places(), 0);
}

/// A program's folder is read whole only where no folder of the user's is: outside the home,
/// or in `~/bin` or below a dot-folder of it; never the home, a folder above it, one of its
/// own folders, a dot-folder itself, or a folder that is or holds one kept out.
#[test]
fn a_programs_folder_is_read_whole_only_where_no_folder_of_the_users_is() {
    let home = Path::new("/Users/someone");
    let cli_home = home.join(".codex");
    let data = home.join("Library/Application Support/ilo");
    let kept_out = [cli_home.as_path(), data.as_path()];
    let whole = |dir: &str| program_folder_whole(Path::new(dir), home, &kept_out);
    for dir in [
        "/opt/homebrew/Cellar/codex/1.0.0/bin",
        "/usr/local/lib/node_modules/@openai/codex/bin",
        "/Users/someone/.local/share/claude/versions",
        "/Users/someone/.nvm/versions/node/v22.0.0/bin",
        "/Users/someone/.local/bin",
        "/Users/someone/bin",
        "/Users/someone/bin/tools",
    ] {
        assert!(whole(dir), "{dir} isn't read whole");
    }
    for dir in [
        "/",
        "/Users",
        "/Users/someone",
        "/Users/someone/Desktop",
        "/Users/someone/Documents/tools",
        "/Users/someone/Downloads",
        "/Users/someone/.local",
        "/Users/someone/.codex",
        "/Users/someone/Library/Application Support",
        "/Users/someone/Library/Application Support/ilo",
    ] {
        assert!(!whole(dir), "{dir} is read whole");
    }
    // An assistant's home outside the user's keeps the folders above it out too.
    let elsewhere = [Path::new("/opt/codex-home")];
    assert!(!program_folder_whole(Path::new("/opt"), home, &elsewhere));
    assert!(program_folder_whole(
        Path::new("/opt/bin"),
        home,
        &elsewhere
    ));
}

#[cfg(unix)]
#[test]
fn a_sign_in_that_is_a_link_isnt_boxed_and_leaves_no_place() {
    let made = Made::new("sign-in");
    std::fs::write(made.at("elsewhere.json"), "").unwrap();
    std::os::unix::fs::symlink(made.at("elsewhere.json"), made.at("home/.codex/auth.json"))
        .unwrap();
    assert_eq!(
        built_as(&made, Cli::Codex, false, None).unwrap_err(),
        WallRefused::Failed(SIGN_IN_LINK.into())
    );
    assert_eq!(made.places(), 0);
}

#[test]
fn a_run_folder_too_deep_for_the_rules_isnt_boxed_and_leaves_no_place() {
    let made = Made::new("deep");
    let deep = (0..launch::PARAMETERS_MAX)
        .fold(made.at("Tasks/task"), |path, at| path.join(at.to_string()));
    std::fs::create_dir_all(&deep).unwrap();
    assert_eq!(
        built_as(&made, Cli::Claude, false, Some(("task_dir", &deep))).unwrap_err(),
        WallRefused::Failed(rules::TOO_DEEP.into())
    );
    assert_eq!(made.places(), 0);
}

#[test]
fn a_run_with_no_bridge_beside_ilo_isnt_boxed_and_leaves_no_place() {
    let made = Made::new("bridge");
    std::fs::remove_file(made.at("ilo/mcp-bridge")).unwrap();
    let no_bridge = WallRefused::Failed(NO_BRIDGE.into());
    assert_eq!(built(&made).unwrap_err(), no_bridge);
    // A folder by its name is no bridge either.
    std::fs::create_dir(made.at("ilo/mcp-bridge")).unwrap();
    assert_eq!(built(&made).unwrap_err(), no_bridge);
    assert_eq!(made.places(), 0);
}

/// Where there's no launcher the box isn't put up and its place goes; on the Mac it's up, in a
/// place of its own under the tasks' and with its listener, kept or not.
#[test]
fn a_box_is_put_up_in_a_place_of_its_own_or_leaves_none() {
    let made = Made::new("up");
    for keeps in [false, true] {
        let built = built_as(&made, Cli::Claude, keeps, None);
        if cfg!(target_os = "macos") {
            let wall = built.unwrap();
            assert!(wall.tmp().unwrap().starts_with(made.at("Tasks/.tmp")));
            assert!(wall.port().is_some());
            drop(wall);
        } else {
            assert!(matches!(built, Err(WallRefused::Failed(_))));
            assert_eq!(made.places(), 0);
        }
    }
}

#[test]
fn a_run_reads_none_of_another_runs_files() {
    let tasks = Path::new("/nonexistent/Tasks");
    let [one, other] =
        ["one", "other"].map(|agent| (tasks.join(agent), tasks.join(format!("{agent}.json"))));
    let reads = |(dir, config): &(PathBuf, PathBuf)| {
        let nowhere = Path::new("/nonexistent");
        ilo_reads(&WallFor {
            cli: Cli::Claude,
            task_dir: dir,
            config,
            program: nowhere,
            data: nowhere,
            home: nowhere,
            socket: nowhere,
            bridge: Some(nowhere),
            places: nowhere,
            cli_home: nowhere,
            claude_config: None,
            keeps: false,
        })
    };
    for (mine, theirs) in [(&one, &other), (&other, &one)] {
        let mine = reads(mine);
        for path in reads(theirs) {
            assert!(
                mine.iter()
                    .all(|read| !path.starts_with(read) && !read.starts_with(&path)),
                "{path:?}"
            );
        }
        assert!(
            mine.iter()
                .all(|read| read != tasks && read.starts_with(tasks))
        );
    }
}

/// A Granted printed names no folder.
#[test]
fn a_granted_folder_printed_names_no_folder() {
    let granted = Granted::named_for_test("Private Letters");
    for printed in [
        format!("{granted:?}"),
        format!("{granted:#?}"),
        format!("{:?}", Some(granted.clone())),
    ] {
        assert!(printed.starts_with("Granted") || printed.starts_with("Some"));
        assert!(
            !printed.contains("nonexistent") && !printed.contains("Letters"),
            "{printed}"
        );
    }
}

/// The Mac boxes a run in, with ilo's launcher beside it, which a test's build made; Windows has
/// no sandbox, and any other system no box yet.
#[test]
fn only_the_mac_boxes_a_run_in() {
    let expected = match () {
        _ if cfg!(windows) => Err(Unavailable::NoSandbox),
        _ if cfg!(target_os = "macos") => Ok(()),
        _ => Err(Unavailable::NotBuilt),
    };
    assert_eq!(available(), expected);
}

/// Claude Code runs only in its box where a box could be, and as the user's program only where
/// the system has no sandbox, its own tools off either way (runner.rs). Codex is as it was.
#[test]
fn claude_code_runs_in_its_box_or_where_no_sandbox_is() {
    assert_eq!(runs(Cli::Claude, Ok(())), Runs::Boxed);
    assert_eq!(
        runs(Cli::Claude, Err(Unavailable::NoSandbox)),
        Runs::Unboxed
    );
    assert_eq!(runs(Cli::Claude, Err(Unavailable::NotBuilt)), Runs::Not);
    for available in [
        Ok(()),
        Err(Unavailable::NoSandbox),
        Err(Unavailable::NotBuilt),
    ] {
        assert_eq!(runs(Cli::Codex, available), Runs::Unboxed);
    }
}

/// A folder of the test's own, as the folder panel resolves it.
fn picked(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("ilo-wall-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder.canonicalize().unwrap()
}

#[test]
fn only_a_folder_itself_is_picked() {
    let made = picked("only");
    let file = made.join("file");
    std::fs::write(&file, "").unwrap();
    assert_eq!(Granted::picked(file), Err(Refusal::Gone));
    assert_eq!(Granted::picked(made.join("missing")), Err(Refusal::Gone));
    #[cfg(unix)]
    {
        let link = made.join("link");
        std::os::unix::fs::symlink(&made, &link).unwrap();
        assert_eq!(Granted::picked(link), Err(Refusal::Gone));
    }
    assert!(Granted::picked(made.clone()).is_ok());
    std::fs::remove_dir_all(made).unwrap();
}

/// A folder swapped at the picked path since the pick isn't the one picked.
#[cfg(unix)]
#[test]
fn a_folder_swapped_since_the_pick_is_not_the_one_picked() {
    let made = picked("swapped");
    let folder = made.join("folder");
    std::fs::create_dir(&folder).unwrap();
    let granted = Granted::picked(folder.clone()).unwrap();
    assert!(granted.still_picked());
    std::fs::rename(&folder, made.join("away")).unwrap();
    std::fs::create_dir(&folder).unwrap();
    assert!(!granted.still_picked());
    std::fs::remove_dir_all(made).unwrap();
}

/// The system's panel is the one way a folder is granted, from the composer's plus, so no other
/// source file makes one.
#[test]
fn only_the_folder_panel_grants_a_folder() {
    let made = ["Granted", "::picked("].concat();
    let mut folders = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    let mut makers = Vec::new();
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && std::fs::read_to_string(&path).unwrap().contains(&made)
            {
                makers.push(path.file_name().unwrap().to_string_lossy().into_owned());
            }
        }
    }
    // The composer's plus, and this file's tests.
    makers.sort();
    assert_eq!(makers, ["command_bar.rs", "tests.rs"]);
}
