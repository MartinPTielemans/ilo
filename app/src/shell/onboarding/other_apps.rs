//! First run's step 5, Other apps (first-run-v9 §8, #4451): two cards side by side, a way in each
//! for an agent the user runs elsewhere. The command, with its skill, at the left, with Install;
//! MCP at the right, with Connect. Each shows what's added now, so a button reads Install until the
//! user presses it, again where it failed, and Installed after a quit. Done, a button is a check
//! and its word, and nothing presses it: undoing one is Settings › Other apps'. Only a press sets
//! Settings' outside switch, on as what it added lands.
//!
//! Under the cards, one door and one result: a cord from each card to a knot, and from the knot to
//! a sidebar card as the sidebar draws a thread's. As a button finishes, at the first frame the
//! window shows, its plate types in, its cord draws to the knot, a bead runs on through it and
//! down, and the card arrives; undone in Settings, the cord falls back and, with both undone, the
//! card goes back to its outline.

use std::time::Instant;

use gpui::{
    AnyElement, ClipboardItem, Context, FontWeight, MouseButton, PathBuilder, Rgba, SharedString,
    Window, accesskit::Role, canvas, div, point, prelude::*, px, rgb, svg,
};

use super::card::{self, Inks};
use super::{Shell, Step, Switch, arrive, cubic_bezier, lerp, opt_in_frame};
use crate::agents::{Agents, Client, cli, named};
use crate::settings_rows::named_button;
use crate::tokens::shell::onboarding::card as t;
use crate::tokens::shell::task_card as thread;

/// Where a way is, as its button says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Progress {
    /// Not added, or its Add failed: the button.
    Off,
    /// Being added: the ring and its word.
    Working,
    /// Added: a check and its word, which nothing presses.
    Done,
}

impl Progress {
    /// The words for `switch` at this point: the button's, or what it shows in the button's place.
    pub(super) fn label(self, switch: Switch) -> &'static str {
        match (switch, self) {
            (Switch::Command, Progress::Off) => "Install",
            (Switch::Command, Progress::Working) => "Installing…",
            (Switch::Command, Progress::Done) => "Installed",
            (_, Progress::Off) => "Connect",
            (_, Progress::Working) => "Connecting…",
            (_, Progress::Done) => "Connected",
        }
    }
}

/// A way's cord from its card to the knot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Cord {
    /// Faint: its way isn't done.
    Faint,
    /// Done at this frame: its plate types in, it draws to the knot and its bead runs.
    Tying(Instant),
    /// Drawn: done before the step came, or tied a while ago.
    Tied,
    /// Undone at this frame, falling back to faint.
    Falling(Instant),
}

/// The sidebar card under the knot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Arrival {
    /// Its outline, a still ring and the first program's name: the card to come.
    Outline,
    /// The first cord tied at this frame: it arrives once the bead has run, with its ring, then
    /// becomes its finished self.
    Coming(Instant),
    /// Here, finished.
    Here,
    /// Both ways undone at this frame: it goes back to its outline.
    Going(Instant),
}

/// The picture under the cards, as the last frame had it.
#[derive(Clone, Debug)]
pub(super) struct Motion {
    /// Each way as the last frame drew it, the command first; none before the step's first.
    seen: [Option<Progress>; 2],
    pub(super) cords: [Cord; 2],
    pub(super) card: Arrival,
    /// Something in it moved, so the card asks for another frame.
    pub(super) moving: bool,
}

impl Default for Motion {
    fn default() -> Self {
        Motion {
            seen: [None; 2],
            cords: [Cord::Faint; 2],
            card: Arrival::Outline,
            moving: false,
        }
    }
}

/// How the picture's times run: with Reduce Motion each travel is a fade.
#[derive(Clone, Copy)]
struct Times {
    still: bool,
}

impl Times {
    /// From a cord's tie to the bead's end, where the card starts to arrive.
    fn travel(self) -> f32 {
        match self.still {
            true => t::WAYS_STILL_FADE,
            false => t::WAYS_DRAW + t::WAYS_BEAD[1],
        }
    }

    /// From the card's start to its finished self.
    fn arrival(self) -> f32 {
        t::WAYS_DONE
    }
}

