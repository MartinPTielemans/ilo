//! A thread's own files in its tools (#4367, #4368): read_file, and upload with `files`. Each
//! file is judged where it is by the path floor and road B's table, at the level the thread is
//! at as the call comes, so a level the user lowered mid-turn holds for its next call. Only the
//! product's own threads Claude runs have them: an outside app's agent gets no file tool, and a
//! Codex thread none until its deny-root profile lands, as its own tools read the whole disk.

use std::path::PathBuf;

use gpui::Context;

use super::files::read::Pdf;
use super::runner::Cli;
use super::{Act, Agents};

/// What waits on a PDF read's answer, by the core's read.
pub(super) type PdfDone = Box<dyn FnOnce(&mut Agents, Pdf, &mut Context<Agents>)>;

/// An upload whose files wait on the core to go into the page's field.
pub(super) struct Putting {
    pub act: Act,
    /// The files by their names alone, for its lines and the log.
    pub names: Vec<String>,
    pub site: String,
    /// The folder of this upload's copies, which goes if they never went in.
    pub into: PathBuf,
}

/// What a call to a file tool from a thread that has none is told.
pub(super) const NO_FILES: &str =
    "Not done: this task has no file tools. Go on without the file, or ask the user for it.";

impl Agents {
    /// Whether `agent` is a thread whose file tools it may use: the product's own, and one Claude
    /// runs.
    pub(super) fn has_files(&self, agent: &str) -> bool {
        self.task(agent)
            .is_some_and(|task| task.cli.is_some() && task.cli != Some(Cli::Codex))
    }

    /// The pages of a PDF read_file read, read by the engine (Browser::pdf_read), then `done` once
    /// with what it answered. False for a read the core refused, with no answer.
    pub(super) fn read_pdf(
        &mut self,
        bytes: &[u8],
        first: usize,
        count: usize,
        done: impl FnOnce(&mut Self, Pdf, &mut Context<Self>) + 'static,
    ) -> bool {
        let read = self.browser.pdf_read(bytes, first, count);
        if read != 0 {
            self.pdf_reading.insert(read, Box::new(done));
        }
        read != 0
    }

    /// The core's answer to the PDF read `read`, for what waits on it.
    pub fn pdf_read_done(&mut self, read: u64, pdf: Pdf, cx: &mut Context<Self>) {
        if let Some(done) = self.pdf_reading.remove(&read) {
            done(self, pdf, cx);
        }
    }

    /// What came of a thread's files put into the page's `field`: a BC_FILE_FIELD_ `result`,
    /// and with PUT the `count` that went in.
    pub fn file_field_done(
        &mut self,
        field: u64,
        result: i32,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(putting) = self.putting.remove(&field) else {
            return;
        };
        #[cfg(unix)]
        self.put_done(putting, result, count, cx);
        #[cfg(not(unix))]
        let _ = (putting, result, count, cx);
    }
}

#[cfg(unix)]
mod unix {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;

    use gpui::Context;
    use serde_json::Value;

    use super::{NO_FILES, Putting};
    use crate::agents::files::give::words::{self as upload_words, NotUploaded};
    use crate::agents::files::give::{self, NotGiven, Reach};
    use crate::agents::files::resolve::NeverIds;
    use crate::agents::files::tool::{self, Got, NotRead};
    use crate::agents::files::{Listed, Never, Op, Row, shown_name, words};
    use crate::agents::tier::Tier;
    use crate::agents::{Act, Agents, ToolResult, site};

    /// A thread's reach, as its call came: copied so the read is judged off the UI thread at
    /// what the thread had then.
    pub(in crate::agents) struct Reaches {
        tier: Tier,
        folders: Vec<Listed>,
        attached: Option<PathBuf>,
        tasks: PathBuf,
    }

    impl Reaches {
        /// `with` the thread's reach, off the UI thread: the never list and its roots'
        /// identities are taken here, as taking them reads the disk.
        fn with<T>(&self, with: impl FnOnce(&Reach) -> T) -> T {
            let never = Never::here(Some(self.tasks.clone()));
            let never_ids = NeverIds::of(&never.by_identity());
            let reach = Reach {
                tier: self.tier,
                folders: &self.folders,
                attached: self.attached.as_deref(),
                never: &never,
                never_ids: &never_ids,
            };
            with(&reach)
        }
    }

    impl Agents {
        fn reaches(&self, agent: &str) -> Option<Reaches> {
            let task = self.task(agent).filter(|_| self.has_files(agent))?;
            Some(Reaches {
                tier: task.tier,
                folders: (task.files.folder.iter())
                    .map(|folder| Listed::of(folder, Row::Change))
                    .collect(),
                attached: task.files.attached.clone(),
                tasks: self.tasks_dir(),
            })
        }

