//! The computer's translator for a page in another language (#671): the core says the language
//! each page declares and gathers its words, the platform's translator turns them into the user's
//! language with the models the system keeps, and the core shows them in the page. The words never
//! leave the computer (the owner, 2026-10-29). On the Mac it's Apple's Translation, from macOS 26
//! (platform/mac/translate.swift). Linux and Windows have none yet, so nothing's offered there.
//!
//! A check run on any of them sets BROWSER_TRANSLATE=stand-in with BROWSER_DRIVE: a stand-in
//! translator into "en" that marks each text "[en]", so the run proves the page's words go there
//! and back without a model.

use std::cell::OnceCell;

use super::*;
use browser::shell::Translation;

thread_local! {
    /// The read under way for each tab being translated, with the languages it's between. A tab
    /// gone back to its own words, or to another page, has none, and a read that comes back for
    /// it then is dropped.
    static READS: RefCell<HashMap<u64, Read>> = RefCell::default();
    static NEXT_READ: Cell<u64> = const { Cell::new(1) };
    /// The user's language, asked of the system once, as the URL field asks each time it draws.
    static TARGET: OnceCell<Option<String>> = const { OnceCell::new() };
    /// The languages the user reads, asked of the system once, as TARGET is.
    static PREFERRED: OnceCell<Vec<String>> = const { OnceCell::new() };
}

struct Read {
    request: u64,
    from: String,
    to: String,
}

fn stand_in() -> bool {
    std::env::var_os("BROWSER_DRIVE").is_some()
        && std::env::var("BROWSER_TRANSLATE").as_deref() == Ok("stand-in")
}

/// The language pages are translated into here, or none without a translator.
pub(crate) fn target() -> Option<String> {
    TARGET.with(|target| {
        target
            .get_or_init(|| match stand_in() {
                true => Some("en".into()),
                false => system_target(),
            })
            .clone()
    })
}

