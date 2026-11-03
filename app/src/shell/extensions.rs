//! The extensions of the shown Space's profile: the menu bar's Extensions menu lists and runs them,
//! as their toolbar buttons do in Chrome, the pinned ones' actions show at the URL field's end, the
//! Site Controls' grid lists every turned-on one's, and their keyboard commands run from the page
//! and the UI (docs/ux-spec.md › Extensions).

use std::sync::Arc;

use gpui::{
    Action, AnyElement, App, Bounds, Context, FontWeight, Image, Keystroke, MenuItem, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString, div, img, point, prelude::*, px, rgb, rgba, size,
    svg,
};

use crate::site_controls;

use super::{
    MenuAction, MenuEntry, NAV, NAV_HOVERED, NAV_RADIUS, PLATE_INSET, PRESSED, Shell,
    ShellHandle, URL_BUTTON_GLYPH, URL_BUTTON_INSET, URL_BUTTON_SIZE, URL_FIELD_TEXT_X,
    URL_FIELD_TOP, UserKeys, user_keys, written_as,
};

use crate::tokens::shell::extensions::HOST_ROOM;
use crate::tokens::shell::extensions::ACTION_ICON;
use crate::tokens::shell::extensions::ACTION_DIMMED;
use crate::tokens::shell::extensions::BADGE_COLOR;
use crate::tokens::shell::extensions::BADGE_HEIGHT;
use crate::tokens::shell::extensions::BADGE_TEXT;
use crate::tokens::shell::extensions::BADGE_RADIUS;
use crate::tokens::shell::extensions::BADGE_TEXT_COLOR;

/// An extension of a profile's, one Chrome's extensions page lists.
#[derive(Clone, PartialEq, Debug)]
pub struct Extension {
    pub id: SharedString,
    pub name: SharedString,
    pub enabled: bool,
    /// Its action shows at the URL field's end.
    pub pinned: bool,
    /// The page its options are on, or "" for none.
    pub options: SharedString,
    pub commands: Vec<ExtensionCommand>,
    /// The user may remove it, which the browser's own ad blocker they turn off in Settings instead.
    pub removable: bool,
    /// The user allowed it in Incognito, so it runs in private windows too.
    pub incognito: bool,
    /// The browser turned it off as it crashed twice, and the user hasn't been told.
    pub crashed: bool,
}

impl Extension {
    /// Whether it runs in a window, `private` or not: Chrome runs only the extensions the user
    /// allowed in Incognito in a private session.
    pub fn runs(&self, private: bool) -> bool {
        self.enabled && (self.incognito || !private)
    }
}

/// One of an extension's keyboard commands that has a key.
#[derive(Clone, PartialEq, Debug)]
pub struct ExtensionCommand {
    /// Its name in the manifest, or "_execute_action" for its action.
    pub name: SharedString,
    pub description: SharedString,
    /// Its key, as the browser's shortcuts write them: "shift-cmd-y".
    pub keystroke: SharedString,
    /// The key is the one the extension suggested, not one the user set.
    pub suggested: bool,
}

/// An extension's action as the window shows it, for the tab shown.
#[derive(Clone)]
pub struct ExtensionAction {
    pub id: SharedString,
    pub title: SharedString,
    pub icon: Option<Arc<Image>>,
    pub badge: SharedString,
    /// 0xAARRGGBB, or 0 for Chrome's.
    pub badge_color: u32,
    /// It acts on the tab.
    pub enabled: bool,
    /// It shows at the URL field's end. The core reports the others only while the Site Controls
    /// show.
    pub pinned: bool,
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = browser, no_json)]
struct RunExtension(SharedString);

#[derive(Clone, PartialEq, Action)]
#[action(namespace = browser, no_json)]
struct PinExtension(SharedString);

/// The menu bar's item that runs `extension`.
pub fn menu_item(extension: &Extension) -> MenuItem {
    MenuItem::action(extension.name.clone(), RunExtension(extension.id.clone()))
}

/// The menu bar's item that pins `extension`'s action or unpins it, checked while it's pinned.
pub fn pin_menu_item(extension: &Extension) -> MenuItem {
    MenuItem::action(extension.name.clone(), PinExtension(extension.id.clone()))
        .checked(extension.pinned)
}