        /// read_file: the file is resolved and judged, then read off the UI thread, a PDF's
        /// pages by the engine. Its step names the file by what it resolved to.
        pub(in crate::agents) fn read_file(
            &mut self,
            agent: &str,
            connection: u64,
            call: u64,
            arguments: &Value,
            reply: mpsc::Sender<ToolResult>,
            cx: &mut Context<Self>,
        ) {
            let Some(reaches) = self.reaches(agent) else {
                _ = reply.send(ToolResult::error(NO_FILES));
                return;
            };
            let asked = match tool::asked(arguments, &reaches.folders) {
                Ok(asked) => asked,
                Err(told) => {
                    _ = reply.send(ToolResult::error(told));
                    return;
                }
            };
            let (agent, first, count) = (agent.to_string(), asked.first, asked.count);
            let reading = (cx.background_executor())
                .spawn(async move { reaches.with(|reach| tool::read(&asked, reach)) });
            cx.spawn(async move |agents, cx| {
                let read = reading.await;
                _ = agents.update(cx, |agents, cx| {
                    let name = read.name.clone();
                    match read.got {
                        Ok(Got::Pdf(bytes)) => {
                            let reply_pdf = reply.clone();
                            let (agent_pdf, name_pdf) = (agent.clone(), name.clone());
                            let done = move |agents: &mut Agents, pdf, _: &mut Context<Agents>| {
                                let answer = tool::pdf_answer(&pdf, first).map_err(NotRead::Unread);
                                let ended =
                                    answer.as_ref().map(|read| read.cut).map_err(|not| *not);
                                agents.read_ended(&agent_pdf, connection, call, &name_pdf, ended);
                                _ = reply_pdf.send(match answer {
                                    Ok(read) => ToolResult {
                                        pictures: read.pictures,
                                        ..ToolResult::text(read.text)
                                    },
                                    Err(not) => ToolResult::error(tool::told(not)),
                                });
                            };
                            if !agents.read_pdf(&bytes, first, count, done) {
                                agents.read_ended(
                                    &agent,
                                    connection,
                                    call,
                                    &name,
                                    Err(NotRead::Failed),
                                );
                                _ = reply.send(ToolResult::error(tool::told(NotRead::Failed)));
                            }
                        }
                        Ok(Got::Text(text)) => {
                            agents.read_ended(&agent, connection, call, &name, Ok(None));
                            _ = reply.send(ToolResult::text(text));
                        }
                        Ok(Got::Picture(picture)) => {
                            agents.read_ended(&agent, connection, call, &name, Ok(None));
                            use base64::Engine as _;
                            let jpeg =
                                base64::engine::general_purpose::STANDARD.encode(&picture.jpeg);
                            _ = reply.send(ToolResult {
                                pictures: vec![jpeg],
                                ..ToolResult::text(format!(
                                    "The picture, {} by {} pixels as you see it.",
                                    picture.width, picture.height
                                ))
                            });
                        }
                        Err(not) => {
                            agents.read_ended(&agent, connection, call, &name, Err(not));
                            _ = reply.send(ToolResult::error(tool::told(not)));
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }

        /// A read's step says what it read, or why not, and the log has it, never its name.
        fn read_ended(
            &mut self,
            agent: &str,
            connection: u64,
            call: u64,
            name: &str,
            read: Result<Option<(usize, usize, usize)>, NotRead>,
        ) {
            self.name_step(agent, call, name, tool::line(name, read));
            let refused = read.err().map(tool::told).unwrap_or_default();
            self.note(agent, connection, "read_file", "", &refused);
        }

        /// The upload's `files`, the assistant's paths, from a thread that has file tools: None
        /// for a call without them, which hands the field to the user as ever.
        pub(in crate::agents) fn upload_files(
            &self,
            agent: &str,
            arguments: &Value,
        ) -> Option<Result<Vec<String>, String>> {
            let files = arguments.get("files")?;
            if !self.has_files(agent) {
                return Some(Err(NO_FILES.into()));
            }
            let paths: Option<Vec<String>> = (files.as_array().into_iter().flatten())
                .map(|path| path.as_str().map(String::from))
                .collect();
            Some(match paths {
                Some(paths) if !paths.is_empty() => Ok(paths),
                _ => Err("upload's files takes the paths of the files to put in the field.".into()),
            })
        }

        /// The thread's files go into the field the core described, `field`, as copies of what
        /// was checked, or nothing does.
        pub(in crate::agents) fn put_files(
            &mut self,
            act: Act,
            field: u64,
            many: bool,
            cx: &mut Context<Self>,
        ) {
            let site = site(&self.browser.tab(act.tab, cx).url);
            let Some(reaches) = self.reaches(&act.agent) else {
                self.browser.file_field_drop(field);
                _ = act.reply.send(ToolResult::error(NO_FILES));
                return;
            };
            self.uploads += 1;
            let into = tool::uploads(&reaches.tasks, &act.agent).join(self.uploads.to_string());
            let folders = reaches.folders.clone();
            let paths: Vec<PathBuf> = (act.give.iter())
                .map(|path| match Path::new(path).is_absolute() {
                    true => PathBuf::from(path),
                    false => folders
                        .first()
                        .map_or(PathBuf::from(path), |folder| folder.root().join(path)),
                })
                .collect();
            let (copies_into, paths_in) = (into.clone(), paths.clone());
            let giving = (cx.background_executor()).spawn(async move {
                reaches.with(|reach| give::give(&paths_in, many, reach, &copies_into))
            });
            cx.spawn(async move |agents, cx| {
                let given = giving.await;
                _ = agents.update(cx, |agents, cx| {
                    let names = match &given {
                        Ok(copies) => copies.iter().map(|copy| shown_name(copy)).collect(),
                        Err(_) => paths.iter().map(|path| shown_name(path)).collect(),
                    };
                    let putting = Putting {
                        act,
                        names,
                        site,
                        into,
                    };
                    match given {
                        Ok(copies) if agents.browser.file_field_put(field, &copies) => {
                            // check.sh reads this line, which names no file.
                            log!(
                                "browser: agent {} puts {} files in {field}",
                                putting.act.agent,
                                copies.len()
                            );
                            agents.putting.insert(field, putting);
                        }
                        Ok(_) => agents.not_put(putting, NotUploaded::FieldGone, field, cx),
                        Err(not) => agents.not_put(putting, NotUploaded::Given(not), field, cx),
                    }
                    cx.notify();
                });
            })
            .detach();
        }

        /// The core's answer to the files put in the field.
        pub(in crate::agents) fn put_done(
            &mut self,
            putting: Putting,
            result: i32,
            count: usize,
            cx: &mut Context<Self>,
        ) {
            const PUT: i32 = 0;
            const LEFT: i32 = 2;
            match result {
                PUT => {
                    let names: Vec<&str> = putting.names.iter().map(String::as_str).collect();
                    let line = upload_words::uploading(&names, &putting.site, true);
                    self.upload_ended(&putting, line, "");
                    let files = match count {
                        1 => "1 file".to_string(),
                        count => format!("{count} files"),
                    };
                    _ = putting.act.reply.send(ToolResult::text(format!(
                        "Put {files} in the field. Take a snapshot to see what the page shows of them; nothing is sent until the page's form is."
                    )));
                }
                LEFT => self.not_put(putting, NotUploaded::FieldGone, 0, cx),
                _ => {
                    give::forget(&putting.into);
                    let line = (upload_words::not_uploaded(
                        &putting.names.iter().map(String::as_str).collect::<Vec<_>>(),
                        &putting.site,
                        NotUploaded::FieldGone,
                    ))
                    .unwrap_or_default();
                    let told = "Not done: the files didn't go into the field.";
                    self.upload_ended(&putting, line, told);
                    _ = putting.act.reply.send(ToolResult::error(told));
                }
            }
        }

        /// Nothing went in: the copies go, and the step says why. `field`, unless 0, is the
        /// core's still, and goes too.
        fn not_put(
            &mut self,
            putting: Putting,
            why: NotUploaded,
            field: u64,
            _: &mut Context<Self>,
        ) {
            if field != 0 {
                self.browser.file_field_drop(field);
            }
            give::forget(&putting.into);
            let names: Vec<&str> = putting.names.iter().map(String::as_str).collect();
            let line = upload_words::not_uploaded(&names, &putting.site, why).unwrap_or_default();
            let told = match why {
                NotUploaded::FieldGone => {
                    "Not done: the page changed before the files went in. Take a snapshot and try again.".to_string()
                }
                NotUploaded::Given(NotGiven::None) => "upload's files takes the paths of the files to put in the field.".into(),
                NotUploaded::Given(NotGiven::Many) => {
                    "Not done: this field takes one file. Upload one.".into()
                }
                NotUploaded::Given(NotGiven::File { why, .. }) => words::told(Op::Read, why),
                NotUploaded::Given(NotGiven::Copy { .. }) => {
                    "Not done: the file couldn't be copied for the page. Go on without it.".into()
                }
            };
            self.upload_ended(&putting, line, &told);
            _ = putting.act.reply.send(ToolResult::error(told));
        }

        /// An upload's step line, and its log entry: the files' names and the field's site,
        /// never a path.
        fn upload_ended(&mut self, putting: &Putting, line: String, refused: &str) {
            let act = &putting.act;
            self.name_step(&act.agent, act.call, &putting.names.join(", "), line);
            let entry = ("", Vec::new(), "");
            let names = putting.names.join(", ");
            let by = (act.connection, act.tab);
            self.note_entry(
                &act.agent,
                by,
                "upload",
                &putting.site,
                &names,
                refused,
                entry,
            );
        }
    }
}
