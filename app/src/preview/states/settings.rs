//! Settings' pages, pictured with made-up states.

use crate::*;

/// A Settings group whose row is chosen from drawings, on its card as Settings draws it.
#[derive(Clone, Copy)]
pub(crate) enum Pictures {
    /// Appearance: Appearance, the one `chosen`, Page zoom, and Sidebar position, `right` chosen,
    /// and the Top bar under it.
    Appearance {
        appearance: browser::theme::Appearance,
        right: bool,
    },
    /// General's groups as a profile has them, with ilo not the default browser.
    General,
    /// Tabs & Sidebar's Share favorites, the scope chosen, in its group of one.
    Favorites(browser::shell::FavoritesScope),
    /// Agents' first row, its apps that connect, and the ilo command's row under them.
    CommandLine(CommandLine),
    /// Your assistant's rows in their states, `signed_in` or before (#4100).
    Assistants { signed_in: bool },
    /// Agents' Site notes (#4122): the switch, on, and the sites' rows, the first `open`.
    SiteNotes { open: bool },
}

/// Where the ilo command is (#3667).
#[derive(Clone, Copy)]
pub(crate) enum CommandLine {
    Off,
    On,
    /// In a folder the user's shell doesn't search.
    Unsearched,
    /// In a Flatpak, not added, and added with the line to run on the host.
    FlatpakOff,
    FlatpakOn,
}

/// `shown`'s groups in a window of Settings' size.
pub(crate) fn preview_pictures(shown: Pictures, cx: &mut App) {
    use browser::tokens::settings::{HEIGHT, WIDTH};
    // PREVIEW_SETTINGS_WIDTH=<points> narrows the window, to Settings' least for one.
    let width = states::var("PREVIEW_SETTINGS_WIDTH")
        .ok()
        .and_then(|width| width.parse().ok())
        .unwrap_or(WIDTH);
    let (mut x, mut y) = (0., 0.);
    unsafe { host_centered(width as f64, HEIGHT as f64, &mut x, &mut y) };
    let bounds = Bounds::new(
        point(px(x as f32), px(y as f32)),
        size(px(width), px(HEIGHT)),
    );
    // As the app opens Settings, so a capture shows what it draws (#4129).
    let options = browser::settings_column::window_options(bounds, "Settings".into());
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|cx| {
            let look = browser::settings_rows::Look::settings(true);
            let search = cx.new(|cx| {
                browser::text_input::TextInput::new("Search", cx)
                    .placeholder_color(look.detail)
                    .caret_color(look.ink)
            });
            PicturesPage {
                shown,
                card: Card::default(),
                search,
                logged: Rc::default(),
            }
        })
    });
    if let Err(error) = opened {
        log!("preview: couldn't open the pictures' window: {error}");
    }
}

pub(crate) struct PicturesPage {
    shown: Pictures,
    card: Card,
    search: gpui::Entity<browser::text_input::TextInput>,
    logged: browser::settings_rows::Drawn,
}