#[cfg(target_os = "macos")]
fn system_target() -> Option<String> {
    let mut out = [0 as c_char; 64];
    let found = unsafe { platform::host_translation_target(out.as_mut_ptr(), out.len()) };
    found.then(|| {
        unsafe { CStr::from_ptr(out.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    })
}

#[cfg(not(target_os = "macos"))]
fn system_target() -> Option<String> {
    None
}

/// Whether the user reads `language`, a language alone such as "es": one the system prefers, or
/// the one pages are translated into. The stand-in's are BROWSER_TRANSLATE_READS's, or "en".
pub(crate) fn reads(language: &str) -> bool {
    PREFERRED.with(|preferred| {
        let preferred = preferred.get_or_init(|| {
            let listed = match stand_in() {
                true => std::env::var("BROWSER_TRANSLATE_READS").unwrap_or_default(),
                false => system_preferred(),
            };
            (listed.split(','))
                .map(|code| code.trim().to_ascii_lowercase())
                .filter(|code| !code.is_empty())
                .chain(target())
                .collect()
        });
        preferred.iter().any(|code| code == language)
    })
}

#[cfg(target_os = "macos")]
fn system_preferred() -> String {
    let mut out = [0 as c_char; 512];
    match unsafe { platform::host_preferred_languages(out.as_mut_ptr(), out.len()) } {
        true => unsafe { CStr::from_ptr(out.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
        false => String::new(),
    }
}

#[cfg(not(target_os = "macos"))]
fn system_preferred() -> String {
    String::new()
}

/// Where the languages always translated are kept, a language alone on each line, beside View ›
/// Appearance's choice. They're the computer's, as its translator and its languages are, so every
/// profile and its private windows translate them.
fn always_path() -> String {
    format!("{}/Always Translated", crate::profile_dir())
}

/// Reads the languages always translated (`shell::AlwaysTranslated`).
pub(crate) fn read_always(cx: &mut App) {
    let text = std::fs::read_to_string(always_path()).unwrap_or_default();
    let languages = (text.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| SharedString::from(line.to_owned()))
        .collect();
    cx.set_global(shell::AlwaysTranslated(languages));
}

/// Keeps the languages always translated for the next launch.
pub(crate) fn save_always(languages: &[SharedString]) {
    let text: String = languages
        .iter()
        .map(|language| format!("{language}\n"))
        .collect();
    if let Err(error) = std::fs::write(always_path(), text) {
        log!("couldn't save the languages always translated: {error}");
    }
}

/// `language`'s name in the user's language, as the system has it.
pub(crate) fn language_name(language: &str) -> String {
    #[cfg(target_os = "macos")]
    if !stand_in() {
        let mut out = [0 as c_char; 128];
        let code = CString::new(language).unwrap_or_default();
        if unsafe { platform::host_language_name(code.as_ptr(), out.as_mut_ptr(), out.len()) } {
            return unsafe { CStr::from_ptr(out.as_ptr()) }
                .to_string_lossy()
                .into_owned();
        }
    }
    // The stand-in's, in English as its target is.
    let name = match language.split('-').next().unwrap_or_default() {
        "de" => "German",
        "en" => "English",
        "es" => "Spanish",
        "fr" => "French",
        "it" => "Italian",
        "ja" => "Japanese",
        "nl" => "Dutch",
        "pt" => "Portuguese",
        "zh" => "Chinese",
        _ => language,
    };
    name.to_owned()
}

/// Translates `tab`'s page from `from` to `to`: the core gathers its words (tab_translate_texts).
pub(crate) fn translate(tab: u64, from: &str, to: &str) {
    // Every window hears a page's language, and each that always translates it asks: one read is
    // enough.
    let asked = READS.with_borrow(|reads| {
        (reads.get(&tab)).is_some_and(|read| read.from == from && read.to == to)
    });
    if asked {
        return;
    }
    let request = NEXT_READ.replace(NEXT_READ.get() + 1);
    let read = Read {
        request,
        from: from.to_owned(),
        to: to.to_owned(),
    };
    READS.with_borrow_mut(|reads| reads.insert(tab, read));
    // check.sh reads this line.
    log!("browser: translating {tab} from {from} to {to}");
    unsafe { bc::bc_tab_translate_read(tab, request) };
}

/// Puts `tab`'s page's own words back, and drops a translation still under way.
pub(crate) fn use_original(tab: u64) {
    READS.with_borrow_mut(|reads| reads.remove(&tab));
    // check.sh reads this line.
    log!("browser: original {tab}");
    unsafe { bc::bc_tab_translate_revert(tab) };
}

/// Opens System Settings at Language & Region, where Translation Languages are added.
pub(crate) fn open_settings() {
    #[cfg(target_os = "macos")]
    unsafe {
        platform::host_open_translation_settings()
    };
}

fn finish(tab: u64, translation: Translation) {
    // check.sh reads this line.
    log!(
        "browser: translation {tab} {}",
        format!("{translation:?}").to_lowercase()
    );
    update_shells(|shell, cx| shell.set_translation(tab, translation, cx));
}

pub(crate) unsafe extern "C" fn tab_language(
    _: *mut c_void,
    tab: bc::bc_id,
    language: *const c_char,
) {
    let language = match language.is_null() {
        true => String::new(),
        false => unsafe { CStr::from_ptr(language) }
            .to_string_lossy()
            .into_owned(),
    };
    // A new page has none of a translation under way on the last.
    READS.with_borrow_mut(|reads| reads.remove(&tab));
    // check.sh reads this line.
    log!("browser: language {tab} {language}");
    update_shells(|shell, cx| shell.set_page_language(tab, &language, cx));
}

pub(crate) unsafe extern "C" fn tab_translate_texts(
    _: *mut c_void,
    tab: bc::bc_id,
    request: u64,
    texts: *const c_char,
) {
    let Some((from, to)) = READS.with_borrow(|reads| {
        (reads.get(&tab))
            .filter(|read| read.request == request)
            .map(|read| (read.from.clone(), read.to.clone()))
    }) else {
        return;
    };
    if texts.is_null() {
        READS.with_borrow_mut(|reads| reads.remove(&tab));
        return finish(tab, Translation::Failed);
    }
    let texts = unsafe { CStr::from_ptr(texts) }
        .to_string_lossy()
        .into_owned();
    translate_texts(
        &from,
        &to,
        texts,
        Box::new(move |translated| {
            // Shown only while it's still wanted: not once Use Original or another page came.
            let wanted = READS.with_borrow_mut(|reads| {
                let wanted = reads.get(&tab).is_some_and(|read| read.request == request);
                if wanted {
                    reads.remove(&tab);
                }
                wanted
            });
            if !wanted {
                return;
            }
            match translated {
                Ok(json) => {
                    let json = CString::new(json).unwrap_or_default();
                    unsafe { bc::bc_tab_translate_show(tab, request, json.as_ptr()) };
                    finish(tab, Translation::Translated);
                }
                Err(translation) => finish(tab, translation),
            }
        }),
    );
}

type Done = Box<dyn FnOnce(Result<String, Translation>)>;

/// Translates `texts`, a JSON array of strings, into another of as many, or says why it didn't.
fn translate_texts(from: &str, to: &str, texts: String, done: Done) {
    if stand_in() {
        let texts: Vec<String> = serde_json::from_str(&texts).unwrap_or_default();
        let marked: Vec<String> = (texts.into_iter())
            .map(|text| match text.is_empty() {
                true => text,
                false => format!("[{to}] {text}"),
            })
            .collect();
        return done(serde_json::to_string(&marked).map_err(|_| Translation::Failed));
    }
    #[cfg(target_os = "macos")]
    {
        let (from, to) = (
            CString::new(from).unwrap_or_default(),
            CString::new(to).unwrap_or_default(),
        );
        let texts = CString::new(texts).unwrap_or_default();
        let context = Box::into_raw(Box::new(done)).cast::<c_void>();
        unsafe {
            platform::host_translate(
                from.as_ptr(),
                to.as_ptr(),
                texts.as_ptr(),
                context,
                translated,
            )
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (from, to, texts);
        done(Err(Translation::Failed));
    }
}

/// What the Mac's translator says, on the main thread: 0 with the translations, as JSON; 1 for a
/// language whose model isn't on the Mac; anything else for one it didn't translate.
#[cfg(target_os = "macos")]
extern "C" fn translated(context: *mut c_void, status: c_int, texts: *const c_char) {
    let done = unsafe { Box::from_raw(context.cast::<Done>()) };
    let result = match status {
        0 if !texts.is_null() => Ok(unsafe { CStr::from_ptr(texts) }
            .to_string_lossy()
            .into_owned()),
        1 => Err(Translation::Missing),
        _ => Err(Translation::Failed),
    };
    done(result);
}
