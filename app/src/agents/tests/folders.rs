//! Settings › Agents › Sound and Folders (#4092): the app's own, kept with the agents' settings.
//! A folder comes only from the user's pick, each change is in the log by the folder's name, and
//! the thread's own folder is always there.

use std::path::Path;

use super::*;
use crate::agents::files::Row;
use crate::agents::folders::{self, Folder};

/// Agents saving to a file of their own, and the file.
fn saving(cx: &mut TestAppContext) -> (Entity<Agents>, PathBuf, Rc<Made>, Arc<Configs>) {
    let dir = std::env::temp_dir().join(format!("agents-folders-{}", random_token()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Agents");
    let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
    let agents = agents(&browser, &configs, Some(path.clone()), cx);
    (agents, path, browser, configs)
}

/// What a later launch reads from `path`.
fn later(
    path: &Path,
    browser: &Rc<Made>,
    configs: &Arc<Configs>,
    cx: &mut TestAppContext,
) -> Entity<Agents> {
    let saved = std::fs::read_to_string(path).unwrap();
    let later = agents(browser, configs, None, cx);
    later.update(cx, |agents, cx| agents.load(&saved, cx));
    later
}

#[gpui::test]
fn sound_is_off_until_the_user_turns_it_on_and_stays_so(cx: &mut TestAppContext) {
    let (agents, path, browser, configs) = saving(cx);
    assert!(!agents.read_with(cx, |agents, _| agents.sound()));
    agents.update(cx, |agents, cx| agents.set_sound(true, cx));
    let later = later(&path, &browser, &configs, cx);
    assert!(later.read_with(cx, |agents, _| agents.sound()));
    agents.update(cx, |agents, cx| agents.set_sound(false, cx));
    let again = self::later(&path, &browser, &configs, cx);
    assert!(!again.read_with(cx, |agents, _| agents.sound()));
}

#[gpui::test]
fn can_edit_starts_with_the_threads_own_folder_and_can_view_empty(cx: &mut TestAppContext) {
    let (agents, ..) = saving(cx);
    agents.read_with(cx, |agents, _| {
        assert_eq!(agents.folders(Row::Change), [Folder::Thread]);
        assert_eq!(agents.folders(Row::Read), []);
        assert!(agents.listed_folders().is_empty());
    });
}

/// The folders below are made in the system's temporary folder, which the Mac's and Windows'
/// never lists refuse as the system's own.
#[cfg(not(any(windows, target_os = "macos")))]
mod picked {
    use super::*;

    /// A folder named `name` the user could pick, by its real path.
    fn folder(name: &str) -> PathBuf {
        let at = std::env::temp_dir()
            .join(format!("pick-{}", random_token()))
            .join(name);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::canonicalize(at).unwrap()
    }

    /// The log's newest entry of the folders' own.
    fn last_entry(agents: &Entity<Agents>, cx: &mut TestAppContext) -> Activity {
        agents.read_with(cx, |agents, _| {
            agents.activity(folders::AGENT).next().unwrap().clone()
        })
    }

    #[gpui::test]
    fn a_folder_the_user_picks_is_listed_logged_by_its_name_and_comes_back(
        cx: &mut TestAppContext,
    ) {
        let (agents, path, browser, configs) = saving(cx);
        let taxes = folder("Taxes");
        let added = agents.update(cx, |agents, cx| agents.add_folder(Row::Change, &taxes, cx));
        assert!(added);
        let entry = last_entry(&agents, cx);
        assert_eq!(entry.action.as_ref(), folders::ADDED);
        assert_eq!(entry.target.as_ref(), "Can edit: Taxes");
        let parent = taxes.parent().unwrap().to_string_lossy().into_owned();
        assert!(
            !format!("{entry:?}").contains(&parent),
            "the log names its path"
        );
        let later = later(&path, &browser, &configs, cx);
        later.read_with(cx, |agents, _| {
            let shown = [Folder::Thread, Folder::Added("Taxes".into())];
            assert_eq!(agents.folders(Row::Change), shown);
            let listed = agents.listed_folders();
            assert_eq!(listed.len(), 1);
            assert_eq!(
                (listed[0].root(), listed[0].row),
                (taxes.as_path(), Row::Change)
            );
        });
    }

    #[gpui::test]
    fn the_threads_own_folder_cant_be_removed_and_an_added_one_can(cx: &mut TestAppContext) {
        let (agents, path, browser, configs) = saving(cx);
        let taxes = folder("Taxes");
        agents.update(cx, |agents, cx| agents.add_folder(Row::Change, &taxes, cx));
        agents.update(cx, |agents, cx| agents.remove_folder(Row::Change, 0, cx));
        let shown = agents.read_with(cx, |agents, _| agents.folders(Row::Change));
        assert_eq!(shown, [Folder::Thread, Folder::Added("Taxes".into())]);
        assert_eq!(last_entry(&agents, cx).action.as_ref(), folders::ADDED);
        agents.update(cx, |agents, cx| agents.remove_folder(Row::Change, 1, cx));
        let entry = last_entry(&agents, cx);
        assert_eq!(entry.action.as_ref(), folders::REMOVED);
        assert_eq!(entry.target.as_ref(), "Can edit: Taxes");
        let later = later(&path, &browser, &configs, cx);
        assert_eq!(
            later.read_with(cx, |agents, _| agents.folders(Row::Change)),
            [Folder::Thread]
        );
    }

    #[gpui::test]
    fn a_folder_is_in_one_row_at_a_time(cx: &mut TestAppContext) {
        let (agents, ..) = saving(cx);
        let photos = folder("Photos");
        agents.update(cx, |agents, cx| agents.add_folder(Row::Read, &photos, cx));
        assert_eq!(
            agents.read_with(cx, |agents, _| agents.folders(Row::Read)),
            [Folder::Added("Photos".into())]
        );
        agents.update(cx, |agents, cx| agents.add_folder(Row::Change, &photos, cx));
        agents.read_with(cx, |agents, _| {
            assert_eq!(agents.folders(Row::Read), []);
            let shown = [Folder::Thread, Folder::Added("Photos".into())];
            assert_eq!(agents.folders(Row::Change), shown);
        });
    }

    #[gpui::test]
    fn a_pick_the_floor_refuses_isnt_added_nor_logged(cx: &mut TestAppContext) {
        let (agents, ..) = saving(cx);
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        for refused in [
            home.as_path(),
            Path::new("/"),
            Path::new("/etc"),
            Path::new("/proc"),
        ] {
            for row in [Row::Read, Row::Change] {
                let added = agents.update(cx, |agents, cx| agents.add_folder(row, refused, cx));
                assert!(!added, "{}", refused.display());
            }
        }
        agents.read_with(cx, |agents, _| {
            assert!(agents.listed_folders().is_empty());
            assert!(agents.activity(folders::AGENT).next().is_none());
        });
    }

    #[gpui::test]
    fn a_kept_folder_gone_or_refused_by_now_is_left_out(cx: &mut TestAppContext) {
        let (browser, configs) = (Made::with_agent_space(), Arc::new(Configs::default()));
        let kept = folder("Kept");
        let gone = folder("Gone");
        std::fs::remove_dir(&gone).unwrap();
        let saved = json!({ "folders": {
            "view": [kept.to_string_lossy(), gone.to_string_lossy(), "/etc"],
            "edit": ["/", "relative/path"],
        }});
        let later = agents(&browser, &configs, None, cx);
        later.update(cx, |agents, cx| agents.load(&saved.to_string(), cx));
        later.read_with(cx, |agents, _| {
            assert_eq!(agents.folders(Row::Read), [Folder::Added("Kept".into())]);
            assert_eq!(agents.folders(Row::Change), [Folder::Thread]);
        });
    }
}
