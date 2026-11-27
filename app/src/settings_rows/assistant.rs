//! Your assistant's rows (#4100): one for each assistant tasks run on, and another app's.

use super::*;

/// An assistant's one button to sign in, as its row says it.
pub const SIGN_IN: &str = "Sign In";
/// The line under the assistants' rows (M1): what a sign-in leaves where.
pub const SIGN_INS_STAY: &str = "Sign-ins stay on this computer.";
/// ChatGPT's row's line, signed in.
const CHATGPT_SIGNED_IN: &str = "Your ChatGPT plan";

/// The row's line before it's signed in, its program here or not: whose plan it works on.
fn on_plan(cli: crate::agents::runner::Cli) -> String {
    format!("On your {} plan", assistant_name(cli))
}

/// What an assistant's row calls it: the one the user signs in to.
pub fn assistant_name(cli: crate::agents::runner::Cli) -> &'static str {
    use crate::agents::runner::Cli;
    match cli {
        Cli::Claude => "Claude",
        Cli::Codex => "ChatGPT",
    }
}

/// The ring that turns while a sign-in is under way: the product's one sign of work, a turn a
/// second, drawn only while it is.
fn working(look: &Look, index: usize) -> impl IntoElement {
    use gpui::{Animation, AnimationExt as _, Radians, Transformation};
    svg()
        .path("icons/working.svg")
        .flex_none()
        .size(px(WORKING))
        .text_color(look.ink)
        .with_animation(
            ("assistant-working", index),
            Animation::new(std::time::Duration::from_secs(1)).repeat(),
            |ring, turned| {
                ring.with_transformation(Transformation::rotate(Radians(
                    std::f32::consts::TAU * turned,
                )))
            },
        )
}

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
/// assistant step, a login now: Claude's or ChatGPT's glyph, its name and what `shown` says of it,
/// then Sign In (its maker's page, where its program is missing), Cancel or Try Again, or, signed in, its ⋯ menu, opening at `menu_at`, and the mark,
/// `marked` when tasks run on it. A press calls `act`.
pub fn assistant_row(
    look: &Look,
    cli: crate::agents::runner::Cli,
    shown: &crate::agents::sign_in::Shown,
    marked: bool,
    a11y: bool,
    menu_at: impl IntoElement,
    mark_at: impl IntoElement,
    act: Rc<dyn Fn(Act, &mut gpui::Window, &mut gpui::App)>,
) -> gpui::Div {
    use crate::agents::sign_in::Offer;
    let index = cli as usize;
    let name = assistant_name(cli);
    let press = |id: &'static str, title: SharedString, what: Act| {
        let act = act.clone();
        stop_button(look, (id, index), title, a11y)
            .on_click(move |_, window, cx| act(what, window, cx))
            .into_any_element()
    };
    // A login now: Sign In whether its program is here (its own sign-in), missing (its maker's
    // page, as Get opened) or its last sign-in failed; a task's card keeps Offer's own words.
    let sign_in = || SharedString::from(SIGN_IN);
    let controls = match shown.offer {
        Offer::Nothing => Vec::new(),
        Offer::Get => vec![press("assistant-get", sign_in(), Act::Get)],
        Offer::Update => vec![press(
            "assistant-update",
            Offer::Update.label(cli).unwrap_or_default().into(),
            Act::Get,
        )],
        Offer::SignIn => vec![press("assistant-sign-in", sign_in(), Act::SignIn)],
        Offer::TryAgain => vec![press("assistant-try-again", sign_in(), Act::SignIn)],
        // Under way, the ring the product turns while it works, and the way out.
        Offer::Cancel => vec![
            working(look, index).into_any_element(),
            press(
                "assistant-cancel",
                Offer::Cancel.label(cli).unwrap_or_default().into(),
                Act::Cancel,
            ),
        ],
        Offer::SignedIn => {
            let more = more(look, cli, a11y, menu_at, act.clone());
            let mark = {
                let act = act.clone();
                div()
                    .id(("assistant-mark", index))
                    .when(a11y, |mark| {
                        named_check(mark, format!("Run threads with {name}"), marked)
                    })
                    .size(px(CONTROL_HEIGHT))
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(mark_at)
                    .child(self::mark(look, marked, look.on, look.knob_on))
                    .on_click(move |_, window, cx| act(Act::Mark, window, cx))
            };
            vec![more.into_any_element(), mark.into_any_element()]
        }
    };
    // The table's words where it has them; its others as the assistant's state says them.
    let line: SharedString = match shown.offer {
        Offer::Get | Offer::SignIn => on_plan(cli).into(),
        // ChatGPT's signed in says whose plan, not the program it runs through (M2).
        Offer::SignedIn if matches!(cli, crate::agents::runner::Cli::Codex) => {
            CHATGPT_SIGNED_IN.into()
        }
        _ => shown.line.clone().into(),
    };
    let text = match shown.command() {
        Some(command) => commanded(look, name, line, command).into_any_element(),
        None => named(name, Some(lined(look).child(line))).into_any_element(),
    };
    wrapping(look, cli.glyph(), text, controls)
}

/// An assistant's ⋯, which opens its menu at `menu_at` with a press.
fn more(
    look: &Look,
    cli: crate::agents::runner::Cli,
    a11y: bool,
    menu_at: impl IntoElement,
    act: Rc<dyn Fn(Act, &mut gpui::Window, &mut gpui::App)>,
) -> gpui::Stateful<gpui::Div> {
    let name = assistant_name(cli);
    icon_button(look, ("assistant-more", cli as usize), "icons/ellipsis.svg")
        .relative()
        .child(menu_at)
        .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
            act(Act::Menu, window, cx)
        })
        .when(a11y, |more| named_button(more, format!("{name} options")))
}

