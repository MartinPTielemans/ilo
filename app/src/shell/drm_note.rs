//! A note that a page needs protected-media playback Chromium doesn't have, such as Netflix's video:
//! a card under the URL field, where a page's question shows, until the tab goes to another site.
//! Safari's engine plays these sites with Apple's DRM, so its one button shows the tab's page there,
//! in the tab, and the profile keeps the site for it: its pages open there from then on, with no
//! note, until the Site Controls turn it off (decision 1). The site's session goes with the tab, so
//! the user stays signed in. Where the Mac has no engine to give, on macOS 13, and in an agent's
//! Space, whose tabs never switch, it opens the page in Safari instead and leaves the tab as it is.
//! On Windows, while ilo isn't the system's default browser, it offers that browser instead, and on
//! Linux nothing (Elsewhere::on).
//! Its ×, shown under the pointer, and escape while the UI has the keyboard close it for the site in
//! the profile, across launches, or for a private window's session: the core no longer reports it
//! there, and the Site Controls offer Safari's engine for the site instead, the way back to its
//! button. The site's own error stays in the page. Agents get no path: it hands the user's page to
//! another engine or app. The core reports it (tab_media_unsupported) and keeps the sites it was
//! closed for (tab_close_media_note).
//!
//! On Linux a Widevine page's note offers Google's Widevine module instead, named as Google's and
//! downloaded from Google only at the button's yes; Linux loads it at launch, so the note then
//! offers a restart, which brings the page back. Its × is the no, and Settings › Advanced keeps the
//! way back. In an agent's Space it says why and offers nothing, as the yes is the user's alone.
//!
//! On Windows, Chromium plays sites that offer Microsoft's PlayReady where the PC can, so the note
//! shows only where it can't, and says why: it needs Windows 11, an x64 PC and graphics with
//! hardware DRM, outside private windows (bc_playready_needs). It offers nothing, as there's no
//! other engine to give.
//!
//! In Safari's engine the note says why a protected video shows black when the display connection's
//! protection (HDCP) falls short, as a dock's can, which the site never says. A statement, with
//! nothing to offer: the fix is the user's cable. Its × and escape close it for the tab, until it
//! goes to another site, as the cause is the desk's and not the site's.

use gpui::{
    App, Context, FontWeight, KeyBinding, MouseButton, SharedString, actions, div, prelude::*, px,
};

use super::{
    KEY_CONTEXT, Shell, TITLE,
    permission::{BESIDE_CLOSE, BUTTONS_TOP, card_button_wrapping, card_close, card_listener},
};
use crate::default_browser::Os;
use crate::tooltip::{Anchor, Tip};
use crate::widevine::Widevine;

/// Ours, as neither Arc nor Dia has protected media to explain: short and plain. It says the video
/// plays here, in this tab. There's no sign-in to warn of, as the core brings the site's cookies and
/// localStorage over with the tab.
const SWITCH_WORDS: &str = "This site's video needs Safari's engine. Play it in this tab.";
const SWITCH: &str = "Play in This Tab";
const WORDS: &str = concat!(
    "This site's video needs a module ",
    env!("APP_NAME"),
    " doesn't have yet."
);
const PLAYREADY_WINDOWS_11: &str = concat!(
    "This site's video needs Windows 11, where ",
    env!("APP_NAME"),
    " plays it."
);
const PLAYREADY_X64: &str = concat!(
    "This site's video needs an Intel or AMD PC. ",
    env!("APP_NAME"),
    " can't play it on ARM."
);
const PLAYREADY_HARDWARE: &str =
    "This site's video needs hardware DRM, which this PC's graphics don't offer.";