pub(super) fn bind(cx: &mut App) {
    // While the UI has the keyboard. The page hands its keys over before it sees them.
    cx.intercept_keystrokes(|event, _, cx| {
        if crate::dialog::asking(cx) {
            return;
        }
        let Some(shell) = cx.try_global::<ShellHandle>().map(|handle| handle.0.clone()) else {
            return;
        };
        let keystroke = keystroke_text(&event.keystroke);
        let ran = shell.update(cx, |shell, cx| shell.run_extension_key(&keystroke, "ui", cx));
        if ran.unwrap_or(false) {
            cx.stop_propagation();
        }
    })
    .detach();
    cx.on_action(|run: &RunExtension, cx| {
        if crate::dialog::asking(cx) {
            return;
        }
        if let Some(shell) = cx.try_global::<ShellHandle>().map(|handle| handle.0.clone()) {
            shell.update(cx, |shell, cx| shell.run_extension(&run.0, cx)).ok();
        }
    });
    cx.on_action(|pin: &PinExtension, cx| {
        if crate::dialog::asking(cx) {
            return;
        }
        if let Some(shell) = cx.try_global::<ShellHandle>().map(|handle| handle.0.clone()) {
            shell.update(cx, |shell, cx| shell.toggle_pinned(&pin.0, cx)).ok();
        }
    });
}

impl Shell {
    /// `profile`'s extensions, by name.
    pub fn set_extensions(
        &mut self,
        profile: u64,
        extensions: Vec<Extension>,
        cx: &mut Context<Self>,
    ) {
        // check.sh reads these lines.
        let ids: Vec<&str> = extensions.iter().map(|extension| extension.id.as_ref()).collect();
        log!("browser: extensions {profile} {}", ids.join(" "));
        let pinned = extensions.iter().filter(|extension| extension.pinned);
        let pinned: Vec<&str> = pinned.map(|extension| extension.id.as_ref()).collect();
        log!("browser: pinned extensions {profile} {}", pinned.join(" "));
        for extension in extensions.iter().filter(|extension| !extension.commands.is_empty()) {
            let keys: Vec<String> = (extension.commands.iter())
                .map(|command| {
                    let suggested = if command.suggested { " suggested" } else { "" };
                    format!("{}={}{suggested}", command.name, command.keystroke)
                })
                .collect();
            log!("browser: extension keys {} {}", extension.id, keys.join(", "));
        }
        // One the user added from an import's list that the other browser pinned is pinned as it
        // comes in, by whichever window hears of it first.
        if let Some(had) = self.extensions.get(&profile) {
            let had: Vec<&str> = had.iter().map(|extension| extension.id.as_ref()).collect();
            let now = extensions.iter().map(|each| (each.id.as_ref(), each.pinned));
            for id in crate::import::extensions::to_pin(&had, now) {
                log!("browser: extension {id} pinned as it was in the browser imported from");
                self.browser.set_extension_pinned(profile, &id, true, cx);
            }
        }
        self.extensions.insert(profile, extensions);
        self.update_menus(cx);
    }

    /// The actions the window shows, for its tab shown: the pinned ones, then, while the Site
    /// Controls show, the others, which only their grid lists.
    pub fn set_extension_actions(&mut self, actions: Vec<ExtensionAction>, cx: &mut Context<Self>) {
        let (pinned, unpinned) = actions.into_iter().partition(|action| action.pinned);
        self.extension_actions = pinned;
        self.unpinned_actions = unpinned;
        // check.sh waits for this line.
        let shown: Vec<String> = (self.extension_actions.iter())
            .map(|action| match action.badge.is_empty() {
                true => action.id.to_string(),
                false => format!("{}:{}", action.id, action.badge),
            })
            .collect();
        log!("browser: extension actions {}", shown.join(" "));
        // The panel takes them only while it shows for the tab.
        let grid = self.site_controls_extensions();
        self.browser.site_controls_extensions(self.selected, grid, cx);
        cx.notify();
    }

    /// The Site Controls' grid: every turned-on extension of the shown Space's profile, as its
    /// action is for the tab shown.
    pub(super) fn site_controls_extensions(&self) -> Vec<site_controls::Extension> {
        let profile = self.shown_space().map_or(0, |space| space.profile);
        let extensions = self.extensions.get(&profile).map_or(&[][..], Vec::as_slice);
        grid(&self.extension_actions, &self.unpinned_actions, extensions)
    }

    /// The name of the extension whose page `url` is, in the shown Space's profile, which the URL
    /// field shows in place of its id, as Chrome's omnibox does.
    pub(super) fn extension_name(&self, url: &str) -> Option<SharedString> {
        let id = url.strip_prefix("chrome-extension://")?.split('/').next()?;
        let profile = self.shown_space()?.profile;
        let extensions = self.extensions.get(&profile)?;
        let extension = extensions.iter().find(|extension| extension.id == id)?;
        Some(extension.name.clone())
    }

