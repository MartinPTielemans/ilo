//! The one way the browser starts a program of its own (#1676).

use std::ffi::OsStr;
use std::process::Command;

/// A command for `program`, as `Command::new` makes one, but that on Windows opens no console
/// window: the windowed browser has no console, so each console program it starts, and the
/// cmd.exe that runs an npm .cmd shim, would flash one of its own. What the program starts in turn
/// shares its hidden console.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }

    /// Every program the app starts goes through `command`, so none opens a console window on
    /// Windows: a `Command::new` anywhere else in the app's code fails here.
    #[test]
    fn every_program_starts_through_command() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        for dir in ["src", "platform", "windows"] {
            sources(&root.join(dir), &mut files);
        }
        assert!(files.len() > 50, "the scan found the app's code");
        let helper = root.join("src").join("child.rs");
        let bare: Vec<String> = (files.iter())
            .filter(|file| **file != helper)
            .flat_map(|file| {
                let text = std::fs::read_to_string(file).unwrap();
                let file = file.strip_prefix(root).unwrap().display().to_string();
                (text.lines().enumerate())
                    .filter(|(_, line)| line.contains("Command::new("))
                    .map(move |(number, line)| format!("{file}:{}: {}", number + 1, line.trim()))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(
            bare.is_empty(),
            "start these with browser::child::command:\n{}",
            bare.join("\n")
        );
    }
}
