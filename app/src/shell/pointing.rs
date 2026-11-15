//! Point's mode in a window (#2352): once the user presses Point on an agent's ask, the window
//! that shows the tab frames its page, with "Pointing for Claude" on the frame's tab, marks what's
//! under the pointer as Arrange marks it (#2347), and shows the foot bar: who asks over their
//! question, then the notes' count, Not Now and Send. A click opens the note panel at the element,
//! whose ↩ Add numbers it. The marks are drawn in the window over the page (`Shell::over_page`),
//! never in it, so pointing adds nothing a page could find. The mode stays on its tab: another tab
//! shown hides the frame and the bar until it's shown again. The Agents keep the ask and its
//! answer (agents::pointing); this is what the user sees of it.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Context, FontWeight, Pixels, SharedString, div, prelude::*, px, svg,
};

use super::Shell;
use crate::agents::PointMode;
use crate::foot_bar::{self, Bar, Button, Lead};
use crate::note_panel;
use crate::point::{Element, Note};
use crate::tokens::shell::point::{
    FRAME, LABEL_GAP, LABEL_HEIGHT, LABEL_INSET, LABEL_RADIUS, LABEL_SIZE, LABEL_WIDTH, MARK_LINE,
    MARK_RADIUS, TAB_GAP, TAB_GLYPH, TAB_HEIGHT, TAB_INSET, TAB_RADIUS, TAB_SIZE,
};
use crate::tooltip::{Anchor, Tip};

/// The agent's glyph on the bar, as its task card has it.
const AGENT_GLYPH: &str = "icons/terminal.svg";
/// The frame's tab's glyph, Lucide's mouse-pointer-2, as the agent's own pointer's.
const TAB_ICON: &str = "icons/mouse-pointer-2.svg";

/// Point's mode as this window keeps it.
pub(super) struct Pointing {
    pub(super) mode: PointMode,
    /// What's under the pointer: its label and its box in the page's view.
    hover: Option<(SharedString, Bounds<Pixels>)>,
    pub(super) notes: Vec<Note>,
    /// The element just clicked, its note being written in the panel.
    writing: Option<Element>,
    /// This window shows the foot bar for it, so it's this window's to hide.
    barred: bool,
}

impl Shell {
    /// The Agents' Point mode, or none: the window holding its tab frames the page and shows the
    /// bar. A new ask starts with no notes; the same one keeps them.
    pub fn set_point_mode(&mut self, mode: Option<PointMode>, cx: &mut Context<Self>) {
        match (&mut self.point, mode) {
            (Some(pointing), Some(mode)) if pointing.mode.id == mode.id => pointing.mode = mode,
            (old, mode) => {
                let barred = old.as_ref().is_some_and(|old| old.barred);
                *old = mode.map(|mode| Pointing {
                    mode,
                    hover: None,
                    notes: Vec::new(),
                    writing: None,
                    barred,
                });
                if barred && old.is_none() {
                    foot_bar::hide(cx);
                }
            }
        }
        self.point_followed(cx);
        cx.notify();
    }

    /// The tab the user points in, if this window shows its page.
    pub(super) fn pointing_shown(&self) -> Option<u64> {
        let pointing = self.point.as_ref()?;
        self.pane_of(pointing.mode.tab).map(|_| pointing.mode.tab)
    }

    /// Shows the bar and the note panel as the mode is now, where its page is now, or hides them
    /// while another tab shows: on the mode's changes, a tab shown, and the page laid out.
    pub(super) fn point_followed(&mut self, cx: &mut Context<Self>) {
        let window = self.browser.window();
        let bar = self.point_bar(cx);
        let panel = self.point_panel(cx);
        let Some(pointing) = self.point.as_mut() else {
            return;
        };
        match bar {
            Some((bar, page)) => {
                pointing.barred = true;
                foot_bar::show(window, page, self.viewport, bar, cx);
            }
            None if std::mem::take(&mut pointing.barred) => foot_bar::hide(cx),
            None => {}
        }
        match panel {
            Some(panel) => note_panel::show(window, panel, cx),
            None => note_panel::hide(window, cx),
        }
    }

    /// The bar while this window shows the page pointed in, and where the page is.
    fn point_bar(&self, cx: &mut Context<Self>) -> Option<(Bar, Bounds<Pixels>)> {
        let pointing = self.point.as_ref()?;
        let page = self.pane_of(pointing.mode.tab)?;
        let shell = cx.weak_entity();
        let count = pointing.notes.len();
        let hint = match count {
            0 => "Click what you mean".to_owned(),
            1 => "1 note".to_owned(),
            count => format!("{count} notes"),
        };
        let not_now = {
            let shell = shell.clone();
            Rc::new(move |cx: &mut App| {
                _ = shell.update(cx, |shell, cx| shell.point_not_now(cx));
            })
        };
        let send = Rc::new(move |cx: &mut App| {
            _ = shell.update(cx, |shell, cx| shell.point_send(cx));
        });
        let bar = Bar {
            lead: Some(Lead {
                glyph: Some(AGENT_GLYPH),
                text: format!("{} asks", pointing.mode.agent).into(),
            }),
            words: pointing.mode.question.clone(),
            hint: Some(hint.into()),
            buttons: vec![
                Button {
                    label: "Not Now".into(),
                    key: Some("escape"),
                    primary: false,
                    enabled: true,
                    name: "point-not-now",
                    run: not_now,
                },
                Button {
                    label: "Send".into(),
                    key: Some("cmd-enter"),
                    primary: true,
                    enabled: count > 0,
                    name: "point-send",
                    run: send,
                },
            ],
        };
        Some((bar, page))
    }