    /// Runs the extension's action on the tab shown, as a click on its toolbar button does in
    /// Chrome. Its popup hangs from the action's button if it's shown, or the overflow button it's
    /// behind, else under the URL field, or from the page's top left corner while the sidebar is
    /// hidden.
    pub fn run_extension(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.selected == 0 {
            return;
        }
        let (shown, overflow) = self.shown_actions();
        let button = (self.extension_actions.iter().position(|action| action.id == id))
            .map(|index| index.min(shown))
            .filter(|&place| place < shown || overflow)
            .filter(|_| self.url_field_bounds().is_some())
            .map(|place| self.action_bounds(place));
        let anchor = button.or(self.url_field_bounds()).unwrap_or_else(|| {
            let corner = point(px(self.card_left() + PLATE_INSET), px(PLATE_INSET));
            Bounds::new(corner, Default::default())
        });
        self.run_extension_from(id, anchor, cx);
    }

    /// Runs the extension's action on the tab shown, its popup hanging from `anchor` in the
    /// window, as from its Site Controls tile.
    pub fn run_extension_from(&mut self, id: &str, anchor: Bounds<Pixels>, cx: &mut Context<Self>) {
        if self.selected == 0 {
            return;
        }
        let ran = self.browser.run_extension(id, anchor, cx);
        // check.sh waits for these lines.
        let (origin, size) = (anchor.origin, anchor.size);
        log!(
            "browser: run extension {id} from {} {} {} {}",
            f32::from(origin.x),
            f32::from(origin.y),
            f32::from(size.width),
            f32::from(size.height)
        );
        log!("browser: run extension {id} {}", if ran { "ran" } else { "refused" });
    }

    /// Runs the keyboard command that `keystroke` is in the shown Space's profile, if there's one,
    /// and says whether it ran: an action runs as a click on its button does, and another command
    /// runs if its extension is listening.
    pub fn run_extension_key(
        &mut self,
        keystroke: &str,
        via: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let profile = self.shown_space().map_or(0, |space| space.profile);
        let extensions = self.extensions.get(&profile).map_or(&[][..], Vec::as_slice);
        let keys = user_keys(cx);
        let Some((id, name)) = (command_for(keystroke, extensions, self.private, keys))
            .map(|(extension, command)| (extension.id.clone(), command.name.clone()))
        else {
            return false;
        };
        let ran = match name.as_ref() {
            "_execute_action" => {
                self.run_extension(&id, cx);
                true
            }
            name => self.browser.run_extension_command(&id, name, cx),
        };
        // check.sh waits for this line.
        let result = if ran { "ran" } else { "refused" };
        log!("browser: extension command {id} {name} {result} via {via}");
        ran
    }

    /// Pins the extension's action in the shown Space's profile, or unpins it.
    fn toggle_pinned(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(profile) = self.shown_space().map(|space| space.profile) else {
            return;
        };
        let extensions = self.extensions.get(&profile).into_iter().flatten();
        let pinned = extensions.filter(|extension| extension.id == id).any(|each| each.pinned);
        // check.sh waits for this line.
        log!("browser: {} extension {id}", if pinned { "unpin" } else { "pin" });
        self.browser.set_extension_pinned(profile, id, !pinned, cx);
    }

    /// Makes the extension's action the first pinned in the shown Space's profile, pinning it if it
    /// isn't. The core pins after the others, so the others pinned go again, in their order.
    fn move_to_top(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(profile) = self.shown_space().map(|space| space.profile) else {
            return;
        };
        let pinned: Vec<SharedString> =
            self.extension_actions.iter().map(|action| action.id.clone()).collect();
        // check.sh waits for this line.
        log!("browser: move extension {id} to top");
        if !pinned.iter().any(|each| each == id) {
            self.browser.set_extension_pinned(profile, id, true, cx);
        }
        for other in pinned.iter().filter(|each| *each != id) {
            self.browser.set_extension_pinned(profile, other, false, cx);
            self.browser.set_extension_pinned(profile, other, true, cx);
        }
    }

    /// Does what the extension's Site Controls tile's menu chose, as the pinned action's menu does.
    pub fn extension_item(
        &mut self,
        id: &SharedString,
        item: site_controls::ExtensionItem,
        cx: &mut Context<Self>,
    ) {
        let profile = self.shown_space().map_or(0, |space| space.profile);
        let Some(extension) = (self.extensions.get(&profile).into_iter().flatten())
            .find(|extension| extension.id == *id)
            .cloned()
        else {
            return;
        };
        match item {
            site_controls::ExtensionItem::MoveToTop => self.move_to_top(id, cx),
            site_controls::ExtensionItem::Pin | site_controls::ExtensionItem::Unpin => {
                self.toggle_pinned(id, cx)
            }
            site_controls::ExtensionItem::Options => {
                self.browser.open_extension_options(&extension.options, cx)
            }
            site_controls::ExtensionItem::Remove => {
                self.remove_extension(profile, id.clone(), extension.name, cx)
            }
            site_controls::ExtensionItem::Manage => self.browser.manage_extensions(Some(id), cx),
        }
    }

