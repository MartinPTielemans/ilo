//! The `+` menu's Add Files… and Add Folder… (#4101): the system's own panel is the only way a
//! path comes into a task, and this file is the only place a folder is granted (`Granted::picked`).
//! The folder is checked against what ilo never gives as the panel returns it, and again at the
//! start. Neither row is drawn until this system can wall a task in with its files.

use std::path::PathBuf;

use gpui::{Context, PathPromptOptions};

use crate::agents::files::{self, Never, words};
use crate::agents::wall::Granted;

use super::super::Shell;

impl Shell {
    /// Add Files…: the system's panel, for one file or several.
    pub(in crate::shell) fn add_files(&mut self, cx: &mut Context<Self>) {
        self.open_panel(false, cx);
    }

    /// Add Folder…: the system's panel, for one folder.
    pub(in crate::shell) fn add_folder(&mut self, cx: &mut Context<Self>) {
        self.open_panel(true, cx);
    }

    fn open_panel(&mut self, folder: bool, cx: &mut Context<Self>) {
        let asked = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: !folder,
            // The system's own word on its button: the table gives no other.
            prompt: None,
        });
        cx.spawn(async move |shell, cx| {
            let paths = (asked.await.ok())
                .and_then(Result::ok)
                .flatten()
                .unwrap_or_default();
            shell
                .update(cx, |shell, cx| match folder {
                    true => {
                        if let Some(picked) = paths.into_iter().next() {
                            shell.grant_folder(picked, cx);
                        }
                    }
                    false => shell.attach_files(paths, cx),
                })
                .ok();
        })
        .detach();
    }

    /// The files the panel gave, attached to the next task, each once. ilo copies them at the
    /// start; until then they're only named on the line.
    pub(in crate::shell) fn attach_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !resting.bar.read(cx).on_ask() {
            return;
        }
        for path in paths {
            if path.is_file() && !resting.files.contains(&path) {
                resting.files.push(path);
            }
        }
        // A count, never a name.
        log!("browser: composer files {}", resting.files.len());
        cx.notify();
    }

    /// The folder the panel gave, its links resolved, granted to the next task, unless ilo never
    /// gives it: then the line under the composer's says so (X4), and nothing is granted.
    pub(in crate::shell) fn grant_folder(&mut self, picked: PathBuf, cx: &mut Context<Self>) {
        let Some(resting) = &mut self.resting else {
            return;
        };
        if !resting.bar.read(cx).on_ask() {
            return;
        }
        let never = Never::here(None);
        let resolved = files::resolved(&picked);
        match resolved.filter(|root| !files::never_grantable(root, &never)) {
            Some(root) => {
                resting.folder = Some(Granted::picked(root));
                resting.refused = None;
                log!("browser: composer folder granted");
            }
            None => {
                let named = files::shown_name(&picked);
                resting.refused = Some(words::never_given(&named).into());
                log!("browser: composer folder refused");
            }
        }
        cx.notify();
    }

    /// A pick of an attached file on its chip's menu: it's no longer attached.
    pub(in crate::shell) fn drop_file(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        if let Some(resting) = &mut self.resting {
            resting.files.retain(|each| *each != file);
            log!("browser: composer files {}", resting.files.len());
            cx.notify();
        }
    }

    /// Revoke on the folder's chip before the start: the next task has no folder.
    pub(in crate::shell) fn revoke_composer_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(resting) = &mut self.resting {
            resting.folder = None;
            log!("browser: composer folder revoked");
            cx.notify();
        }
    }
}
