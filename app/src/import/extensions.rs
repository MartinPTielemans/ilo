//! A Chromium profile's extensions, which the import lists with their Chrome Web Store pages and
//! never installs (docs/ux-spec.md › Import from other browsers): their names from the manifests
//! in its `Extensions` folder, and which were pinned from its `Preferences`. Another ad blocker
//! stays off the list, as the browser blocks ads itself, until the user asks for it. One the user
//! adds from the list that was pinned there is pinned here once it's in (owner, 2026-10-29).

use std::cell::RefCell;
use std::path::Path;

use serde_json::Value;

/// Where a Web Store extension's manifest says its updates come from.
const WEB_STORE_UPDATES: &str = "https://clients2.google.com/service/update2/crx";

/// The Web Store's ad blockers most people use, by id: uBlock Origin and its Lite, AdBlock,
/// Adblock Plus, AdGuard and Ghostery. Others are known by their names (Extension::blocks_ads).
const AD_BLOCKERS: [&str; 6] = [
    "cjpalhdlnbpafiamejdnhcphjbkeiagm",
    "ddkjiahejlhfcafbddmgiahcphecmpfh",
    "gighmmpiobklfepjocnamgkkbiglidom",
    "cfhdojbkjhnklbpkdaibdccddilifddb",
    "bgnkhhnnamicmpeenaelnjfhikgbkllg",
    "mlomiejdfkolichcflejclcbmpeaniij",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Extension {
    /// Chrome's id for it, the same in the Web Store and here.
    pub id: String,
    pub name: String,
    /// It was pinned to the other browser's toolbar.
    pub pinned: bool,
}

impl Extension {
    pub fn store_page(&self) -> String {
        format!("https://chromewebstore.google.com/detail/{}", self.id)
    }

    /// Whether it blocks ads, which the browser does itself with uBlock Origin built in.
    pub fn blocks_ads(&self) -> bool {
        let name = self.name.to_lowercase();
        AD_BLOCKERS.contains(&self.id.as_str())
            || ["adblock", "ad block", "ublock", "adguard"]
                .iter()
                .any(|word| name.contains(word))
    }
}

/// The Web Store extensions the profile at `path` has, by name, but for themes and apps, which
/// don't come here.
pub fn read(path: &Path) -> Vec<Extension> {
    let pinned = pinned(path);
    let Ok(folders) = std::fs::read_dir(path.join("Extensions")) else {
        return Vec::new();
    };
    let mut extensions: Vec<Extension> = folders
        .flatten()
        .filter_map(|folder| {
            let id = folder.file_name().to_string_lossy().into_owned();
            let name = name(&folder.path())?;
            let pinned = pinned.contains(&id);
            Some(Extension { id, name, pinned })
        })
        .collect();
    extensions.sort_by_key(|extension| extension.name.to_lowercase());
    extensions
}

/// The ids of the extensions pinned to the toolbar, from the profile's `Preferences`.
fn pinned(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path.join("Preferences")) else {
        return Vec::new();
    };
    let Ok(preferences) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let ids = preferences
        .pointer("/extensions/pinned_extensions")
        .and_then(Value::as_array);
    ids.into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// The name of the Web Store extension in `folder`, from its newest version's manifest, in its
/// default language; none for one that isn't the Web Store's, a theme or an app.
fn name(folder: &Path) -> Option<String> {
    let mut versions: Vec<_> = std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .map(|version| version.path())
        .filter(|version| version.join("manifest.json").is_file())
        .collect();
    versions.sort();
    let version = versions.pop()?;
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(version.join("manifest.json")).ok()?).ok()?;
    let text = |key: &str| manifest.get(key).and_then(Value::as_str);
    let from_store = text("update_url") == Some(WEB_STORE_UPDATES);
    if !from_store || manifest.get("theme").is_some() || manifest.get("app").is_some() {
        return None;
    }
    let name = text("name")?.trim();
    let Some(key) = name
        .strip_prefix("__MSG_")
        .and_then(|key| key.strip_suffix("__"))
    else {
        return Some(name.to_string());
    };
    // A name the manifest leaves to its messages, which Chrome looks up ignoring case.
    let locale = text("default_locale").unwrap_or("en");
    let messages = version.join("_locales").join(locale).join("messages.json");
    let messages: Value = serde_json::from_str(&std::fs::read_to_string(messages).ok()?).ok()?;
    let message = messages
        .as_object()?
        .iter()
        .find(|(each, _)| each.eq_ignore_ascii_case(key));
    let name = message?.1.get("message").and_then(Value::as_str)?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// The extensions the last import listed, from the browser named `from`, for the import view to
/// show wherever it's drawn, in Settings or the first launch, until each is added or the app quits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Listed {
    pub from: String,
    pub extensions: Vec<Extension>,
}

thread_local! {
    static LISTED: RefCell<Listed> = RefCell::default();
    /// The extensions added from the list that were pinned in the other browser, to pin once
    /// they're in.
    static TO_PIN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Lists `extensions` from `from`, in place of what an import listed before.
pub fn list(from: &str, extensions: Vec<Extension>) {
    LISTED.set(Listed {
        from: from.into(),
        extensions,
    });
}

pub fn listed() -> Listed {
    LISTED.with_borrow(Listed::clone)
}

/// Takes the extension `id` off the list, as the user adds it, and returns it. One that was
/// pinned is pinned once it's in (to_pin).
pub fn take(id: &str) -> Option<Extension> {
    let at =
        LISTED.with_borrow(|listed| listed.extensions.iter().position(|each| each.id == id))?;
    let extension = LISTED.with_borrow_mut(|listed| listed.extensions.remove(at));
    if extension.pinned {
        TO_PIN.with_borrow_mut(|to_pin| to_pin.push(extension.id.clone()));
    }
    Some(extension)
}

/// Of a profile's extensions `now`, each with whether it's pinned, those that weren't among `had`,
/// were pinned in the other browser and aren't pinned here yet: each comes once, to pin.
pub fn to_pin<'a>(had: &[&str], now: impl IntoIterator<Item = (&'a str, bool)>) -> Vec<String> {
    let added: Vec<(&str, bool)> = now
        .into_iter()
        .filter(|(id, _)| !had.contains(id))
        .collect();
    TO_PIN.with_borrow_mut(|to_pin| {
        let mut pin = Vec::new();
        to_pin.retain(|id| match added.iter().find(|(each, _)| each == id) {
            Some((_, pinned)) => {
                if !pinned {
                    pin.push(id.clone());
                }
                false
            }
            None => true,
        });
        pin
    })
}