    /// How many of the actions show, and whether the overflow button follows them, in the places
    /// that leave the host its room with the field's button shown, so they keep their places as
    /// it comes and goes.
    pub(super) fn shown_actions(&self) -> (usize, bool) {
        let field = self.width() - 2. * PLATE_INSET;
        let room = field - (URL_FIELD_TEXT_X - PLATE_INSET) - HOST_ROOM - URL_BUTTON_INSET;
        let places = ((room - URL_BUTTON_SIZE) / URL_BUTTON_SIZE).floor().max(0.) as usize;
        fit_actions(places, self.extension_actions.len())
    }

    /// Place `index`'s button in the window, the places in a row at the URL field's end.
    fn action_bounds(&self, index: usize) -> Bounds<Pixels> {
        let (shown, overflow) = self.shown_actions();
        let end = self.width() - PLATE_INSET - URL_BUTTON_INSET;
        let left = end - (shown + overflow as usize - index) as f32 * URL_BUTTON_SIZE;
        Bounds::new(
            point(px(left), px(URL_FIELD_TOP + URL_BUTTON_INSET)),
            size(px(URL_BUTTON_SIZE), px(URL_BUTTON_SIZE)),
        )
    }

    /// The shown actions' buttons, for the URL field's end: a click runs one, and a right-click
    /// shows its menu. Then the overflow button, whose menu runs the others.
    pub(super) fn action_buttons(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (shown, overflow) = self.shown_actions();
        let places = shown + overflow as usize;
        let actions = self.extension_actions.iter().take(shown).enumerate();
        let mut buttons: Vec<AnyElement> = actions
            .map(|(index, action)| {
                let id = action.id.clone();
                let menu_id = action.id.clone();
                let badge = (!action.badge.is_empty()).then(|| {
                    let color = match action.badge_color {
                        0 => rgb(BADGE_COLOR),
                        // 0xAARRGGBB to GPUI's 0xRRGGBBAA.
                        argb => rgba(argb << 8 | argb >> 24),
                    };
                    div()
                        .absolute()
                        .right(px(2.))
                        .bottom(px(3.))
                        .h(px(BADGE_HEIGHT))
                        .min_w(px(BADGE_HEIGHT))
                        .px(px(2.))
                        .rounded(px(BADGE_RADIUS))
                        .bg(color)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(BADGE_TEXT))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(BADGE_TEXT_COLOR))
                        .child(action.badge.clone())
                });
                div()
                    .id(SharedString::from(format!("action-{}", action.id)))
                    .absolute()
                    .top(px(URL_BUTTON_INSET))
                    .right(px(URL_BUTTON_INSET + (places - 1 - index) as f32 * URL_BUTTON_SIZE))
                    .size(px(URL_BUTTON_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(NAV_RADIUS))
                    .block_mouse_except_scroll()
                    .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                    .active(|style| style.bg(self.palette.plate(PRESSED)))
                    .on_click(cx.listener(move |shell, _, _, cx| shell.run_extension(&id, cx)))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.action_menu(menu_id.clone(), event.position, cx)
                        }),
                    )
                    .children(action.icon.clone().map(|icon| {
                        let icon = img(icon).size(px(ACTION_ICON));
                        icon.when(!action.enabled, |icon| icon.opacity(ACTION_DIMMED))
                    }))
                    .children(badge)
                    .into_any_element()
            })
            .collect();
        if overflow {
            buttons.push(
                div()
                    .id("actions-overflow")
                    .absolute()
                    .top(px(URL_BUTTON_INSET))
                    .right(px(URL_BUTTON_INSET))
                    .size(px(URL_BUTTON_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(NAV_RADIUS))
                    .block_mouse_except_scroll()
                    .hover(|style| style.bg(self.palette.plate(NAV_HOVERED)))
                    .active(|style| style.bg(self.palette.plate(PRESSED)))
                    .on_click(cx.listener(|shell, _, _, cx| shell.overflow_menu(cx)))
                    .child(
                        svg()
                            .path("icons/ellipsis.svg")
                            .size(px(URL_BUTTON_GLYPH))
                            .text_color(self.palette.glyph(NAV)),
                    )
                    .into_any_element(),
            );
        }
        buttons
    }

    /// The overflow button's menu, under it: the actions that don't fit, by title, each of which
    /// runs its action, its popup hanging from the button.
    fn overflow_menu(&mut self, cx: &mut Context<Self>) {
        let (shown, _) = self.shown_actions();
        let items: Vec<(MenuEntry, MenuAction)> = (self.extension_actions.iter().skip(shown))
            .map(|action| {
                let id = action.id.clone();
                let run: MenuAction = Box::new(move |shell, cx| shell.run_extension(&id, cx));
                (MenuEntry::named(action.title.clone(), true), run)
            })
            .collect();
        let button = self.action_bounds(shown);
        self.show_menu(items, button.bottom_left(), None, cx);
    }

    /// A pinned action's menu, as Chrome's toolbar gives one: Unpin, Options if it has them, Open
    /// Side Panel if it has one for the tab, Remove if the user may, and Manage Extension, which
    /// opens its details on Chrome's extensions page.
    fn action_menu(&mut self, id: SharedString, position: Point<Pixels>, cx: &mut Context<Self>) {
        let profile = self.shown_space().map_or(0, |space| space.profile);
        let Some(extension) = (self.extensions.get(&profile).into_iter().flatten())
            .find(|extension| extension.id == id)
            .cloned()
        else {
            return;
        };
        let (unpin, remove, manage) = (id.clone(), id.clone(), id.clone());
        let mut items: Vec<(MenuEntry, MenuAction)> = vec![(
            MenuEntry::new("Unpin", true),
            Box::new(move |shell, cx| shell.toggle_pinned(&unpin, cx)),
        )];
        if !extension.options.is_empty() {
            let options = extension.options.clone();
            items.push((
                MenuEntry::new("Options", true),
                Box::new(move |shell, cx| shell.browser.open_extension_options(&options, cx)),
            ));
        }
        if self.browser.has_side_panel(&id) {
            let label = match self.side_panel_shown(&id) {
                true => "Close Side Panel",
                false => "Open Side Panel",
            };
            items.push((
                MenuEntry::new(label, true),
                Box::new(move |shell, cx| shell.browser.toggle_side_panel(&id, cx)),
            ));
        }
        if extension.removable {
            items.push((
                MenuEntry::new("Remove", true).destructive(),
                Box::new(move |shell, cx| {
                    shell.remove_extension(profile, remove, extension.name, cx)
                }),
            ));
        }
        items.push((
            MenuEntry::new("Manage Extension", true),
            Box::new(move |shell, cx| shell.browser.manage_extensions(Some(&manage), cx)),
        ));
        self.show_menu(items, position, None, cx);
    }

    /// Removes the extension from the profile once the user confirms, as Chrome asks first.
    fn remove_extension(
        &mut self,
        profile: u64,
        id: SharedString,
        name: SharedString,
        cx: &mut Context<Self>,
    ) {
        let shell = cx.weak_entity();
        self.browser.confirm(
            format!("Remove “{name}”?"),
            "Its settings and the data it keeps go with it.".into(),
            "Remove",
            Box::new(move |cx| {
                shell
                    .update(cx, |shell, cx| {
                        let removed = shell.browser.remove_extension(profile, &id, cx);
                        // check.sh waits for this line.
                        let result = if removed { "removed" } else { "refused" };
                        log!("browser: remove extension {id} {result}");
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// Tells the user, in the browser's dialog, of an extension of `profile`'s that the browser
    /// turned off as it crashed twice: Turn On Again turns it on, Keep Off leaves it off, and
    /// Remove… asks to remove it. The core hears once it's been told, so it asks once, and it asks
    /// at the next launch if the dialog was busy or the browser went first.
    pub fn tell_crashed_extension(&mut self, profile: u64, cx: &mut Context<Self>) {
        let extensions = self.extensions.get(&profile).map_or(&[][..], Vec::as_slice);
        let Some(extension) = extensions.iter().find(|extension| extension.crashed).cloned() else {
            return;
        };
        let Extension { id, name, removable, .. } = extension;
        let shell = cx.weak_entity();
        let also = removable.then(|| {
            let (id, name) = (id.clone(), name.clone());
            let remove: Box<dyn FnOnce(&mut App)> = Box::new(move |cx| {
                shell.update(cx, |shell, cx| shell.remove_extension(profile, id, name, cx)).ok();
            });
            (SharedString::from("Remove…"), remove)
        });
        let question = crate::dialog::Question {
            icon: None,
            message: format!("“{name}” was turned off").into(),
            detail: "It crashed twice, so the browser turned it off. It stays off until you turn \
                     it on again."
                .into(),
            list: Vec::new(),
            action: "Turn On Again".into(),
            cancel: "Keep Off".into(),
            tone: crate::dialog::Tone::Accent,
            also,
            fields: Vec::new(),
            check: None,
            on_return: crate::dialog::Return::Action,
        };
        let browser = self.browser.clone();
        let turn_on = id.clone();
        let asked = self.browser.tell(
            question,
            Box::new(move |_, cx| {
                let on = browser.turn_on_extension(profile, &turn_on, cx);
                // check.sh waits for this line.
                log!("browser: turn on extension {turn_on} {}", if on { "on" } else { "refused" });
            }),
            cx,
        );
        if asked {
            self.browser.extension_crash_told(profile, &id, cx);
            // check.sh waits for this line.
            log!("browser: told extension {id} crashed");
        }
    }
}

/// The command `keystroke` runs among the `extensions`' that run in a window, `private` or not,
/// as Chrome gives them keys: never one of the browser's reserved shortcuts, and one of its others
/// only if the user set it, as a suggested key that's taken isn't given.
fn command_for<'a>(
    keystroke: &str,
    extensions: &'a [Extension],
    private: bool,
    keys: &UserKeys,
) -> Option<(&'a Extension, &'a ExtensionCommand)> {
    if keys.command_for(keystroke, true).is_some() {
        return None;
    }
    let ours = keys.command_for(keystroke, false).is_some();
    extensions.iter().filter(|extension| extension.runs(private)).find_map(|extension| {
        (extension.commands.iter())
            .find(|command| command.keystroke == keystroke && !(ours && command.suggested))
            .map(|command| (extension, command))
    })
}

/// `keystroke` as the browser's shortcuts write it, with the modifiers in their order, and off the
/// Mac in the Mac's names, as the core lists the extensions' keys.
pub(super) fn keystroke_text(keystroke: &Keystroke) -> String {
    let modifiers = &keystroke.modifiers;
    let mut text = String::new();
    for (held, name) in [
        (modifiers.control, "ctrl-"),
        (modifiers.alt, "alt-"),
        (modifiers.shift, "shift-"),
        (modifiers.platform, "cmd-"),
    ] {
        if held {
            text.push_str(name);
        }
    }
    // Off the Mac the extensions' keys are written as the page's: cmd-y for ctrl-y.
    written_as(&(text + &keystroke.key))
}

/// The Site Controls' grid, from the window's `pinned` and `unpinned` actions and the profile's
/// `extensions`, which are by name: the pinned in the URL field's order, then the others by name,
/// leaving out any the profile has turned off. An action that doesn't act on the tab is dimmed.
fn grid(
    pinned: &[ExtensionAction],
    unpinned: &[ExtensionAction],
    extensions: &[Extension],
) -> Vec<site_controls::Extension> {
    let turned_on = |id: &SharedString| {
        extensions.iter().find(|extension| extension.id == *id).is_none_or(|each| each.enabled)
    };
    let tile = |action: &ExtensionAction| {
        let extension = extensions.iter().find(|extension| extension.id == action.id);
        site_controls::Extension {
            id: action.id.clone(),
            title: action.title.clone(),
            icon: action.icon.clone(),
            enabled: action.enabled,
            pinned: action.pinned,
            options: extension.is_some_and(|extension| !extension.options.is_empty()),
            removable: extension.is_some_and(|extension| extension.removable),
        }
    };
    let others = (extensions.iter())
        .filter(|extension| extension.enabled)
        .filter_map(|extension| unpinned.iter().find(|action| action.id == extension.id));
    let pinned = pinned.iter().filter(|action| turned_on(&action.id));
    pinned.chain(others).map(tile).collect()
}

/// How many of `count` actions show in `places`, and whether the overflow button takes the last
/// place, for the rest.
fn fit_actions(places: usize, count: usize) -> (usize, bool) {
    match count <= places {
        true => (count, false),
        false => (places.saturating_sub(1), places > 0),
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{AppContext, TestAppContext, point, px};

    use super::{Extension, ExtensionAction, ExtensionCommand, UserKeys, command_for, fit_actions};
    use crate::shell::{Command, Folders, Shell, recorder::Recorder};
    use crate::site_controls;

    fn extension(id: &str, enabled: bool, commands: &[(&str, &str, bool)]) -> Extension {
        Extension {
            id: id.to_string().into(),
            name: id.to_string().into(),
            enabled,
            pinned: false,
            options: Default::default(),
            commands: (commands.iter())
                .map(|&(name, keystroke, suggested)| ExtensionCommand {
                    name: name.to_string().into(),
                    description: Default::default(),
                    keystroke: keystroke.to_string().into(),
                    suggested,
                })
                .collect(),
            removable: true,
            incognito: false,
            crashed: false,
        }
    }

    /// An extension the browser turned off as it crashed twice is told of once, in the browser's
    /// dialog, whose Turn On Again turns it on; with the dialog busy, it waits to be told.
    #[gpui::test]
    fn a_crashed_extension_is_told_of_once_and_turns_on_again(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        let crashed = Extension { crashed: true, ..extension("wallet", false, &[]) };
        host.dialog_busy.set(true);
        shell.update(cx, |shell, cx| {
            shell.set_extensions(3, vec![extension("notes", true, &[]), crashed.clone()], cx);
            shell.tell_crashed_extension(3, cx);
        });
        assert!(host.told.borrow().is_empty());
        assert!(!host.take().iter().any(|call| call.starts_with("crash told")));

        host.dialog_busy.set(false);
        shell.update(cx, |shell, cx| shell.tell_crashed_extension(3, cx));
        assert_eq!(
            *host.told.borrow(),
            ["“wallet” was turned off: Turn On Again / Keep Off / Remove…"]
        );
        assert!(host.take().contains(&"crash told 3 wallet".to_string()));
        let turn_on = host.told_action.borrow_mut().take().expect("its action");
        cx.update(|cx| turn_on(Vec::new(), cx));
        assert!(host.take().contains(&"turn on 3 wallet".to_string()));

        // Told, the core reports it no longer crashed, and nothing asks again.
        shell.update(cx, |shell, cx| {
            let told = Extension { crashed: false, ..crashed };
            shell.set_extensions(3, vec![told], cx);
            shell.tell_crashed_extension(3, cx);
        });
        assert_eq!(host.told.borrow().len(), 1);
    }

    /// The browser's own ad blocker comes back at each launch, so its menu offers no Remove: the
    /// user turns it off in Settings.
    #[gpui::test]
    fn the_ad_blockers_menu_has_no_remove(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        let menu = |id: &'static str, cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| shell.action_menu(id.into(), point(px(0.), px(0.)), cx));
            let items = host.menus.borrow().last().cloned().unwrap_or_default();
            items.into_iter().map(|(title, _)| title.to_string()).collect::<Vec<_>>()
        };
        shell.update(cx, |shell, cx| {
            shell.set_spaces(vec![(1, "Space 1".into(), None, 0, "".into(), "".into(), "".into())], cx);
            let blocker = Extension { removable: false, ..extension("ublock", true, &[]) };
            shell.set_extensions(0, vec![extension("notes", true, &[]), blocker], cx);
            shell.set_space(1, cx);
        });
        assert_eq!(menu("notes", cx), ["Unpin", "Remove", "Manage Extension"]);
        assert_eq!(menu("ublock", cx), ["Unpin", "Manage Extension"]);
    }

    /// A key the user set wins over the browser's, except a reserved one, and a key an extension
    /// suggested never takes one of the browser's.
    #[test]
    fn extension_keys_give_way_to_the_browsers_as_in_chrome() {
        let extensions = [
            extension("off", false, &[("hello", "shift-cmd-y", false)]),
            extension("a", true, &[("hello", "shift-cmd-y", true), ("tab", "cmd-t", false)]),
            extension("b", true, &[("library", "shift-cmd-l", true), ("side", "cmd-s", false)]),
        ];
        let run = |keystroke| {
            command_for(keystroke, &extensions, false, &UserKeys::default())
                .map(|(extension, command)| format!("{} {}", extension.id, command.name))
        };
        assert_eq!(run("shift-cmd-y").as_deref(), Some("a hello"));
        assert_eq!(run("cmd-t"), None);
        assert_eq!(run("shift-cmd-l"), None);
        assert_eq!(run("cmd-s").as_deref(), Some("b side"));
        assert_eq!(run("alt-shift-p"), None);
    }

    /// A private window runs the keys of, and lists in its menu bar, only the extensions the user
    /// allowed in Incognito, as Chrome runs only those in a private session.
    #[gpui::test]
    fn a_private_window_runs_only_extensions_allowed_in_incognito(cx: &mut TestAppContext) {
        let allowed = extension("allowed", true, &[("go", "shift-cmd-y", true)]);
        let allowed = Extension { incognito: true, ..allowed };
        let other = extension("other", true, &[("side", "shift-cmd-u", true)]);
        let extensions = vec![other, allowed];
        let run = |keystroke, private| {
            command_for(keystroke, &extensions, private, &UserKeys::default())
                .map(|(extension, _)| extension.id.to_string())
        };
        assert_eq!(run("shift-cmd-u", false).as_deref(), Some("other"));
        assert_eq!(run("shift-cmd-u", true), None);
        assert_eq!(run("shift-cmd-y", true).as_deref(), Some("allowed"));

        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::private_window(host.clone(), 1, "Private".into(), 7, cx));
        let listed = shell.update(cx, |shell, cx| {
            shell.set_extensions(7, extensions.clone(), cx);
            let bar = shell.menu_bar(cx);
            bar.extensions.iter().map(|extension| extension.id.to_string()).collect::<Vec<_>>()
        });
        assert_eq!(listed, ["allowed"]);
    }

    fn action(id: &str, pinned: bool) -> ExtensionAction {
        ExtensionAction {
            id: id.to_string().into(),
            title: id.to_string().into(),
            icon: None,
            badge: Default::default(),
            badge_color: 0,
            enabled: true,
            pinned,
        }
    }

    /// The Site Controls list every turned-on extension of the shown Space's profile: the pinned
    /// ones first, in the URL field's order, then the others by name, which the core reports once
    /// the panel opens. A turned-off one stays off the grid, and the URL field keeps the pinned.
    #[gpui::test]
    fn site_controls_list_every_turned_on_extension_pinned_first(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space 1".into(), None, 7, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
            let pinned = |mut extension: Extension| {
                extension.pinned = true;
                extension
            };
            // By name, as the core lists them.
            let extensions = vec![
                extension("alpha", true, &[]),
                pinned(extension("beta", true, &[])),
                extension("delta", true, &[]),
                extension("gamma", false, &[]),
                pinned(extension("zeta", true, &[])),
            ];
            shell.set_extensions(7, extensions, cx);
            // Zeta was pinned first.
            shell.set_extension_actions(vec![action("zeta", true), action("beta", true)], cx);
            shell.run(Command::SiteControls, "ui", cx);
        });
        let last = || host.site_controls_extensions.borrow().last().cloned().unwrap_or_default();
        assert_eq!(host.site_controls.borrow().len(), 1);
        assert_eq!(last(), ["zeta", "beta"]);
        shell.update(cx, |shell, cx| {
            let others = [action("delta", false), action("gamma", false), action("alpha", false)];
            let actions = [action("zeta", true), action("beta", true)].into_iter().chain(others);
            shell.set_extension_actions(actions.collect(), cx);
        });
        assert_eq!(last(), ["zeta", "beta", "alpha", "delta"]);
        let toolbar: Vec<String> = (shell.read_with(cx, |shell, _| shell.extension_actions.clone()))
            .iter()
            .map(|action| action.id.to_string())
            .collect();
        assert_eq!(toolbar, ["zeta", "beta"]);
    }

    /// Move to Top makes an extension the first pinned, pinning it if it isn't: the core pins after
    /// the others, so the others pinned go again, in their order.
    #[gpui::test]
    fn move_to_top_pins_it_first(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space 1".into(), None, 7, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            let extensions = ["alpha", "beta", "zeta"].map(|id| extension(id, true, &[]));
            shell.set_extensions(7, extensions.to_vec(), cx);
            shell.set_extension_actions(vec![action("zeta", true), action("beta", true)], cx);
        });
        host.take();
        let move_to_top = |id: &'static str, cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| {
                shell.extension_item(&id.into(), site_controls::ExtensionItem::MoveToTop, cx)
            });
            host.take()
        };
        assert_eq!(move_to_top("beta", cx), ["pin 7 zeta false", "pin 7 zeta true"]);
        assert_eq!(
            move_to_top("alpha", cx),
            [
                "pin 7 alpha true",
                "pin 7 zeta false",
                "pin 7 zeta true",
                "pin 7 beta false",
                "pin 7 beta true"
            ]
        );
    }

    /// Every pinned action has a place to be run from: its own, or the overflow button's.
    #[test]
    fn actions_that_dont_fit_are_behind_the_overflow_button() {
        assert_eq!(fit_actions(2, 0), (0, false));
        assert_eq!(fit_actions(2, 2), (2, false));
        assert_eq!(fit_actions(2, 3), (1, true));
        assert_eq!(fit_actions(1, 5), (0, true));
        // With no place at all, the Extensions menu runs them.
        assert_eq!(fit_actions(0, 1), (0, false));
    }
}
