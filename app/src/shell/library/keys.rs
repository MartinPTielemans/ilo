//! The Library with the keyboard (#3238). Its key or command, or a section's, gives the section's
//! search field the keyboard as it opens, so a screen reader, which follows the keyboard, says
//! where it is, as the command bar's field does. Spaces has no field: a ring goes on its first
//! card instead, as Focus Sidebar's does on a row, left and right move it along the cards and
//! return shows the ringed card's Space, as a click on it does. Escape still closes the Library,
//! as neither binds it. A swipe or a click on the Library moves no keyboard. Closed by its key, a
//! section's or Escape, the Library gives the keyboard back to whoever had it as it opened, the
//! page or the UI (#3313); closed any other way, to the window.

use gpui::{
    App, Context, Focusable, KeyBinding, Role, Stateful, Window, actions, div, prelude::*, px,
};

use super::{Section, Shell};
use crate::tokens::shell::{KEYED, KEYED_RING, library::CARD_RADIUS};

/// Spaces' key context while its cards have the keyboard.
const CARDS: &str = "LibraryCards";

actions!(library_keys, [CardLeft, CardRight, CardShow]);

/// The Library's keyboard, between the command that opens it and its close.
#[derive(Default)]
pub(in crate::shell) struct LibraryKeys {
    /// A command opened the Library or showed a section: its field or first card takes the
    /// keyboard on the next frame.
    asked: bool,
    /// It opened the Library, and whether its key came from the page.
    opened: Option<bool>,
    /// Who had the keyboard as the Library took it.
    before: Option<Before>,
    /// Its key, a section's or Escape closed it.
    back: bool,
}

enum Before {
    Page,
    Ui(gpui::FocusHandle),
}

pub(in crate::shell) fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("left", CardLeft, Some(CARDS)),
        KeyBinding::new("right", CardRight, Some(CARDS)),
        KeyBinding::new("enter", CardShow, Some(CARDS)),
    ]);
}

impl Shell {
    /// Runs a command that opens the Library or shows one of its sections, whose field or first
    /// card then takes the keyboard, or closes it. `from` is where its key came from (`run`).
    pub(in crate::shell) fn library_command(
        &mut self,
        from: &str,
        cx: &mut Context<Self>,
        run: impl FnOnce(&mut Self, &mut Context<Self>),
    ) {
        let open = self.library_open;
        run(self, cx);
        let keys = &mut self.library_keys;
        keys.asked = self.library_open;
        if !open && self.library_open {
            keys.opened = Some(from == "page");
        }
        keys.back = open && !self.library_open;
    }

    /// Escape closes the Library, giving the keyboard back as its key does.
    pub(in crate::shell) fn escape_library(&mut self, cx: &mut Context<Self>) {
        self.library_keys.back = true;
        self.set_library(false, cx);
    }

