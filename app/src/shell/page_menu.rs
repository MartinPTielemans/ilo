//! A page's own menu, from a right-click on it: Chrome's, as the core builds it with its
//! extensions' items and spelling suggestions, shown in ilo's menu over the page (#332).

use gpui::{Context, Pixels, Point, SharedString};

use super::{MenuEntry, Shell};
use crate::bc::BC_PAGE_MENU_PREVIEW_LINK;

/// A row of a page's menu, as the core gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct PageMenuItem {
    /// What the core runs once it's chosen.
    pub command: i32,
    /// "" for a line between rows.
    pub title: SharedString,
    /// The row whose submenu it's in, which comes before it.
    pub parent: Option<usize>,
    pub enabled: bool,
    pub checked: bool,
}

/// ilo's menu's entries for the page menu's `items`, one for each, so a choice's index is the
/// row's. A row can only be in the submenu of one before it, as the menu lays out a submenu's
/// parent first; one the core says is in another's goes at the top.
fn entries(items: &[PageMenuItem]) -> Vec<MenuEntry> {
    (items.iter().enumerate())
        .map(|(at, item)| MenuEntry {
            parent: item.parent.filter(|&parent| parent < at),
            checked: item.checked,
            ..MenuEntry::named(item.title.clone(), item.enabled && !item.title.is_empty())
        })
        .collect()
}

/// `items` without the rows that run `command`, each submenu's rows still under their own.
fn without(items: Vec<PageMenuItem>, command: i32) -> Vec<PageMenuItem> {
    let gone: Vec<usize> = (items.iter().enumerate())
        .filter(|(_, item)| item.command == command)
        .map(|(at, _)| at)
        .collect();
    (items.into_iter().enumerate())
        .filter(|(at, _)| !gone.contains(at))
        .map(|(_, item)| {
            let before = |parent: usize| gone.iter().filter(|&&at| at < parent).count();
            PageMenuItem {
                parent: item.parent.map(|parent| parent - before(parent)),
                ..item
            }
        })
        .collect()
}

/// A made-up page menu of `kind` (link, image, text or field), as Chromium 156 builds it once the
/// core has taken out what Arc's leaves out and added Open Link in Peek, for the preview to show
/// without the engine. On the Mac it's in title case, with the Mac's Look Up, Speech and Writing
/// Direction; elsewhere it's in Chromium's sentence case, and a misspelt word's Language settings
/// is in Spell check instead.
pub fn made_up(kind: &str, mac: bool) -> Vec<PageMenuItem> {
    let row = |command: i32, title: &str| PageMenuItem {
        command,
        title: title.to_string().into(),
        parent: None,
        enabled: true,
        checked: false,
    };
    let os = |on_mac: &str, elsewhere: &str| if mac { on_mac } else { elsewhere }.to_string();
    let line = || row(0, "");
    let under = |parent: usize, item: PageMenuItem| PageMenuItem {
        parent: Some(parent),
        ..item
    };
    let off = |item: PageMenuItem| PageMenuItem {
        enabled: false,
        ..item
    };
    let mut items = match kind {
        "image" => vec![
            row(50120, &os("Open Image in New Tab", "Open image in new tab")),
            row(50121, &os("Save Image As…", "Save image as…")),
            row(50122, &os("Copy Image", "Copy image")),
            row(50123, &os("Copy Image Address", "Copy image address")),
        ],
        "text" => {
            let mut items = vec![
                row(50150, "Copy"),
                row(50151, "Search DuckDuckGo for “ilo”"),
                row(35003, "Print…"),
            ];
            if mac {
                items.splice(0..0, [row(50160, "Look Up “ilo”"), line()]);
                items.extend([
                    line(),
                    row(50170, "Speech"),
                    under(6, row(50171, "Start Speaking")),
                    under(6, off(row(50172, "Stop Speaking"))),
                ]);
            }
            items
        }
        "field" => {
            let mut items = vec![
                row(41000, "receive"),
                row(41001, "relieve"),
                row(41100, &os("Add to Dictionary", "Add to dictionary")),
            ];
            if mac {
                items.push(row(41101, "Language Settings"));
            }
            items.extend([
                line(),
                off(row(50151, "Cut")),
                off(row(50150, "Copy")),
                row(50152, "Paste"),
                row(50153, &os("Paste and Match Style", "Paste as plain text")),
                row(50154, &os("Select All", "Select all")),
            ]);
            if mac {
                items.extend([
                    row(50180, "Writing Direction"),
                    under(
                        10,
                        PageMenuItem {
                            checked: true,
                            ..row(50181, "Default")
                        },
                    ),
                    under(10, row(50182, "Left to Right")),
                    under(10, row(50183, "Right to Left")),
                    line(),
                    row(50170, "Speech"),
                    under(15, row(50171, "Start Speaking")),
                    under(15, off(row(50172, "Stop Speaking"))),
                ]);
            }
            items
        }
        _ => vec![
            row(50100, &os("Open Link in New Tab", "Open link in new tab")),
            row(
                BC_PAGE_MENU_PREVIEW_LINK,
                &os("Open Link in Peek", "Open link in Peek"),
            ),
            row(
                50101,
                &os("Open Link in New Window", "Open link in new window"),
            ),
            row(
                50105,
                &os("Open Link in Split View", "Open link in split view"),
            ),
            row(
                50102,
                &os(
                    "Open Link in Incognito Window",
                    "Open link in incognito window",
                ),
            ),
            line(),
            row(50103, &os("Save Link As…", "Save link as…")),
            row(50104, &os("Copy Link Address", "Copy link address")),
        ],
    };
    items.extend([line(), row(26500, "Inspect")]);
    items
}

