//! A thread's own files in its tools (#4367, #4368): read_file reads a text file, a picture and a
//! PDF's pages, upload puts the thread's copies of its files in a page's field, and each is judged
//! as a read at the thread's level as the call comes. An outside app's agent and a Codex thread
//! get no file tool. The thread's folder is a real one of the test's own; the engine is the
//! stand-in, which records the PDF reads and the files put.

use std::path::PathBuf;

use super::*;
use crate::agents::files::read::{PDF_LOCKED, PDF_READ, Pdf};
use crate::agents::ready::{Found, Ready};
use crate::agents::task::State;
use crate::agents::wall::Granted;

/// A thread at `tier`, with the folder home/Taxes, which holds a text file
/// and a PDF, in a tree of the test's own, its profile's folder in it too.
struct Thread {
    agents: Entity<Agents>,
    browser: Rc<Made>,
    agent: String,
    base: PathBuf,
}

impl Thread {
    fn new(name: &str, cli: Cli, tier: Tier, cx: &mut TestAppContext) -> Thread {
        // On the Mac the temporary folder is under /private, which the never list holds.
        let root = match cfg!(target_os = "macos") {
            true => PathBuf::from(std::env::var_os("HOME").unwrap()).join("Library/Caches"),
            false => std::env::temp_dir(),
        };
        let base = root.join(format!("ilo-file-tools-{name}-{}", std::process::id()));
        _ = std::fs::remove_dir_all(&base);
        for (path, bytes) in [
            ("home/Taxes/notes.txt", &b"first\nsecond\n"[..]),
            ("home/Taxes/statement.pdf", b"%PDF-1.7 the statement"),
            ("home/Elsewhere/receipt.txt", b"a receipt"),
        ] {
            let path = base.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        let base = base.canonicalize().unwrap();
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let agents = agents(&browser, &configs, None, cx);
        agents.update(cx, |agents, cx| {
            agents.set_threads(true, cx);
            for cli in Cli::ALL {
                let found = Found {
                    ready: Ready::SignedIn,
                    ..Default::default()
                };
                agents.tasks.found.insert(cli, found);
            }
            agents.tasks.dir = Some(base.join("profile"));
        });
        let agent = agents
            .update(cx, |agents, cx| {
                agents.start_task(cli, "Send the statement", None, 1, tier, cx)
            })
            .expect("started");
        let folder = Granted::at_for_test(base.join("home/Taxes"));
        agents.update(cx, |agents, _| {
            let task = agents.task_mut(&agent).unwrap();
            task.state = State::Working;
            task.files.folder = Some(folder);
        });
        Thread {
            agents,
            browser,
            agent,
            base,
        }
    }

    fn at(&self, path: &str) -> String {
        self.base.join(path).to_string_lossy().into_owned()
    }

    /// Calls `tool` as the thread, as a call of its own whose step its trace has: its id, and its
    /// answer's receiver once the UI and the background are idle.
    fn call(
        &self,
        tool: &str,
        arguments: Value,
        cx: &mut TestAppContext,
    ) -> (u64, mpsc::Receiver<ToolResult>) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        let call = Call {
            id,
            agent: self.agent.clone(),
            connection: 1,
            tool: tool.into(),
            arguments,
            reply,
            at: std::time::Instant::now(),
        };
        (self.agents).update(cx, |agents, cx| agents.handle(Event::Call(call), cx));
        cx.run_until_parked();
        (id, answer)
    }

    /// The answer to call `id`, which ends its step as the server's answer does.
    fn answer(
        &self,
        id: u64,
        answer: mpsc::Receiver<ToolResult>,
        cx: &mut TestAppContext,
    ) -> ToolResult {
        let result = answer.try_recv().expect("answered");
        let answered = Event::Answered {
            agent: self.agent.clone(),
            id,
            at: std::time::Instant::now(),
            error: result.error.then(|| result.text.clone()),
            kind: result.kind,
            image: result.image.clone(),
            notes: None,
        };
        (self.agents).update(cx, |agents, cx| agents.handle(answered, cx));
        result
    }

    fn read(&self, path: &str, cx: &mut TestAppContext) -> ToolResult {
        let (id, answer) = self.call("read_file", json!({ "path": path }), cx);
        self.answer(id, answer, cx)
    }

    fn set(&self, tier: Tier, cx: &mut TestAppContext) {
        let agent = self.agent.clone();
        (self.agents).update(cx, |agents, cx| agents.set_tier(&agent, tier, cx));
    }