const PLAYREADY_PRIVATE: &str = "This site's video doesn't play in a private window.";
const OPEN_SAFARI: &str = "Open in Safari";
/// A private window's page leaves it for the other browser's own window, which keeps history.
const OPEN_SAFARI_PRIVATE: &str = "Open in Safari, not private";
/// Named for what it does, as the user picked the browser and ilo needn't (#2468).
const OPEN_DEFAULT: &str = "Open in Default Browser";
const OPEN_DEFAULT_PRIVATE: &str = "Open in Default Browser, not private";
/// Ours, naming the module as Google's, as the user downloads a proprietary file from Google.
const WIDEVINE: &str = "com.widevine.alpha";
const WIDEVINE_WORDS: &str = concat!(
    "This site's video needs Google's Widevine module, which ",
    env!("APP_NAME"),
    " can download from Google."
);
const WIDEVINE_DOWNLOAD: &str = "Download from Google";
const WIDEVINE_FAILED_WORDS: &str = "Google's Widevine module didn't download.";
const WIDEVINE_RETRY: &str = "Try Again";
const WIDEVINE_DOWNLOADING_WORDS: &str = "Downloading Google's Widevine module…";
const WIDEVINE_RESTART_WORDS: &str = concat!(
    "Google's Widevine module is ready. Restart ",
    env!("APP_NAME"),
    " to play this video."
);
const WIDEVINE_RESTART: &str = crate::widevine::RESTART;
/// Ours: what the × does, as there's no second control for it.
const CLOSE_TIP: &str = "Don't Show for This Site";
/// What the core reports for a page in Safari's engine whose protected video is kept off the
/// screen, EME's own word for it.
const OUTPUT_RESTRICTED: &str = "output-restricted";
/// Ours, as the issue put it: what's wrong and the likely cause, without HDCP's name.
const OUTPUT_WORDS: &str =
    "This video needs a protected display connection. A dock or adapter may be limiting it.";
const OUTPUT_CLOSE_TIP: &str = "Close";
const CLOSE_KEY: &str = "drm-note-close";

actions!(drm_note, [CloseNote]);

/// The other browser the note offers for video ilo can't play (#2468).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elsewhere {
    Safari,
    DefaultBrowser,
}

impl Elsewhere {
    /// Safari on the Mac, which plays these sites with Apple's DRM. On Windows the default browser
    /// while it isn't ilo, as Edge and Chrome play what ilo can't on Windows 10, on ARM and without
    /// the graphics' hardware DRM. On Linux none: there a Widevine page's note offers Google's
    /// module, no Linux browser has the other DRMs, and a Flatpak can't tell whether it's the
    /// default, so the page could come back to ilo.
    pub fn on(os: Os, is_default: impl FnOnce() -> bool) -> Option<Self> {
        match os {
            Os::Mac => Some(Self::Safari),
            Os::Windows => (!is_default()).then_some(Self::DefaultBrowser),
            Os::Linux | Os::Flatpak => None,
        }
    }

    fn label(self, private: bool) -> &'static str {
        match (self, private) {
            (Self::Safari, false) => OPEN_SAFARI,
            (Self::Safari, true) => OPEN_SAFARI_PRIVATE,
            (Self::DefaultBrowser, false) => OPEN_DEFAULT,
            (Self::DefaultBrowser, true) => OPEN_DEFAULT_PRIVATE,
        }
    }
}

/// What the note's one button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Offer {
    /// Shows the page in Safari's engine, in the tab.
    Switch,
    DownloadWidevine,
    RestartForWidevine,
    /// Opens the page in the other browser, leaving the tab as it is.
    Elsewhere,
}

impl Offer {
    fn id(self) -> &'static str {
        match self {
            Offer::Switch => "drm-note-switch",
            Offer::DownloadWidevine => "drm-note-widevine",
            Offer::RestartForWidevine => "drm-note-restart",
            Offer::Elsewhere => "drm-note-elsewhere",
        }
    }

    fn take(self, shell: &mut Shell, cx: &mut Context<Shell>) {
        match self {
            Offer::Switch => shell.use_system_engine(cx),
            Offer::DownloadWidevine => shell.download_widevine(cx),
            Offer::RestartForWidevine => shell.restart_for_widevine(cx),
            Offer::Elsewhere => shell.open_elsewhere(cx),
        }
    }
}

/// What Windows' PlayReady needs that the PC may lack, as the core says (bc_playready_needs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayReady {
    /// Not Windows.
    #[default]
    None,
    Windows11,
    X64,
    /// Windows 11 on x64: the graphics' hardware DRM.
    Hardware,
}

impl PlayReady {
    pub fn from_core(needs: i32) -> Self {
        match u32::try_from(needs).unwrap_or(crate::bc::BC_PLAYREADY_NONE) {
            crate::bc::BC_PLAYREADY_WINDOWS_11 => Self::Windows11,
            crate::bc::BC_PLAYREADY_X64 => Self::X64,
            crate::bc::BC_PLAYREADY_HARDWARE => Self::Hardware,
            _ => Self::None,
        }
    }
}

/// Escape while the UI has the keyboard, over Stop's, which it falls through to unless it closes
/// the note. The page keeps its own escape, which reaches Stop if it hands it back.
pub(super) fn escape_binding() -> KeyBinding {
    KeyBinding::new("escape", CloseNote, Some(KEY_CONTEXT))
}