/// How long `plate`'s two lines take to type in.
fn typing(plate: &(SharedString, SharedString)) -> f32 {
    let [each, gap] = t::WAYS_TYPE;
    let first = plate.0.chars().count() as f32 * each;
    let second = plate.1.chars().count() as f32 * each;
    match first > 0. {
        true => first + gap + second,
        false => second,
    }
}

impl Motion {
    /// Follows the ways as this frame finds them, `typed` the seconds each plate takes to type in:
    /// a way that finished since the last frame the window showed ties its cord, and one undone
    /// lets its cord fall. Returns the words each way shows now, where they changed, for the log.
    fn follow(
        &mut self,
        now: [Progress; 2],
        typed: [f32; 2],
        visible: bool,
        at: Instant,
        times: Times,
        since: impl Fn(Instant) -> f32,
    ) -> Vec<(usize, Progress)> {
        let mut changed = Vec::new();
        for (way, &progress) in now.iter().enumerate() {
            match self.seen[way] {
                None => {
                    if progress == Progress::Done {
                        self.cords[way] = Cord::Tied;
                        self.card = Arrival::Here;
                    }
                }
                Some(was) if was == progress => continue,
                // It plays from the first frame seen, so a window not yet shown keeps it waiting.
                Some(_) if progress == Progress::Done && !visible => continue,
                Some(_) if progress == Progress::Done => {
                    self.cords[way] = Cord::Tying(at);
                    if matches!(self.card, Arrival::Outline | Arrival::Going(_)) {
                        self.card = Arrival::Coming(at);
                    }
                }
                Some(Progress::Done) => self.cords[way] = Cord::Falling(at),
                Some(_) => {}
            }
            self.seen[way] = Some(progress);
            changed.push((way, progress));
        }
        let tied = (self.cords.iter()).any(|cord| matches!(cord, Cord::Tying(_) | Cord::Tied));
        if !tied && matches!(self.card, Arrival::Coming(_) | Arrival::Here) {
            self.card = Arrival::Going(at);
        }
        for (way, cord) in self.cords.iter_mut().enumerate() {
            *cord = match *cord {
                Cord::Tying(from) if since(from) >= times.travel().max(typed[way]) => Cord::Tied,
                Cord::Falling(from) if since(from) >= t::WAYS_FALL => Cord::Faint,
                cord => cord,
            };
        }
        self.card = match self.card {
            Arrival::Coming(from) if since(from) >= times.travel() + times.arrival() => {
                Arrival::Here
            }
            Arrival::Going(from) if since(from) >= t::WAYS_FALL => Arrival::Outline,
            card => card,
        };
        self.moving = (self.cords.iter())
            .any(|cord| matches!(cord, Cord::Tying(_) | Cord::Falling(_)))
            || matches!(self.card, Arrival::Coming(_) | Arrival::Going(_));
        changed
    }
}

/// A way in as its card shows it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Way {
    /// Added, or on its way.
    pub(super) on: bool,
    /// Where it is, as its button says.
    pub(super) progress: Progress,
    /// Its line under the name: one line, or the command's sentence on two off PATH.
    pub(super) line: SharedString,
    /// The plate's two lines, the first in the third ink; off PATH, the line to paste alone.
    pub(super) plate: (SharedString, SharedString),
    /// The small line under the plate.
    pub(super) small: SharedString,
    /// What its small line's bare button copies: the line to paste off PATH.
    pub(super) copies: Option<SharedString>,
}