    /// The thread's steps as its trace has them: each one's tool, its title, and whether it holds
    /// a picture.
    fn steps(&self, cx: &mut TestAppContext) -> Vec<(String, String, bool)> {
        (self.agents).read_with(cx, |agents, _| {
            (agents.task(&self.agent).unwrap().trace.iter())
                .map(|step| {
                    let tense = match (step.ended, &step.error) {
                        (None, _) => tools::Tense::Now,
                        (Some(_), None) => tools::Tense::Done,
                        (Some(_), Some(_)) => tools::Tense::Failed,
                    };
                    (
                        step.did.tool.clone(),
                        tools::step_title(&step.did, tense),
                        step.picture.is_some(),
                    )
                })
                .collect()
        })
    }

    /// The thread's log entries: each one's action, site, target and refusal.
    fn log(&self, cx: &mut TestAppContext) -> Vec<(String, String, String, String)> {
        (self.agents).read_with(cx, |agents, _| {
            (agents.activity(&self.agent))
                .map(|entry| {
                    let each = [&entry.action, &entry.site, &entry.target, &entry.refused];
                    let [action, site, target, refused] = each.map(|text| text.to_string());
                    (action, site, target, refused)
                })
                .collect()
        })
    }

    fn uploads(&self) -> PathBuf {
        self.base.join("profile/Tasks/Uploads").join(&self.agent)
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        _ = std::fs::remove_dir_all(&self.base);
    }
}

/// An outside app's agent is given no file tool: read_file isn't in its list, nor upload's files,
/// and a call with either reads nothing and puts nothing.
#[gpui::test]
fn an_outside_agent_has_no_file_tools(cx: &mut TestAppContext) {
    let outside = tools::list("claude-code");
    let upload = (outside.as_array().unwrap().iter())
        .find(|tool| tool["name"] == "upload")
        .unwrap();
    assert!(upload.pointer("/inputSchema/properties/files").is_none());
    for tool in tools::FILE_TOOLS {
        assert!(
            !tools::names("claude-code").contains(&tool.to_string()),
            "{tool}"
        );
        assert!(tools::names("task-1").contains(&tool.to_string()), "{tool}");
    }
    let (agents, browser, _) = connected(cx);
    let read = call(
        &agents,
        "claude-code",
        "read_file",
        json!({ "path": "/etc/hosts" }),
        cx,
    );
    cx.run_until_parked();
    assert!(read.recv().unwrap().error);
    let upload = json!({ "tab": 20, "ref": "e1", "files": ["/etc/hosts"] });
    let put = call(&agents, "claude-code", "upload", upload, cx);
    cx.run_until_parked();
    assert!(put.recv().unwrap().error);
    assert!(browser.puts.borrow().is_empty() && browser.pdf_reads.borrow().is_empty());
}

/// A Codex thread is handed no file tool, as its own tools read the whole disk until its deny-root
/// profile lands, and a read it calls anyway is refused before anything is read. A thread Claude
/// runs has them.
#[gpui::test]
fn a_codex_thread_is_handed_no_file_tools(cx: &mut TestAppContext) {
    // A thread's strand is handed its thread's tools, so none of these either.
    for agent in ["task-1", "task-1-1"] {
        let (codex, claude) = (
            tools::handed(agent, Cli::Codex),
            tools::handed(agent, Cli::Claude),
        );
        for tool in tools::FILE_TOOLS {
            assert!(
                !codex.contains(&tool.to_string()) && claude.contains(&tool.to_string()),
                "{agent} {tool}"
            );
        }
    }
    let thread = Thread::new("codex", Cli::Codex, Tier::Work, cx);
    let read = thread.read(&thread.at("home/Taxes/notes.txt"), cx);
    assert!(read.error && read.text.starts_with("Not done:"), "{read:?}");
    assert!(!read.text.contains("first"));
}

/// A text file reads as its numbered lines, and its step names it by its name alone.
#[gpui::test]
fn a_thread_reads_a_text_file_in_its_folder(cx: &mut TestAppContext) {
    let thread = Thread::new("text", Cli::Claude, Tier::Work, cx);
    let read = thread.read("notes.txt", cx);
    assert_eq!(read, ToolResult::text("1\tfirst\n2\tsecond\n"));
    let steps = thread.steps(cx);
    assert_eq!(
        steps.last().unwrap(),
        &(
            "read_file".into(),
            "Read \u{201c}notes.txt\u{201d}".into(),
            false
        )
    );
    // The log names neither the file nor where it is.
    let log = thread.log(cx);
    assert_eq!(
        log.first().unwrap(),
        &(
            "read_file".into(),
            String::new(),
            String::new(),
            String::new()
        )
    );
}