impl Shell {
    /// Shows the page menu of `tab`, the core's `items`, with its top left at `at` in the window,
    /// and once it closes tells the core the row chosen, or none.
    pub fn show_page_menu(
        &mut self,
        tab: u64,
        items: Vec<PageMenuItem>,
        at: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        // The core offers Open Link in Peek for any web link, as only the app knows where a link
        // preview can open.
        let items = match self.peeks_links_of(tab) {
            true => items,
            false => without(items, BC_PAGE_MENU_PREVIEW_LINK),
        };
        if items.is_empty() {
            self.browser.close_page_menu(tab, None, cx);
            return;
        }
        let items = match self.translate_row(tab) {
            Some(title) => with_translate(items, title),
            None => items,
        };
        let commands: Vec<i32> = items.iter().map(|item| item.command).collect();
        // Over the page, so in the window even while the sidebar is hidden.
        self.context_menu_in(
            entries(&items),
            at,
            false,
            Box::new(move |chosen, shell, cx| {
                let command = chosen.and_then(|at| commands.get(at).copied());
                // The app's own row, which the core knows nothing of.
                if command == Some(TRANSLATE) {
                    shell.browser.close_page_menu(tab, None, cx);
                    shell.run_translate_row(tab, cx);
                    return;
                }
                shell.browser.close_page_menu(tab, command, cx);
            }),
            cx,
        );
    }
}

/// The command of the page menu's translation row, which the app runs itself: none of the core's,
/// which are Chrome's ids above 0 and its own just below, as BC_PAGE_MENU_PREVIEW_LINK is.
const TRANSLATE: i32 = i32::MIN;

/// Chrome's page items, for where the translation row goes.
const VIEW_SOURCE: i32 = 35002;
const PRINT: i32 = 35003;
const CAST: i32 = 35011;

