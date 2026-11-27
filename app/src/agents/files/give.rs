//! A thread's own files for a page's file field (upload with files, #4368): each file the
//! assistant names is resolved by the path floor and judged as a read by road B's table, and what
//! the page gets is the app's copy of it, made from the held handle at the check, never the file's
//! own path. The page reads its field only as its form is sent, perhaps minutes later, so a path
//! would let the file under that name change or be swapped in between.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use super::resolve::{self, For, Kind, NeverIds, Unresolved};
use super::{At, Decision, Listed, Never, Op, Refused, decide};
use crate::agents::tier::Tier;

/// What a thread's upload may reach: its level, its folders, ilo's copies of its attached files,
/// and the never list.
pub struct Reach<'a> {
    pub tier: Tier,
    pub folders: &'a [Listed],
    pub attached: Option<&'a Path>,
    pub never: &'a Never,
    pub never_ids: &'a NeverIds,
}

/// Why nothing was put in the field. Nothing is put unless every file passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotGiven {
    /// No files named.
    None,
    /// Several for a field that takes one.
    Many,
    /// The `index`th file was refused, as a read of it would be.
    File { index: usize, why: Refused },
    /// Copying the `index`th file failed; `full` when the disk said it's full.
    Copy { index: usize, full: bool },
}

/// The app's copies of `paths` for a field that takes several or not (`many`), each in its own
/// folder under `into`, the thread's uploads folder, by the name it has, in the order named. Each
/// is checked before any is copied. A file outside its folders at Work, which a read would ask
/// about, is refused as outside them until the ask exists (road B slice 5). Off the UI thread.
pub fn give(
    paths: &[PathBuf],
    many: bool,
    reach: &Reach,
    into: &Path,
) -> Result<Vec<PathBuf>, NotGiven> {
    match paths.len() {
        0 => return Err(NotGiven::None),
        1 => {}
        _ if !many => return Err(NotGiven::Many),
        _ => {}
    }
    let mut held = Vec::with_capacity(paths.len());
    for (index, path) in paths.iter().enumerate() {
        let refused = |why| NotGiven::File { index, why };
        held.push(check(path, reach).map_err(refused)?);
    }
    let copies = (|| {
        make_private(into).map_err(|error| (0, error))?;
        let mut copies = Vec::with_capacity(held.len());
        for (index, (fd, name)) in held.into_iter().enumerate() {
            let copied = (|| {
                let folder = into.join(index.to_string());
                make_private(&folder)?;
                let to = folder.join(name);
                copy(fd, &to)?;
                io::Result::Ok(to)
            })();
            copies.push(copied.map_err(|error| (index, error))?);
        }
        Ok(copies)
    })();
    copies.map_err(|(index, error): (usize, io::Error)| {
        _ = std::fs::remove_dir_all(into);
        let full = error.kind() == io::ErrorKind::StorageFull;
        NotGiven::Copy { index, full }
    })
}

/// The thread's uploads go: as it's closed or deleted, and a private window's thread's as the
/// window closes.
pub fn forget(into: &Path) {
    match std::fs::remove_dir_all(into) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            log!(
                "browser: a thread's uploads weren't removed: {:?}",
                error.kind()
            );
        }
        _ => {}
    }
}

/// The path floor and the table on one file to send: Look only never sends one, then as a read.
fn check(path: &Path, reach: &Reach) -> Result<(OwnedFd, std::ffi::OsString), Refused> {
    // Sending a file to a site is an act on a page, which Look only never takes.
    if reach.tier == Tier::Look {
        return Err(Refused::Needs(Tier::Work));
    }
    open(path, reach)
}

/// The path floor and the table on one file, as a read, read_file's too: its held handle and
/// name, or why not. Outside its folders at Work, which a read would ask about, it's refused as
/// outside them until the ask exists (road B slice 5).
pub fn open(path: &Path, reach: &Reach) -> Result<(OwnedFd, std::ffi::OsString), Refused> {
    let resolved = resolve::resolve(path, For::Read, reach.never_ids);
    // Where it is is judged before whether it's there, so a refusal never says whether a file
    // exists outside what the thread was given. A path that didn't resolve is judged as written.
    let (at, other_names) = match &resolved {
        Ok(resolved) => {
            let at = (reach.never).at(&resolved.real, reach.folders, reach.attached);
            // ilo's copies of the attached files are in its own folder, which the walk passes.
            let at = match resolved.never && at != At::Attached {
                true => At::Never,
                false => at,
            };
            let other_names = (resolved.target.as_ref()).is_some_and(|held| held.other_names);
            (at, other_names && at != At::Never)
        }
        Err(_) => (reach.never.at(path, reach.folders, reach.attached), false),
    };
    match decide(reach.tier, at, Op::Read, other_names) {
        Decision::Allow => {}
        Decision::Ask => return Err(Refused::Guarded),
        Decision::Refuse(why) => return Err(why),
    }
    let resolved = resolved.map_err(|why| match why {
        Unresolved::NotAFile | Unresolved::Loop => Refused::NotOpened,
        _ => Refused::Gone,
    })?;
    let target = resolved.target.ok_or(Refused::Gone)?;
    // A folder never goes in: Chrome would give the page read access under it.
    match target.kind {
        Kind::File => Ok((target.fd, resolved.name)),
        _ => Err(Refused::NotOpened),
    }
}

fn make_private(folder: &Path) -> io::Result<()> {
    DirBuilder::new().mode(0o700).recursive(true).create(folder)
}

