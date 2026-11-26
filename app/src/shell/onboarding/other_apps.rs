//! First run's step 6, Other apps (first-run-v9 §8): two cards side by side, a way in each for an
//! agent the user runs elsewhere. The command, with its skill, at the left, MCP at the right, each
//! with Settings › Other apps' own switch. Each shows what's added now, so a switch reads as off
//! until the user turns it on, back off where its Add failed, and on again after a quit. Only a
//! click on one sets Settings' outside switch: on with either, off once both are.

use std::time::Instant;

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, FontWeight, KeyboardButton, KeyboardClickEvent,
    MouseButton, SharedString, Window, div, prelude::*, px, rgb, svg,
};

use super::card::{self, Inks};
use super::{Shell, Switch, opt_in_frame};
use crate::agents::{Agents, Client, cli, named};
use crate::import::Look;
use crate::settings_rows::{self, stop_switch};
use crate::tokens::shell::onboarding::card as t;

/// A way in as its card shows it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Way {
    /// Added, or on its way.
    pub(super) on: bool,
    /// Its line under the name: one line, or the command's sentence on two off PATH.
    pub(super) line: SharedString,
    /// The plate's two lines, the first in the third ink; off PATH, the line to paste alone.
    pub(super) plate: (SharedString, SharedString),
    /// The small line under the plate.
    pub(super) small: SharedString,
    /// What its small line's bare button copies: the line to paste off PATH.
    pub(super) copies: Option<SharedString>,
}

/// The command's card: added in a folder the shell looks in, or not, or its Add failed.
pub(super) fn command(agents: &Agents) -> Way {
    let on = agents.is_connected(Client::Command) || agents.is_pending(Client::Command);
    let found = named(&agents.skill_programs());
    let flatpak = agents.command_flatpak_line();
    let windows = agents.command_windows_folder();
    let detail = cli::detail(
        agents.command_state(),
        flatpak.as_deref(),
        windows.as_deref(),
    );
    match (on, detail.export) {
        // Added where the shell doesn't look: the build's own sentence and line (cli::detail).
        (true, Some(export)) if agents.is_connected(Client::Command) => {
            let kept = named(&agents.skills());
            Way {
                on,
                line: detail.line.into(),
                plate: (SharedString::default(), export.clone().into()),
                small: match kept.is_empty() {
                    true => SharedString::default(),
                    false => format!("Skill added for {kept}.").into(),
                },
                copies: Some(export.into()),
            }
        }
        (false, _) if agents.failure(Client::Command).is_some() => Way {
            small: "Couldn't add the command.".into(),
            ..command_way(on, &found, false)
        },
        // No skill is written on Windows or in a Flatpak, so there the line names no program.
        _ => command_way(on, &found, cfg!(unix) && flatpak.is_none()),
    }
}

/// The command's card with nothing more to say: its skill's line names `found` where a skill is
/// `written`.
fn command_way(on: bool, found: &str, written: bool) -> Way {
    let command = cli::LINK;
    Way {
        on,
        line: Switch::Command.words().1,
        plate: (
            format!("using the {command} skill").into(),
            format!("$ {command} open islandferries.example").into(),
        ),
        small: match written {
            true => format!("A skill tells {found} when to use it.").into(),
            false => SharedString::default(),
        },
        copies: None,
    }
}

/// MCP's card, its small line `small`.
fn mcp_way(on: bool, small: String) -> Way {
    Way {
        on,
        line: Switch::Mcp.words().1,
        plate: (
            format!("tools  {} · open_tab · read_page …", cli::LINK).into(),
            "read_page { tab: 2 }".into(),
        ),
        small: small.into(),
        copies: None,
    }
}

/// The programs the preview's step finds, as it has no Agents to look.
const PREVIEW_FOUND: [Client; 2] = [Client::ClaudeCode, Client::Codex];

/// MCP's card: each program found connected through its own setup, those whose Connect failed
/// named as they fell. It's off only when none is connected or on its way.
pub(super) fn mcp(agents: &Agents) -> Way {
    let apps = agents.skill_programs();
    let connected: Vec<Client> = (apps.iter().copied())
        .filter(|app| agents.is_connected(*app))
        .collect();
    let on = apps
        .iter()
        .any(|app| agents.is_connected(*app) || agents.is_pending(*app));
    let failed: Vec<Client> = (apps.iter().copied())
        .filter(|app| {
            agents.failure(*app).is_some() && !agents.is_connected(*app) && !agents.is_pending(*app)
        })
        .collect();
    let small = match (on, failed.is_empty(), connected.is_empty()) {
        (false, false, _) => format!("Couldn't connect {}.", named(&failed)),
        (true, false, false) => format!(
            "Connected {}. {} failed.",
            named(&connected),
            named(&failed)
        ),
        _ => format!("Connects {}.", named(&apps)),
    };
    mcp_way(on, small)
}

