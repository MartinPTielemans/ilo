//! What Help › Copy Build Info copies and Help › Report a Problem… fills a new issue with: the
//! app's version and build, its engine's, the system's and the chip. Nothing leaves the computer
//! until the user submits the issue GitHub shows them.

use std::sync::LazyLock;

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
}

impl BuildInfo {
    /// This copy's, read from the system when asked.
    pub fn here() -> Self {
        BuildInfo {
            version: version(),
            engine: ENGINE_VERSION.clone(),
            system: system(),
            chip: chip(),
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
        lines.join("\n")
    }

    /// A new issue at `new_issue` on the bug report form, its version, Chromium and system fields
    /// filled with these (.github/ISSUE_TEMPLATE/bug_report.yml's ids).
    pub fn issue_url(&self, new_issue: &str) -> String {
        let mut fields = vec![("template", "bug_report.yml".to_owned())];
        fields.push(("version", self.version.clone()));
        fields.extend(self.engine.clone().map(|engine| ("chromium", engine)));
        fields.push(("os", format!("{}, {}", self.system, self.chip)));
        let query: Vec<String> = fields
            .iter()
            .map(|(name, value)| format!("{name}={}", escape(value)))
            .collect();
        format!("{new_issue}?{}", query.join("&"))
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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn system() -> String {
    std::env::consts::OS.to_owned()
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
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
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

    #[test]
    fn copies_no_engine_line_without_one() {
        let info = BuildInfo {
            engine: None,
            ..made_up()
        };
        assert!(!info.text().contains("Chromium"));
        assert!(!info.issue_url(NEW_ISSUE).contains("chromium="));
    }

    #[test]
    fn fills_the_bug_reports_fields() {
        let url = made_up().issue_url("https://github.com/o/r/issues/new");
        assert_eq!(
            url,
            "https://github.com/o/r/issues/new?template=bug_report.yml&version=0.1.0%20%284321%29\
             &chromium=156.0.7600.12&os=macOS%2015.1%2C%20Apple%20M2%20Pro"
        );
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

    #[test]
    fn reads_this_computer() {
        let info = BuildInfo::here();
        assert!(info.version.starts_with(env!("CARGO_PKG_VERSION")));
        assert!(!info.system.is_empty());
        assert!(!info.chip.is_empty());
    }
}
