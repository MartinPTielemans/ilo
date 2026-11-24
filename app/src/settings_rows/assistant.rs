//! Your assistant's rows (#4100): one for each assistant tasks run on, and another app's.

use super::*;

/// What a press on an assistant's row asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// Get: the maker's install page.
    Get,
    /// Sign in or Try Again.
    SignIn,
    Cancel,
    /// The mark, which has tasks run on it.
    Mark,
    /// The ⋯ menu, a press on its button.
    Menu,
}

/// An assistant's row (#4100), one component for Settings › Agents' Your assistant and first run's
/// assistant step: Claude Code's or Codex's glyph, its name and what `shown` says of it, then Get,
/// Sign in, Cancel or Try Again, or, signed in, its ⋯ menu, opening at `menu_at`, and the mark,
/// `marked` when tasks run on it. A press calls `act`.
pub fn assistant_row(
    look: &Look,
    cli: crate::agents::runner::Cli,
    shown: &crate::agents::sign_in::Shown,
    marked: bool,
    a11y: bool,
    menu_at: impl IntoElement,
    act: Rc<dyn Fn(Act, &mut gpui::Window, &mut gpui::App)>,
) -> gpui::Div {
    use crate::agents::sign_in::Offer;
    let index = cli as usize;
    let name = cli.assistant();
    let press = |id: &'static str, title: SharedString, what: Act| {
        let act = act.clone();
        stop_button(look, (id, index), title, a11y)
            .on_click(move |_, window, cx| act(what, window, cx))
            .into_any_element()
    };
    let title = |offer: Offer| offer.label(cli).unwrap_or_default().into();
    let controls = match shown.offer {
        Offer::Nothing => Vec::new(),
        Offer::Get => vec![press("assistant-get", title(Offer::Get), Act::Get)],
        Offer::SignIn => vec![press(
            "assistant-sign-in",
            title(Offer::SignIn),
            Act::SignIn,
        )],
        Offer::TryAgain => vec![press(
            "assistant-try-again",
            title(Offer::TryAgain),
            Act::SignIn,
        )],
        Offer::Cancel => vec![press("assistant-cancel", title(Offer::Cancel), Act::Cancel)],
        Offer::SignedIn => {
            let more = {
                let act = act.clone();
                icon_button(look, ("assistant-more", index), "icons/ellipsis.svg")
                    .relative()
                    .child(menu_at)
                    .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                        act(Act::Menu, window, cx)
                    })
                    .when(a11y, |more| named_button(more, format!("{name} options")))
            };
            let mark = {
                let act = act.clone();
                div()
                    .id(("assistant-mark", index))
                    .when(a11y, |mark| {
                        named_check(mark, format!("Run tasks with {name}"), marked)
                    })
                    .size(px(CONTROL_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self::mark(look, marked, look.on, look.knob_on))
                    .on_click(move |_, window, cx| act(Act::Mark, window, cx))
            };
            vec![more.into_any_element(), mark.into_any_element()]
        }
    };
    let line: SharedString = shown.line.clone().into();
    let text = match shown.command() {
        Some(command) => commanded(look, name, line, command).into_any_element(),
        None => text(look, name, Some(line)).into_any_element(),
    };
    frame()
        .child(glyph_plate(look, cli.glyph(), true))
        .child(text)
        .when(!controls.is_empty(), |row| row.child(end(controls)))
}

/// A row's text whose line has a command in it, set in the face a terminal shows it in, which only
/// the computer's fonts can say, so it's found as it's drawn.
#[derive(IntoElement)]
struct Commanded {
    look: Look,
    name: SharedString,
    line: SharedString,
    command: std::ops::Range<usize>,
}

fn commanded(
    look: &Look,
    name: impl Into<SharedString>,
    line: SharedString,
    command: std::ops::Range<usize>,
) -> Commanded {
    Commanded {
        look: *look,
        name: name.into(),
        line,
        command,
    }
}

impl RenderOnce for Commanded {
    fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let mono = crate::face::mono(&cx.text_system().all_font_names());
        let run = |len: usize, family: &'static str| gpui::TextRun {
            len,
            font: gpui::font(family),
            color: self.look.detail.into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = [
            run(self.command.start, crate::face::UI),
            run(self.command.len(), mono),
            run(self.line.len() - self.command.end, crate::face::UI),
        ]
        .into_iter()
        .filter(|run| run.len > 0);
        text(&self.look, self.name, None).child(
            div()
                .mt(px(DETAIL_TOP))
                .text_size(px(SMALL))
                .line_height(px(DETAIL_LINE))
                .text_color(self.look.detail)
                .child(gpui::StyledText::new(self.line).with_runs(runs.collect())),
        )
    }
}

/// Your assistant's last row: another app, which connects as Apps that connect has it, where
/// How to Connect goes.
pub fn another_agent_row(
    look: &Look,
    a11y: bool,
    how: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    another_app(look, a11y, how, Some("Any app that uses MCP".into()))
}

/// First run's Another app: its name alone, centred on the row's height, and How to Connect.
pub fn another_agent_name_row(
    look: &Look,
    a11y: bool,
    how: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    another_app(look, a11y, how, None)
}

fn another_app(
    look: &Look,
    a11y: bool,
    how: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
    line: Option<SharedString>,
) -> gpui::Div {
    let how = stop_button(look, "assistant-another-how", "How to Connect", a11y)
        .on_click(move |_, window, cx| how(window, cx));
    frame()
        .child(glyph_plate(look, "icons/cable.svg", true))
        .child(text(look, "Another app", line))
        .child(end([how.into_any_element()]))
}
