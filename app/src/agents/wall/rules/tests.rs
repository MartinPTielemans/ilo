use super::*;
use crate::agents::wall::leftovers::CONTROL;
use crate::agents::wall::leftovers::Place as Places;

/// The text's lines that allow a write, each as written.
fn write_allows(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("(allow file-write"))
        .collect()
}

/// A folder of a test's own under the system's temp folder, gone with it.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Folder {
        let path = std::env::temp_dir().join(format!("ilo-rules-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Folder(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn codex_writes_in_its_home_its_four_sign_in_names_alone_each_directly_in_it() {
    let text = home(Cli::Codex);
    let allows = write_allows(&text);
    assert_eq!(
        allows,
        [
            "(allow file-write* (regex (string-append \"^\" (regex-quote (param \"CLI_HOME\")) \
             \"/\\\\.auth-[^/]+$\")))",
            "(allow file-write* (literal (string-append (param \"CLI_HOME\") \"/auth.json\")))",
            "(allow file-write* (literal (string-append (param \"CLI_HOME\") \"/auth.lock\")))",
            "(allow file-write* (literal (string-append (param \"CLI_HOME\") \"/auth.json.lock\")))",
        ]
    );
    // Claude Code's one fallback file, and nothing else of its home.
    let text = home(Cli::Claude);
    assert_eq!(
        write_allows(&text),
        [
            "(allow file-write* (literal (string-append (param \"CLI_HOME\") \"/.credentials.json\")))"
        ]
    );
}

#[test]
fn no_allow_reaches_a_name_a_later_start_reads() {
    for cli in Cli::ALL {
        let text = home(cli);
        let denied = (text.lines())
            .find(|line| line.starts_with("(deny file-write* (") && line.contains("subpath"))
            .unwrap();
        // The deny is made from the one list, name by name.
        for name in kept_out(cli) {
            assert!(denied.contains(&kept_out_filter(name)), "{name}");
            let bare = name.trim_end_matches(['/', '*']);
            assert!(
                !sign_in(cli).iter().any(|allowed| allowed.starts_with(bare)),
                "{name}"
            );
            assert!(
                sign_in_temp(cli).is_none_or(|start| !bare.starts_with(start)),
                "{name}"
            );
        }
        // And it comes after every allow, so it wins over each.
        let at = |line: &str| text.lines().position(|l| l == line).unwrap();
        assert!(
            write_allows(&text)
                .iter()
                .all(|allow| at(allow) < at(denied))
        );
    }
    assert!(home(Cli::Claude).contains("(deny file-write* (literal (param \"CLAUDE_CONFIG\")))"));
}

#[test]
fn nothing_but_a_plain_file_is_made_under_a_sign_in_name_and_that_rule_is_last() {
    for cli in Cli::ALL {
        let text = home(cli);
        let last = text.lines().last().unwrap();
        let filters = sign_in_filters(cli).join(" ");
        assert_eq!(
            last,
            format!(
                "(deny file-write-create (require-all (require-any {filters}) \
                 (require-not (vnode-type REGULAR-FILE))))"
            )
        );
        // The system's own type names: `SYMLINK`, never `SYMBOLIC-LINK`, which starts nothing.
        assert!(!text.contains("SYMBOLIC-LINK"));
    }
}

#[test]
fn every_parameter_inside_a_regex_is_quoted() {
    let shapes = shapes();
    let texts = shapes.iter().map(text);
    for text in [home(Cli::Codex), home(Cli::Claude), mark()]
        .into_iter()
        .chain(texts)
    {
        let mut rest = text.as_str();
        while let Some(at) = rest.find("(regex ") {
            rest = &rest[at + "(regex ".len()..];
            let regex = &rest[..rest.find('\n').unwrap_or(rest.len())];
            let regex = &regex[..regex.find(")) ").map_or(regex.len(), |end| end + 2)];
            assert_eq!(
                regex.matches("(param ").count(),
                regex.matches("(regex-quote (param ").count(),
                "{regex}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn each_parameter_is_where_its_links_lead() {
    let folder = Folder::new("params");
    let real = folder.0.join("home");
    std::fs::create_dir(&real).unwrap();
    let link = folder.0.join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let place = Places::make(&folder.0.join("places")).unwrap();
    let params = params(
        vec![
            (CLI_HOME, link.clone()),
            // A file the first save makes: its folder resolved, its name kept.
            (CLAUDE_CONFIG, link.join("not-yet.json")),
        ],
        place.mark(),
    )
    .unwrap();
    let real = std::fs::canonicalize(&real).unwrap();
    assert_eq!(params[0], (CLI_HOME, real.clone()));
    assert_eq!(params[1], (CLAUDE_CONFIG, real.join("not-yet.json")));
    assert_eq!(params[2].0, MARK);
}

#[test]
fn the_mark_is_a_parameter_alone_and_no_line_allows_the_control() {
    let folder = Folder::new("mark");
    let place = Places::make(&folder.0.join("places")).unwrap();
    let mark_name = place.mark().as_cstr().to_str().unwrap().to_string();
    let params = params(Vec::new(), place.mark()).unwrap();
    assert_eq!(params, [(MARK, PathBuf::from(&mark_name))]);
    let control = CONTROL.to_str().unwrap();
    let shapes = shapes();
    let texts = shapes.iter().map(|shape| ours(&text(shape)).to_string());
    for text in [home(Cli::Codex), home(Cli::Claude), mark()]
        .into_iter()
        .chain(texts)
    {
        assert!(!text.contains(&mark_name) && !text.contains(control));
        for line in text
            .lines()
            .filter(|line| line.starts_with("(allow mach-lookup"))
        {
            assert!(
                line.starts_with("(allow mach-lookup (global-name "),
                "{line}"
            );
            assert!(!line.contains("global-name-prefix") && !line.contains("global-name-regex"));
            assert!(!line.contains("\"ilo.mark."), "{line}");
        }
    }
    // Every name of ilo's refused, then this wall's own allowed, as the Mac runs showed them.
    assert_eq!(
        mark(),
        "(deny mach-lookup (global-name-prefix \"ilo.mark.\"))\n\
         (allow mach-lookup (global-name (param \"MARK\")))\n"
    );
}

#[test]
fn every_line_closes_what_it_opens() {
    let shapes = shapes();
    let texts = shapes.iter().map(|shape| ours(&text(shape)).to_string());
    for text in [home(Cli::Codex), home(Cli::Claude), mark()]
        .into_iter()
        .chain(texts)
    {
        for line in text.lines() {
            let mut depth = 0i32;
            let mut quoted = false;
            let mut escaped = false;
            for c in line.chars() {
                match (quoted, escaped, c) {
                    (true, false, '\\') => escaped = true,
                    (true, true, _) => escaped = false,
                    (_, false, '"') => quoted = !quoted,
                    (false, _, '(') => depth += 1,
                    (false, _, ')') => depth -= 1,
                    _ => {}
                }
                assert!(depth >= 0, "{line}");
            }
            assert!(depth == 0 && !quoted, "{line}");
        }
    }
}

/// Every shape a text is made for: each assistant and level, with and without each place, the
/// program by its folder and alone, and three folders above.
fn shapes() -> Vec<Shape> {
    let mut shapes = Vec::new();
    for cli in Cli::ALL {
        for level in Tier::ALL {
            for (folder, attached) in [(true, false), (false, true), (true, true)] {
                for program_folder in [true, false] {
                    shapes.push(Shape {
                        cli,
                        level,
                        folder,
                        attached,
                        program_folder,
                        port: 49152,
                        ancestors: 3,
                    });
                }
            }
        }
    }
    shapes
}

/// A program named alone is one literal, read and mapped as the bridge is, with nothing of its
/// folder; named by its folder, it's that folder's subpath and no literal.
#[test]
fn a_program_named_alone_is_its_one_file_and_nothing_of_its_folder() {
    for shape in shapes() {
        let text = text(&shape);
        let file = format!("(literal (param \"{PROGRAM}\"))");
        let folder = format!("(subpath (param \"{PROGRAM_DIR}\"))");
        let line = |name: &str| {
            (text.lines())
                .find(|line| line.contains(name))
                .map(str::to_owned)
        };
        match shape.program_folder {
            true => {
                assert!(!text.contains(&format!("\"{PROGRAM}\"")), "{shape:?}");
                let line = line(&folder).expect("the folder's line");
                assert!(line.contains("file-map-executable"), "{line}");
            }
            false => {
                assert!(!text.contains(PROGRAM_DIR), "{shape:?}");
                let line = line(&file).expect("the program's line");
                assert_eq!(
                    line,
                    format!(
                        "(allow file-read-data file-read-metadata file-test-existence \
                         file-map-executable {file})"
                    )
                );
            }
        }
    }
}

/// ilo's part of a text: what follows the base, which is upstream's as it is.
fn ours(text: &str) -> &str {
    text.strip_prefix(BASE)
        .expect("the base comes first, whole")
}

/// The operations of the one allow on `key`'s subpath, as the text writes them.
fn place_line_ops<'t>(text: &'t str, key: &str) -> Option<Vec<&'t str>> {
    let end = format!(" (subpath (param \"{key}\")))");
    let line = text
        .lines()
        .find(|line| line.starts_with("(allow ") && line.ends_with(&end))?;
    Some(
        line["(allow ".len()..line.len() - end.len()]
            .split(' ')
            .collect(),
    )
}

#[test]
fn each_place_gets_its_level_cell_by_cell() {
    for shape in shapes() {
        let text = text(&shape);
        for (key, given, place) in [
            (FOLDER, shape.folder, Place::Folder),
            (ATTACHED, shape.attached, Place::Attached),
        ] {
            let Some(ops) = place_line_ops(&text, key) else {
                assert!(!given, "{shape:?}");
                continue;
            };
            assert!(given, "{shape:?}");
            for op in Op::ALL {
                let asked = seatbelt(op);
                if files::allows(shape.level, place, op) {
                    assert!(
                        asked.iter().all(|each| ops.contains(each)),
                        "{shape:?} {op:?}"
                    );
                } else {
                    // Some part of what a refused cell needs is missing, so the system refuses it.
                    assert!(
                        !asked.iter().all(|each| ops.contains(each)),
                        "{shape:?} {op:?}"
                    );
                }
            }
            // And nothing no cell asks for.
            let every: Vec<&str> = (Op::ALL.iter())
                .filter(|op| files::allows(shape.level, place, **op))
                .flat_map(|op| seatbelt(*op).iter().copied())
                .collect();
            assert!(ops.iter().all(|op| every.contains(op)), "{shape:?}");
        }
    }
}

#[test]
fn the_base_comes_first_whole_and_the_mark_last_of_the_lookups() {
    for shape in shapes() {
        let text = text(&shape);
        let ours = ours(&text);
        // The text ends with the mark's two lines, so no lookup rule after them reopens a name.
        assert!(ours.ends_with(&mark()));
        let lines: Vec<&str> = text.lines().collect();
        let deny = lines
            .iter()
            .position(|line| line.contains("\"ilo.mark.\""))
            .unwrap();
        assert_eq!(deny, lines.len() - 2);
        assert_eq!(text.matches("ilo.mark.").count(), 1);
    }
}

#[test]
fn every_parameter_is_one_of_the_named() {
    let named = [
        FOLDER,
        ATTACHED,
        TASK_DIR,
        CONFIG,
        CONFIG_MD,
        TMP,
        SOCKET,
        PROGRAM_DIR,
        PROGRAM,
        BRIDGE,
        CLI_HOME,
        CLAUDE_CONFIG,
        MARK,
    ];
    for shape in shapes() {
        let text = text(&shape);
        for (at, _) in text.match_indices("(param \"") {
            let rest = &text[at + "(param \"".len()..];
            let key = &rest[..rest.find('"').unwrap()];
            let up = (UP[..shape.ancestors]).contains(&key);
            assert!(named.contains(&key) || up, "{key}");
        }
    }
}

#[test]
fn the_folders_above_are_their_names_alone_each_a_literal() {
    for shape in shapes() {
        let text = text(&shape);
        let lines: Vec<&str> = (text.lines())
            .filter(|line| line.contains("(param \"UP_"))
            .collect();
        assert_eq!(lines.len(), shape.ancestors);
        for (line, up) in lines.iter().zip(UP) {
            assert_eq!(
                *line,
                format!(
                    "(allow file-read-metadata file-test-existence (literal (param \"{up}\")))"
                )
            );
        }
    }
    let above = ancestors(
        [
            Path::new("/a/b/folder"),
            Path::new("/a/b/c/file"),
            Path::new("/x"),
        ]
        .into_iter(),
    );
    // The root is the system's, and no named path is its own folder above.
    let above: Vec<&str> = above.iter().map(|up| up.to_str().unwrap()).collect();
    assert_eq!(above, ["/a/b", "/a", "/a/b/c"]);
}

#[test]
fn no_system_root_is_a_users_or_a_volumes() {
    let mut roots = Vec::new();
    for allow in system::SYSTEM {
        for root in allow.roots {
            roots.push((allow.ops, allow.filter, *root));
        }
    }
    for (ops, filter, root) in roots {
        // A regex root is the system's devices alone.
        if filter == "regex" {
            assert!(root.starts_with("^/dev/"), "{root}");
            continue;
        }
        for kept in [
            "/Users",
            "/System/Volumes/Data/Users",
            "/Volumes",
            "/private/var/folders",
            "/var/folders",
            "/private/tmp",
            "/Applications",
        ] {
            let under = root
                .strip_prefix(kept)
                .is_some_and(|rest| rest.starts_with('/'));
            assert!(!under, "{root}");
            if root == kept {
                // The folder of homes is read for its name alone.
                assert_eq!(
                    (ops, filter),
                    ("file-read-metadata file-test-existence", "literal")
                );
            }
        }
        // Nothing of /var or /private/var whole, which hold every user's temporary folders, nor
        // the system's library whole, which holds its keychains and every service's bundle.
        if filter == "subpath" {
            let whole = [
                "/",
                "/var",
                "/private/var",
                "/private",
                "/System",
                "/System/Library",
            ];
            assert!(!whole.contains(&root));
        }
    }
}

/// The system's Perl starts: its own tree is read and mapped, and nothing wider. The site's and
/// the network's module folders, which someone other than the system writes, stay unread.
#[test]
fn the_systems_perl_is_its_own_tree_read_and_mapped() {
    let text = system::text();
    let perl: Vec<&str> = (text.lines())
        .filter(|line| line.contains("Perl"))
        .collect();
    assert_eq!(
        perl,
        ["(allow file-read* file-test-existence file-map-executable \
             (subpath \"/System/Library/Perl\"))"]
    );
}

#[test]
fn the_network_is_the_listener_and_the_sockets_alone() {
    for shape in shapes() {
        let text = text(&shape);
        assert!(!text.contains("syslog"));
        let ours = ours(&text);
        let network: Vec<&str> = (ours.lines())
            .filter(|line| line.contains("(allow network"))
            .collect();
        assert_eq!(
            network,
            [
                "(allow network-outbound (remote ip \"localhost:49152\"))",
                "(allow network-outbound (remote unix-socket (path-literal (param \"SOCKET\"))))",
                "(allow network-bind (local unix-socket (subpath (param \"TMP\"))))",
                "(allow network-outbound (remote unix-socket (subpath (param \"TMP\"))))",
            ]
        );
        let deny = ours
            .lines()
            .position(|line| line == "(deny network*)")
            .unwrap();
        let first = ours
            .lines()
            .position(|line| network.contains(&line))
            .unwrap();
        assert!(deny < first);
    }
}

#[test]
fn the_home_writes_come_after_every_other_write_allow() {
    for shape in shapes() {
        let text = text(&shape);
        let lines: Vec<&str> = ours(&text).lines().collect();
        let denied = lines
            .iter()
            .position(|line| line.starts_with("(deny file-write* ("))
            .unwrap();
        let last_allow = lines
            .iter()
            .rposition(|line| line.starts_with("(allow ") && line.contains("file-write"))
            .unwrap();
        // The home's own allows sit just above its deny; every other is above them.
        assert!(last_allow < denied);
        let home_allows = write_allows(&home(shape.cli)).len();
        let others = lines[..denied - home_allows]
            .iter()
            .filter(|line| line.contains("file-write"))
            .count();
        assert!(others > 0);
    }
}

#[cfg(unix)]
#[test]
fn the_rules_name_each_path_resolved_and_fit_the_launcher() {
    let folder = Folder::new("whole");
    let made = |name: &str| {
        let path = folder.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let (granted, task, tmp, program, home) = (
        made("granted"),
        made("Tasks/task"),
        made("Tasks/.tmp/0123456789abcdef"),
        made("program"),
        made("home"),
    );
    let bridge = made("ilo.app/Contents/MacOS").join("mcp-bridge");
    std::fs::write(&bridge, "").unwrap();
    let place = Places::make(&folder.0.join("places")).unwrap();
    let config = folder.0.join("Tasks/task.json");
    let paths = Paths {
        folder: Some(&granted),
        attached: None,
        task_dir: &task,
        config: &config,
        tmp: &tmp,
        socket: &folder.0.join("Agents.sock"),
        program: Program::Folder(&program),
        bridge: &bridge,
        cli_home: &home,
        claude_config: None,
    };
    let rules = rules(Cli::Codex, Tier::Work, 49152, &paths, place.mark()).unwrap();
    let keys: Vec<&str> = rules.params.iter().map(|(key, _)| *key).collect();
    for (at, key) in keys.iter().enumerate() {
        assert!(!keys[..at].contains(key), "{key}");
    }
    assert!(keys.len() <= PARAMETERS_MAX);
    let root = std::fs::canonicalize(&folder.0).unwrap();
    let ups: Vec<&PathBuf> = (rules.params.iter())
        .filter(|(key, _)| key.starts_with("UP_"))
        .map(|(_, path)| path)
        .collect();
    // The test's own folder and each above it, and the folders in it with something named.
    assert!(ups.contains(&&root) && ups.contains(&&root.join("Tasks")));
    assert!(ups.contains(&&root.join("Tasks/.tmp")));
    assert!(ups.contains(&&root.join("ilo.app/Contents/MacOS")));
    assert_eq!(ups.len(), root.ancestors().count() - 1 + 2 + 3);
    assert!(
        rules
            .text
            .contains(&format!("(param \"UP_{}\")", ups.len() - 1))
    );
    assert!(
        !rules
            .text
            .contains(&format!("(param \"UP_{}\")", ups.len()))
    );
    // A text with no path in it: nothing of the test's folder.
    assert!(!rules.text.contains(root.to_str().unwrap()));
    // The longest text leaves the launcher's hand-off room for every parameter at the length of
    // an ordinary profile's paths; past it put_up refuses, and nothing starts.
    let longest = shapes()
        .iter()
        .map(|shape| text(shape).len())
        .max()
        .unwrap();
    assert!(longest + PARAMETERS_MAX * (64 + 16) < super::super::launch::HANDOFF_MAX);
    // A folder deeper than the parameters reach isn't walled.
    let deep = (0..PARAMETERS_MAX).fold(granted.clone(), |path, at| path.join(at.to_string()));
    std::fs::create_dir_all(&deep).unwrap();
    let paths = Paths {
        folder: Some(&deep),
        ..paths
    };
    let refused = rules_for(&paths, place.mark()).unwrap_err();
    assert_eq!(refused.to_string(), TOO_DEEP);
}

/// The bridge is read as the one file it is, by its parameter alone: no line reaches into ilo's
/// bundle, and the folders above the bridge are their names alone.
#[cfg(unix)]
#[test]
fn the_bridge_is_its_own_literal_and_nothing_of_the_bundle_is_a_subpath() {
    let bridge_line = format!(
        "(allow file-read-data file-read-metadata file-test-existence \
         (literal (param \"{BRIDGE}\")))"
    );
    for shape in shapes() {
        let text = text(&shape);
        let named: Vec<&str> = (text.lines())
            .filter(|line| line.contains(&format!("\"{BRIDGE}\"")))
            .collect();
        assert_eq!(named, [bridge_line.as_str()]);
        assert!(!text.contains("Contents/MacOS") && !text.contains("mcp-bridge"));
    }
    let folder = Folder::new("bridge");
    let made = |name: &str| {
        let path = folder.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let (task, tmp, program, home) = (
        made("Tasks/task"),
        made("Tasks/.tmp/0123456789abcdef"),
        made("program"),
        made("home"),
    );
    let bundle = made("ilo.app");
    let bridge = made("ilo.app/Contents/MacOS").join("mcp-bridge");
    std::fs::write(&bridge, "").unwrap();
    let place = Places::make(&folder.0.join("places")).unwrap();
    let config = folder.0.join("Tasks/task.json");
    let paths = Paths {
        folder: None,
        attached: None,
        task_dir: &task,
        config: &config,
        tmp: &tmp,
        socket: &folder.0.join("Agents.sock"),
        program: Program::Folder(&program),
        bridge: &bridge,
        cli_home: &home,
        claude_config: None,
    };
    let rules = rules_for(&paths, place.mark()).unwrap();
    let bridge = std::fs::canonicalize(&bridge).unwrap();
    let bundle = std::fs::canonicalize(&bundle).unwrap();
    for (key, path) in &rules.params {
        if *key == BRIDGE {
            assert_eq!(path, &bridge);
        } else if path.starts_with(&bundle) || bundle.starts_with(path) {
            // The bundle's folders, and those above it, only ever by name.
            assert!(key.starts_with("UP_"), "{key}");
            assert!(!rules.text.contains(&format!("(subpath (param \"{key}\"))")));
        }
    }
}

fn rules_for(paths: &Paths, mark: &Mark) -> io::Result<Rules> {
    rules(Cli::Codex, Tier::Work, 49152, paths, mark)
}

/// The first run of the whole text: the system takes it, and a shell started inside it at Work
/// makes a file in its folder, can't delete it, and reads nothing beside the folder.
#[cfg(target_os = "macos")]
#[test]
fn a_shell_starts_inside_the_whole_text_and_has_its_folder_at_its_level() {
    use std::process::Stdio;
    let folder = Folder::new("shell");
    let made = |name: &str| {
        let path = folder.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let (granted, task, tmp, home) = (
        made("granted"),
        made("Tasks/task"),
        made("Tasks/.tmp/0123456789abcdef"),
        made("home"),
    );
    let outside = folder.0.join("outside");
    std::fs::write(&outside, "kept out").unwrap();
    let place = Places::make(&folder.0.join("places")).unwrap();
    let config = folder.0.join("Tasks/task.json");
    let paths = Paths {
        folder: Some(&granted),
        attached: None,
        task_dir: &task,
        config: &config,
        tmp: &tmp,
        socket: &folder.0.join("Agents.sock"),
        program: Program::Folder(Path::new("/bin")),
        bridge: Path::new("/bin/sh"),
        cli_home: &home,
        claude_config: None,
    };
    let rules = rules_for(&paths, place.mark()).unwrap();
    let wall = super::super::Wall::inside(None, &rules, &[]).unwrap();
    let mut command = wall.command(Path::new("/bin/sh"));
    let script = "echo made > \"$0/new\" && /bin/cat \"$0/new\"; \
                  /bin/cat \"$1\" 2>/dev/null || echo refused; \
                  /bin/rm \"$0/new\" 2>/dev/null || echo kept";
    command
        .args(["-c", script])
        .args([&granted, &outside])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, rising) = wall.start(&mut command).unwrap();
    drop(child.stdin.take());
    let risen = rising.expect("a walled start").risen();
    let output = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(risen, Ok(()), "{err}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "made\nrefused\nkept\n",
        "{err}"
    );
}

/// The whole text with the program named alone, as a program kept loose in a folder of the
/// user's is: the system takes it, and the program starts inside it.
#[cfg(target_os = "macos")]
#[test]
fn a_program_named_alone_starts_inside_the_whole_text() {
    use std::process::Stdio;
    let folder = Folder::new("alone");
    let made = |name: &str| {
        let path = folder.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let (task, tmp, home) = (
        made("Tasks/task"),
        made("Tasks/.tmp/0123456789abcdef"),
        made("home"),
    );
    let place = Places::make(&folder.0.join("places")).unwrap();
    let config = folder.0.join("Tasks/task.json");
    let paths = Paths {
        folder: None,
        attached: None,
        task_dir: &task,
        config: &config,
        tmp: &tmp,
        socket: &folder.0.join("Agents.sock"),
        program: Program::File(Path::new("/bin/sh")),
        bridge: Path::new("/bin/sh"),
        cli_home: &home,
        claude_config: None,
    };
    let rules = rules_for(&paths, place.mark()).unwrap();
    assert!(!rules.text.contains(PROGRAM_DIR));
    let wall = super::super::Wall::inside(None, &rules, &[]).unwrap();
    let mut command = wall.command(Path::new("/bin/sh"));
    command
        .args(["-c", "echo started"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (mut child, rising) = wall.start(&mut command).unwrap();
    drop(child.stdin.take());
    let risen = rising.expect("a walled start").risen();
    let output = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(risen, Ok(()), "{err}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "started\n",
        "{err}"
    );
}
