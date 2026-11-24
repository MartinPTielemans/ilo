//! The files wall's launcher, started as the runner starts it, around stand-ins: the system's
//! shell and tools in folders the tests make, under rules the tests write. No assistant, no folder
//! of the user's, no window and nothing the system asks the user.

use super::*;

/// Rules that allow everything, which each test's own lines then take from.
const OPEN: &str = "(version 1)(allow default)";

fn rules(text: &str) -> Rules {
    Rules {
        text: text.into(),
        params: Vec::new(),
    }
}

/// Rules whose hand-off is `total` bytes, padded with a comment.
fn rules_of(total: usize) -> Rules {
    let empty = handoff(&rules(&format!("{OPEN}\n;")), &[]).unwrap().len();
    rules(&format!("{OPEN}\n;{}", "x".repeat(total - empty)))
}

#[test]
fn the_writer_and_the_launcher_hold_the_same_numbers() {
    let source = include_str!("../../../../platform/mac/wall_launcher.c");
    let defined = |name: &str| -> usize {
        let start = format!("#define {name} ");
        let value = source.lines().find_map(|line| line.strip_prefix(&start));
        value
            .unwrap_or_else(|| panic!("no {name}"))
            .parse()
            .unwrap()
    };
    assert_eq!(defined("WALL_HANDOFF_VERSION"), VERSION as usize);
    assert_eq!(defined("WALL_HANDOFF_MAX"), HANDOFF_MAX);
    assert_eq!(defined("WALL_PARAMETERS_MAX"), PARAMETERS_MAX);
    assert_eq!(defined("WALL_KEPT_MAX"), KEPT_MAX);
    #[cfg(target_os = "macos")]
    {
        let codes: Vec<i32> = (source.lines())
            .filter_map(|line| line.trim().strip_prefix("NO_")?.split_once(" = "))
            .filter_map(|(_, code)| code.split(',').next()?.parse().ok())
            .collect();
        assert_eq!(codes, CODES.map(|(code, _)| code));
    }
}

#[test]
fn a_hand_off_fits_up_to_the_cap_and_not_a_byte_over() {
    assert_eq!(
        handoff(&rules_of(HANDOFF_MAX), &[]).map(|whole| whole.len()),
        Some(HANDOFF_MAX)
    );
    assert_eq!(handoff(&rules_of(HANDOFF_MAX + 1), &[]), None);
    // Refused where the wall is made, so nothing is ever started for it.
    assert_eq!(
        Wall::inside(None, &rules_of(HANDOFF_MAX + 1), &[]).unwrap_err(),
        WallRefused::Failed(TOO_LONG.into())
    );
}