/// The level is the thread's as each call comes: a file outside its folders read at Full is
/// refused once the user lowers it to Work on pages, mid-turn. On the Mac the test's tree is in
/// ~/Library, where all but a granted folder is never-listed whatever the level.
#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn a_level_lowered_mid_turn_refuses_the_next_read(cx: &mut TestAppContext) {
    let thread = Thread::new("lowered", Cli::Claude, Tier::Full, cx);
    let receipt = thread.at("home/Elsewhere/receipt.txt");
    assert_eq!(
        thread.read(&receipt, cx),
        ToolResult::text("1\ta receipt\n")
    );
    thread.set(Tier::Work, cx);
    let refused = thread.read(&receipt, cx);
    assert!(
        refused.error && !refused.text.contains("receipt"),
        "{refused:?}"
    );
    assert_eq!(
        thread.steps(cx).last().unwrap().1,
        "Couldn't read \u{201c}receipt.txt\u{201d}: add its folder first"
    );
}

/// A PDF's bytes go to the engine's reader, from the page asked, and its pages come back headed
/// with their numbers; a scan's picture reaches the model alone, never the step.
#[gpui::test]
fn a_pdf_is_read_by_the_engine_and_its_pictures_go_to_the_model_alone(cx: &mut TestAppContext) {
    let thread = Thread::new("pdf", Cli::Claude, Tier::Work, cx);
    let (id, answer) = thread.call(
        "read_file",
        json!({ "path": "statement.pdf", "pages": "2-3" }),
        cx,
    );
    let (read, bytes, first, count) = thread.browser.pdf_reads.borrow()[0].clone();
    assert_eq!(
        (bytes.as_slice(), first, count),
        (&b"%PDF-1.7 the statement"[..], 1, 2)
    );
    assert!(answer.try_recv().is_err(), "it answered before the engine");
    let pdf = Pdf {
        result: PDF_READ,
        page_count: 3,
        pages: vec![
            ("Totals".into(), None),
            (String::new(), Some(vec![0xff, 0xd8, 0xff])),
        ],
    };
    (thread.agents).update(cx, |agents, cx| agents.pdf_read_done(read, pdf, cx));
    let read = thread.answer(id, answer, cx);
    assert!(!read.error, "{read:?}");
    assert!(
        read.text.starts_with("Page 2 of 3\n\nTotals"),
        "{}",
        read.text
    );
    assert_eq!(
        (read.image.as_deref(), read.pictures.as_slice()),
        (None, &["/9j/".to_string()][..])
    );
    let parts = read.to_json()["content"].as_array().unwrap().clone();
    assert_eq!(parts.last().unwrap()["type"], "image");
    let step = thread.steps(cx).last().unwrap().clone();
    assert_eq!(
        step,
        (
            "read_file".into(),
            "Read \u{201c}statement.pdf\u{201d} \u{b7} pages 2\u{2013}3 of 3".into(),
            false
        )
    );
}

/// A locked PDF is answered as one, in its step and to the assistant. A never-listed PDF is
/// refused before the engine reads a byte of it.
#[gpui::test]
fn a_pdf_not_read_says_so_and_a_never_listed_one_reaches_no_engine(cx: &mut TestAppContext) {
    let thread = Thread::new("locked", Cli::Claude, Tier::Full, cx);
    let (id, answer) = thread.call("read_file", json!({ "path": "statement.pdf" }), cx);
    let read = thread.browser.pdf_reads.borrow()[0].0;
    let locked = Pdf {
        result: PDF_LOCKED,
        ..Default::default()
    };
    (thread.agents).update(cx, |agents, cx| agents.pdf_read_done(read, locked, cx));
    let read = thread.answer(id, answer, cx);
    assert!(read.error && read.text.contains("password"), "{read:?}");
    assert_eq!(
        thread.steps(cx).last().unwrap().1,
        "Couldn't read \u{201c}statement.pdf\u{201d}: it's locked with a password"
    );
    let profile = thread.base.join("profile/Tasks/kept.pdf");
    std::fs::create_dir_all(profile.parent().unwrap()).unwrap();
    std::fs::write(&profile, "%PDF-1.7 the thread's own").unwrap();
    let never = thread.read(&profile.to_string_lossy(), cx);
    assert!(never.error, "{never:?}");
    assert_eq!(thread.browser.pdf_reads.borrow().len(), 1);
}

