//! What Help › Copy Build Info copies and Help › Report a Problem… fills a new issue with: the
//! app's version and build, its engine's, the system's and the chip, and on Linux and Windows how
//! the app is installed, with Linux's session and desktop. Nothing leaves the computer until the
//! user submits the issue GitHub shows them.

use std::sync::LazyLock;

use gpui::App;

use crate::dialog::{Field, Question, Return, Tone};

use crate::updates::{self, Updater};

/// Where a new issue is filed: a default service URL, which a fork changes (branding/issues).
pub const NEW_ISSUE: &str = env!("APP_ISSUES");
/// The site's help and release notes, at a default service URL (branding/site).
pub const HELP_CENTER: &str = concat!(env!("APP_SITE"), "/help");
pub const WHATS_NEW: &str = concat!(env!("APP_SITE"), "/changelog");
/// What ilo sends and keeps, and where, on the same site.
pub const PRIVACY_POLICY: &str = concat!(env!("APP_SITE"), "/privacy");

/// The details of this copy of the app and the computer it runs on.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildInfo {
    /// The version, with the build number when the build has one: "0.1.0 (4321)".
    pub version: String,
    /// Chromium's version, where it can be read.
    pub engine: Option<String>,
    /// The system and its version: "macOS 15.1".
    pub system: String,
    /// The processor: "Apple M2 Pro", else the architecture.
    pub chip: String,
    /// How this copy is installed and, on Linux, the session and desktop it runs in, as the bug
    /// report form's Linux or Windows field asks: "Flatpak, Wayland, Hyprland". None on the Mac,
    /// where the form asks for none.
    pub setup: Option<String>,
}

impl BuildInfo {
    /// This copy's, read from the system when asked.
    pub fn here(cx: &App) -> Self {
        BuildInfo {
            version: version(),
            engine: ENGINE_VERSION.clone(),
            system: system(),
            chip: chip(),
            setup: here_setup(updates::updater(cx)),
        }
    }

    /// One detail a line, as Copy Build Info copies them.
    pub fn text(&self) -> String {
        let mut lines = vec![format!("{} {}", env!("APP_NAME"), self.version)];
        lines.extend(
            self.engine
                .as_ref()
                .map(|engine| format!("Chromium {engine}")),
        );
        lines.push(self.system.clone());
        lines.push(self.chip.clone());
        lines.extend(self.setup.clone());
        lines.join("\n")
    }

    /// Report a Problem…'s sheet: what happened, in the user's words, over these details, which
    /// Continue fills a new issue with. Everything stays editable in GitHub's form, and nothing
    /// leaves until the user submits it there (#2458).
    pub fn report(&self) -> Question {
        Question {
            icon: None,
            message: "Report a Problem".into(),
            detail: concat!(
                "Continue opens a new issue on GitHub with these details. Nothing is sent until ",
                "you submit it there, and you can change any of it first."
            )
            .into(),
            list: self
                .text()
                .lines()
                .map(|line| (line.to_owned().into(), Vec::new()))
                .collect(),
            action: "Continue".into(),
            cancel: "Cancel".into(),
            tone: Tone::Accent,
            also: None,
            fields: vec![Field {
                placeholder: "What happened?".into(),
                text: "".into(),
                secure: false,
            }],
            check: None,
            on_return: Return::Action,
        }
    }

    /// A new issue at `new_issue` on the bug report form, its version, Chromium, operating system,
    /// system and this system's install fields filled with these
    /// (.github/ISSUE_TEMPLATE/bug_report.yml's ids), and its title with `title` unless empty.
    pub fn issue_url(&self, new_issue: &str, title: &str) -> String {
        let mut fields = vec![("template", "bug_report.yml".to_owned())];
        let title = title.trim();
        if !title.is_empty() {
            fields.push(("title", title.to_owned()));
        }
        fields.push(("version", self.version.clone()));
        fields.extend(self.engine.clone().map(|engine| ("chromium", engine)));
        fields.push(("platform", PLATFORM.to_owned()));
        fields.push(("os", format!("{}, {}", self.system, self.chip)));
        // The form's field for this system's install: one for Linux, one for Windows.
        let field = if cfg!(windows) { "windows" } else { "linux" };
        fields.extend(self.setup.clone().map(|setup| (field, setup)));
        let query: Vec<String> = fields
            .iter()
            .map(|(name, value)| format!("{name}={}", escape(value)))
            .collect();
        format!("{new_issue}?{}", query.join("&"))
    }
}