    /// Before the frame's drawn: gives the section shown the keyboard a command asked for, ends
    /// Spaces' ring once something else has the keyboard, and gives the keyboard back as the
    /// Library closes.
    pub(super) fn step_library_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.spaces.first().map(|space| space.id);
        let Some(library) = &mut self.library else {
            return;
        };
        let keys = &mut self.library_keys;
        let asked = std::mem::take(&mut keys.asked) && self.library_open;
        let (search, cards) = (library.search.focus_handle(cx), library.cards.clone());
        // Who has the keyboard as the Library takes it: the page, if the key came from it, as
        // GPUI's focus doesn't see the page's, else the UI's focus.
        if let Some(from_page) = keys.opened.take().filter(|_| asked) {
            keys.before = match from_page {
                true => Some(Before::Page),
                false => window.focused(cx).map(Before::Ui),
            };
        }
        if asked {
            // The page may have the keyboard.
            self.browser.focus_ui(cx);
            match self.section {
                Section::Spaces => {
                    library.keyed_card = first;
                    window.focus(&cards, cx);
                }
                _ => window.focus(&search, cx),
            }
        } else if library.keyed_card.is_some() && !cards.is_focused(window) {
            library.keyed_card = None;
        }
        if let Some(card) = library.keyed_card.filter(|_| asked) {
            log!("browser: library card keyed {card}");
        }
        if self.library_open {
            return;
        }
        // The field and cards go with the Library. Its key or Escape gives the keyboard back to
        // whoever had it. Otherwise the window takes it, where Tab rings the sidebar, and not the
        // page: a click on a row may be opening another, and a click elsewhere took it already.
        let LibraryKeys { before, back, .. } = std::mem::take(&mut self.library_keys);
        if !(search.is_focused(window) || cards.is_focused(window)) {
            return;
        }
        match before.filter(|_| back) {
            Some(Before::Page) => {
                window.focus(&self.focus_handle, cx);
                self.browser.focus_page(cx);
                // check.sh reads this line.
                log!("browser: library gave the keyboard back to the page");
            }
            Some(Before::Ui(before)) => window.focus(&before, cx),
            None => window.focus(&self.focus_handle, cx),
        }
    }

    /// Spaces' cards, taking their keys while they have the keyboard. An assistive app reads them
    /// as Spaces, and, as they have the keyboard, the ringed card.
    pub(super) fn with_card_keys(
        &self,
        cards: Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<gpui::Div> {
        let Some(library) = &self.library else {
            return cards;
        };
        cards
            .when(self.a11y, |cards| {
                cards.role(Role::Group).aria_label("Spaces")
            })
            .key_context(CARDS)
            .track_focus(&library.cards)
            .on_action(cx.listener(|shell, _: &CardLeft, _, cx| shell.card_step(false, cx)))
            .on_action(cx.listener(|shell, _: &CardRight, _, cx| shell.card_step(true, cx)))
            .on_action(cx.listener(|shell, _: &CardShow, _, cx| shell.card_show(cx)))
    }

    /// A Space's card as an assistive app reads it, by the Space's name, and with the ring while
    /// the keys are on it.
    pub(super) fn card_keyed(
        &self,
        card: Stateful<gpui::Div>,
        space: u64,
        name: gpui::SharedString,
    ) -> Stateful<gpui::Div> {
        let keyed = self.library.as_ref().and_then(|library| library.keyed_card) == Some(space);
        card.when(self.a11y, |card| {
            card.role(Role::Button)
                .aria_label(name)
                .when(keyed, |card| card.aria_active_descendant())
        })
        .when(keyed, |card| {
            card.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(CARD_RADIUS))
                    .border(px(KEYED_RING))
                    .border_color(self.palette.plate(KEYED)),
            )
        })
    }

    /// Moves the ring to the next card along or back, stopping at the ends.
    fn card_step(&mut self, along: bool, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.spaces.iter().map(|space| space.id).collect();
        let Some(library) = &mut self.library else {
            return;
        };
        let Some(at) = (library.keyed_card).and_then(|card| ids.iter().position(|&id| id == card))
        else {
            return cx.propagate();
        };
        let next = match along {
            true => ids.get(at + 1),
            false => at.checked_sub(1).and_then(|at| ids.get(at)),
        };
        if let Some(&next) = next {
            library.keyed_card = Some(next);
            log!("browser: library card keyed {next}");
            cx.notify();
        }
    }

    /// Return: shows the ringed card's Space, as a click on the card does.
    fn card_show(&mut self, cx: &mut Context<Self>) {
        let Some(card) = self
            .library
            .as_mut()
            .and_then(|library| library.keyed_card.take())
        else {
            return cx.propagate();
        };
        self.pick_space(card, cx);
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, Role, TestAppContext, VisualTestContext};

    use crate::shell::{Command, Folders, Section, Shell, commands::bind_keys, recorder::Recorder};

    /// A window on Space 1 of three, a tab showing, the Library to open on `section`.
    fn window(
        section: Section,
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(bind_keys);
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        cx.simulate_a11y_activation();
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
            shell.set_spaces(vec![space(1), space(2), space(3)], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
            shell.section = section;
        });
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        cx.run_until_parked();
        host.take();
        (shell, host, cx)
    }

    fn run(shell: &Entity<Shell>, command: Command, cx: &mut VisualTestContext) {
        run_from(shell, command, "test", cx);
    }

    /// Runs `command` for a key from `from`, "page" for one the page handed back.
    fn run_from(shell: &Entity<Shell>, command: Command, from: &str, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, cx| {
            shell.run(command, from, cx);
            settle(shell);
        });
        cx.run_until_parked();
    }

    /// The Library's motion and the sidebar's fade at their ends.
    fn settle(shell: &mut Shell) {
        shell.library_motion = None;
        shell.sidebar_fade = None;
    }

    fn root_keyed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| shell.read(cx).focus_handle.is_focused(window))
    }

    /// What has the keyboard, as a screen reader reads it: its role and name, or, inside it, the
    /// active one's.
    fn keyed(cx: &mut VisualTestContext) -> (Role, String) {
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let (_, node) = (tree.nodes.iter())
            .find(|(id, _)| *id == tree.focus)
            .expect("something has the keyboard");
        (node.role(), node.label().unwrap_or_default().to_string())
    }

    /// The Library's key gives History's search field the keyboard, taking it from the page, and
    /// the window takes it back as the Library closes. A section's own command gives its field it.
    #[gpui::test]
    fn the_librarys_key_gives_its_search_field_the_keyboard(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        run(&shell, Command::ViewLibrary, cx);
        assert!(host.take().contains(&"focus ui".to_owned()));
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        run(&shell, Command::ViewDownloads, cx);
        assert_eq!(keyed(cx), (Role::TextInput, "Search Downloads".into()));
        run(&shell, Command::ViewLibrary, cx);
        assert!(
            root_keyed(&shell, cx),
            "the window didn't take the keyboard back"
        );
    }

    /// Opened by its key from the page, the Library gives the page the keyboard back as its key or
    /// a section's closes it, or Escape does, and the window's UI lets it go (#3313).
    #[gpui::test]
    fn the_page_gets_the_keyboard_back_as_a_key_closes_the_library(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        let focused_page = |host: &Recorder| host.take().contains(&"focus page".to_owned());
        run_from(&shell, Command::ViewDownloads, "page", cx);
        assert_eq!(keyed(cx), (Role::TextInput, "Search Downloads".into()));
        // Another section's key keeps the Library open, and the page's turn for the keyboard.
        run(&shell, Command::ViewArchive, cx);
        assert!(
            !focused_page(&host),
            "the page took it while the Library was open"
        );
        run(&shell, Command::ViewArchive, cx);
        assert!(
            focused_page(&host),
            "a section's key didn't give the page it back"
        );
        assert!(root_keyed(&shell, cx), "the field kept the UI's keyboard");

        run_from(&shell, Command::ViewLibrary, "page", cx);
        run(&shell, Command::ViewLibrary, cx);
        assert!(
            focused_page(&host),
            "the Library's key didn't give the page it back"
        );

        run_from(&shell, Command::ViewLibrary, "page", cx);
        assert_eq!(keyed(cx), (Role::TextInput, "Search History".into()));
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, _| settle(shell));
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_open),
            "escape left it open"
        );
        assert!(focused_page(&host), "escape didn't give the page it back");
    }

    /// Closed as a click closes it, the Library opened from the page gives the window the
    /// keyboard: the click went where the user wants it. Opened from the UI, its key gives the UI
    /// the keyboard back, never the page.
    #[gpui::test]
    fn a_click_or_the_uis_key_never_gives_the_page_the_keyboard(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        run_from(&shell, Command::ViewLibrary, "page", cx);
        shell.update(cx, |shell, cx| {
            shell.set_library(false, cx);
            settle(shell);
        });
        cx.run_until_parked();
        assert!(!host.take().contains(&"focus page".to_owned()));
        assert!(root_keyed(&shell, cx));

        run_from(&shell, Command::ViewLibrary, "ui", cx);
        run(&shell, Command::ViewLibrary, cx);
        assert!(!host.take().contains(&"focus page".to_owned()));
        assert!(root_keyed(&shell, cx));
    }

    /// Opened by a swipe, or set open as the gesture does, the Library leaves the keyboard where
    /// it was.
    #[gpui::test]
    fn a_swipe_moves_no_keyboard(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Archive, cx);
        shell.update(cx, |shell, cx| shell.set_library(true, cx));
        cx.run_until_parked();
        assert!(!host.take().contains(&"focus ui".to_owned()));
    }

    /// In Spaces, which has no field, the ring goes on the first card, read by its Space's name;
    /// left and right move it along the cards, stopping at the ends, and return shows its Space.
    /// Escape closes the Library.
    #[gpui::test]
    fn spaces_rings_its_first_card(cx: &mut TestAppContext) {
        let (shell, host, cx) = window(Section::Spaces, cx);
        run(&shell, Command::ViewLibrary, cx);
        let card = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| shell.library.as_ref()?.keyed_card)
        };
        assert_eq!(card(&shell, cx), Some(1));
        assert_eq!(keyed(cx), (Role::Button, "Space 1".into()));
        cx.simulate_keystrokes("left");
        assert_eq!(
            card(&shell, cx),
            Some(1),
            "the ring went past the first card"
        );
        cx.simulate_keystrokes("right right right");
        assert_eq!(
            card(&shell, cx),
            Some(3),
            "the ring went past the last card"
        );
        cx.run_until_parked();
        assert_eq!(keyed(cx), (Role::Button, "Space 3".into()));
        cx.simulate_keystrokes("left");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            host.shown.borrow().last(),
            Some(&2),
            "return showed no Space"
        );
        run(&shell, Command::ViewLibrary, cx);
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, _| {
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_open),
            "escape left it open"
        );
    }
}