pub(super) fn bind(cx: &mut App) {
    cx.bind_keys([escape_binding()]);
    cx.on_action(|_: &CloseNote, cx| {
        let shell = super::commands::key_shell(cx);
        let closed = shell.and_then(|shell| shell.update(cx, Shell::escape_drm_note).ok());
        if !closed.unwrap_or(false) {
            cx.propagate();
        }
    });
}

impl Shell {
    /// The core reports the protected-media key system `tab`'s page asked for that ilo can't play,
    /// that its page in Safari's engine has its protected video kept off the screen
    /// (`output-restricted`), or that there's no note any more (empty).
    pub fn set_media_unsupported(&mut self, tab: u64, key_system: &str, cx: &mut Context<Self>) {
        let changed = match key_system.is_empty() {
            true => self.drm_notes.remove(&tab).is_some(),
            false => {
                self.drm_notes
                    .insert(tab, key_system.to_owned().into())
                    .as_deref()
                    != Some(key_system)
            }
        };
        if changed {
            cx.notify();
        }
    }

    /// The key system behind the note of the tab shown, if its page needs one. A page's question
    /// takes the note's place until it's answered, as an agent's does, as all hang from the same
    /// place. Each note is its engine's: Chromium's until the tab is in Safari's, which has only the
    /// output's.
    pub(super) fn drm_note(&self) -> Option<&SharedString> {
        let playing = self.page(self.selected).system_engine;
        let asked = self.page_asks() || self.agent_question.is_some();
        self.drm_notes
            .get(&self.selected)
            .filter(|key_system| (key_system.as_ref() == OUTPUT_RESTRICTED) == playing && !asked)
    }

    /// Whether the note of the tab shown is the output's, which offers nothing.
    fn output_note(&self) -> bool {
        self.drm_note()
            .is_some_and(|key_system| key_system.as_ref() == OUTPUT_RESTRICTED)
    }

    /// Closes the note of the tab shown for its site, which the core keeps from asking again. It
    /// goes at once, before the core takes it back from the site's other tabs. The output's goes
    /// for the tab alone, which the core tells once a site.
    fn close_drm_note(&mut self, cx: &mut Context<Self>) {
        if self.drm_note().is_none() {
            return;
        }
        let tab = self.selected;
        // check.sh reads this line.
        log!("browser: drm note {tab} closed");
        if !self.output_note() {
            self.browser.close_media_note(tab, cx);
        }
        self.drm_notes.remove(&tab);
        cx.notify();
    }

    /// The pointer has come onto the note's × or left it: after a moment it says what it does,
    /// hung under it as a header button's tooltip is.
    fn hover_drm_close(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let busy = self.dragging().is_some() || self.menu_row.is_some();
        let frame = self.button_frames.borrow().get(CLOSE_KEY).copied();
        let name = if self.output_note() {
            OUTPUT_CLOSE_TIP
        } else {
            CLOSE_TIP
        };
        let tip = frame
            .filter(|_| hovered && !busy)
            .map(|frame| (Tip::named(name.into()), Anchor::under(frame, self.viewport)));
        self.hover_tip(tip, cx);
    }

    /// Escape closes the note when it's what the sidebar shows in front, and says whether it did.
    fn escape_drm_note(&mut self, cx: &mut Context<Self>) -> bool {
        let shown = !self.sidebar_hidden || self.revealed;
        let in_front = shown && self.library.is_none() && self.form().is_none();
        if !in_front || self.drm_note().is_none() {
            return false;
        }
        self.close_drm_note(cx);
        true
    }

    /// Whether the note's button shows the page in Safari's engine, in the tab: where the Mac has
    /// one.
    fn can_switch(&self) -> bool {
        self.browser.can_use_system_engine()
    }

    /// Shows the page of the tab shown in Safari's engine. The core takes the note away.
    fn use_system_engine(&mut self, cx: &mut Context<Self>) {
        if self.drm_note().is_none() || self.output_note() || !self.can_switch() {
            return;
        }
        self.browser.use_system_engine(self.selected, cx);
    }

    /// Where Google's Widevine module is, if the note of the tab shown offers it: for a Widevine
    /// page, where the core offers the module.
    fn widevine_offer(&self) -> Option<Widevine> {
        let state = self.browser.widevine();
        let offered = state != Widevine::None;
        let widevine = |key_system: &&SharedString| offered && key_system.as_ref() == WIDEVINE;
        self.drm_note().filter(widevine).map(|_| state)
    }