/// The system's family, as the bug report form's Operating system offers it.
const PLATFORM: &str = if cfg!(target_os = "macos") {
    "macOS"
} else if cfg!(windows) {
    "Windows"
} else {
    "Linux"
};

/// How this copy is installed, and on Linux its session and desktop, read when asked.
#[cfg(target_os = "linux")]
fn here_setup(_: Updater) -> Option<String> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
    Some(linux_setup(
        crate::flatpak::in_flatpak(),
        gpui::guess_compositor(),
        desktop.as_deref(),
    ))
}

#[cfg(windows)]
fn here_setup(updater: Updater) -> Option<String> {
    Some(windows_install(updater).to_owned())
}

/// The Mac's form asks for none, and the preview's web build has none to tell.
#[cfg(not(any(target_os = "linux", windows)))]
fn here_setup(_: Updater) -> Option<String> {
    None
}

/// The Flatpak from ilo's releases or not, which the form takes as built from source, the session
/// GPUI connects to, and the desktop, as the form's Linux field asks for them.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_setup(flatpak: bool, session: &str, desktop: Option<&str>) -> String {
    let install = if flatpak {
        "Flatpak"
    } else {
        "built from source"
    };
    let mut words = vec![install, session];
    words.extend(desktop.filter(|desktop| !desktop.is_empty()));
    words.join(", ")
}

/// What installed this copy, from what updates it: winget runs the same installer as a download
/// does, so the two can't be told apart, and a build that updates itself not at all is a dev build.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_install(updater: Updater) -> &'static str {
    match updater {
        Updater::Store => "the Microsoft Store",
        Updater::Own => "the installer or winget",
        Updater::None => "built from source",
    }
}

/// This copy's version, as the release names it. This crate's: on Linux and Windows src/main.rs
/// is built in a crate of its own (linux/, windows/), whose version isn't the app's.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The version Settings › About shows, with the build number build.sh gives a build.
pub fn version() -> String {
    match option_env!("APP_BUILD") {
        Some(build) => format!("{VERSION} ({build})"),
        None => VERSION.to_owned(),
    }
}

/// The engine's version: on the Mac, the name of the folder its framework's current version is in,
/// as Chromium names it; elsewhere what the engine carries beside chrome (`engine_beside`).
pub static ENGINE_VERSION: LazyLock<Option<String>> = LazyLock::new(|| {
    let exe = std::env::current_exe().ok()?;
    if !cfg!(target_os = "macos") {
        return engine_beside(exe.parent()?);
    }
    let frameworks = exe.parent()?.parent()?.join("Frameworks");
    std::fs::read_dir(frameworks)
        .ok()?
        .flatten()
        .find_map(|entry| {
            let name = entry.file_name();
            if !name.to_str()?.ends_with(" Framework.framework") {
                return None;
            }
            let current = std::fs::read_link(entry.path().join("Versions/Current")).ok()?;
            Some(current.to_str()?.to_owned())
        })
});

/// The version of the engine in `dir`: Windows's build names a manifest after it, such as
/// "156.0.8078.47.manifest", and the Linux engine's package writes it in VERSION
/// (engine/linux/package.sh). An engine packaged before that has neither. Windows's engine also
/// carries the manifests of the versions its runner built before (#1983), so the newest is its own.
fn engine_beside(dir: &std::path::Path) -> Option<String> {
    // Its four numbers, for comparing, with the text.
    let version = |text: &str| -> Option<(Vec<u64>, String)> {
        let number = |part: &str| {
            part.bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| part.parse().ok())?
        };
        let parts: Vec<u64> = text.split('.').map(number).collect::<Option<_>>()?;
        (parts.len() == 4).then(|| (parts, text.to_owned()))
    };
    if let Ok(text) = std::fs::read_to_string(dir.join("VERSION")) {
        return version(text.trim()).map(|(_, text)| text);
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| version(entry.file_name().to_str()?.strip_suffix(".manifest")?))
        .max()
        .map(|(_, text)| text)
}

