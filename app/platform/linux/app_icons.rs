//! A browser's app icon on Linux, for the import's rows (#1673), which the Linux host asks for: its desktop entry's `Icon`,
//! found in the icon theme's hicolor PNGs or the pixmaps, as a launcher finds it. Flatpak's and
//! Snap's exports are data folders like the others. An SVG icon isn't read, so its row has the
//! globe plate.

use std::path::{Path, PathBuf};

use image::RgbaImage;

/// The sizes looked for, the nearest above the plate's 56 pixels first.
const SIZES: &[&str] = &[
    "64x64", "128x128", "96x96", "256x256", "48x48", "512x512", "32x32",
];

/// The data folders desktop entries and icons are in: the user's under `home`, and unless
/// `only_home`, as for a check run's made-up home, the system's, Flatpak's and Snap's.
pub fn data_dirs(home: &Path, only_home: bool) -> Vec<PathBuf> {
    let mut dirs = vec![
        home.join(".local/share"),
        home.join(".local/share/flatpak/exports/share"),
    ];
    if !only_home {
        let system = std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|dirs| !dirs.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        dirs.extend(system.split(':').map(PathBuf::from));
        dirs.push("/var/lib/flatpak/exports/share".into());
        dirs.push("/var/lib/snapd/desktop".into());
    }
    dirs
}

/// The icon of the first of `ids` with a desktop entry in `dirs` and an icon found for it,
/// `size` pixels square.
pub fn find(dirs: &[PathBuf], ids: &[String], size: u32) -> Option<RgbaImage> {
    ids.iter().find_map(|id| {
        let entry = (dirs.iter())
            .map(|dir| dir.join("applications").join(format!("{id}.desktop")))
            .find(|entry| entry.is_file())?;
        let icon = icon_name(&std::fs::read_to_string(entry).ok()?)?;
        let file = icon_file(dirs, &icon)?;
        let decoded = image::open(file).ok()?.into_rgba8();
        Some(image::imageops::resize(
            &decoded,
            size,
            size,
            image::imageops::FilterType::Lanczos3,
        ))
    })
}

/// The `Icon` of a desktop entry's `[Desktop Entry]` group.
pub fn icon_name(entry: &str) -> Option<String> {
    let mut main = false;
    for line in entry.lines().map(str::trim) {
        if line.starts_with('[') {
            main = line == "[Desktop Entry]";
        } else if main && let Some(icon) = line.strip_prefix("Icon=") {
            return Some(icon.trim().to_string()).filter(|icon| !icon.is_empty());
        }
    }
    None
}

/// A PNG for `icon`: a path as it is, else the name in hicolor's apps at the sizes looked for,
/// then the pixmaps.
fn icon_file(dirs: &[PathBuf], icon: &str) -> Option<PathBuf> {
    let png = |path: &Path| path.extension().is_some_and(|ext| ext == "png") && path.is_file();
    let path = Path::new(icon);
    if path.is_absolute() {
        return png(path).then(|| path.to_path_buf());
    }
    let themed = SIZES.iter().flat_map(|size| {
        dirs.iter()
            .map(move |dir| dir.join(format!("icons/hicolor/{size}/apps/{icon}.png")))
    });
    let pixmaps = dirs
        .iter()
        .map(|dir| dir.join(format!("pixmaps/{icon}.png")));
    themed.chain(pixmaps).find(|path| png(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The icon is the main group's, not an action's.
    #[test]
    fn reads_the_main_groups_icon() {
        let entry =
            "[Desktop Action new]\nIcon=other\n[Desktop Entry]\nName=Chrome\nIcon=google-chrome";
        assert_eq!(icon_name(entry).as_deref(), Some("google-chrome"));
        assert_eq!(icon_name("[Desktop Entry]\nName=x\n"), None);
    }

    /// A Flatpak's desktop entry in its exports, its icon in hicolor at 128, comes at the size
    /// asked; an id with no entry, or an entry whose icon isn't there, gives none.
    #[test]
    fn finds_an_icon_through_its_desktop_entry() {
        let home = std::env::temp_dir().join(format!("app-icons-{}", std::process::id()));
        let share = home.join(".local/share/flatpak/exports/share");
        std::fs::create_dir_all(share.join("applications")).unwrap();
        std::fs::create_dir_all(share.join("icons/hicolor/128x128/apps")).unwrap();
        std::fs::write(
            share.join("applications/org.example.Browser.desktop"),
            "[Desktop Entry]\nIcon=org.example.Browser\n",
        )
        .unwrap();
        std::fs::write(
            share.join("applications/org.example.Bare.desktop"),
            "[Desktop Entry]\nIcon=gone\n",
        )
        .unwrap();
        RgbaImage::from_pixel(128, 128, image::Rgba([200, 40, 40, 255]))
            .save(share.join("icons/hicolor/128x128/apps/org.example.Browser.png"))
            .unwrap();
        let dirs = data_dirs(&home, true);
        let ids = |id: &str| vec!["missing".to_string(), id.to_string()];
        let icon = find(&dirs, &ids("org.example.Browser"), 56).expect("no icon found");
        assert_eq!(icon.dimensions(), (56, 56));
        assert_eq!(icon.get_pixel(28, 28).0, [200, 40, 40, 255]);
        assert!(find(&dirs, &ids("org.example.Bare"), 56).is_none());
        std::fs::remove_dir_all(home).unwrap();
    }
}