    /// The note panel for the element just clicked, at it, while its page shows.
    fn point_panel(&self, cx: &mut Context<Self>) -> Option<note_panel::Panel> {
        let pointing = self.point.as_ref()?;
        let element = pointing.writing.clone()?;
        let page = self.pane_of(pointing.mode.tab)?;
        let at = Bounds::new(page.origin + element.frame.origin, element.frame.size);
        let shell = cx.weak_entity();
        let added = {
            let shell = shell.clone();
            Rc::new(move |words: String, cx: &mut App| {
                _ = shell.update(cx, |shell, cx| shell.point_add(words, cx));
            })
        };
        let dropped = Rc::new(move |cx: &mut App| {
            _ = shell.update(cx, |shell, cx| shell.point_note_dropped(cx));
        });
        Some(note_panel::Panel {
            number: pointing.notes.len() + 1,
            agent: pointing.mode.agent.clone(),
            element,
            at,
            page,
            palette: self.palette,
            added,
            dropped,
        })
    }

    /// Not Now, or esc: the agent hears nothing was pointed at.
    pub(super) fn point_not_now(&mut self, cx: &mut Context<Self>) {
        if let Some(pointing) = &self.point {
            // check.sh reads this line.
            log!("browser: point {} not now", pointing.mode.id);
            self.browser.answer_point(pointing.mode.id, None, cx);
        }
    }

    /// Send, or ⌘↩, once there's a note: the notes are the agent's answer.
    pub(super) fn point_send(&mut self, cx: &mut Context<Self>) {
        let Some(pointing) = self.point.as_ref().filter(|each| !each.notes.is_empty()) else {
            return;
        };
        self.browser
            .answer_point(pointing.mode.id, Some(pointing.notes.clone()), cx);
    }

    /// The core's hit test under the pointer in `tab` (tab_point_hover): the element's label and
    /// box in the page's view, or nothing there.
    pub fn point_hover(
        &mut self,
        tab: u64,
        hover: Option<(SharedString, Bounds<Pixels>)>,
        cx: &mut Context<Self>,
    ) {
        let Some(pointing) = self.point.as_mut().filter(|each| each.mode.tab == tab) else {
            return;
        };
        if pointing.hover != hover {
            pointing.hover = hover;
            cx.notify();
        }
    }

    /// The user clicked `element` in `tab` (tab_point_picked): its note opens at it.
    pub fn point_picked(&mut self, tab: u64, element: Element, cx: &mut Context<Self>) {
        let Some(pointing) = self.point.as_mut().filter(|each| each.mode.tab == tab) else {
            return;
        };
        // check.sh reads this line.
        log!(
            "browser: point picked {} {}{}",
            element.kind,
            element.tag,
            if element.withheld { " withheld" } else { "" }
        );
        pointing.writing = Some(element);
        pointing.hover = None;
        self.point_followed(cx);
        cx.notify();
    }

    /// The panel's ↩ Add: the element clicked is noted, with the user's words.
    fn point_add(&mut self, words: String, cx: &mut Context<Self>) {
        let Some(pointing) = self.point.as_mut() else {
            return;
        };
        let Some(element) = pointing.writing.take() else {
            return;
        };
        pointing.notes.push(Note {
            element,
            words: words.trim().to_owned(),
        });
        // check.sh reads this line.
        log!("browser: point note {} added", pointing.notes.len());
        self.point_followed(cx);
        cx.notify();
    }

    /// The panel closed without Add, with esc or a click outside it: the element isn't noted.
    fn point_note_dropped(&mut self, cx: &mut Context<Self>) {
        if let Some(pointing) = self.point.as_mut()
            && pointing.writing.take().is_some()
        {
            self.point_followed(cx);
            cx.notify();
        }
    }

    /// Point's keys, from the page pointed in (`page`) or the UI: esc for Not Now and ⌘↩ for
    /// Send, and in the page ↑ and ↓, out to what holds the element marked and back in. Says
    /// whether it took the key. While the note panel is open, its field has them.
    pub fn point_key(
        &mut self,
        keystroke: &str,
        page: Option<u64>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(tab) = self.pointing_shown() else {
            return false;
        };
        // Another pane's page keeps its keys.
        if page.is_some_and(|page| page != tab) {
            return false;
        }
        let from_page = page.is_some();
        if self
            .point
            .as_ref()
            .is_some_and(|each| each.writing.is_some())
        {
            return false;
        }
        let send = keystroke == "cmd-enter" || (super::OFF_THE_MAC && keystroke == "ctrl-enter");
        match keystroke {
            "escape" => self.point_not_now(cx),
            _ if send => self.point_send(cx),
            "up" | "down" if from_page => self.browser.point_do(tab, keystroke == "down"),
            _ => return false,
        }
        true
    }

