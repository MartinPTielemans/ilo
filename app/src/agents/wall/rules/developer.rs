//! The developer tools from an app's own Developer folder, when the system's link picks one: the
//! compilers and git start from there inside the wall as they do from the Command Line Tools'.
//! Only an app installed by an administrator is taken, so no program running as the user could
//! have put one of the user's files into the folder the wall then reads.
//!
//! Every check that fails leaves the wall as it is without it, and none says why: no sentence and
//! no log line names the folder or the app.

use std::path::{Component, Path, PathBuf};

/// The system's link to the selected developer tools, from the root.
const LINK: &str = "private/var/select/developer_dir";

/// `target` when it is exactly `/Applications/<name>.app/Contents/Developer`: one app, directly in
/// the system's Applications folder, its name not hidden, and nothing that walks up or around.
pub fn developer_folder(target: &Path) -> Option<&Path> {
    let mut names = Vec::new();
    for (at, component) in target.components().enumerate() {
        match (at, component) {
            (0, Component::RootDir) => {}
            (0, _) => return None,
            (_, Component::Normal(name)) => names.push(name.to_str()?),
            _ => return None,
        }
    }
    let ["Applications", app, "Contents", "Developer"] = names[..] else {
        return None;
    };
    let stem = app.strip_suffix(".app")?;
    // Its text exactly as its names spell it: no `.`, doubled or trailing slash that a component
    // reading of it would pass over.
    let spelled = format!("/Applications/{app}/Contents/Developer");
    let exact = target.as_os_str() == spelled.as_str();
    (exact && !stem.is_empty() && !app.starts_with('.')).then_some(target)
}

/// The Developer folder the system's link picks under `root`, if an administrator put it there:
/// the link, its folder, the app, its `Contents` and the Developer folder each owned by `owner`,
/// each folder with no write for its group or others, no link on the way down, and nothing of it
/// in the user's `home`. `root` is `/` and `owner` root, but for tests.
#[cfg(unix)]
pub fn developer(root: &Path, owner: u32, home: &Path) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let held = |path: &Path, link: bool| {
        std::fs::symlink_metadata(path).is_ok_and(|meta| {
            let kind = meta.file_type();
            // A link's own mode governs nothing: who may replace it is its folder's to say.
            let shaped = match link {
                true => kind.is_symlink(),
                false => kind.is_dir() && meta.mode() & 0o022 == 0,
            };
            shaped && meta.uid() == owner
        })
    };
    let link = root.join(LINK);
    if !held(&link, true) || !held(link.parent()?, false) {
        return None;
    }
    let target = std::fs::read_link(&link).ok()?;
    let inside = developer_folder(&target)?.strip_prefix("/").ok()?;
    let root = std::fs::canonicalize(root).ok()?;
    let folder = root.join(inside);
    // No link anywhere on the way: what the system matches is the path as it's named.
    if std::fs::canonicalize(&folder).ok()? != folder || folder.starts_with(home) {
        return None;
    }
    let contents = folder.parent()?;
    let plain = [contents.parent()?, contents, &folder]
        .into_iter()
        .all(|each| held(each, false));
    plain.then_some(folder)
}