/// `text` for a URL's query: what isn't a letter, a digit or `-._~` as UTF-8 bytes in %XX.
fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                escaped.push(byte as char)
            }
            _ => escaped.push_str(&format!("%{byte:02X}")),
        }
    }
    escaped
}

#[cfg(target_os = "macos")]
fn system() -> String {
    let version = (plist::Value::from_file("/System/Library/CoreServices/SystemVersion.plist")
        .ok())
    .and_then(|plist| {
        Some(
            plist
                .as_dictionary()?
                .get("ProductVersion")?
                .as_string()?
                .to_owned(),
        )
    });
    format!("macOS {}", version.as_deref().unwrap_or("(unknown)"))
}

#[cfg(target_os = "linux")]
fn system() -> String {
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    (release.lines())
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|name| name.trim_matches('"').to_owned())
        .unwrap_or_else(|| "Linux".to_owned())
}

#[cfg(windows)]
fn system() -> String {
    let read = |name| windows_current_version(name);
    windows_system(
        read("ProductName").as_deref(),
        read("DisplayVersion").as_deref(),
        read("CurrentBuildNumber").as_deref(),
    )
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn system() -> String {
    std::env::consts::OS.to_owned()
}

/// Windows's edition, release and build: "Windows 11 Pro 24H2 (build 26100)". Windows 11 still
/// names itself Windows 10 in the registry, so its build, 22000 on, says which it is.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_system(product: Option<&str>, release: Option<&str>, build: Option<&str>) -> String {
    fn given(text: Option<&str>) -> Option<&str> {
        text.map(str::trim).filter(|text| !text.is_empty())
    }
    let build: Option<u32> = given(build).and_then(|build| build.parse().ok());
    let mut words = vec![given(product).unwrap_or("Windows").to_owned()];
    if build.is_some_and(|build| build >= WINDOWS_11) {
        words[0] = words[0].replacen("Windows 10", "Windows 11", 1);
    }
    words.extend(given(release).map(str::to_owned));
    words.extend(build.map(|build| format!("(build {build})")));
    words.join(" ")
}

/// The first build of Windows 11.
const WINDOWS_11: u32 = 22000;

/// Whether this is Windows 11, which rounds a window's corners, by its build.
#[cfg(windows)]
pub fn windows_11() -> bool {
    static BUILD: LazyLock<u32> = LazyLock::new(|| {
        windows_current_version("CurrentBuildNumber")
            .and_then(|build| build.trim().parse().ok())
            .unwrap_or(0)
    });
    *BUILD >= WINDOWS_11
}

/// A text value of Windows's own key for its version, as the registry holds it.
#[cfg(windows)]
fn windows_current_version(name: &str) -> Option<String> {
    windows_machine_text(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", name)
}

/// A text value under HKEY_LOCAL_MACHINE's `folder`, if it's there.
#[cfg(windows)]
fn windows_machine_text(folder: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let wide = |text: &str| text.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (folder, name) = (wide(folder), wide(name));
    let mut text = [0u16; 256];
    let mut size = std::mem::size_of_val(&text) as u32;
    // SAFETY: both names end in a nul, and `size` is `text`'s size in bytes, which the call
    // writes no further than, and sets to what it wrote.
    let read = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            folder.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            text.as_mut_ptr().cast(),
            &mut size,
        )
    };
    (read == 0).then(|| {
        let text = &text[..(size as usize / 2).min(text.len())];
        String::from_utf16_lossy(text)
            .trim_end_matches('\0')
            .to_owned()
    })
}