/// `items` with the translation row `title`, where Chrome's and Arc's page menu has Translate:
/// after Print… and Cast…, before the line over View Page Source. Only a page's menu has those, so
/// a link's, an image's or a selection's gets none.
fn with_translate(mut items: Vec<PageMenuItem>, title: SharedString) -> Vec<PageMenuItem> {
    let top = |command: i32| {
        (items.iter()).rposition(|item| item.command == command && item.parent.is_none())
    };
    if top(VIEW_SOURCE).is_none() {
        return items;
    }
    let Some(after) = top(CAST).or_else(|| top(PRINT)) else {
        return items;
    };
    // The rows after it that point at a row at or past it point one further.
    for item in &mut items[after + 1..] {
        if let Some(parent) = item.parent.as_mut().filter(|parent| **parent > after) {
            *parent += 1;
        }
    }
    items.insert(
        after + 1,
        PageMenuItem {
            command: TRANSLATE,
            title,
            parent: None,
            enabled: true,
            checked: false,
        },
    );
    items
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{AppContext, TestAppContext, point, px};

    use super::*;
    use crate::shell::{Folders, recorder::Recorder};

    fn row(command: i32, title: &str) -> PageMenuItem {
        PageMenuItem {
            command,
            title: title.into(),
            parent: None,
            enabled: true,
            checked: false,
        }
    }

    fn line() -> PageMenuItem {
        row(0, "")
    }

    /// Chrome's menu for a misspelt word in a text field, with its Writing Direction submenu.
    fn field_menu() -> Vec<PageMenuItem> {
        vec![
            row(1, "receive"),
            row(2, "Add to Dictionary"),
            line(),
            PageMenuItem {
                enabled: false,
                ..row(3, "Cut")
            },
            row(4, "Copy"),
            row(5, "Paste"),
            line(),
            row(6, "Writing Direction"),
            PageMenuItem {
                parent: Some(7),
                checked: true,
                ..row(7, "Default")
            },
            PageMenuItem {
                parent: Some(7),
                ..row(8, "Left to Right")
            },
            line(),
            row(9, "Inspect"),
        ]
    }

    fn window_showing_tab(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            shell.set_spaces(
                vec![(
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                )],
                cx,
            );
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
        });
        host.take();
        (shell, host)
    }

    #[test]
    fn rows_become_the_menus_entries_with_their_lines_checks_and_submenus() {
        let entries = entries(&field_menu());
        let titles: Vec<&str> = entries.iter().map(|entry| entry.title.as_ref()).collect();
        assert_eq!(
            titles,
            [
                "receive",
                "Add to Dictionary",
                "",
                "Cut",
                "Copy",
                "Paste",
                "",
                "Writing Direction",
                "Default",
                "Left to Right",
                "",
                "Inspect",
            ]
        );
        assert_eq!(entries[8].parent, Some(7));
        assert_eq!(entries[9].parent, Some(7));
        assert!(entries[8].checked && !entries[9].checked);
        assert!(!entries[3].enabled, "Cut is off with nothing selected");
        assert!(!entries[2].enabled, "a line isn't chosen");
        assert!(
            entries
                .iter()
                .all(|entry| entry.shortcut.is_none() && !entry.destructive)
        );
    }

    fn titles(items: &[PageMenuItem]) -> Vec<&str> {
        items.iter().map(|item| item.title.as_ref()).collect()
    }

    /// The made-up link menu's items read as Chromium's do on each system, as check/split.sh
    /// chooses them there.
    #[test]
    fn the_made_up_link_menu_is_in_each_systems_case() {
        assert_eq!(
            titles(&made_up("link", true)),
            [
                "Open Link in New Tab",
                "Open Link in Peek",
                "Open Link in New Window",
                "Open Link in Split View",
                "Open Link in Incognito Window",
                "",
                "Save Link As…",
                "Copy Link Address",
                "",
                "Inspect",
            ]
        );
        assert_eq!(
            titles(&made_up("link", false)),
            [
                "Open link in new tab",
                "Open link in Peek",
                "Open link in new window",
                "Open link in split view",
                "Open link in incognito window",
                "",
                "Save link as…",
                "Copy link address",
                "",
                "Inspect",
            ]
        );
    }

    /// Look Up, Speech and Writing Direction are the Mac's, and off it Language settings moves to
    /// Spell check, which a misspelt word's menu leaves out. Each submenu row is under its own.
    #[test]
    fn the_macs_own_items_are_only_on_the_mac() {
        let macs = [
            "Look Up “ilo”",
            "Speech",
            "Writing Direction",
            "Language Settings",
        ];
        for kind in ["link", "image", "text", "field"] {
            for mac in [true, false] {
                let items = made_up(kind, mac);
                for item in &items {
                    let parent = item.parent.map(|parent| items[parent].title.as_ref());
                    assert!(matches!(
                        parent,
                        None | Some("Speech" | "Writing Direction")
                    ));
                }
                if !mac {
                    assert!(
                        titles(&items).iter().all(|title| !macs.contains(title)),
                        "{kind}"
                    );
                }
            }
        }
        let field = made_up("field", false);
        assert!(titles(&field).contains(&"Paste as plain text"));
        assert!(titles(&made_up("field", true)).contains(&"Paste and Match Style"));
    }

    #[test]
    fn a_row_left_out_leaves_each_submenu_under_its_own() {
        let mut items = field_menu();
        items.insert(1, row(BC_PAGE_MENU_PREVIEW_LINK, "Open Link in Peek"));
        for item in &mut items[2..] {
            item.parent = item.parent.map(|parent| parent + 1);
        }
        assert_eq!(without(items, BC_PAGE_MENU_PREVIEW_LINK), field_menu());
    }

    #[test]
    fn a_row_in_the_submenu_of_one_after_it_goes_at_the_top() {
        let items = vec![
            PageMenuItem {
                parent: Some(1),
                ..row(1, "Early")
            },
            row(2, "Later"),
        ];
        assert_eq!(entries(&items)[0].parent, None);
    }

    #[gpui::test]
    fn the_row_chosen_runs_its_command(cx: &mut TestAppContext) {
        let (shell, host) = window_showing_tab(cx);
        shell.update(cx, |shell, cx| {
            shell.show_page_menu(10, field_menu(), point(px(400.), px(300.)), cx)
        });
        assert_eq!(host.entries.borrow().len(), 12);
        cx.update(|cx| host.choose("receive", cx));
        assert_eq!(host.take(), ["page menu 10 runs 1"]);
    }

    #[gpui::test]
    fn a_row_in_a_submenu_runs_its_own_command(cx: &mut TestAppContext) {
        let (shell, host) = window_showing_tab(cx);
        shell.update(cx, |shell, cx| {
            shell.show_page_menu(10, field_menu(), point(px(400.), px(300.)), cx)
        });
        cx.update(|cx| host.choose("Left to Right", cx));
        assert_eq!(host.take(), ["page menu 10 runs 8"]);
    }

    #[gpui::test]
    fn the_menu_shows_over_the_page_while_the_sidebar_is_hidden(cx: &mut TestAppContext) {
        let (shell, host) = window_showing_tab(cx);
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.show_page_menu(10, field_menu(), point(px(400.), px(300.)), cx)
        });
        assert_eq!(*host.menus_in_reveal.borrow(), [false]);
    }

    #[gpui::test]
    #[should_panic(expected = "no Cut in the menu")]
    fn a_disabled_row_cant_be_chosen(cx: &mut TestAppContext) {
        let (shell, host) = window_showing_tab(cx);
        shell.update(cx, |shell, cx| {
            shell.show_page_menu(10, field_menu(), point(px(400.), px(300.)), cx)
        });
        cx.update(|cx| host.choose("Cut", cx));
    }

    #[gpui::test]
    fn closing_the_menu_without_a_choice_runs_nothing(cx: &mut TestAppContext) {
        let (shell, host) = window_showing_tab(cx);
        shell.update(cx, |shell, cx| {
            shell.show_page_menu(10, field_menu(), point(px(400.), px(300.)), cx)
        });
        cx.update(|cx| host.dismiss(cx));
        assert_eq!(host.take(), ["page menu 10 closes"]);
    }

    #[gpui::test]
    fn an_empty_menu_closes_at_once(cx: &mut TestAppContext) {
        let (shell, host) = window_showing_tab(cx);
        shell.update(cx, |shell, cx| {
            shell.show_page_menu(10, Vec::new(), point(px(0.), px(0.)), cx)
        });
        assert!(host.menus.borrow().is_empty());
        assert_eq!(host.take(), ["page menu 10 closes"]);
    }

    /// Chrome's menu for a page, as Arc's has it.
    fn page_menu() -> Vec<PageMenuItem> {
        vec![
            row(33000, "Back"),
            row(33001, "Forward"),
            row(33002, "Reload"),
            line(),
            row(35004, "Save As…"),
            row(PRINT, "Print…"),
            row(CAST, "Cast…"),
            line(),
            row(VIEW_SOURCE, "View Page Source"),
            row(50162, "Inspect"),
        ]
    }

    /// The translation row goes after Cast…, or Print… without it, and only in a page's menu.
    #[test]
    fn the_translation_row_goes_where_arcs_translate_is() {
        let with = with_translate(page_menu(), "Translate to English".into());
        assert_eq!(
            titles(&with),
            [
                "Back",
                "Forward",
                "Reload",
                "",
                "Save As…",
                "Print…",
                "Cast…",
                "Translate to English",
                "",
                "View Page Source",
                "Inspect",
            ]
        );
        let uncast = without(page_menu(), CAST);
        let with = with_translate(uncast, "Use Original".into());
        assert_eq!(titles(&with)[5..7], ["Print…", "Use Original"]);
        // A link's has neither View Page Source nor Print….
        let link = made_up("link", true);
        assert_eq!(with_translate(link.clone(), "Use Original".into()), link);
    }

    /// A submenu after the row still opens under its own row.
    #[test]
    fn a_submenu_after_the_translation_row_keeps_its_row() {
        let mut items = page_menu();
        items.push(row(6, "Writing Direction"));
        items.push(PageMenuItem {
            parent: Some(10),
            ..row(7, "Default")
        });
        let with = with_translate(items, "Use Original".into());
        assert_eq!(with[11].title.as_ref(), "Writing Direction");
        assert_eq!(with[12].parent, Some(11));
    }

    /// A page in a language the user doesn't read has Translate to English in its menu, which
    /// translates it, and then Use Original, which takes it back.
    #[gpui::test]
    fn the_page_menu_translates_and_takes_it_back(cx: &mut TestAppContext) {
        let (shell, host) = window_showing_tab(cx);
        let at = point(px(400.), px(300.));
        shell.update(cx, |shell, cx| {
            shell.show_page_menu(10, page_menu(), at, cx)
        });
        assert!(
            !host
                .entries
                .borrow()
                .iter()
                .any(|entry| entry.title.starts_with("Translate"))
        );
        host.take();
        shell.update(cx, |shell, cx| {
            shell.set_page_language(10, "es", cx);
            shell.show_page_menu(10, page_menu(), at, cx)
        });
        cx.update(|cx| host.choose("Translate to English", cx));
        assert_eq!(host.take(), ["page menu 10 closes", "translate 10 es en"]);
        shell.update(cx, |shell, cx| {
            shell.show_page_menu(10, page_menu(), at, cx)
        });
        cx.update(|cx| host.choose("Use Original", cx));
        assert_eq!(host.take(), ["page menu 10 closes", "use original 10"]);
    }
}