    /// The user's yes to Google's Widevine module, from the note of the tab shown.
    fn download_widevine(&mut self, cx: &mut Context<Self>) {
        if !self.widevine_offer().is_some_and(Widevine::downloadable) {
            return;
        }
        // check/widevine.sh reads this line.
        log!("browser: widevine download");
        self.browser.download_widevine(cx);
    }

    /// Restarts the browser, which loads the module downloaded, from the note of the tab shown.
    fn restart_for_widevine(&mut self, cx: &mut Context<Self>) {
        if self.widevine_offer() != Some(Widevine::Restart) {
            return;
        }
        log!("browser: widevine restart");
        self.browser.restart(cx);
    }

    /// The note's words and button, if it has one, for Google's Widevine module in `state`.
    fn widevine_note(state: Widevine) -> (&'static str, Option<(Offer, &'static str)>) {
        let download = |label| Some((Offer::DownloadWidevine, label));
        match state {
            Widevine::Absent => (WIDEVINE_WORDS, download(WIDEVINE_DOWNLOAD)),
            Widevine::Failed => (WIDEVINE_FAILED_WORDS, download(WIDEVINE_RETRY)),
            Widevine::Downloading => (WIDEVINE_DOWNLOADING_WORDS, None),
            Widevine::Restart => (
                WIDEVINE_RESTART_WORDS,
                Some((Offer::RestartForWidevine, WIDEVINE_RESTART)),
            ),
            // Loaded, the core reports no note; one shown after Remove plays until a restart.
            Widevine::Ready | Widevine::None => (WORDS, None),
        }
    }

    /// What the note says where it has no engine to give: on Windows, what PlayReady needs.
    fn unplayable_words(&self) -> &'static str {
        match self.browser.playready_needs() {
            PlayReady::None => WORDS,
            PlayReady::Windows11 => PLAYREADY_WINDOWS_11,
            PlayReady::X64 => PLAYREADY_X64,
            // A private window keeps no licence, so it's the one place a capable PC can't play.
            PlayReady::Hardware if self.private => PLAYREADY_PRIVATE,
            PlayReady::Hardware => PLAYREADY_HARDWARE,
        }
    }

    /// Opens the page of the tab shown in the other browser, where there's one. The tab keeps its
    /// page, and its note.
    fn open_elsewhere(&mut self, cx: &mut Context<Self>) {
        if self.drm_note().is_none() || self.output_note() || self.browser.elsewhere().is_none() {
            return;
        }
        let url = self.page(self.selected).url;
        self.browser.open_elsewhere(&url, cx);
    }

    /// What the note of the tab shown says, and its button's offer and words, if it has one. Where
    /// there's nothing to offer, it says why.
    fn note_offer(&self) -> (&'static str, Option<(Offer, &'static str)>) {
        if self.output_note() {
            (OUTPUT_WORDS, None)
        } else if let Some(state) = self.widevine_offer() {
            Self::widevine_note(state)
        } else if self.can_switch() {
            (SWITCH_WORDS, Some((Offer::Switch, SWITCH)))
        } else {
            let open = self.browser.elsewhere();
            let open = open.map(|elsewhere| (Offer::Elsewhere, elsewhere.label(self.private)));
            (self.unplayable_words(), open)
        }
    }

    /// The note of the tab shown, as a card under the URL field.
    pub(super) fn drm_note_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        if self.drm_note().is_none() {
            // Drawn again, it's logged again.
            if self.drm_drawn.take().is_some() {
                // check.sh waits for this line.
                log!("browser: drm note hidden");
            }
            return None;
        }
        let (words, offer) = self.note_offer();
        let button = offer.map(|(offer, label)| {
            card_button_wrapping(&self.palette, offer.id(), label)
                .on_click(cx.listener(move |shell, _, _, cx| offer.take(shell, cx)))
        });
        // Shown under the pointer, as a row's ×, and always to VoiceOver's cursor, which is no
        // pointer. It takes clicks either way, as the pointer is over the card to click it.
        let close = card_close(&self.palette, CLOSE_KEY)
            .when(!self.a11y, |close| {
                close
                    .opacity(0.)
                    .group_hover("drm-note", |style| style.opacity(1.))
            })
            .child(self.button_frame(CLOSE_KEY))
            .on_hover(
                cx.listener(|shell, hovered: &bool, _, cx| shell.hover_drm_close(*hovered, cx)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, cx| shell.browser.tooltip(None, cx)),
            )
            .on_click(cx.listener(|shell, _, _, cx| shell.close_drm_note(cx)));
        Some(
            self.plate("drm-note")
                .group("drm-note")
                .child(card_listener("drm note", &self.drm_drawn, self.selected))
                .child(
                    div()
                        .pr(px(BESIDE_CLOSE))
                        .text_color(self.palette.text(TITLE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(words),
                )
                .children(button.map(|button| div().mt(px(BUTTONS_TOP)).flex().child(button)))
                .child(close),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{App, AppContext, Modifiers, TestAppContext};

    use super::{
        CLOSE_KEY, OPEN_DEFAULT, OPEN_DEFAULT_PRIVATE, OPEN_SAFARI, OUTPUT_RESTRICTED, Offer,
        PLAYREADY_PRIVATE, PLAYREADY_WINDOWS_11, PlayReady, SWITCH, WIDEVINE_DOWNLOAD,
        WIDEVINE_WORDS, WORDS,
    };

    use crate::default_browser::Os;

    use crate::widevine::Widevine;

    use crate::shell::{Folders, Page, Question, Shell, bind_keys, recorder::Recorder};

    fn window_with_tabs(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = (
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            );
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(
                1,
                Vec::new(),
                vec![10, 11],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.set_space(1, cx);
            let page = |url: &str| Page {
                url: url.to_owned().into(),
                ..Page::default()
            };
            shell.set_page(10, page("https://www.netflix.com/browse"), cx);
            shell.set_page(11, page("https://example.com/"), cx);
            shell.set_selected(10, cx);
        });
        (shell, host)
    }

    #[gpui::test]
    fn the_note_shows_for_the_tab_reported_and_only_that_tab(cx: &mut TestAppContext) {
        let (shell, _) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            assert!(shell.drm_note().is_none());
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            let noted = shell.drm_note().map(|key_system| key_system.to_string());
            assert_eq!(noted.as_deref(), Some("com.widevine.alpha"));
            // Another tab shown has none, and the first has it again once it's back.
            shell.set_selected(11, cx);
            assert!(shell.drm_note().is_none());
            shell.set_selected(10, cx);
            assert!(shell.drm_note().is_some());
        });
    }

    #[gpui::test]
    fn the_note_goes_when_the_core_clears_it(cx: &mut TestAppContext) {
        let (shell, _) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            // Clearing a tab that has none changes nothing.
            shell.set_media_unsupported(11, "", cx);
            assert!(shell.drm_note().is_some());
            shell.set_media_unsupported(10, "", cx);
            assert!(shell.drm_note().is_none());
        });
    }

    #[gpui::test]
    fn a_page_question_takes_the_notes_place_until_answered(cx: &mut TestAppContext) {
        let (shell, _) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            let question = Question {
                site: "netflix.com".into(),
                permissions: Vec::new(),
            };
            shell.set_question(10, Some(question), cx);
            assert!(shell.drm_note().is_none());
            shell.set_question(10, None, cx);
            assert!(shell.drm_note().is_some());
        });
    }

    #[gpui::test]
    fn the_button_shows_the_tabs_page_in_safaris_engine_where_the_mac_has_it(
        cx: &mut TestAppContext,
    ) {
        let (shell, host) = window_with_tabs(cx);
        host.system_engine.set(true);
        shell.update(cx, |shell, cx| {
            // Without a note, there's nothing to switch.
            shell.use_system_engine(cx);
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            host.take();
            shell.use_system_engine(cx);
        });
        assert_eq!(host.take(), ["system engine 10"]);
    }

    #[gpui::test]
    fn a_tab_in_safaris_engine_has_no_note(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        host.system_engine.set(true);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            let page = Page {
                system_engine: true,
                ..shell.page(10)
            };
            shell.set_page(10, page, cx);
            assert!(shell.drm_note().is_none());
        });
    }

    #[gpui::test]
    fn without_safaris_engine_the_button_switches_nothing(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            host.take();
            shell.use_system_engine(cx);
        });
        assert!(host.take().is_empty());
    }

    /// The card's words and its button's offer and words, for the tab shown.
    fn offered(
        shell: &gpui::Entity<Shell>,
        cx: &mut TestAppContext,
    ) -> (&'static str, Option<(Offer, &'static str)>) {
        shell.read_with(cx, |shell, _| shell.note_offer())
    }

    /// Takes what the card's button offers, as its click does, and returns what the host was asked.
    fn take_offer(
        shell: &gpui::Entity<Shell>,
        host: &Recorder,
        cx: &mut TestAppContext,
    ) -> Vec<String> {
        host.take();
        let (_, offer) = offered(shell, cx);
        let (offer, _) = offer.expect("the card offers nothing");
        shell.update(cx, |shell, cx| offer.take(shell, cx));
        host.take()
    }

    /// The Mac, where Safari's engine isn't there to give, as on macOS 13: the card offers Safari,
    /// which opens the tab's page, and the tab keeps its page and its note.
    #[gpui::test]
    fn on_the_mac_the_card_opens_the_page_in_safari(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        host.os.set(Some(Os::Mac));
        shell.update(cx, |shell, cx| {
            // Without a note, there's nothing to open.
            shell.open_elsewhere(cx);
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
        });
        assert_eq!(
            offered(&shell, cx),
            (WORDS, Some((Offer::Elsewhere, OPEN_SAFARI)))
        );
        assert_eq!(
            take_offer(&shell, &host, cx),
            ["open elsewhere https://www.netflix.com/browse"]
        );
        shell.update(cx, |shell, _| assert!(shell.drm_note().is_some()));
        // Where Safari's engine is, the card plays the page in the tab instead.
        host.system_engine.set(true);
        assert_eq!(offered(&shell, cx).1, Some((Offer::Switch, SWITCH)));
        assert_eq!(take_offer(&shell, &host, cx), ["system engine 10"]);
    }

    /// Windows, while ilo isn't the default browser: the card offers that browser, named for what
    /// it does, which opens the tab's page; a private window's says it won't be private. Once ilo is
    /// the default, it offers nothing, as the page would come back. It never names Safari.
    #[gpui::test]
    fn on_windows_the_card_opens_the_page_in_the_default_browser_unless_thats_ilo(
        cx: &mut TestAppContext,
    ) {
        let (shell, host) = window_with_tabs(cx);
        host.os.set(Some(Os::Windows));
        host.default_browser.set(false);
        host.system_engine.set(false);
        host.playready.set(PlayReady::Windows11);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.microsoft.playready", cx)
        });
        assert_eq!(
            offered(&shell, cx),
            (PLAYREADY_WINDOWS_11, Some((Offer::Elsewhere, OPEN_DEFAULT)))
        );
        assert_eq!(
            take_offer(&shell, &host, cx),
            ["open elsewhere https://www.netflix.com/browse"]
        );
        shell.update(cx, |shell, _| shell.private = true);
        assert_eq!(
            offered(&shell, cx).1,
            Some((Offer::Elsewhere, OPEN_DEFAULT_PRIVATE))
        );
        host.default_browser.set(true);
        assert_eq!(offered(&shell, cx), (PLAYREADY_WINDOWS_11, None));
        host.take();
        shell.update(cx, |shell, cx| shell.open_elsewhere(cx));
        assert!(host.take().is_empty(), "the hidden button still opened");
        let (words, _) = offered(&shell, cx);
        assert!(!words.contains("Safari"));
    }

    /// Linux, the Flatpak's and a native install's, default browser or not: a Widevine page's card
    /// offers Google's module, which downloads it, and any other's offers nothing, never Safari.
    #[gpui::test]
    fn on_linux_the_card_offers_the_module_or_nothing(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        host.system_engine.set(false);
        for os in [Os::Linux, Os::Flatpak] {
            host.os.set(Some(os));
            for default in [false, true] {
                host.default_browser.set(default);
                host.widevine.set(Widevine::Absent);
                shell.update(cx, |shell, cx| {
                    shell.set_media_unsupported(10, "com.widevine.alpha", cx)
                });
                assert_eq!(
                    offered(&shell, cx),
                    (
                        WIDEVINE_WORDS,
                        Some((Offer::DownloadWidevine, WIDEVINE_DOWNLOAD))
                    )
                );
                assert_eq!(take_offer(&shell, &host, cx), ["download widevine"]);
                shell.update(cx, |shell, cx| {
                    shell.set_media_unsupported(10, "com.microsoft.playready", cx)
                });
                assert_eq!(
                    offered(&shell, cx),
                    (WORDS, None),
                    "{os:?}, default {default}"
                );
                host.widevine.set(Widevine::None);
                shell.update(cx, |shell, cx| {
                    shell.set_media_unsupported(10, "com.widevine.alpha", cx)
                });
                assert_eq!(
                    offered(&shell, cx),
                    (WORDS, None),
                    "{os:?}, default {default}"
                );
                host.take();
                shell.update(cx, |shell, cx| shell.open_elsewhere(cx));
                assert!(host.take().is_empty(), "{os:?} opened another browser");
            }
        }
    }

    /// Closing asks the core to keep the site closed: it stops reporting the note for each of the
    /// site's tabs, then and later, which the clear check proves with the engine.
    #[gpui::test]
    fn closing_the_note_hides_it_and_has_the_core_keep_the_site(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            // Without a note, there's nothing to close.
            shell.close_drm_note(cx);
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            host.take();
            shell.close_drm_note(cx);
            assert!(shell.drm_note().is_none());
        });
        assert_eq!(host.take(), ["close media note 10"]);
    }

    #[gpui::test]
    fn another_sites_note_still_shows(cx: &mut TestAppContext) {
        let (shell, _) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            shell.set_media_unsupported(11, "com.widevine.alpha", cx);
            shell.close_drm_note(cx);
            shell.set_selected(11, cx);
            assert!(
                shell.drm_note().is_some(),
                "closing one site's note closed another's"
            );
        });
    }

    /// The ×, under the pointer, says what it does, and a click on it closes the note for the
    /// site.
    #[gpui::test]
    fn the_close_button_says_what_it_does_and_closes_the_note(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            );
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            let page = Page {
                url: "https://www.netflix.com/browse".into(),
                ..Page::default()
            };
            shell.set_page(10, page, cx);
            shell.set_selected(10, cx);
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
        });
        cx.run_until_parked();
        let frame = |shell: &Shell, _: &App| shell.button_frames.borrow()[CLOSE_KEY].center();
        let close = shell.read_with(cx, frame);
        host.take();
        cx.simulate_mouse_move(close, None, Modifiers::none());
        cx.run_until_parked();
        assert!(
            host.take()
                .contains(&"tooltip Don't Show for This Site ".to_string())
        );
        cx.simulate_click(close, Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"close media note 10".to_string()));
        shell.read_with(cx, |shell, _| assert!(shell.drm_note().is_none()));
    }

    /// A page in Safari's engine whose protected video is kept off the screen says why, and
    /// offers nothing to switch or open.
    #[gpui::test]
    fn the_outputs_note_shows_in_safaris_engine_and_offers_nothing(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        host.system_engine.set(true);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, OUTPUT_RESTRICTED, cx);
            // Not while the tab is Chromium's, as the note is Safari's engine's.
            assert!(shell.drm_note().is_none());
            let page = Page {
                system_engine: true,
                ..shell.page(10)
            };
            shell.set_page(10, page, cx);
            assert!(shell.output_note());
            host.take();
            shell.use_system_engine(cx);
            shell.open_elsewhere(cx);
        });
        assert!(host.take().is_empty());
    }

    /// Its ×, under the pointer, says it closes, and a click closes it for the tab alone: the
    /// core is told nothing, as the cause is the display connection and not the site.
    #[gpui::test]
    fn the_outputs_note_closes_for_the_tab(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            );
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            let url = "https://www.disneyplus.com/play/1".into();
            shell.set_page(
                10,
                Page {
                    url,
                    system_engine: true,
                    ..Page::default()
                },
                cx,
            );
            shell.set_selected(10, cx);
            shell.set_media_unsupported(10, OUTPUT_RESTRICTED, cx);
        });
        cx.run_until_parked();
        let frame = |shell: &Shell, _: &App| shell.button_frames.borrow()[CLOSE_KEY].center();
        let close = shell.read_with(cx, frame);
        host.take();
        cx.simulate_mouse_move(close, None, Modifiers::none());
        cx.run_until_parked();
        assert!(host.take().contains(&"tooltip Close ".to_string()));
        cx.simulate_click(close, Modifiers::none());
        cx.run_until_parked();
        assert!(
            !host
                .take()
                .iter()
                .any(|asked| asked.starts_with("close media note"))
        );
        shell.read_with(cx, |shell, _| assert!(shell.drm_note().is_none()));
    }

    /// On Linux, the core offers Google's Widevine module: a Widevine page's note offers it and
    /// downloads it at the user's yes, the button, and at nothing else.
    #[gpui::test]
    fn the_note_offers_googles_widevine_module_and_downloads_it_at_the_yes(
        cx: &mut TestAppContext,
    ) {
        let (shell, host) = window_with_tabs(cx);
        host.widevine.set(Widevine::Absent);
        shell.update(cx, |shell, cx| {
            // Without a note, there's nothing to say yes to.
            shell.download_widevine(cx);
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            assert_eq!(shell.widevine_offer(), Some(Widevine::Absent));
            host.take();
            shell.download_widevine(cx);
        });
        assert_eq!(host.take(), ["download widevine"]);
    }

    /// The no: the × closes the note for the site, as anywhere, and downloads nothing.
    #[gpui::test]
    fn closing_the_widevine_note_downloads_nothing(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        host.widevine.set(Widevine::Absent);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            host.take();
            shell.close_drm_note(cx);
            shell.download_widevine(cx);
        });
        assert_eq!(host.take(), ["close media note 10"]);
    }

    /// While it downloads, the note offers nothing; a download that failed is offered again.
    #[gpui::test]
    fn a_failed_download_is_offered_again(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        host.widevine.set(Widevine::Downloading);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            host.take();
            shell.download_widevine(cx);
            shell.restart_for_widevine(cx);
        });
        assert!(host.take().is_empty());
        host.widevine.set(Widevine::Failed);
        shell.update(cx, |shell, cx| shell.download_widevine(cx));
        assert_eq!(host.take(), ["download widevine"]);
    }

    /// Linux loads the module at launch, so once it's downloaded the note restarts the browser,
    /// and downloads nothing more.
    #[gpui::test]
    fn once_downloaded_the_note_restarts_the_browser(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        host.widevine.set(Widevine::Restart);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            host.take();
            shell.download_widevine(cx);
            shell.restart_for_widevine(cx);
        });
        assert_eq!(host.take(), ["restart"]);
    }

    /// The module plays Widevine alone, and where the core offers none, nothing changes.
    #[gpui::test]
    fn the_module_is_offered_for_widevine_where_the_core_has_it(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            assert_eq!(shell.widevine_offer(), None);
            host.widevine.set(Widevine::Absent);
            shell.set_media_unsupported(10, "com.microsoft.playready", cx);
            assert_eq!(shell.widevine_offer(), None);
        });
    }

    /// On Windows the note shows only where PlayReady can't play, and says what the PC lacks:
    /// Windows 11 first, then an x64 PC, then the graphics' hardware DRM.
    #[gpui::test]
    fn on_windows_the_note_says_what_playready_needs(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            shell.set_media_unsupported(10, "com.microsoft.playready", cx);
            assert_eq!(
                shell.unplayable_words(),
                WORDS,
                "a Mac or Linux note named Windows"
            );
            let said = |needs| {
                host.playready.set(needs);
                shell.unplayable_words()
            };
            assert!(said(PlayReady::Windows11).contains("Windows 11"));
            assert!(said(PlayReady::X64).contains("ARM"));
            assert!(said(PlayReady::Hardware).contains("hardware DRM"));
        });
    }

    /// A private window can't keep PlayReady's licences, so on a PC that plays, the note says
    /// that's why, and elsewhere still what the PC lacks.
    #[gpui::test]
    fn a_private_window_says_it_cant_play(cx: &mut TestAppContext) {
        let (shell, host) = window_with_tabs(cx);
        shell.update(cx, |shell, cx| {
            shell.private = true;
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
            host.playready.set(PlayReady::Hardware);
            assert_eq!(shell.unplayable_words(), PLAYREADY_PRIVATE);
            host.playready.set(PlayReady::Windows11);
            assert_eq!(shell.unplayable_words(), PLAYREADY_WINDOWS_11);
        });
    }

    /// Escape with the UI focused closes the note in front, and with none it stops, as before:
    /// here, in full screen with nothing loading, it leaves full screen.
    #[gpui::test]
    fn escape_closes_the_note_and_otherwise_stops(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(bind_keys);
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            );
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            let page = Page {
                url: "https://www.netflix.com/browse".into(),
                ..Page::default()
            };
            shell.set_page(10, page, cx);
            shell.set_selected(10, cx);
            shell.set_full_screen(true, cx);
            shell.set_media_unsupported(10, "com.widevine.alpha", cx);
        });
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        host.take();
        cx.simulate_keystrokes("escape");
        shell.read_with(cx, |shell, _| assert!(shell.drm_note().is_none()));
        let asked = host.take();
        assert!(asked.contains(&"close media note 10".to_string()));
        assert!(
            !asked.contains(&"window button zoom".to_string()),
            "escape also stopped"
        );
        cx.simulate_keystrokes("escape");
        assert!(host.take().contains(&"window button zoom".to_string()));
    }
}