/// The held file's bytes as they are now, into a new file `to` only this user reads: a clone
/// where the disk makes one, which costs no space until either changes, or a copy.
fn copy(from: OwnedFd, to: &Path) -> io::Result<()> {
    let mut from = File::from(from);
    if clone_named(&from, to).is_ok() {
        // A clone takes the file's own permissions, so it's made this user's alone.
        let made = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(to)?;
        return made.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600));
    }
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(to)?;
    if clone_into(&from, &out).is_ok() {
        return Ok(());
    }
    from.seek(SeekFrom::Start(0))?;
    io::copy(&mut from, &mut out)?;
    out.sync_all()
}

/// The Mac's clone, by the held file and a name to make, as APFS clones.
#[cfg(target_os = "macos")]
fn clone_named(from: &File, to: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn fclonefileat(
            from: libc::c_int,
            folder: libc::c_int,
            to: *const libc::c_char,
            flags: u32,
        ) -> libc::c_int;
    }
    let name = std::ffi::CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: an open file of ours, the current folder for a full path, and a NUL-ended name.
    match unsafe { fclonefileat(from.as_raw_fd(), libc::AT_FDCWD, name.as_ptr(), 0) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

#[cfg(not(target_os = "macos"))]
fn clone_named(_: &File, _: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

/// Linux's FICLONE: the new file shares the held file's blocks, copied on a write to either.
#[cfg(target_os = "linux")]
fn clone_into(from: &File, to: &File) -> io::Result<()> {
    // SAFETY: two open files of ours; the ioctl reads only the source's descriptor.
    match unsafe { libc::ioctl(to.as_raw_fd(), libc::FICLONE, from.as_raw_fd()) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

#[cfg(not(target_os = "linux"))]
fn clone_into(_: &File, _: &File) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

/// The upload's lines in the thread. `site` is the field's frame's, as the core named it; a file
/// is named by its name alone, never its path.
pub mod words {
    use super::{NotGiven, Refused};
    use crate::agents::files::words::quoted;
    use crate::product::PRODUCT;

    /// The step's line while it's doing and once it's done, naming one file or counting several.
    /// The done line for several lists each file under it in one file's done form, `uploaded`.
    pub fn uploading(files: &[&str], site: &str, done: bool) -> String {
        match (files, done) {
            ([file], false) => format!("Uploading {} to {site}", quoted(file)),
            ([file], true) => uploaded(file, site),
            (files, false) => format!("Uploading {} files to {site}", files.len()),
            (files, true) => format!("Uploaded {} files to {site}", files.len()),
        }
    }

    /// One file's done line, as the line for several lists each.
    pub fn uploaded(file: &str, site: &str) -> String {
        format!("Uploaded {} to {site}", quoted(file))
    }

    /// What a refused or failed upload's line says, for the call's `files` (names only), or none
    /// for a call with no files, which only the assistant is told of. A refused or failed file
    /// names the first in the call's order, as nothing goes in when one doesn't.
    pub fn not_uploaded(files: &[&str], site: &str, why: NotUploaded) -> Option<String> {
        let count = files.len();
        let named = |index: usize, reason: &str| {
            let file = files.get(index).copied().unwrap_or_default();
            format!("Couldn't upload to {site}: {} {reason}", quoted(file))
        };
        Some(match (why, files) {
            (NotUploaded::Given(NotGiven::None), _) | (_, []) => return None,
            (NotUploaded::Given(NotGiven::Many), _) => {
                format!("Couldn't upload {count} files to {site}: the field takes one file")
            }
            (NotUploaded::FieldGone, [file]) => {
                format!(
                    "Couldn't upload {} to {site}: the page changed first",
                    quoted(file)
                )
            }
            (NotUploaded::FieldGone, _) => {
                format!("Couldn't upload {count} files to {site}: the page changed first")
            }
            (NotUploaded::Given(NotGiven::File { index, why }), _) => match clause(why) {
                Some(reason) => named(index, &reason),
                // A refusal no upload meets, as a read never makes it: the step's failed line.
                None => match files {
                    [file] => format!("Couldn't upload {} to {site}", quoted(file)),
                    _ => format!("Couldn't upload {count} files to {site}"),
                },
            },
            (NotUploaded::Given(NotGiven::Copy { full: true, .. }), _) => {
                format!("Couldn't upload to {site}: the disk is full")
            }
            (NotUploaded::Given(NotGiven::Copy { index, .. }), _) => {
                named(index, "couldn't be copied")
            }
        })
    }

    /// Whether the refusal's line has the button that adds the file's folder, `ADD_FOLDER`.
    pub fn adds_folder(why: NotUploaded) -> bool {
        matches!(
            why,
            NotUploaded::Given(NotGiven::File {
                why: Refused::Guarded,
                ..
            })
        )
    }

    /// Why an upload didn't happen: refused or failed before the core, or the field went first.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum NotUploaded {
        Given(NotGiven),
        /// The core's BC_FILE_FIELD_LEFT: the page changed or navigated before the files went in.
        FieldGone,
    }

    /// A refused file's clause, as the table writes it with the file named before it: the
    /// upload's own, never a read's sentences. None for a refusal no upload meets.
    fn clause(why: Refused) -> Option<String> {
        Some(match why {
            Refused::Never => format!("is in a place {PRODUCT} keeps from threads"),
            // Outside its folders at Work on pages, until the ask, or a guarded place no folder
            // was picked in.
            Refused::Guarded => "isn't in a folder you've added".into(),
            Refused::NotOpened => format!("isn't a file {PRODUCT} can open"),
            Refused::Needs(level) => format!("needs {}", level.label()),
            Refused::Gone => "isn't there".into(),
            Refused::Cloud => "isn't downloaded".into(),
            Refused::Attached
            | Refused::OtherNames
            | Refused::Changed
            | Refused::TooLarge
            | Refused::NotText => return None,
        })
    }
}

#[cfg(test)]
mod tests;