/// The processor's name, else the architecture's.
fn chip() -> String {
    #[cfg(target_os = "macos")]
    let name = (crate::child::command("/usr/sbin/sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok())
    .and_then(|output| String::from_utf8(output.stdout).ok());
    #[cfg(target_os = "linux")]
    let name = (std::fs::read_to_string("/proc/cpuinfo").ok()).and_then(|info| {
        let line = info.lines().find(|line| line.starts_with("model name"))?;
        Some(line.split_once(':')?.1.to_owned())
    });
    #[cfg(windows)]
    let name = windows_machine_text(
        r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
        "ProcessorNameString",
    );
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    let name: Option<String> = None;
    (name.map(|name| name.trim().to_owned()))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::env::consts::ARCH.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn made_up() -> BuildInfo {
        BuildInfo {
            version: "0.1.0 (4321)".into(),
            engine: Some("156.0.7600.12".into()),
            system: "macOS 15.1".into(),
            chip: "Apple M2 Pro".into(),
            setup: None,
        }
    }

    #[test]
    fn copies_a_detail_a_line() {
        let text = made_up().text();
        assert_eq!(
            text,
            concat!(
                env!("APP_NAME"),
                " 0.1.0 (4321)\nChromium 156.0.7600.12\nmacOS 15.1\nApple M2 Pro"
            )
        );
    }

    /// On Windows, the system and the chip come from the registry, not the fallbacks, which
    /// named them "windows" and "x86_64" (#2458).
    #[cfg(windows)]
    #[test]
    fn reads_windows_version_and_processor() {
        let system = system();
        assert!(
            system.starts_with("Windows ") && system.contains("(build "),
            "expected the edition and build from Windows's version key, read: {system}"
        );
        let chip = chip();
        assert_ne!(
            chip,
            std::env::consts::ARCH,
            "expected the processor's name from the registry, read only the architecture"
        );
    }

    /// Windows is named by its edition, release and build, Windows 11 by its build though the
    /// registry calls it Windows 10, and what the registry lacks is left out (#2458).
    #[test]
    fn names_windows_edition_release_and_build() {
        let named = |product, release, build| windows_system(product, release, build);
        assert_eq!(
            named(Some("Windows 10 Pro"), Some("24H2"), Some("26100")),
            "Windows 11 Pro 24H2 (build 26100)"
        );
        assert_eq!(
            named(Some("Windows 10 Home"), Some("22H2"), Some("19045")),
            "Windows 10 Home 22H2 (build 19045)"
        );
        assert_eq!(
            named(
                Some("Windows Server 2022 Datacenter"),
                Some(""),
                Some("20348")
            ),
            "Windows Server 2022 Datacenter (build 20348)"
        );
        assert_eq!(named(None, None, None), "Windows");
        assert_eq!(named(Some(" "), None, Some("x")), "Windows");
    }

    #[test]
    fn copies_no_engine_line_without_one() {
        let info = BuildInfo {
            engine: None,
            ..made_up()
        };
        assert!(!info.text().contains("Chromium"));
        assert!(!info.issue_url(NEW_ISSUE, "").contains("chromium="));
    }

    #[test]
    fn fills_the_bug_reports_fields() {
        let url = made_up().issue_url("https://github.com/o/r/issues/new", "");
        assert_eq!(
            url,
            format!(
                "https://github.com/o/r/issues/new?template=bug_report.yml&version=0.1.0%20%284321%29\
                 &chromium=156.0.7600.12&platform={PLATFORM}&os=macOS%2015.1%2C%20Apple%20M2%20Pro"
            )
        );
    }

    /// What the user typed in the sheet titles the issue, trimmed and escaped, and nothing typed
    /// leaves the title to GitHub's form (#2458).
    #[test]
    fn titles_the_issue_with_what_happened() {
        let new = "https://github.com/o/r/issues/new";
        let titled = made_up().issue_url(new, "  Tabs & Spaces vanish ");
        assert!(titled.starts_with(&format!(
            "{new}?template=bug_report.yml&title=Tabs%20%26%20Spaces%20vanish&version="
        )));
        assert!(!made_up().issue_url(new, "   ").contains("title="));
    }

    /// The sheet lists every detail Continue fills the issue with, one a line as Copy Build Info
    /// copies them, under one field for what happened, which ↩ continues from (#2458).
    #[test]
    fn the_sheet_shows_what_the_issue_is_filled_with() {
        let info = BuildInfo {
            setup: Some("Flatpak, Wayland, Hyprland".into()),
            ..made_up()
        };
        let sheet = info.report();
        let listed: Vec<&str> = sheet.list.iter().map(|(line, _)| line.as_ref()).collect();
        assert_eq!(listed, info.text().lines().collect::<Vec<_>>());
        assert_eq!(listed.len(), 5);
        assert_eq!(sheet.fields.len(), 1);
        assert_eq!(sheet.fields[0].text.as_ref(), "");
        assert_eq!(sheet.on_return, Return::Action);
    }

    /// Linux's and Windows's install go in the form's field for that system, and Copy Build Info
    /// copies them last (#2458).
    #[test]
    fn fills_the_install_field() {
        let info = BuildInfo {
            setup: Some("Flatpak, Wayland, Hyprland".into()),
            ..made_up()
        };
        let field = if cfg!(windows) { "windows" } else { "linux" };
        assert!(
            info.issue_url(NEW_ISSUE, "")
                .ends_with(&format!("&{field}=Flatpak%2C%20Wayland%2C%20Hyprland")),
        );
        assert!(
            info.text()
                .ends_with("\nApple M2 Pro\nFlatpak, Wayland, Hyprland")
        );
    }

    #[test]
    fn names_linuxs_install_session_and_desktop() {
        assert_eq!(
            linux_setup(true, "Wayland", Some("Hyprland")),
            "Flatpak, Wayland, Hyprland"
        );
        assert_eq!(
            linux_setup(false, "X11", Some("")),
            "built from source, X11"
        );
        assert_eq!(linux_setup(false, "X11", None), "built from source, X11");
    }

    #[test]
    fn names_what_installed_a_windows_copy() {
        assert_eq!(windows_install(Updater::Store), "the Microsoft Store");
        assert_eq!(windows_install(Updater::Own), "the installer or winget");
        assert_eq!(windows_install(Updater::None), "built from source");
    }

    #[test]
    fn escapes_what_a_query_cant_hold() {
        assert_eq!(escape("a&b=c/d é"), "a%26b%3Dc%2Fd%20%C3%A9");
    }

    /// The Linux engine's VERSION and Windows's newest manifest each name the engine's version; a
    /// file that isn't one, or an engine with neither, names none.
    #[test]
    fn reads_the_engines_version_beside_chrome() {
        let dir = std::env::temp_dir().join(format!("ilo-engine-version-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
        file("chrome.exe.manifest", "");
        assert_eq!(engine_beside(&dir), None);
        file("156.0.8078.47.manifest", "");
        assert_eq!(engine_beside(&dir).as_deref(), Some("156.0.8078.47"));
        // Older builds' manifests beside the newest, one of them after it as text.
        file("156.0.8078.9.manifest", "");
        file("156.0.8078.79.manifest", "");
        file("155.0.9999.99.manifest", "");
        assert_eq!(engine_beside(&dir).as_deref(), Some("156.0.8078.79"));
        file("VERSION", "156.0.8078.48\n");
        assert_eq!(engine_beside(&dir).as_deref(), Some("156.0.8078.48"));
        file("VERSION", "MAJOR=156\n");
        assert_eq!(engine_beside(&dir), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[gpui::test]
    fn reads_this_computer(cx: &mut gpui::TestAppContext) {
        let info = cx.update(|cx| BuildInfo::here(cx));
        assert!(info.version.starts_with(env!("CARGO_PKG_VERSION")));
        assert!(!info.system.is_empty());
        assert!(!info.chip.is_empty());
    }
}