/// First run's form of an assistant's row (v9 §4): its line as `shown` words it, then Sign In,
/// the ring and Cancel while it waits, the ring alone while a look or a sign-out is under way, or,
/// signed in, `done` (the card's check disc) and the ⋯. The mark stays Settings › Models' alone,
/// as the card draws none.
pub fn first_run_row(
    look: &Look,
    cli: crate::agents::runner::Cli,
    shown: &crate::agents::sign_in::Shown,
    a11y: bool,
    done: impl IntoElement,
    act: Rc<dyn Fn(Act, &mut gpui::Window, &mut gpui::App)>,
) -> gpui::Div {
    use crate::agents::sign_in::Offer;
    let index = cli as usize;
    let press = |id: &'static str, title: SharedString, what: Act| {
        let act = act.clone();
        stop_button(look, (id, index), title, a11y)
            .on_click(move |_, window, cx| act(what, window, cx))
            .into_any_element()
    };
    let sign_in = || SharedString::from(SIGN_IN);
    let controls = match shown.offer {
        Offer::Nothing => vec![working(look, index).into_any_element()],
        Offer::Get => vec![press("assistant-get", sign_in(), Act::Get)],
        Offer::Update => vec![press(
            "assistant-update",
            Offer::Update.label(cli).unwrap_or_default().into(),
            Act::Get,
        )],
        Offer::SignIn | Offer::TryAgain => vec![press("assistant-sign-in", sign_in(), Act::SignIn)],
        Offer::Cancel => vec![
            working(look, index).into_any_element(),
            press(
                "assistant-cancel",
                Offer::Cancel.label(cli).unwrap_or_default().into(),
                Act::Cancel,
            ),
        ],
        Offer::SignedIn => vec![
            done.into_any_element(),
            more(look, cli, a11y, div(), act.clone()).into_any_element(),
        ],
    };
    // One line in every state (FR19), so the row is never taller in one than another: each
    // state's words are written to fit the card's column, and one that doesn't is cut where it's
    // drawn but read whole.
    let words = SharedString::from(shown.line.clone());
    let said = div()
        .id(("assistant-line", index))
        .truncate()
        .child(words.clone());
    let said = match a11y {
        true => said.role(Role::Label).aria_label(words),
        false => said,
    };
    let line = lined(look).child(said);
    wrapping(
        look,
        cli.glyph(),
        named(assistant_name(cli), Some(line)),
        controls,
    )
}

/// An assistant's row while the inside switch is off and the user hasn't clicked it on this
/// showing of the page: its glyph and name alone, since its state would mean running its program.
/// A click calls `look`; while that look is under way, `looking`, the ring turns at its end.
pub fn assistant_name_alone(
    look: &Look,
    cli: crate::agents::runner::Cli,
    looking: bool,
    a11y: bool,
    on_look: Rc<dyn Fn(&mut gpui::Window, &mut gpui::App)>,
) -> gpui::Div {
    let index = cli as usize;
    let name = assistant_name(cli);
    let controls = match looking {
        true => vec![working(look, index).into_any_element()],
        false => Vec::new(),
    };
    let row = wrapping(look, cli.glyph(), named(name, None), controls)
        .id(("assistant-name-alone", index))
        .cursor_pointer()
        .when(a11y, |row| named_button(row, name))
        .on_click(move |_, window, cx| on_look(window, cx));
    div().child(row)
}

/// An assistant's row, or another app's: its glyph, its name and line, then its controls at the
/// end. Where the row has no room for the whole name beside them, as first run's half of the
/// sheet has none beside Sign In to Claude Code, the controls go under it, at the row's end, so a
/// name is never cut.
fn wrapping(
    look: &Look,
    glyph: &'static str,
    text: impl IntoElement,
    controls: Vec<gpui::AnyElement>,
) -> gpui::Div {
    let any = !controls.is_empty();
    let lead = div()
        .flex_auto()
        .min_w_0()
        .flex()
        .items_center()
        .child(glyph_plate(look, glyph, true))
        .child(text)
        .when(any, |lead| lead.mr(px(END_GAP)));
    frame()
        .flex_wrap()
        .gap_y(px(CONTROL_GAP))
        .child(lead)
        .when(any, |row| row.child(end(controls).ml_auto()))
}

/// A row's name and the line under it, for `wrapping`. Only the name asks for room: the line,
/// which wraps, takes the width the name's leaves it.
fn named(name: impl Into<SharedString>, line: Option<gpui::Div>) -> gpui::Div {
    let name: SharedString = name.into();
    let selector = name.clone();
    div()
        .flex_auto()
        .min_w_0()
        .child(name_of(name).debug_selector(move || format!("row-name {selector}")))
        .children(line.map(|line| line.w_0().min_w_full()))
}

/// A row's line, in its grey, under its name.
fn lined(look: &Look) -> gpui::Div {
    detail_type().mt(px(DETAIL_TOP)).text_color(look.detail)
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
        named(
            self.name,
            Some(
                lined(&self.look).child(gpui::StyledText::new(self.line).with_runs(runs.collect())),
            ),
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
    let line = line.map(|line| lined(look).child(line));
    wrapping(
        look,
        "icons/cable.svg",
        named("Another app", line),
        vec![how.into_any_element()],
    )
}