impl Shell {
    /// Turns the command's or MCP's switch, shown `on`, with the other shown `other`: the command
    /// is added or removed, each program found connected or disconnected. Settings' outside switch
    /// goes on as what a click on turns on lands, so a failed one leaves it as it was, and off at
    /// once as the second is turned off. Only this click sets them.
    pub(super) fn onboarding_way(
        &mut self,
        switch: Switch,
        on: bool,
        other: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(agents) = crate::agents::global(cx) else {
            return;
        };
        let ways = move |agents: &Agents| match switch {
            Switch::Command => vec![Client::Command],
            _ => agents.skill_programs(),
        };
        // Outside this update, as Settings' switch is.
        cx.defer(move |cx| {
            agents.update(cx, |agents, cx| {
                let ways = ways(agents);
                match on {
                    false => agents.connect_for_outside(&ways, cx),
                    true => {
                        for way in ways {
                            agents.disconnect(way, cx);
                        }
                        if !other {
                            agents.set_other_apps(false, cx);
                        }
                    }
                }
            });
        });
    }

    /// The two cards, centred under the step's heading, both taller while the command's line is
    /// on two lines, and the door under them.
    pub(super) fn other_apps_ways(
        &mut self,
        inks: Inks,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (command, mcp, first) = match (crate::agents::global(cx), self.preview_ways) {
            (Some(agents), _) => {
                let agents = agents.read(cx);
                let first = agents.skill_programs().first().copied();
                (command(agents), mcp(agents), first)
            }
            (None, Some([command, mcp])) => {
                let found = named(&PREVIEW_FOUND);
                (
                    command_way(command, &found, cfg!(unix)),
                    mcp_way(mcp, format!("Connects {found}.")),
                    PREVIEW_FOUND.first().copied(),
                )
            }
            (None, None) => return div().into_any_element(),
        };
        let on = [command.on, mcp.on];
        let [top, width, height, gap, _] = t::WAYS;
        let grown = match command.copies.is_some() {
            true => t::WAY_GROW,
            false => 0.,
        };
        let left = (t::WIDTH - 2. * width - gap) / 2.;
        // The controls' column starts at the body's top in the centred column.
        let column = (t::WIDTH - t::CENTRED_COLUMN) / 2.;
        let mono = crate::face::mono(&cx.text_system().all_font_names());
        let ways = card::at(left - column, top - t::BODY_TOP)
            .flex()
            .gap(px(gap))
            .child(self.way_card(Switch::Command, command, height + grown, inks, mono, cx))
            .child(self.way_card(Switch::Mcp, mcp, height + grown, inks, mono, cx));
        let first = first.unwrap_or(Client::ClaudeCode).name();
        let door = self.other_apps_door(on, first, grown, inks, window, cx);
        card::at(0., 0.)
            .size_full()
            .child(ways)
            .child(door)
            .into_any_element()
    }

