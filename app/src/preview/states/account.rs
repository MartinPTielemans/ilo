//! The account page and its made-up states.

use crate::*;

/// Settings › Sync alone, on the card as Settings shows a section, in a window of Settings' size.
pub(crate) fn preview_account(state: &str, cx: &mut App) {
    use account::{Device, State, Status};
    use browser::tokens::settings::{HEIGHT, WIDTH};
    account::bind_keys(cx);
    // PREVIEW_SIZE=<width>x<height> as for the browser's window, Settings' own size by default.
    let (width, height) = std::env::var("PREVIEW_SIZE")
        .ok()
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        })
        .unwrap_or((WIDTH, HEIGHT));
    // PREVIEW_SCROLL=<points> scrolls the card that far down once it's drawn.
    let scroll: f32 = std::env::var("PREVIEW_SCROLL")
        .ok()
        .and_then(|points| points.parse().ok())
        .unwrap_or(0.);
    let (mut x, mut y) = (0., 0.);
    unsafe { host_centered(width as f64, height as f64, &mut x, &mut y) };
    let bounds = Bounds::new(
        point(px(x as f32), px(y as f32)),
        size(px(width), px(height)),
    );
    let status = match state {
        "words" => Status::FirstDevice,
        "waiting" => Status::Pairing,
        "signed-in" => Status::On,
        _ => Status::SignedOut,
    };
    let no_server = state == "no-server";
    // PREVIEW_SERVER=<address> signs in to a server of the user's, which the line names by its host.
    let own = std::env::var("PREVIEW_SERVER").ok();
    let state = State {
        status,
        problem: None,
        server: match (&own, no_server) {
            (_, true) => "".into(),
            (Some(own), false) => own.clone().into(),
            (None, false) => "https://sync.ilo.app".into(),
        },
        default_server: !no_server && own.is_none(),
        email: "sam@example.com".into(),
        code: if status == Status::Pairing {
            "312568".into()
        } else {
            "".into()
        },
        settings: true,
        extensions: false,
    };
    let device = |id: &str, name: &str, os: &str, current| Device {
        id: id.to_string().into(),
        name: name.to_string().into(),
        os: os.to_string().into(),
        current,
    };
    let devices = vec![
        device("1", "Sam's MacBook Pro", "macOS", true),
        device("2", "MacBook Air", "macOS", false),
        device("3", "ThinkPad X1", "Linux", false),
    ];
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let page = cx.new(|cx| {
            let dialog = dialog::in_window(Rc::new(PreviewDialog), cx);
            let host = Rc::new(PreviewAccount {
                dialog: dialog.clone(),
            });
            let account = cx.new(|cx| {
                let mut account = Account::new(host, cx);
                account.set_state(state, cx);
                if status == Status::On {
                    account.set_devices(devices, cx);
                }
                account
            });
            let look = browser::settings_rows::Look::settings(true);
            let search = cx.new(|cx| {
                browser::text_input::TextInput::new("Search", cx)
                    .placeholder_color(look.detail)
                    .caret_color(look.ink)
            });
            AccountPage {
                account,
                dialog,
                card: Card::default(),
                search,
            }
        });
        window.focus(&page.read(cx).account.focus_handle(cx), cx);
        if scroll > 0. {
            let scrolled = page.clone();
            window.on_next_frame(move |_, cx| {
                scrolled.update(cx, |page, cx| {
                    page.card.scroll.set_offset(point(px(0.), px(-scroll)));
                    cx.notify();
                });
            });
        }
        page
    });
    if let Err(error) = opened {
        log!("preview: couldn't open the account's window: {error}");
    }
}

/// The account's view on Settings' card, beside Settings' column with Sync lit, with the dialog
/// it asks in over all of it, as Settings has on Linux.
pub(crate) struct AccountPage {
    account: gpui::Entity<Account>,
    dialog: gpui::Entity<dialog::Dialog>,
    card: Card,
    /// Settings' search field, at the column's top.
    search: gpui::Entity<browser::text_input::TextInput>,
}