impl gpui::Render for PicturesPage {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use browser::agents::cli;
        use browser::palette::{INK, PAPER};
        use browser::settings_card::Colors;
        use browser::settings_column::{self, Section};
        use browser::settings_pictures::{appearance_picker, favorites_picker, picker};
        use browser::settings_rows::{
            Look, command_export, command_text, drawn, end, frame, glyph_plate, group, pictured,
            popup, stop_button, stop_switch, text as row_text,
        };
        use browser::theme::Appearance;
        use browser::tokens::palette::{LIGHT_STAND_IN, STAND_IN};
        use browser::tokens::settings::{
            BUTTONS_TOP, CONTROL_GAP, DETAIL_LINE, FIRST_GROUP, GROUP_GAP, HEADING_BOTTOM,
            HEADING_TOP, NAME, POPUP_WIDTH, SHARE_TOP, SIDE_ROOM, TEXT_X,
        };
        use browser::tokens::text;
        use gpui::{FontWeight, ParentElement, Styled, div, rgb};
        // Settings takes Light's or Dark's look when it's chosen, and the system's for Automatic.
        let dark = match self.shown {
            Pictures::Appearance {
                appearance: Appearance::Light,
                ..
            } => false,
            Pictures::Appearance {
                appearance: Appearance::Dark,
                ..
            } => true,
            _ => preview_tone() != Some(Tone::Light),
        };
        let look = Look::settings(dark);
        let base = if dark { rgb(PAPER) } else { rgb(INK) };
        let a11y = window.is_a11y_active();
        let field = browser::settings_search::field(
            &self.search,
            base,
            gpui::canvas(|_, _, _| {}, |_, _, _, _| {}),
            |_, _| {},
            cx,
        );
        let section = match self.shown {
            Pictures::Favorites(_) => Section::TabsAndSidebar,
            Pictures::Appearance { .. } => Section::Appearance,
            Pictures::General => Section::General,
            Pictures::CommandLine(_) | Pictures::Assistants { .. } => Section::Agents,
            Pictures::SiteNotes { .. } => Section::Agents,
        };
        let column = settings_column::column(
            &Default::default(),
            section,
            dark,
            Rc::default(),
            gpui::IntoElement::into_any_element(field),
            None,
            Rc::new(|_, _, _| {}),
            Rc::new(|_, _| {}),
            a11y,
        );
        let page = cx.entity().downgrade();
        let switch = |name: &'static str, on: bool| {
            end([gpui::IntoElement::into_any_element(stop_switch(
                &look, name, name, on, 1., look.on, a11y,
            ))])
        };
        // A line under the first group, as Settings' Reduce Transparency line is.
        let mut under: Option<&str> = None;
        let blocks: Vec<(Option<&str>, Vec<gpui::Div>)> = match self.shown {
            Pictures::Appearance { appearance, right } => {
                let three = appearance_picker(
                    &look,
                    base,
                    appearance,
                    a11y,
                    self.logged.clone(),
                    move |appearance, _, cx| {
                        log!("preview: appearance {appearance:?}");
                        page.update(cx, |page, cx| {
                            page.shown = Pictures::Appearance { appearance, right };
                            cx.notify();
                        })
                        .ok();
                    },
                );
                let page = cx.entity().downgrade();
                let pair = picker(
                    &look,
                    base,
                    dark,
                    right,
                    a11y,
                    self.logged.clone(),
                    move |side, _, cx| {
                        log!(
                            "preview: sidebar position {}",
                            browser::settings_pictures::title(side)
                        );
                        page.update(cx, |page, cx| {
                            page.shown = Pictures::Appearance {
                                appearance,
                                right: side,
                            };
                            cx.notify();
                        })
                        .ok();
                    },
                );
                let line = match cfg!(target_os = "macos") {
                    true => "Light, dark, or as macOS is. View › Appearance has it too.",
                    false => "Light, dark, or as the system is.",
                };
                let zoom = frame()
                    .child(row_text(
                        &look,
                        "Page zoom",
                        Some(
                            "Every page's size, until you zoom a site, which keeps its own.".into(),
                        ),
                    ))
                    .child(end([gpui::IntoElement::into_any_element(popup(
                        &look,
                        "100%",
                        POPUP_WIDTH,
                        false,
                    ))]));
                let top_bar = frame()
                    .child(row_text(
                        &look,
                        "Top bar",
                        Some(
                            "The URL field and the sidebar's buttons in a row across the window's \
                             top, the page under it. ⇧⌘D turns it on and off."
                                .into(),
                        ),
                    ))
                    .child(switch("top bar", false));
                // Transparency under Appearance, at the level kept, as Settings has it (#4129).
                let transparency = browser::transparency::row(
                    &look,
                    browser::transparency::has_glass(),
                    self.logged.clone(),
                    cx,
                );
                let mut looks = vec![pictured(&look, "Appearance", line.into(), three)];
                let mut groups = Vec::new();
                match transparency {
                    Some(transparency) if cfg!(target_os = "macos") => {
                        looks.push(transparency);
                        under = Some(browser::transparency::REDUCE_TRANSPARENCY_WINS);
                        groups.push((None, looks));
                        groups.push((None, vec![zoom]));
                    }
                    transparency => {
                        looks.extend(transparency);
                        looks.push(zoom);
                        groups.push((None, looks));
                    }
                }
                groups.extend([(
                    Some("Layout"),
                    vec![
                        pictured(
                            &look,
                            "Sidebar position",
                            "The side of the window the sidebar, the Library and the \
                                 window's buttons are on."
                                .into(),
                            pair,
                        ),
                        top_bar,
                    ],
                )]);
                groups
            }
            Pictures::General => {
                let button = |id: &'static str, title: &'static str| {
                    gpui::IntoElement::into_any_element(stop_button(&look, id, title, a11y))
                };
                let row = |name: &'static str, line: Option<&str>, control| {
                    frame()
                        .child(row_text(
                            &look,
                            name,
                            line.map(|line| line.to_string().into()),
                        ))
                        .child(end([control]))
                };
                let folder = div()
                    .flex()
                    .items_center()
                    .gap(px(CONTROL_GAP))
                    .child(div().text_color(look.detail).child("~/Downloads"))
                    .child(button("download-folder", "Change…"));
                vec![
                    (
                        Some("Opening and quitting"),
                        vec![
                            row(
                                "Reopen your windows and tabs",
                                Some(
                                    "Off: ilo opens one empty window, and Today tabs go to the \
                                     archive.",
                                ),
                                gpui::IntoElement::into_any_element(stop_switch(
                                    &look, "restore", "restore", true, 1., look.on, a11y,
                                )),
                            ),
                            row(
                                "Ask before quitting",
                                Some("⌘Q asks first, so a stray press can't close every window."),
                                gpui::IntoElement::into_any_element(stop_switch(
                                    &look, "warn", "warn", true, 1., look.on, a11y,
                                )),
                            ),
                        ],
                    ),
                    (
                        Some("Search and downloads"),
                        vec![
                            row(
                                "Search engine",
                                None,
                                gpui::IntoElement::into_any_element(popup(
                                    &look,
                                    "DuckDuckGo",
                                    POPUP_WIDTH,
                                    false,
                                )),
                            ),
                            row(
                                "Save downloads to",
                                None,
                                gpui::IntoElement::into_any_element(folder),
                            ),
                            row(
                                "Ask where to save each file",
                                Some(
                                    "Each download asks where it goes, rather than saving to its \
                                     folder.",
                                ),
                                gpui::IntoElement::into_any_element(stop_switch(
                                    &look, "ask", "ask", false, 1., look.on, a11y,
                                )),
                            ),
                        ],
                    ),
                ]
            }
            Pictures::Favorites(chosen) => {
                let three = favorites_picker(
                    &look,
                    base,
                    dark,
                    chosen,
                    a11y,
                    self.logged.clone(),
                    move |scope, _, cx| {
                        log!("preview: share favorites {}", scope.title());
                        page.update(cx, |page, cx| {
                            page.shown = Pictures::Favorites(scope);
                            cx.notify();
                        })
                        .ok();
                    },
                );
                let shared = frame()
                    .flex_col()
                    .items_stretch()
                    .child(row_text(
                        &look,
                        "Share favorites",
                        Some("Which Spaces show the same favorites.".into()),
                    ))
                    .child(
                        div()
                            .flex()
                            .mt(px(SHARE_TOP))
                            .ml(px(-SIDE_ROOM))
                            .child(three),
                    );
                vec![(None, vec![shared])]
            }
            Pictures::CommandLine(state) => {
                let button = |id: &'static str, title: &'static str| {
                    gpui::IntoElement::into_any_element(stop_button(&look, id, title, a11y))
                };
                let feature = |glyph, name: &'static str, line: &str, controls| {
                    frame()
                        .child(glyph_plate(&look, glyph, true))
                        .child(row_text(&look, name, Some(line.to_string().into())))
                        .child(end(controls))
                };
                let master = feature(
                    Section::Agents.glyph(),
                    "Let agents use the browser",
                    "On: a connected client reaches the browser through the bridge beside it.",
                    vec![gpui::IntoElement::into_any_element(stop_switch(
                        &look,
                        "agents",
                        "Let agents use the browser",
                        true,
                        1.,
                        look.on,
                        a11y,
                    ))],
                );
                let clients = vec![
                    feature(
                        "icons/terminal.svg",
                        "Claude Code",
                        "Connected in ~/.claude.json",
                        vec![button("disconnect-claude", "Disconnect")],
                    ),
                    feature(
                        "icons/square-terminal.svg",
                        "Codex",
                        "Not connected. Connect adds the browser to ~/.codex/config.toml",
                        vec![button("connect-codex", "Connect")],
                    ),
                    feature(
                        "icons/cable.svg",
                        "MCP client",
                        "Any MCP client",
                        vec![button("connect-other", "Connect")],
                    ),
                ];
                // The ilo command's actions, where the parity sheet waits for them to be drawn.
                let action = |title: &'static str| {
                    gpui::IntoElement::into_any_element(
                        stop_button(&look, title, title, a11y)
                            .relative()
                            .child(drawn(
                                format!("ilo-command {}", title.to_lowercase()),
                                self.logged.clone(),
                            )),
                    )
                };
                let flatpak = matches!(state, CommandLine::FlatpakOff | CommandLine::FlatpakOn)
                    .then(|| {
                        cli::flatpak_line(&browser::agents::bridge::launch(
                            std::path::Path::new("/app/ilo/mcp-bridge"),
                            Some("app.ilo.browser"),
                        ))
                    })
                    .flatten();
                let state = match state {
                    CommandLine::Off | CommandLine::FlatpakOff => cli::State::Off,
                    CommandLine::On | CommandLine::FlatpakOn => cli::State::On,
                    CommandLine::Unsearched => cli::State::Unsearched,
                };
                let detail = cli::detail(state, flatpak.as_deref(), None);
                let mono = browser::face::mono(&cx.text_system().all_font_names());
                let mut controls = Vec::new();
                if detail.export.is_some() {
                    controls.push(action("Copy Line"));
                }
                controls.push(action(match state {
                    cli::State::Off => "Add",
                    _ => "Remove",
                }));
                let export =
                    (detail.export.clone()).map(|export| command_export(&look, export, mono));
                let command = frame()
                    .flex_wrap()
                    .child(glyph_plate(&look, "icons/square-code.svg", true))
                    .child(command_text(&look, "ilo CLI", &detail, mono))
                    .child(end(controls))
                    .children(export);
                vec![
                    (None, vec![master]),
                    (Some("Apps that connect"), clients),
                    (Some("Command line"), vec![command]),
                ]
            }
            Pictures::Assistants { signed_in } => {
                use browser::agents::ready::Ready;
                use browser::agents::runner::Cli;
                use browser::agents::sign_in::{Row, shown};
                use browser::settings_rows::{another_agent_row, assistant_row};
                let row = |cli, ready, plan: Option<&str>, row: Row, marked| {
                    let shown = shown(cli, ready, plan, &row);
                    assistant_row(
                        &look,
                        cli,
                        &shown,
                        marked,
                        a11y,
                        gpui::canvas(|_, _, _| {}, |_, _, _, _| {}),
                        Rc::new(|_, _, _| {}),
                    )
                };
                // Where the parity sheet waits for the rows to be drawn.
                let another = || {
                    another_agent_row(&look, a11y, |_, _| {})
                        .relative()
                        .child(drawn("assistant rows".into(), self.logged.clone()))
                };
                // A made-up assistant's words, as a failed sign-in shows them.
                let said = "Your workspace doesn't allow Codex".to_string();
                match signed_in {
                    false => vec![
                        (
                            Some("Your assistant"),
                            vec![
                                row(Cli::Claude, Ready::SignedOut, None, Row::Idle, false),
                                row(Cli::Codex, Ready::NotFound, None, Row::Idle, false),
                                another(),
                            ],
                        ),
                        (
                            Some("Signing in"),
                            vec![row(
                                Cli::Claude,
                                Ready::SignedOut,
                                None,
                                Row::SigningIn,
                                false,
                            )],
                        ),
                        (
                            Some("Failed"),
                            vec![
                                row(
                                    Cli::Codex,
                                    Ready::SignedOut,
                                    None,
                                    Row::Failed(Some(said)),
                                    false,
                                ),
                                row(
                                    Cli::Claude,
                                    Ready::SignedOut,
                                    None,
                                    Row::Failed(None),
                                    false,
                                ),
                            ],
                        ),
                    ],
                    true => vec![
                        (
                            Some("Your assistant"),
                            vec![
                                row(Cli::Claude, Ready::SignedIn, None, Row::Idle, true),
                                row(Cli::Codex, Ready::SignedIn, Some("Plus"), Row::Idle, false),
                                another(),
                            ],
                        ),
                        (
                            Some("Signing out"),
                            vec![row(
                                Cli::Codex,
                                Ready::SignedIn,
                                Some("Plus"),
                                Row::SigningOut,
                                false,
                            )],
                        ),
                    ],
                }
            }
            Pictures::SiteNotes { open } => {
                use browser::agents::notes;
                use browser::settings_rows::site_note_row;
                let on = frame()
                    .child(row_text(
                        &look,
                        notes::HEADING,
                        Some(notes::SWITCH_LINE.into()),
                    ))
                    .child(switch(notes::HEADING, true));
                let rows = (notes::NOTES.iter().enumerate())
                    .map(|(index, note)| {
                        let (opened, logged) = (open && index == 0, self.logged.clone());
                        site_note_row(&look, note, opened, a11y, logged, |_, _, _| {})
                    })
                    .collect();
                vec![(None, vec![on]), (None, rows)]
            }
        };
        let blocks = blocks
            .into_iter()
            .enumerate()
            .map(|(index, (heading, rows))| {
                let heading = heading.map(|heading| {
                    div()
                        .px(px(TEXT_X))
                        .mb(px(HEADING_BOTTOM))
                        .text_size(px(text::SMALL))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(look.heading)
                        .child(heading)
                });
                let top = match (index, heading.is_some()) {
                    (0, _) => FIRST_GROUP,
                    (_, true) => HEADING_TOP,
                    (_, false) => GROUP_GAP,
                };
                let under = under.filter(|_| index == 0).map(|line| {
                    div()
                        .mt(px(BUTTONS_TOP))
                        .px(px(TEXT_X))
                        .text_size(px(text::SMALL))
                        .line_height(px(DETAIL_LINE))
                        .text_color(look.detail)
                        .child(line)
                });
                div()
                    .mt(px(top))
                    .children(heading)
                    .child(group(&look, rows))
                    .children(under)
            });
        // The page's name, and General's sentence and Make Default, as Settings draws them.
        let title = browser::shell::display_text(
            section.name(),
            browser::tokens::shell::library::TITLE_SIZE,
            browser::tokens::shell::library::TITLE_LINE,
            cx,
        )
        .text_color(base.opacity(browser::tokens::settings::TITLE));
        let general = matches!(self.shown, Pictures::General);
        let lede = general.then(|| {
            div()
                .mt(px(browser::tokens::settings::LEDE_TOP))
                .line_height(px(browser::tokens::settings::LEDE_LINE))
                .text_color(base.opacity(browser::tokens::settings::DETAIL))
                .child("ilo isn't your default browser.")
        });
        let make = general.then(|| {
            div()
                .flex_none()
                .ml(px(browser::tokens::settings::END_GAP))
                .child(stop_button(&look, "make-default", "Make Default", a11y))
        });
        let head = div()
            .px(px(TEXT_X))
            .flex()
            .items_start()
            .child(div().flex_1().min_w_0().child(title).children(lede))
            .children(make);
        let colors = Colors {
            line: look.line,
            name: base.opacity(NAME),
            heading: look.heading,
        };
        let card = self.card.draw(
            section.name(),
            div()
                .w_full()
                .max_w(px(browser::tokens::settings::PAGE_WIDTH))
                .mx_auto()
                .flex()
                .flex_col()
                .child(head)
                .children(blocks),
            dark,
            colors,
        );
        div()
            .relative()
            .size_full()
            .flex()
            .bg(rgb(if dark { STAND_IN } else { LIGHT_STAND_IN }))
            .font_family(browser::face::UI)
            .text_size(px(text::BODY))
            .text_color(look.ink)
            .child(column)
            .child(card)
    }
}