    /// Whether Point's frame is on screen: its mode is on and its tab's page shows. The window over
    /// the page draws it, so that window shows while this holds.
    pub(super) fn point_shown(&self) -> bool {
        (self.point.as_ref()).is_some_and(|pointing| self.pane_of(pointing.mode.tab).is_some())
    }

    /// What the window over the page draws of Point: the frame and its tab, the mark under the
    /// pointer, and each note's numbered mark.
    pub(super) fn point_drawn(&self) -> Option<AnyElement> {
        let pointing = self.point.as_ref()?;
        let pane = self.pane_of(pointing.mode.tab)?;
        let accent = self.palette.accent;
        let ink = self.palette.accent_label;
        let mark = |label: SharedString, frame: Bounds<Pixels>| {
            // The label sits over the box, or inside its top where the page's top cuts it.
            let top = match frame.top() - px(LABEL_HEIGHT + LABEL_GAP) < px(0.) {
                true => frame.top() + px(LABEL_GAP),
                false => frame.top() - px(LABEL_HEIGHT + LABEL_GAP),
            };
            let line = px(MARK_LINE);
            div()
                .child(
                    div()
                        .absolute()
                        .left(frame.left() - line)
                        .top(frame.top() - line)
                        .w(frame.size.width + line * 2.)
                        .h(frame.size.height + line * 2.)
                        .rounded(px(MARK_RADIUS))
                        .border(line)
                        .border_color(accent),
                )
                .child(
                    div()
                        .absolute()
                        .left(frame.left() - line)
                        .top(top)
                        .max_w(px(LABEL_WIDTH))
                        .h(px(LABEL_HEIGHT))
                        .px(px(LABEL_INSET))
                        .flex()
                        .items_center()
                        .rounded(px(LABEL_RADIUS))
                        .bg(accent)
                        .text_size(px(LABEL_SIZE))
                        .text_color(ink)
                        .font_weight(FontWeight::MEDIUM)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label),
                )
        };
        let notes = (pointing.notes.iter().enumerate())
            .map(|(index, note)| mark(note.label(index + 1).into(), note.element.frame));
        let writing = (pointing.writing.as_ref()).map(|element| {
            let number = pointing.notes.len() + 1;
            mark(
                format!("{number} · {}", element.label()).into(),
                element.frame,
            )
        });
        let hover = (pointing.writing.is_none())
            .then_some(pointing.hover.as_ref())
            .flatten()
            .map(|(label, frame)| mark(label.clone(), *frame));
        let tab = div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .w(pane.size.width)
            .flex()
            .justify_center()
            .child(
                div()
                    .h(px(TAB_HEIGHT))
                    .px(px(TAB_INSET))
                    .flex()
                    .items_center()
                    .gap(px(TAB_GAP))
                    .rounded_b(px(TAB_RADIUS))
                    .bg(accent)
                    .text_size(px(TAB_SIZE))
                    .text_color(ink)
                    .font_weight(FontWeight::MEDIUM)
                    .whitespace_nowrap()
                    .child(svg().size(px(TAB_GLYPH)).path(TAB_ICON).text_color(ink))
                    .child(format!("Pointing for {}", pointing.mode.agent)),
            );
        Some(
            div()
                .absolute()
                .left(pane.left())
                .top(pane.top())
                .w(pane.size.width)
                .h(pane.size.height)
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .border(px(FRAME))
                        .border_color(accent),
                )
                .children(notes)
                .children(writing)
                .children(hover)
                .child(tab)
                .into_any_element(),
        )
    }

    /// Whether Point can start for `agent` in the tab in front, for a card's Point: 0, or why
    /// not, which dims it and is its tooltip.
    pub(super) fn may_point_here(&self, agent: &str) -> i32 {
        self.browser.may_point(self.selected, agent)
    }

    /// A card's Point, on the ask `id` of `agent`: pointing starts in the tab in front, if it can
    /// there. If it can't, the press does nothing, as its dimmed look and tooltip said.
    pub(super) fn press_point(&mut self, id: u64, agent: &str, cx: &mut Context<Self>) {
        let can = self.may_point_here(agent);
        if can != 0 {
            log!("browser: point {id} can't start: {can}");
            return;
        }
        self.browser.tooltip(None, cx);
        self.browser.start_point(id, self.selected, cx);
    }

    /// The pointer came onto a dimmed Point drawn at `frame`, or left it: its tooltip says why it
    /// can't start, `can` being bc_tab_may_point's answer.
    pub(super) fn point_tip(
        &mut self,
        frame: Option<Bounds<Pixels>>,
        hovered: bool,
        can: i32,
        cx: &mut Context<Self>,
    ) {
        let tip = (frame.filter(|_| hovered))
            .zip(crate::point::why_not(can))
            .map(|(frame, why)| (Tip::named(why.into()), Anchor::under(frame, self.viewport)));
        self.hover_tip(tip, cx);
    }
}

#[cfg(test)]
mod tests;