impl gpui::Render for AccountPage {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use browser::palette::{INK, PAPER};
        use browser::settings_card::Colors;
        use browser::settings_column::{self, Section};
        use browser::settings_rows::Look;
        use browser::tokens::palette::{LIGHT_STAND_IN, STAND_IN};
        use browser::tokens::settings::NAME;
        use browser::tokens::text;
        use gpui::{ParentElement, Styled, div, rgb};
        // Dark, as Settings is in a dark system, unless PREVIEW_APPEARANCE=light.
        let dark = preview_tone() != Some(Tone::Light);
        let look = Look::settings(dark);
        self.account
            .update(cx, |account, cx| account.set_look(look, cx));
        let field = browser::settings_search::field(
            &self.search,
            if dark { rgb(PAPER) } else { rgb(INK) },
            gpui::canvas(|_, _, _| {}, |_, _, _, _| {}),
            |_, _| {},
            cx,
        );
        let column = settings_column::column(
            &Default::default(),
            Section::Sync,
            dark,
            Rc::default(),
            gpui::IntoElement::into_any_element(field),
            None,
            Rc::new(|_, _, _| {}),
            Rc::new(|_, _| {}),
            window.is_a11y_active(),
        );
        let base = if dark { rgb(PAPER) } else { rgb(INK) };
        let colors = Colors {
            line: look.line,
            name: base.opacity(NAME),
            heading: look.heading,
        };
        let card = self.card.draw(
            Section::Sync.name(),
            div().child(self.account.clone()),
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
            .children(dialog::cover(&self.dialog, cx))
    }
}

/// A made-up account: its recovery words are made-up words, and it asks nothing of a server. It
/// asks the user in its window's dialog.
pub(crate) struct PreviewAccount {
    dialog: gpui::Entity<dialog::Dialog>,
}

impl account::Host for PreviewAccount {
    fn set_server(&self, _: &str) -> bool {
        true
    }
    fn sign_in(&self) {}
    fn cancel(&self) {}
    fn sign_out(&self) {}
    fn recovery_words(&self) -> String {
        "harbor velvet nominee orbit quarter skill tumble canyon effort glide lunar prize \
         answer bridge cotton dwarf frozen hollow island jacket kitten ladder mango noble"
            .into()
    }
    fn enter_recovery_words(&self, _: &str) -> bool {
        true
    }
    fn start_over(&self) {}
    fn request_pairing(&self) {}
    fn read_devices(&self) {}
    fn remove_device(&self, _: &str) {}
    fn choose(&self, _: account::Choice, _: bool) {}
    // PREVIEW_SYNC=never, offline, server-error, refused, sign-in, throttled, stopped or update
    // shows that line, else synced at 15:04.
    fn activity(&self) -> account::Activity {
        let shown = std::env::var("PREVIEW_SYNC").unwrap_or_default();
        let trouble = match shown.as_str() {
            "offline" => Some(account::Trouble::Offline),
            "server-error" => Some(account::Trouble::ServerError),
            "refused" => Some(account::Trouble::Refused {
                kind: account::Kind::Spaces,
                count: 3,
            }),
            "sign-in" => Some(account::Trouble::SignIn),
            "throttled" => Some(account::Trouble::Throttled),
            "stopped" => Some(account::Trouble::Stopped),
            "update" => Some(account::Trouble::Update),
            _ => None,
        };
        let synced = if shown == "never" { "" } else { "at 15:04" };
        let lasting = matches!(
            shown.as_str(),
            "offline" | "server-error" | "refused" | "sign-in"
        );
        account::Activity {
            synced: synced.into(),
            trouble,
            since: if lasting { "since 14:52" } else { "" }.into(),
        }
    }
    fn open_sync_details(&self, _: &mut App) {}
    fn delete_account(&self) {}
    fn copy_secret(&self, _: &str, _: Duration, _: &mut App) {}
    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        on_return: dialog::Return,
        confirmed: Box<dyn FnOnce(&mut App)>,
        window: &mut gpui::Window,
        cx: &mut App,
    ) {
        dialog::ask_in(
            &self.dialog,
            message,
            detail,
            action,
            on_return,
            confirmed,
            window,
            cx,
        );
    }
}