/// The page whose one element is a file field, e1, in the thread's tab 30.
fn at_the_form(thread: &Thread, cx: &mut TestAppContext) {
    (thread.browser.tabs.borrow_mut()).push((30, 100, "Apply", "https://jobs.example/apply"));
    let tree = json!({ "nodes": [
        { "nodeId": "1", "role": { "value": "RootWebArea" }, "name": { "value": "Apply" },
          "childIds": ["2"] },
        { "nodeId": "2", "role": { "value": "button" }, "name": { "value": "Statement" },
          "backendDOMNodeId": 61 },
    ]});
    let (id, answer) = thread.call("snapshot", json!({ "tab": 30 }), cx);
    let request = thread.browser.pages.borrow().last().unwrap().1;
    (thread.agents).update(cx, |agents, cx| {
        agents.page_done(request, tree.to_string(), None, cx)
    });
    assert!(!thread.answer(id, answer, cx).error);
}

/// upload with files, once the core described the field: its id and answer's receiver.
fn upload(
    thread: &Thread,
    files: Value,
    cx: &mut TestAppContext,
) -> (u64, mpsc::Receiver<ToolResult>) {
    let (id, answer) = thread.call(
        "upload",
        json!({ "tab": 30, "ref": "e1", "files": files }),
        cx,
    );
    let request = thread.browser.pages.borrow().last().unwrap().1;
    let field = json!({ "field": 7.0, "label": "Statement", "site": "jobs.example" });
    (thread.agents).update(cx, |agents, cx| {
        agents.page_done(request, field.to_string(), None, cx)
    });
    cx.run_until_parked();
    (id, answer)
}

/// A file in the thread's folder goes into the field as its copy, with no card, and the log
/// names it and the site, never where it is. The copies go as the thread is closed.
#[gpui::test]
fn a_threads_file_goes_into_the_field_as_its_copy(cx: &mut TestAppContext) {
    let thread = Thread::new("upload", Cli::Claude, Tier::Work, cx);
    at_the_form(&thread, cx);
    let (id, answer) = upload(&thread, json!(["statement.pdf"]), cx);
    let (field, copies) = thread.browser.puts.borrow()[0].clone();
    assert_eq!(field, 7);
    assert!(copies[0].starts_with(thread.uploads()), "{copies:?}");
    assert_eq!(
        std::fs::read(&copies[0]).unwrap(),
        b"%PDF-1.7 the statement"
    );
    assert!(answer.try_recv().is_err(), "it answered before the core");
    (thread.agents).update(cx, |agents, cx| agents.file_field_done(7, 0, 1, cx));
    let put = thread.answer(id, answer, cx);
    assert!(!put.error, "{put:?}");
    assert_eq!(
        thread.steps(cx).last().unwrap().1,
        "Uploaded \u{201c}statement.pdf\u{201d} to jobs.example"
    );
    let entry = thread.log(cx).first().unwrap().clone();
    assert_eq!(
        entry,
        (
            "upload".into(),
            "jobs.example".into(),
            "statement.pdf".into(),
            String::new()
        )
    );
    let agent = thread.agent.clone();
    (thread.agents).update(cx, |agents, cx| agents.dismiss_task(&agent, cx));
    cx.run_until_parked();
    assert!(!thread.uploads().exists(), "its copies outlived it");
}

/// A file outside the thread's folders is refused at Work on pages, until the ask exists, and
/// nothing goes into the field, the file in its folder named with it included.
#[gpui::test]
fn a_file_outside_its_folders_is_refused_and_nothing_goes_in(cx: &mut TestAppContext) {
    let thread = Thread::new("outside", Cli::Claude, Tier::Work, cx);
    at_the_form(&thread, cx);
    let receipt = thread.at("home/Elsewhere/receipt.txt");
    let (id, answer) = upload(&thread, json!([receipt]), cx);
    let refused = thread.answer(id, answer, cx);
    assert!(
        refused.error && refused.text.starts_with("Not done:"),
        "{refused:?}"
    );
    assert!(thread.browser.puts.borrow().is_empty());
    assert!(
        thread.browser.dropped.borrow().contains(&7),
        "the core kept the field"
    );
    let line = thread.steps(cx).last().unwrap().1.clone();
    assert!(
        line.starts_with("Couldn't upload to jobs.example: \u{201c}receipt.txt\u{201d}"),
        "{line}"
    );
    let entry = thread.log(cx).first().unwrap().clone();
    assert_eq!(
        (entry.0.as_str(), entry.1.as_str(), entry.2.as_str()),
        ("upload", "jobs.example", "receipt.txt")
    );
    assert!(
        !entry.3.contains("Elsewhere") && !entry.3.contains('/'),
        "{entry:?}"
    );
    assert!(!thread.uploads().join("1").exists());
}