#[test]
fn a_hand_off_the_launcher_would_refuse_is_never_written() {
    let open = rules(OPEN);
    assert!(handoff(&open, &["PATH", "HOME"]).is_some());
    for kept in [&["PATH", "PATH"][..], &[""], &["A=B"], &["A\0"]] {
        assert_eq!(handoff(&open, kept), None, "{kept:?}");
    }
    assert_eq!(handoff(&rules("(version 1)\0"), &[]), None);
    let mut many = rules(OPEN);
    many.params = vec![("KEY", PathBuf::from("/nonexistent")); PARAMETERS_MAX];
    assert!(handoff(&many, &[]).is_some());
    many.params.push(("KEY", PathBuf::from("/nonexistent")));
    assert_eq!(handoff(&many, &[]), None);
    let names: Vec<String> = (0..=KEPT_MAX).map(|at| format!("N{at}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    assert!(handoff(&open, &names[..KEPT_MAX]).is_some());
    assert_eq!(handoff(&open, &names), None);
}

#[cfg(target_os = "macos")]
mod walled {
    use std::io::{BufRead, BufReader, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;
    use std::sync::Arc;

    use super::*;
    use crate::agents::runner::{Cli, Launch, Out, Process, Server};
    use crate::agents::wall::leftovers;

    /// A folder of the test's own, by its real path, as rules name paths. Gone with the test.
    struct Folder(PathBuf);

    impl Folder {
        fn new(name: &str) -> Folder {
            let folder =
                std::env::temp_dir().join(format!("ilo-wall-{}-{name}", std::process::id()));
            _ = std::fs::remove_dir_all(&folder);
            std::fs::create_dir_all(&folder).unwrap();
            Folder(folder.canonicalize().unwrap())
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }

        fn text(&self) -> String {
            self.0.display().to_string()
        }

        /// A script of the test's, to run as a program.
        fn script(&self, name: &str, text: &str) -> PathBuf {
            let script = self.join(name);
            std::fs::write(&script, text).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            script
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn wall(text: &str, params: &[(&'static str, &Path)], kept: &[&str]) -> Wall {
        let rules = Rules {
            text: text.into(),
            params: (params.iter())
                .map(|(key, path)| (*key, path.to_path_buf()))
                .collect(),
        };
        Wall::inside(None, &rules, kept).unwrap()
    }

    /// A wall whose launcher is `program`, handed `handoff` as it is.
    fn wall_with(program: PathBuf, handoff: Vec<u8>) -> Wall {
        Wall {
            folder: None,
            started: SystemTime::now(),
            launcher: Some(Launcher { program, handoff }),
            tmp: None,
            place: None,
            listener: None,
        }
    }

    /// A hand-off the writer wouldn't write, for the launcher to refuse.
    fn raw(version: u32, body: &[u8]) -> Wall {
        let mut whole = b"ILOW".to_vec();
        whole.extend(version.to_le_bytes());
        whole.extend((body.len() as u32).to_le_bytes());
        whole.extend(body);
        wall_with(launcher().unwrap(), whole)
    }

    fn text(text: &[u8]) -> Vec<u8> {
        let mut written = (text.len() as u32).to_le_bytes().to_vec();
        written.extend(text);
        written.push(0);
        written
    }

    /// A body with these rules, no parameter and these names, each written as it comes.
    fn body(rules: &[u8], kept: &[&[u8]]) -> Vec<u8> {
        let mut body = text(rules);
        body.extend(0u32.to_le_bytes());
        body.extend((kept.len() as u32).to_le_bytes());
        for name in kept {
            body.extend(text(name));
        }
        body
    }

    struct Ran {
        risen: Result<(), WallRefused>,
        out: String,
        err: String,
    }

    /// Starts `program` inside `wall` as the runner does, its input closed behind the hand-off,
    /// and answers how the wall went up and all the program wrote.
    fn run_in(wall: &Wall, program: &str, args: &[&str], environment: &[(&str, &str)]) -> Ran {
        let mut command = wall.command(Path::new(program));
        command
            .args(args)
            .envs(environment.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (mut child, rising) = wall.start(&mut command).unwrap();
        drop(child.stdin.take());
        let risen = rising.expect("a walled start").risen();
        let output = child.wait_with_output().unwrap();
        Ran {
            risen,
            out: String::from_utf8_lossy(&output.stdout).into_owned(),
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    fn run(wall: &Wall, program: &str, args: &[&str]) -> Ran {
        run_in(wall, program, args, &[])
    }

    fn refused(sentence: &str) -> Result<(), WallRefused> {
        Err(WallRefused::Failed(sentence.into()))
    }

    fn sentence(code: i32) -> &'static str {
        CODES.iter().find(|(each, _)| *each == code).unwrap().1
    }

    #[test]
    fn a_program_reads_and_writes_inside_its_folder_and_not_outside() {
        let (inside, outside) = (Folder::new("files-in"), Folder::new("files-out"));
        std::fs::write(inside.join("seen"), "inside\n").unwrap();
        std::fs::write(outside.join("seen"), "outside\n").unwrap();
        let rules = format!(
            "{OPEN}(deny file-read* (subpath (param \"OUT\")))\
             (deny file-write*)(allow file-write* (subpath (param \"IN\")))"
        );
        let wall = wall(&rules, &[("IN", &inside.0), ("OUT", &outside.0)], &[]);
        let script = r#"cat "$1/seen"; echo made > "$1/made" && echo wrote
            cat "$2/seen" || echo unread; echo made > "$2/made" || echo unwritten"#;
        let args = ["-c", script, "sh", &inside.text(), &outside.text()];
        let ran = run(&wall, "/bin/sh", &args);
        assert_eq!(ran.risen, Ok(()));
        assert_eq!(ran.out, "inside\nwrote\nunread\nunwritten\n");
        assert!(inside.join("made").exists());
        assert!(!outside.join("made").exists());
    }

    const MADE: &str = r#"echo made > "$1/made" && echo wrote || echo unwritten"#;

    #[test]
    fn a_parameter_with_a_space_a_plus_and_a_dot_reaches_the_system_whole() {
        // The twin's name is one the first's would match if any of the three were read as a
        // pattern's, or the name cut at its space.
        let (named, twin) = (Folder::new("a b+c.d"), Folder::new("a bbcxd"));
        let rules = format!(
            "{OPEN}(deny file-write*)(allow file-write* \
             (regex (string-append \"^\" (regex-quote (param \"IN\")) \"/[^/]+$\")))"
        );
        let wall = wall(&rules, &[("IN", &named.0)], &[]);
        for (folder, said) in [(&named, "wrote\n"), (&twin, "unwritten\n")] {
            let ran = run(&wall, "/bin/sh", &["-c", MADE, "sh", &folder.text()]);
            assert_eq!((ran.risen, ran.out.as_str()), (Ok(()), said));
        }
        assert!(named.join("made").exists() && !twin.join("made").exists());
    }

    #[test]
    fn a_parameter_by_a_links_name_matches_nothing() {
        let folder = Folder::new("link");
        let (real, link) = (folder.join("real"), folder.join("link"));
        std::fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let rules = format!("{OPEN}(deny file-write*)(allow file-write* (subpath (param \"IN\")))");
        // The system asks its rules by a path's real name, and the launcher hands a parameter on
        // as it came: who names the folder gives its real name, or the rule allows nothing.
        for (given, said) in [(&link, "unwritten\n"), (&real, "wrote\n")] {
            let wall = wall(&rules, &[("IN", given)], &[]);
            for by in [&real, &link] {
                let by = by.display().to_string();
                let ran = run(&wall, "/bin/sh", &["-c", MADE, "sh", &by]);
                assert_eq!((ran.risen, ran.out.as_str()), (Ok(()), said));
            }
        }
    }

    #[test]
    fn a_program_reaches_the_one_port_and_the_one_socket_and_no_other() {
        let listen = || std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let (port, other_port) = (listen(), listen());
        let ports = [&port, &other_port].map(|each| each.local_addr().unwrap().port().to_string());
        let folder = Folder::new("net");
        let (socket_path, other_path) = (folder.join("one"), folder.join("other"));
        // Each socket answers a caller by hanging up, which is all the stand-in waits for.
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let answering = [&socket_path, &other_path].map(|path| {
            let listener = std::os::unix::net::UnixListener::bind(path).unwrap();
            let done = done.clone();
            std::thread::spawn(move || {
                for _caller in listener.incoming() {
                    if done.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }
                }
            })
        });
        let script = r#"/usr/bin/nc -z 127.0.0.1 "$1" && echo port
            /usr/bin/nc -z 127.0.0.1 "$2" && echo other port
            /usr/bin/nc -U "$3" </dev/null && echo socket
            /usr/bin/nc -U "$4" </dev/null && echo other socket
            true"#;
        let (socket, other_socket) = (
            socket_path.display().to_string(),
            other_path.display().to_string(),
        );
        let args = [
            "-c",
            script,
            "sh",
            &ports[0],
            &ports[1],
            &socket,
            &other_socket,
        ];
        // Outside a wall all four answer, so a refusal inside one is the wall's.
        let outside = crate::child::command("/bin/sh")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&outside.stdout),
            "port\nother port\nsocket\nother socket\n"
        );
        let rules = format!(
            "{OPEN}(deny network*)(allow network-outbound (remote ip \"localhost:{}\"))\
             (allow network-outbound (remote unix-socket (path-literal (param \"SOCKET\"))))",
            ports[0]
        );
        let wall = wall(&rules, &[("SOCKET", &socket_path)], &[]);
        let ran = run(&wall, "/bin/sh", &args);
        assert_eq!(ran.risen, Ok(()));
        assert_eq!(ran.out, "port\nsocket\n");
        done.store(true, std::sync::atomic::Ordering::Release);
        for (path, answering) in [&socket_path, &other_path].into_iter().zip(answering) {
            std::os::unix::net::UnixStream::connect(path).unwrap();
            answering.join().unwrap();
        }
    }

    #[test]
    fn a_program_gets_the_environment_named_and_no_more() {
        let wall = wall(OPEN, &[], &["WALL_KEPT", "WALL_NOT_SET"]);
        let environment = [("WALL_KEPT", "as given"), ("WALL_DROPPED", "1")];
        let ran = run_in(&wall, "/usr/bin/env", &[], &environment);
        assert_eq!(ran.risen, Ok(()));
        // Not PATH or HOME either, which the tests' own environment has.
        assert_eq!(ran.out, "WALL_KEPT=as given\n");
    }

    #[test]
    fn a_program_inherits_no_handle_but_the_standard_three() {
        // A handle the app has open when a program starts, not marked to close as it does.
        let null = std::fs::File::open("/dev/null").unwrap();
        // SAFETY: a copy of a handle held open, closed below.
        let stray = unsafe { libc::fcntl(null.as_raw_fd(), libc::F_DUPFD, 200) };
        assert!(stray >= 200);
        let stray_text = stray.to_string();
        let script = r#"for n in 3 4 5 6 7 8 9 "$1"; do if { : <&"$n"; } 2>/dev/null; then echo "$n"; fi; done"#;
        let args = ["-c", script, "bash", &stray_text];
        let outside = crate::child::command("/bin/bash")
            .args(args)
            .output()
            .unwrap();
        let outside = String::from_utf8_lossy(&outside.stdout).into_owned();
        assert!(outside.lines().any(|line| line == stray_text), "{outside}");
        let ran = run(&wall(OPEN, &[], &[]), "/bin/bash", &args);
        // SAFETY: the copy made above.
        unsafe { libc::close(stray) };
        assert_eq!(ran.risen, Ok(()));
        // Nor the two the launcher opened for its own call, which would be 3 and 4.
        assert_eq!(ran.out, "");
    }

    #[test]
    fn a_program_has_the_launchers_process_and_all_of_its_input() {
        let wall = wall(OPEN, &[], &[]);
        let mut command = wall.command(Path::new("/bin/sh"));
        command
            .args(["-c", "echo $$; exec /bin/cat"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let (mut child, rising) = wall.start(&mut command).unwrap();
        // Right behind the hand-off, before the launcher has read either.
        let mut input = child.stdin.take().unwrap();
        input.write_all(b"behind the hand-off\n").unwrap();
        assert_eq!(rising.unwrap().risen(), Ok(()));
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        assert_eq!(lines.next().unwrap().unwrap(), child.id().to_string());
        assert_eq!(lines.next().unwrap().unwrap(), "behind the hand-off");
        input.write_all(b"and after\n").unwrap();
        assert_eq!(lines.next().unwrap().unwrap(), "and after");
        child.kill().unwrap();
        assert_eq!(child.wait().unwrap().signal(), Some(libc::SIGKILL));
        assert!(lines.next().is_none());
    }

    #[test]
    fn looser_rules_put_on_inside_loosen_nothing() {
        let outside = Folder::new("looser-out");
        let rules = format!("{OPEN}(deny file-write* (subpath (param \"OUT\")))");
        let wall = wall(&rules, &[("OUT", &outside.0)], &[]);
        let write = r#"echo ran; echo made > "$1/made""#;
        let args = ["-p", OPEN, "/bin/sh", "-c", write, "sh", &outside.text()];
        let ran = run(&wall, "/usr/bin/sandbox-exec", &args);
        assert_eq!(ran.risen, Ok(()));
        // The system puts no second set of rules on a process inside one, so nothing ran at all.
        assert_eq!(ran.out, "");
        assert!(!outside.join("made").exists());
    }

    #[test]
    fn rules_the_system_refuses_start_nothing_and_say_nothing() {
        let folder = Folder::new("refused");
        let marker = folder.join("ran");
        let touch = [
            "-c",
            r#"echo ran > "$1""#,
            "sh",
            &marker.display().to_string(),
        ];
        // A fault in the text, and a parameter the rules use that nothing gave.
        let fault = wall(
            &format!("{OPEN}(deny file-write* (subpath"),
            &[("OUT", &folder.0)],
            &[],
        );
        let missing = wall(
            &format!("{OPEN}(deny file-write* (subpath (param \"OUT\")))"),
            &[],
            &[],
        );
        for wall in [fault, missing] {
            let ran = run(&wall, "/bin/sh", &touch);
            assert_eq!(ran.risen, refused(sentence(114)));
            // The system's library says what it found, with the rules' text: not on the handle
            // the assistant's last words are kept from.
            assert_eq!((ran.out.as_str(), ran.err.as_str()), ("", ""));
            assert!(!marker.exists());
            let said = format!("{:?}", ran.risen);
            assert!(!said.contains(&folder.text()) && !said.contains("OUT"));
        }
    }

    #[test]
    fn a_program_that_cannot_start_is_the_launchers_no() {
        let wall = wall(OPEN, &[], &["PATH"]);
        // Not there, and one by a name alone, which a PATH would have to find.
        for program in ["/nonexistent/stand-in", "sh", "bin/sh"] {
            let ran = run_in(&wall, program, &["-c", "echo ran"], &[("PATH", "/bin")]);
            assert_eq!(ran.risen, refused(sentence(115)), "{program}");
            assert_eq!((ran.out.as_str(), ran.err.as_str()), ("", ""));
        }
        // And none at all.
        let Some(launcher) = &wall.launcher else {
            panic!("a walled start")
        };
        let mut command = crate::child::command(&launcher.program);
        command.stdin(Stdio::piped());
        let (mut child, rising) = wall.start(&mut command).unwrap();
        assert_eq!(rising.unwrap().risen(), refused(sentence(115)));
        assert_eq!(child.wait().unwrap().code(), Some(115));
    }

    #[test]
    fn a_hand_off_that_is_not_one_starts_nothing() {
        let open = OPEN.as_bytes();
        let good = body(open, &[b"PATH"]);
        let mut long = good.clone();
        long.push(0);
        let mut counted = text(open);
        counted.extend((PARAMETERS_MAX as u32 + 1).to_le_bytes());
        let mut bad = vec![
            ("another version", raw(VERSION + 1, &good)),
            ("more than its length says", raw(VERSION, &long)),
            ("too many parameters", raw(VERSION, &counted)),
            (
                "a zero byte in the rules",
                raw(VERSION, &body(b"(version 1)\0(allow default)", &[])),
            ),
            (
                "a name twice",
                raw(VERSION, &body(open, &[b"PATH", b"PATH"])),
            ),
            (
                "a name with a value",
                raw(VERSION, &body(open, &[b"PATH=/bin"])),
            ),
            ("an empty name", raw(VERSION, &body(open, &[b""]))),
        ];
        // Not its mark at the start, less than its length says, and a length over the cap.
        let Some(Launcher { handoff, .. }) = raw(VERSION, &good).launcher else {
            unreachable!()
        };
        let mut marked = handoff.clone();
        marked[0] = b'X';
        bad.push(("another mark", wall_with(launcher().unwrap(), marked)));
        let short = handoff[..handoff.len() - 3].to_vec();
        bad.push(("cut short", wall_with(launcher().unwrap(), short)));
        let mut over = handoff[..12].to_vec();
        over[8..12].copy_from_slice(&(HANDOFF_MAX as u32).to_le_bytes());
        bad.push(("over the cap", wall_with(launcher().unwrap(), over)));
        bad.push(("nothing", wall_with(launcher().unwrap(), Vec::new())));
        for (what, wall) in bad {
            let ran = run_in(&wall, "/bin/sh", &["-c", "echo ran"], &[("PATH", "/bin")]);
            assert_eq!(ran.risen, refused(sentence(112)), "{what}");
            assert_eq!((ran.out.as_str(), ran.err.as_str()), ("", ""), "{what}");
        }
        // The same body as the writer makes it goes up.
        let ran = run(&raw(VERSION, &good), "/bin/sh", &["-c", "echo ran"]);
        assert_eq!((ran.risen, ran.out.as_str()), (Ok(()), "ran\n"));
    }

    #[test]
    fn a_hand_off_at_the_cap_is_given_whole_without_a_wait() {
        let wall = Wall::inside(None, &rules_of(HANDOFF_MAX), &[]).unwrap();
        let ran = run(&wall, "/bin/sh", &["-c", "echo ran"]);
        assert_eq!((ran.risen, ran.out.as_str()), (Ok(()), "ran\n"));
        // More than any pipe takes at once isn't waited for: the launcher is ended, unstarted.
        let Some(Launcher { program, handoff }) = wall.launcher else {
            unreachable!()
        };
        let over = [handoff, vec![b' '; 1 << 20]].concat();
        let ran = run(&wall_with(program, over), "/bin/sh", &["-c", "echo ran"]);
        assert_eq!(ran.risen, refused(NOT_GIVEN));
        assert_eq!(ran.out, "");
    }

    #[test]
    fn a_launcher_of_that_name_on_the_path_is_not_the_one_started() {
        let folder = Folder::new("path");
        let marker = folder.join("ran");
        let decoy = format!("#!/bin/sh\necho decoy > '{}'\n", marker.display());
        folder.script("wall-launcher", &decoy);
        let path = format!("{}:/usr/bin:/bin", folder.text());
        let wall = wall(OPEN, &[], &["PATH"]);
        let ran = run_in(&wall, "/bin/sh", &["-c", "echo ran"], &[("PATH", &path)]);
        assert_eq!((ran.risen, ran.out.as_str()), (Ok(()), "ran\n"));
        assert!(!marker.exists());
    }

    #[test]
    fn the_launcher_links_the_systems_library_and_calls_only_these() {
        const CALLS: [&str; 13] = [
            "___error",
            "___stack_chk_fail",
            "___stack_chk_guard",
            "__exit",
            "_close",
            "_dup2",
            "_execve",
            "_fcntl",
            "_getpid",
            "_open",
            "_proc_pidinfo",
            "_read",
            "_sandbox_init_with_parameters",
        ];
        let tool = |tool: &str, flag: &str| {
            let said = crate::child::command(tool)
                .arg(flag)
                .arg(launcher().unwrap())
                .output()
                .unwrap();
            assert!(said.status.success(), "{tool}");
            String::from_utf8_lossy(&said.stdout).into_owned()
        };
        let calls = tool("/usr/bin/nm", "-u");
        let calls: Vec<&str> = calls.lines().map(str::trim).collect();
        assert!(!calls.is_empty());
        for call in &calls {
            assert!(CALLS.contains(call), "{call} isn't one of the launcher's");
        }
        assert!(calls.contains(&"_sandbox_init_with_parameters") && calls.contains(&"_execve"));
        let libraries = tool("/usr/bin/otool", "-L");
        let libraries: Vec<&str> = libraries.lines().skip(1).map(str::trim).collect();
        assert_eq!(libraries.len(), 1, "{libraries:?}");
        assert!(libraries[0].starts_with("/usr/lib/libSystem.B.dylib "));
    }

    /// A mark on a watch's queue, or a look for it: an event of the test's own that never
    /// fires. Tests run side by side and a closed queue's number is the next one's, so the number
    /// alone says nothing, and a count of the process's descriptors is other tests' too.
    fn mark(queue: i32, mark: usize, flags: u16) -> bool {
        let change = libc::kevent {
            ident: mark,
            filter: libc::EVFILT_USER,
            flags,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        let none = std::ptr::null_mut();
        // SAFETY: one change read from `change`, and no event asked for.
        unsafe { libc::kevent(queue, &change, 1, none, 0, std::ptr::null()) == 0 }
    }

    /// Marks the queue `rising` waits on, and answers a look that says whether it's still open.
    fn open_queue(rising: &Rising) -> impl Fn() -> bool + use<> {
        static MARKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
        let State::Watched(watch) = &rising.0 else {
            panic!("not watched")
        };
        let queue = watch.descriptor();
        let made = MARKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert!(mark(queue, made, libc::EV_ADD));
        // Enabling an event finds it only where it was added.
        move || mark(queue, made, libc::EV_ENABLE)
    }

    #[test]
    fn the_watch_is_on_before_the_hand_off_and_closed_after_its_one_answer() {
        // A start that execs and one that fails, each by its own queue.
        for (program, risen) in [
            ("/bin/sh", Ok(())),
            ("/nonexistent", refused(sentence(115))),
        ] {
            let wall = wall(OPEN, &[], &[]);
            let mut command = wall.command(Path::new(program));
            command.args(["-c", "true"]).stdin(Stdio::piped());
            let mut child = command.spawn().unwrap();
            let watch = watch::watch(child.id())
                .unwrap()
                .expect("a launcher that waits");
            // It waits for its hand-off: nothing it did is there to read, least of all an exec.
            assert!(!watch.pending());
            let Some(Launcher { handoff, .. }) = &wall.launcher else {
                unreachable!()
            };
            assert!(hand(child.stdin.as_ref().unwrap(), handoff));
            let rising = Rising(State::Watched(watch));
            let open = open_queue(&rising);
            assert!(open());
            assert_eq!(rising.risen(), risen);
            assert!(!open());
            child.wait().unwrap();
        }
    }

    #[test]
    fn a_launcher_gone_before_the_watch_is_its_end() {
        let mut child = crate::child::command("/bin/sh")
            .args(["-c", "exit 114"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        // Ended, and not yet waited for, as a launcher that fails at once is.
        // SAFETY: waitid writes the one record, and WNOWAIT leaves the child for `give`.
        unsafe {
            let mut ended: libc::siginfo_t = std::mem::zeroed();
            let how = libc::WEXITED | libc::WNOWAIT;
            assert_eq!(libc::waitid(libc::P_PID, child.id(), &mut ended, how), 0);
        }
        assert!(watch::watch(child.id()).unwrap().is_none());
        let rising = give(&mut child, b"never read");
        assert!(matches!(rising.0, State::Known(_)));
        assert_eq!(rising.risen(), refused(sentence(114)));
    }

    /// A launcher that reads what it's given and never becomes anything.
    fn hung(folder: &Folder) -> Wall {
        let script = folder.script("hung", "#!/bin/sh\nwhile read -r line; do :; done\n");
        wall_with(script, handoff(&rules(OPEN), &[]).unwrap())
    }

    #[test]
    fn ending_a_launcher_ends_the_wait_for_it() {
        let folder = Folder::new("hung");
        let wall = hung(&folder);
        let mut command = wall.command(Path::new("/bin/sh"));
        command.stdin(Stdio::piped());
        let (mut child, rising) = wall.start(&mut command).unwrap();
        let rising = rising.unwrap();
        let open = open_queue(&rising);
        let waiting = std::thread::spawn(move || rising.risen());
        child.kill().unwrap();
        assert_eq!(waiting.join().unwrap(), refused(ENDED));
        assert!(!open());
        child.wait().unwrap();
    }

    fn launch(folder: &Folder, wall: Wall) -> Launch {
        // What the runner sends first is a line of JSON that says it's the user's.
        let stand_in = folder.script(
            "stand-in",
            "#!/bin/sh\nread -r line\ncase \"$line\" in *'\"type\":\"user\"'*) echo first >&2 ;; esac\n",
        );
        Launch {
            cli: Cli::Claude,
            program: stand_in,
            path: Some("/usr/bin:/bin".into()),
            dir: folder.join("task"),
            config: folder.join("task.json"),
            server: Server {
                command: "/nonexistent/bridge".into(),
                args: Vec::new(),
                token: "token".into(),
            },
            model: None,
            instructions: String::new(),
            tools: Vec::new(),
            helpers: false,
            wall: Some(Arc::new(wall)),
        }
    }

    /// All a run says, to its end.
    fn said(
        mut events: futures::channel::mpsc::UnboundedReceiver<crate::agents::runner::Event>,
    ) -> Vec<Out> {
        let mut outs = Vec::new();
        while let Some(event) = futures::executor::block_on(futures::StreamExt::next(&mut events)) {
            let gone = matches!(event.out, Out::Gone(_));
            outs.push(event.out);
            if gone {
                break;
            }
        }
        outs
    }

    /// A wall whose rules allow its place's mark and no other of ilo's, as a task's do.
    fn placed(folder: &Folder, text: &str) -> Wall {
        let place = Arc::new(leftovers::Place::make(&folder.join("places")).unwrap());
        let mark = Path::new(place.mark().as_cstr().to_str().unwrap()).to_path_buf();
        wall(text, &[("MARK", mark.as_path())], &[]).in_place(place)
    }

    const MARKED: &str = "(version 1)(allow default)\
        (deny mach-lookup (global-name-prefix \"ilo.mark.\"))\
        (allow mach-lookup (global-name (param \"MARK\")))";

    #[test]
    fn the_runner_says_the_wall_is_up_before_anything_of_the_run() {
        let folder = Folder::new("runner-up");
        let launch = launch(&folder, placed(&folder, MARKED));
        let (events, heard) = futures::channel::mpsc::unbounded();
        let _process = Process::start(&launch, "task-1", "Hello", events).unwrap();
        // The first message went in behind the hand-off and reached the stand-in whole.
        assert_eq!(
            said(heard),
            [Out::Walled(Ok(())), Out::Gone("first\n".into())]
        );
    }

    /// The system's word is the control: rules that allow every name, the control too, aren't a
    /// wall it places the program inside, and neither is a wall with no place, whose rules allow
    /// a mark that isn't its own. Both are rules the system takes, so it's the control that
    /// refuses each.
    #[test]
    fn the_runner_starts_nothing_the_system_doesnt_place_inside_its_wall() {
        let folder = Folder::new("runner-outside");
        let other = leftovers::Place::make(&folder.join("others")).unwrap();
        let stray = Path::new(other.mark().as_cstr().to_str().unwrap()).to_path_buf();
        let placeless = wall(MARKED, &[("MARK", stray.as_path())], &[]);
        for wall in [placed(&folder, OPEN), placeless] {
            let launch = launch(&folder, wall);
            let (events, heard) = futures::channel::mpsc::unbounded();
            let _process = Process::start(&launch, "task-1", "Hello", events).unwrap();
            let outs = said(heard);
            assert_eq!(outs.len(), 2, "{outs:?}");
            assert_eq!(outs[0], Out::Walled(refused(leftovers::OUTSIDE)));
            assert!(matches!(outs[1], Out::Gone(_)));
        }
    }

    #[test]
    fn the_runner_says_why_no_wall_went_up_and_keeps_nothing_the_system_said() {
        let folder = Folder::new("runner-refused");
        let launch = launch(&folder, wall(&format!("{OPEN}(deny"), &[], &[]));
        let (events, heard) = futures::channel::mpsc::unbounded();
        // Its first message has no reader, which is no second failure of the start.
        let _process = Process::start(&launch, "task-1", "Hello", events).unwrap();
        assert_eq!(
            said(heard),
            [
                Out::Walled(refused(sentence(114))),
                Out::Gone(String::new())
            ]
        );
    }

    #[test]
    fn stop_and_a_drop_end_a_launcher_that_hangs() {
        let folder = Folder::new("runner-hung");
        for stop in [true, false] {
            let launch = launch(&folder, hung(&folder));
            let (events, heard) = futures::channel::mpsc::unbounded();
            let mut process = Process::start(&launch, "task-1", "Hello", events).unwrap();
            match stop {
                true => process.interrupt(),
                false => drop(process),
            }
            assert_eq!(
                said(heard),
                [Out::Walled(refused(ENDED)), Out::Gone(String::new())]
            );
        }
    }
}