/// The command's card: installed in a folder the shell looks in, or not, or its Install failed.
pub(super) fn command(agents: &Agents) -> Way {
    let command = cli::LINK;
    let progress = match (
        agents.is_connected(Client::Command),
        agents.is_pending(Client::Command),
    ) {
        (_, true) => Progress::Working,
        (true, false) => Progress::Done,
        (false, false) => Progress::Off,
    };
    let on = progress != Progress::Off;
    let found = named(&agents.skill_programs());
    let flatpak = agents.command_flatpak_line();
    let windows = agents.command_windows_folder();
    let detail = cli::detail(
        agents.command_state(),
        flatpak.as_deref(),
        windows.as_deref(),
    );
    let line = Switch::Command.words().1;
    let plate: (SharedString, SharedString) = (
        format!("using the {command} skill").into(),
        format!("$ {command} open islandferries.example").into(),
    );
    match (on, detail.export) {
        // Added where the shell doesn't look: the build's own sentence and line (cli::detail).
        (true, Some(export)) if agents.is_connected(Client::Command) => {
            let kept = named(&agents.skills());
            Way {
                on,
                progress,
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
            on,
            progress,
            line,
            plate,
            small: COULDNT_INSTALL.into(),
            copies: None,
        },
        // No skill is written on Windows or in a Flatpak, so there the line names no program.
        _ => Way {
            on,
            progress,
            line,
            plate,
            small: match cfg!(unix) && flatpak.is_none() {
                true => format!("A skill tells {found} when to use it.").into(),
                false => SharedString::default(),
            },
            copies: None,
        },
    }
}

/// The command's small line once its Install failed.
pub(super) const COULDNT_INSTALL: &str = "Couldn't install the command.";

/// MCP's card: each program found connected through its own setup, those whose Connect failed
/// named as they fell. It's off only when none is connected or on its way, and done once one is
/// connected and none is on its way.
pub(super) fn mcp(agents: &Agents) -> Way {
    let apps = agents.skill_programs();
    let connected: Vec<Client> = (apps.iter().copied())
        .filter(|app| agents.is_connected(*app))
        .collect();
    let pending = apps.iter().any(|app| agents.is_pending(*app));
    let progress = match (pending, connected.is_empty()) {
        (true, _) => Progress::Working,
        (false, false) => Progress::Done,
        (false, true) => Progress::Off,
    };
    let on = progress != Progress::Off;
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
    Way {
        on,
        progress,
        line: Switch::Mcp.words().1,
        plate: (
            format!("tools  {} · open_tab · read_page …", cli::LINK).into(),
            "read_page { tab: 2 }".into(),
        ),
        small: small.into(),
        copies: None,
    }
}

impl Shell {
    /// Presses the command's Install or MCP's Connect: the command is installed, or each program
    /// found connected. Settings' outside switch goes on as what it added lands, so a failed one
    /// leaves it as it was. Only this press sets them; undoing either is Settings'.
    pub(super) fn onboarding_way(&mut self, switch: Switch, cx: &mut Context<Self>) {
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
                agents.connect_for_outside(&ways, cx);
            });
        });
    }

    /// The ways as this frame finds them, with the picture following them: what the card's
    /// controls draw. check.sh hears each button's words as they change.
    pub(super) fn ways_now(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<[Way; 2]> {
        let agents = crate::agents::global(cx)?;
        let ways = {
            let agents = agents.read(cx);
            [command(agents), mcp(agents)]
        };
        let still = crate::motion::still(cx);
        let visible = window.is_visible();
        let onboarding = self.onboarding.as_mut()?;
        let at = onboarding.clock.now();
        let clock = onboarding.clock.clone();
        let since = move |from: Instant| clock.now().saturating_duration_since(from).as_secs_f32();
        let typed = [typing(&ways[0].plate), typing(&ways[1].plate)];
        let progress = [ways[0].progress, ways[1].progress];
        let coming = matches!(onboarding.ways.card, Arrival::Coming(_));
        let changed =
            (onboarding.ways).follow(progress, typed, visible, at, Times { still }, since);
        // check.sh captures the step once the card has come.
        if coming && onboarding.ways.card == Arrival::Here {
            log!("browser: onboarding sidebar card came");
        }
        // This frame's motion asks for the next itself, as the card's own ask was made before it.
        if onboarding.ways.moving {
            window.request_animation_frame();
        }
        for (way, progress) in changed {
            let switch = [Switch::Command, Switch::Mcp][way];
            log!(
                "browser: onboarding {} {}",
                switch.name(),
                progress.label(switch)
            );
        }
        Some(ways)
    }

    /// The two cards, centred under the step's heading, both taller while the command's line is
    /// on two lines, and the picture under them, all in the card's own points.
    pub(super) fn other_apps_ways(
        &self,
        ways: Option<[Way; 2]>,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some([command, mcp]) = ways else {
            return div().into_any_element();
        };
        let [top, width, height, gap, _] = t::WAYS;
        let grow = if command.copies.is_some() {
            t::WAY_GROW
        } else {
            0.
        };
        let height = height + grow;
        let left = (t::WIDTH - 2. * width - gap) / 2.;
        let mono = crate::face::mono(&cx.text_system().all_font_names());
        let first = (crate::agents::global(cx))
            .and_then(|agents| agents.read(cx).skill_programs().first().copied());
        let picture = self.ways_picture(first, top + height, inks, cx);
        div()
            .absolute()
            .inset_0()
            .child(picture)
            .child(
                card::at(left, top)
                    .flex()
                    .gap(px(gap))
                    .child(self.way_card(Switch::Command, command, height, inks, mono, cx))
                    .child(self.way_card(Switch::Mcp, mcp, height, inks, mono, cx)),
            )
            .into_any_element()
    }

    /// What a way's card has in its button's place: the button, off; the ring and its word while
    /// it works; a check and its word once done, which nothing presses.
    fn way_control(
        &self,
        switch: Switch,
        progress: Progress,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = progress.label(switch);
        let a11y = self.a11y;
        let still = crate::motion::still(cx);
        let id = match switch {
            Switch::Command => "way-command",
            _ => "way-mcp",
        };
        let done = |glyph: AnyElement| {
            div()
                .id(id)
                .flex_none()
                .flex()
                .items_center()
                .gap(px(t::WAY_DONE_GAP))
                .text_size(px(t::BUTTON_TEXT))
                .font_weight(FontWeight::MEDIUM)
                .text_color(inks.text_2())
                .when(a11y, |words| words.role(Role::Label).aria_label(label))
                .child(glyph)
                .child(label)
        };
        match progress {
            Progress::Off => card::quiet(id, label.into(), inks)
                .tab_index(0)
                .focus_from_keys_only()
                .when(a11y, |button| named_button(button, label))
                .on_click(cx.listener(move |shell, _, _, cx| {
                    cx.stop_propagation();
                    shell.onboarding_switch(switch, cx);
                }))
                .into_any_element(),
            Progress::Working => done(ring(id, inks.text(1.), still)).into_any_element(),
            Progress::Done => done(
                svg()
                    .path("icons/check.svg")
                    .flex_none()
                    .size(px(thread::GLYPH))
                    .text_color(inks.text(1.))
                    .into_any_element(),
            )
            .into_any_element(),
        }
    }

    /// One way's card: its glyph, name and button; its line; its plate, typing in as it's done; its
    /// small line.
    fn way_card(
        &self,
        switch: Switch,
        way: Way,
        height: f32,
        inks: Inks,
        mono: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let (settled, frames) = match self.onboarding.as_ref() {
            Some(onboarding) => (onboarding.settled, onboarding.frames.clone()),
            None => (false, Default::default()),
        };
        let way_at = match switch {
            Switch::Command => 0,
            _ => 1,
        };
        let cord = (self.onboarding.as_ref())
            .map_or(Cord::Faint, |onboarding| onboarding.ways.cords[way_at]);
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
        let control = div()
            .relative()
            .flex_none()
            .child(self.way_control(switch, way.progress, inks, cx))
            .child(opt_in_frame(frames.clone(), switch, "button", settled));
        let ground = match inks.light {
            true => inks.text(t::LIGHT_FIELD),
            false => rgb(t::WAY_PLATE_FILL),
        };
        // Typed in as its cord ties, at full ink; the rest, and all of it while the way isn't
        // done, at `way-off`.
        let still = crate::motion::still(cx);
        let typed = match cord {
            Cord::Faint | Cord::Falling(_) => Some(0.),
            Cord::Tying(from) if !still => Some(self.onboarding_secs_since(from)),
            _ => None,
        };
        let [each, gap] = t::WAYS_TYPE;
        let (first, second) = way.plate.clone();
        let first_takes = match first.is_empty() {
            true => 0.,
            false => first.chars().count() as f32 * each + gap,
        };
        let plate_line = |text: SharedString, alpha: f32, from: f32| {
            let shown = typed.map_or(usize::MAX, |typed| {
                ((typed - from) / each).floor().max(0.) as usize
            });
            let split = (text.char_indices())
                .nth(shown)
                .map_or(text.len(), |(at, _)| at);
            let (lit, faint) = text.split_at(split);
            div()
                .h(px(mono_line))
                .flex()
                .whitespace_nowrap()
                .overflow_hidden()
                .child(
                    div()
                        .text_color(inks.text(alpha))
                        .child(SharedString::from(lit.to_string())),
                )
                .child(
                    div()
                        .text_color(inks.text(alpha * t::WAY_OFF))
                        .child(SharedString::from(faint.to_string())),
                )
        };
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
                        plate.child(plate_line(first, t::TEXT_3, 0.))
                    })
                    .child(plate_line(second, 1., first_takes)),
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

    /// Under the cards, from `bottom`: a cord from each to the knot, one from the knot to the
    /// sidebar card, the bead on its run, the card as the picture has it and the line under it.
    fn ways_picture(
        &self,
        first: Option<Client>,
        bottom: f32,
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(onboarding) = self.onboarding.as_ref() else {
            return div().into_any_element();
        };
        let still = crate::motion::still(cx);
        let times = Times { still };
        let motion = onboarding.ways.clone();
        let since = |from: Instant| onboarding.secs_since(from);
        let [cord_width, knot_size, knot_y] = t::WAYS_CORD;
        let [card_width, card_height, card_y, line_gap] = t::WAYS_CARD;
        // Everything under the cards moves down with them as they grow.
        let down = bottom - (t::WAYS[0] + t::WAYS[2]);
        let (knot_y, card_y) = (knot_y + down, card_y + down);
        let centre = t::WIDTH / 2.;
        let [_, width, _, gap, _] = t::WAYS;
        let left = (t::WIDTH - 2. * width - gap) / 2.;
        let froms = [
            (left + width / 2., bottom),
            (left + width + gap + width / 2., bottom),
        ];
        let knot = (centre, knot_y);
        let drop = ((centre, knot_y + knot_size / 2.), (centre, card_y));
        // Each cord: how much of it is lit, at what ink, and where its bead is, if it runs.
        let cords = motion.cords.map(|cord| match cord {
            Cord::Faint => (0., 0., None),
            Cord::Tying(from) if still => (1., (since(from) / t::WAYS_STILL_FADE).min(1.), None),
            Cord::Tying(from) => {
                let at = since(from);
                let drawn = drawing(at / t::WAYS_DRAW);
                let run = (at - t::WAYS_DRAW) / t::WAYS_BEAD[1];
                (drawn, 1., (0. ..1.).contains(&run).then_some(run))
            }
            Cord::Tied => (1., 1., None),
            Cord::Falling(from) => (1., 1. - (since(from) / t::WAYS_FALL).min(1.), None),
        });
        let reached = (motion.cords.iter().zip(cords.iter())).any(|(cord, (drawn, lit, _))| {
            !matches!(cord, Cord::Faint) && *drawn >= 1. && *lit > 0.
        });
        let knot_ink = match reached {
            true => 1.,
            false => t::WAY_OFF,
        };
        // The card: how far it has come, how high it still is, and whether it has finished.
        let (shown, rise, finished, working) = match motion.card {
            Arrival::Outline => (0., 0., false, false),
            Arrival::Coming(from) => {
                let at = since(from) - times.travel();
                let [fade, height, moving] = t::WAYS_ARRIVE;
                match (at >= 0., still) {
                    (false, _) => (0., 0., false, false),
                    (true, true) => ((at / t::WAYS_STILL_FADE).min(1.), 0., false, true),
                    (true, false) => (
                        (at / fade).min(1.),
                        height * (1. - arrive(at / moving)),
                        false,
                        true,
                    ),
                }
            }
            Arrival::Here => (1., 0., true, false),
            Arrival::Going(from) => (1. - (since(from) / t::WAYS_FALL).min(1.), 0., false, false),
        };
        // The drop to the card is lit as the bead passes and while the card is here.
        let bead = (cords.iter()).find_map(|(_, _, run)| *run);
        let split = cord_share(froms[0], knot);
        let dropped = match bead {
            Some(run) if run > split => (run - split) / (1. - split),
            Some(_) => 0.,
            None => shown,
        };
        let bead_at = (cords.iter().enumerate()).find_map(|(way, (_, _, run))| {
            let run = (*run)?;
            Some(match run < split {
                true => cord_at(froms[way], knot, run / split),
                false => {
                    let along = (run - split) / (1. - split);
                    let (from, to) = drop;
                    (from.0, lerp(from.1, to.1, along))
                }
            })
        });
        let off = t::WAYS_CORD_OFF;
        let faint = inks.text(off);
        let lit = cords.map(|(drawn, ink, _)| (drawn, inks.text(lerp(off, 1., ink))));
        let drop_ink = inks.text(lerp(off, 1., dropped.clamp(0., 1.)));
        let cord_lines = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let origin = bounds.origin;
                for (way, from) in froms.iter().enumerate() {
                    paint_cord(origin, *from, knot, 1., cord_width, faint, window);
                    let (drawn, ink) = lit[way];
                    if drawn > 0. {
                        paint_cord(origin, *from, knot, drawn, cord_width, ink, window);
                    }
                }
                let (from, to) = drop;
                paint_cord(origin, from, to, 1., cord_width, faint, window);
                if dropped > 0. {
                    paint_cord(
                        origin,
                        from,
                        to,
                        dropped.min(1.),
                        cord_width,
                        drop_ink,
                        window,
                    );
                }
            },
        )
        .absolute()
        .inset_0();
        let [bead_size, _] = t::WAYS_BEAD;
        let bead = bead_at.map(|(x, y)| {
            card::at(x - bead_size / 2., y - bead_size / 2.)
                .size(px(bead_size))
                .rounded_full()
                .bg(inks.text(1.))
                .children(self.place(Step::OtherApps, "bead"))
        });
        let name = first.map_or_else(SharedString::default, |first| named(&[first]).into());
        let thread_card = self.ways_thread_card(name, (shown, finished, working), inks, cx);
        div()
            .absolute()
            .inset_0()
            .child(cord_lines)
            .child(
                card::at(centre - knot_size / 2., knot_y - knot_size / 2.)
                    .child(card::whole_knot(knot_size, inks.text(knot_ink)))
                    .children(self.place(Step::OtherApps, "ways-knot")),
            )
            .children(bead)
            .child(
                card::at((t::WIDTH - card_width) / 2., card_y + rise)
                    .w(px(card_width))
                    .h(px(card_height))
                    .child(thread_card),
            )
            .child(
                card::at(0., card_y + card_height + line_gap)
                    .w(px(t::WIDTH))
                    .flex()
                    .justify_center()
                    .text_size(px(crate::tokens::text::SMALL))
                    .text_color(inks.text(t::TEXT_3))
                    .child(
                        div()
                            .id("ways-appears")
                            .when(self.a11y, |line| line.role(Role::Label).aria_label(APPEARS))
                            .child(APPEARS),
                    ),
            )
            .into_any_element()
    }

    /// The sidebar card under the knot, `shown` of the way from its outline: a thread's card as the
    /// sidebar draws one, with its tokens, a ring and the program's `name`, then its live line
    /// while `working`, or a check and its finished line once `finished`.
    fn ways_thread_card(
        &self,
        name: SharedString,
        (shown, finished, working): (f32, bool, bool),
        inks: Inks,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let still = crate::motion::still(cx);
        let off = t::WAYS_CORD_OFF;
        let ink = lerp(off, 1., shown);
        let glyph = match (finished, working) {
            (true, _) => svg()
                .path("icons/check.svg")
                .flex_none()
                .size(px(thread::GLYPH))
                .text_color(inks.text(1.))
                .into_any_element(),
            (false, true) => ring("ways-card-ring", inks.text(1.), still),
            (false, false) => svg()
                .path("icons/working.svg")
                .flex_none()
                .size(px(thread::GLYPH))
                .text_color(inks.text(1.))
                .into_any_element(),
        };
        let line = match (finished, working) {
            (true, _) => Some(FINISHED),
            (false, true) => Some(READING),
            (false, false) => None,
        };
        let text_2 = match inks.light {
            true => thread::TEXT_2_LIGHT,
            false => thread::TEXT_2,
        };
        div()
            .size_full()
            .opacity(ink)
            .rounded(px(thread::RADIUS))
            .bg(inks.text(thread::FILL))
            .border(px(thread::OUTLINE_WIDTH))
            .border_color(inks.text(thread::OUTLINE))
            .p(px(thread::PADDING))
            .flex()
            .flex_col()
            .gap(px(thread::GAP))
            .child(
                div()
                    .h(px(thread::TITLE_LINE))
                    .flex()
                    .items_center()
                    .gap(px(thread::GLYPH_GAP))
                    .child(glyph)
                    .child(
                        div()
                            .text_size(px(crate::tokens::text::BODY))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(inks.text(1.))
                            .child(name),
                    ),
            )
            .children(line.map(|line| {
                div()
                    .pl(px(thread::GLYPH + thread::GLYPH_GAP))
                    .h(px(thread::LIVE_LINE))
                    .text_size(px(crate::tokens::text::SMALL))
                    .text_color(inks.text(text_2))
                    .child(line)
            }))
            .children(self.place(Step::OtherApps, "ways-card"))
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

/// The line under the sidebar card, which never changes.
pub(super) const APPEARS: &str = "An app appears in your sidebar when it connects.";
/// The card's live line while it arrives, and its finished one: made up, as the plate's line is.
const READING: &str = "Reading Island Ferries";
const FINISHED: &str = "Island Ferries · €14 · 08:40";

/// DRAW, the curve a cord drawing takes.
fn drawing(x: f32) -> f32 {
    let [x1, y1, x2, y2] = t::DRAW;
    cubic_bezier(x1, y1, x2, y2, x.clamp(0., 1.))
}

/// The ring that turns while something works, the product's one sign of work, a turn a second;
/// still, with Reduce Motion, as nothing loops then.
fn ring(id: &'static str, ink: Rgba, still: bool) -> AnyElement {
    use gpui::{Animation, AnimationExt as _, Radians, Transformation};
    let ring = svg()
        .path("icons/working.svg")
        .flex_none()
        .size(px(thread::GLYPH))
        .text_color(ink);
    match still {
        true => ring.into_any_element(),
        false => ring
            .with_animation(
                id,
                Animation::new(std::time::Duration::from_secs(1)).repeat(),
                |ring, turned| {
                    ring.with_transformation(Transformation::rotate(Radians(
                        std::f32::consts::TAU * turned,
                    )))
                },
            )
            .into_any_element(),
    }
}

/// The point `along` a cord from `from` to `to` that leaves and arrives upright.
fn cord_at(from: (f32, f32), to: (f32, f32), along: f32) -> (f32, f32) {
    let mid = (from.1 + to.1) / 2.;
    let (p0, p1, p2, p3) = (from, (from.0, mid), (to.0, mid), to);
    let u = 1. - along;
    let at = |a: f32, b: f32, c: f32, d: f32| {
        u * u * u * a
            + 3. * u * u * along * b
            + 3. * u * along * along * c
            + along * along * along * d
    };
    (at(p0.0, p1.0, p2.0, p3.0), at(p0.1, p1.1, p2.1, p3.1))
}

/// How long a cord from `from` to the knot is, as a share of it and the drop together, so the bead
/// runs both at one speed.
fn cord_share(from: (f32, f32), knot: (f32, f32)) -> f32 {
    const SEGMENTS: usize = 32;
    let mut length = 0.;
    let mut last = from;
    for each in 1..=SEGMENTS {
        let next = cord_at(from, knot, each as f32 / SEGMENTS as f32);
        length += ((next.0 - last.0).powi(2) + (next.1 - last.1).powi(2)).sqrt();
        last = next;
    }
    let [_, _, card_y, _] = t::WAYS_CARD;
    let [_, knot_size, knot_y] = t::WAYS_CORD;
    let drop = (card_y - knot_y - knot_size / 2.).max(1.);
    length / (length + drop)
}

/// A cord from `from` to `to`, `drawn` of it, `width` wide in `ink`, at the card's `origin`.
fn paint_cord(
    origin: gpui::Point<gpui::Pixels>,
    from: (f32, f32),
    to: (f32, f32),
    drawn: f32,
    width: f32,
    ink: Rgba,
    window: &mut Window,
) {
    const SEGMENTS: usize = 32;
    let last = (drawn.clamp(0., 1.) * SEGMENTS as f32).ceil() as usize;
    if last == 0 {
        return;
    }
    let at = |(x, y): (f32, f32)| point(origin.x + px(x), origin.y + px(y));
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(at(from));
    for each in 1..=last {
        let along = (each as f32 / SEGMENTS as f32).min(drawn);
        path.line_to(at(cord_at(from, to, along)));
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, ink);
    }
}