    /// One way's card: its glyph, name and switch; its line; its plate; its small line. A click
    /// anywhere on it, the switch included, turns it.
    fn way_card(
        &self,
        switch: Switch,
        way: Way,
        height: f32,
        inks: Inks,
        mono: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (settled, frames, slid) = match self.onboarding.as_ref() {
            Some(onboarding) => (
                onboarding.settled,
                onboarding.frames.clone(),
                match onboarding.switched {
                    Some((turned, at)) if turned == switch => {
                        (onboarding.secs_since(at) / settings_rows::SLIDE).min(1.)
                    }
                    _ => 1.,
                },
            ),
            None => (false, Default::default(), 1.),
        };
        let [_, width, _, _, radius] = t::WAYS;
        let [inset_top, inset_side] = t::WAY_INSET;
        let [plate, plate_radius, glyph, glyph_gap, name_size] = t::WAY_GLYPH;
        let [line_gap, line_size, line_line] = t::WAY_LINE;
        let [
            box_gap,
            box_width,
            box_height,
            box_radius,
            box_top,
            box_side,
            mono_size,
            mono_line,
        ] = t::WAY_PLATE;
        let [small_gap, small_size, small_line] = t::WAY_SMALL;
        let name = switch.words().0;
        let icon = match switch {
            Switch::Command => "icons/terminal.svg",
            _ => "icons/cable.svg",
        };
        // On is the text's ink with a knob of the card's, as Settings' are: no green on a control.
        let look = Look {
            knob_on: inks.fill(true),
            ..Look::on(inks.group(), inks.text(1.), inks.text(1.))
        };
        let a11y = self.a11y;
        let on = way.on;
        let control = div()
            .relative()
            .flex_none()
            .child(
                stop_switch(&look, "switch", name.clone(), on, slid, look.on, a11y)
                    // A press is its card's click; here only Space, as ↩ is the step's.
                    .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                        if matches!(
                            event,
                            ClickEvent::Keyboard(KeyboardClickEvent {
                                button: KeyboardButton::Space,
                                ..
                            })
                        ) {
                            shell.onboarding_switch(switch, cx);
                        }
                    })),
            )
            .child(opt_in_frame(frames.clone(), switch, "switch", settled));
        let ground = match inks.light {
            true => inks.text(t::LIGHT_FIELD),
            false => rgb(t::WAY_PLATE_FILL),
        };
        let shown = if on { 1. } else { t::WAY_OFF };
        let plate_line = |text: SharedString, alpha: f32| {
            div()
                .h(px(mono_line))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_color(inks.text(alpha * shown))
                .child(text)
        };
        let (first, second) = way.plate.clone();
        let copied = (self.onboarding.as_ref())
            .and_then(|onboarding| onboarding.copied)
            .is_some_and(|at| self.onboarding_secs_since(at) < t::WAY_COPIED);
        let copy = way.copies.clone().map(|line| {
            div()
                .id("copy-command-line")
                .flex_none()
                .text_color(inks.text_2())
                .hover(move |style| style.text_color(inks.text(1.)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    cx.stop_propagation();
                    cx.write_to_clipboard(ClipboardItem::new_string(line.to_string()));
                    shell.onboarding_copied(cx);
                }))
                .child(if copied { "Copied" } else { "Copy Line" })
        });
        div()
            .id(switch.name())
            .relative()
            .flex_none()
            .w(px(width))
            .h(px(height))
            .rounded(px(radius))
            .bg(inks.group())
            .pt(px(inset_top))
            .px(px(inset_side))
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| shell.onboarding_switch(switch, cx)))
            .child(
                div()
                    .h(px(plate))
                    .flex()
                    .items_center()
                    .gap(px(glyph_gap))
                    .child(
                        div()
                            .flex_none()
                            .size(px(plate))
                            .rounded(px(plate_radius))
                            .bg(inks.button())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path(icon).size(px(glyph)).text_color(inks.text(1.))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(name_size))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(inks.text(1.))
                            .child(name),
                    )
                    .child(control),
            )
            .child(
                div()
                    .mt(px(line_gap))
                    .text_size(px(line_size))
                    .line_height(px(line_line))
                    .text_color(inks.text_2())
                    .child(way.line),
            )
            .child(
                div()
                    .mt(px(box_gap))
                    .w(px(box_width))
                    .h(px(box_height))
                    .flex_none()
                    .rounded(px(box_radius))
                    .bg(ground)
                    .pt(px(box_top))
                    .px(px(box_side))
                    .overflow_hidden()
                    .font_family(mono)
                    .text_size(px(mono_size))
                    .line_height(px(mono_line))
                    .when(!first.is_empty(), |plate| {
                        plate.child(plate_line(first, t::TEXT_3))
                    })
                    .child(plate_line(second, 1.)),
            )
            .child(
                div()
                    .mt(px(small_gap))
                    .w(px(box_width))
                    .flex()
                    .justify_between()
                    .gap(px(glyph_gap))
                    .text_size(px(small_size))
                    .line_height(px(small_line))
                    .text_color(inks.text(t::TEXT_3))
                    .child(div().min_w_0().child(way.small))
                    .children(copy),
            )
            .child(opt_in_frame(frames, switch, "row", settled))
    }

    /// The command's line was copied: Copy Line reads Copied for a moment.
    fn onboarding_copied(&mut self, cx: &mut Context<Self>) {
        let Some(onboarding) = self.onboarding.as_mut() else {
            return;
        };
        onboarding.copied = Some(onboarding.clock.now());
        let wait = std::time::Duration::from_secs_f32(t::WAY_COPIED);
        let timer = onboarding.clock.timer(wait);
        cx.spawn(async move |shell, cx| {
            timer.await;
            shell.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
        cx.notify();
    }

    /// Seconds since `at` on the card's clock.
    fn onboarding_secs_since(&self, at: Instant) -> f32 {
        (self.onboarding.as_ref()).map_or(f32::MAX, |onboarding| onboarding.secs_since(at))
    }
}
