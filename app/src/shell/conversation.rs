//! A task's conversation, in the page's place (Agent task). A click on a task's card shows it: the
//! window shows no page, as a ⌘N window does, and GPUI draws the conversation on the card, which
//! nothing native covers then. Its pages' pictures are along its top, taken only while it shows,
//! then what was asked with its page, what each side said with the steps between folded into a
//! line, what it's doing now, and the reply field at its foot. It comes in from the card's side
//! and goes back into it, and shows the tab shown before as it goes. A tab or a Space shown ends
//! it, as the page takes the card back.

use std::time::Instant;

use gpui::{
    AnyElement, Context, Corners, Entity, Focusable, FontWeight, MouseButton, SharedString, Window, actions,
    canvas, div, prelude::*, px, rgba,
};

use super::{
    Shell, SECONDARY, STATUS_AGENT, STATUS_DOT, TITLE, fade_at,
    permission::card_button,
    picture::{PagePicture, next_request},
    rows::ringed_dot,
};
use crate::agents::task::{Conversation, Line, TaskAction};
use crate::text_input::TextInput;
use crate::tokens::shell::FORM_PLACEHOLDER;
use crate::tokens::shell::conversation::{
    BUBBLE_X, BUBBLE_Y, COLUMN, HEADER, INSET, LINE_GAP, PICTURE_GAP, PICTURE_HEIGHT,
    PICTURE_RADIUS, PICTURE_WIDTH, RADIUS, REPLY_BOTTOM, REPLY_HEIGHT, REPLY_INSET, SITE, SLIDE,
    TIME, TOP,
};
use crate::tokens::shell::empty::KEYS_FILL;
use crate::tokens::text;

actions!(conversation, [SendReply, LeaveConversation, StopTask]);

/// The key context of the conversation, whose keys go ahead of the browser's while the reply field
/// has the keyboard: Return sends, Esc leaves, ⌘. stops.
const KEY_CONTEXT: &str = "Conversation";

pub fn bind_keys(cx: &mut gpui::App) {
    let key = |key, action| super::keys::view_binding(key, action, KEY_CONTEXT, cx);
    let keys = [
        key("enter", Box::new(SendReply)),
        key("escape", Box::new(LeaveConversation)),
        key("cmd-.", Box::new(StopTask)),
    ];
    cx.bind_keys(keys);
}

/// The conversation a window shows.
pub(super) struct Talk {
    pub(super) agent: SharedString,
    /// The Space it shows in, and the tab shown before, which shows again as it goes.
    space: u64,
    from: u64,
    since: Instant,
    leaving: Option<Instant>,
    shown: Option<Conversation>,
    reply: Entity<TextInput>,
    /// Its pages' pictures, by tab: the request out for each, and the picture once it came.
    pictures: Vec<(u64, u64, Option<PagePicture>)>,
}

impl Talk {
    pub(super) fn space(&self) -> u64 {
        self.space
    }
}

impl Shell {
    /// The task whose conversation the window shows, for the Agents to send.
    pub fn talk_agent(&self) -> Option<SharedString> {
        self.talk.as_ref().filter(|talk| talk.leaving.is_none()).map(|talk| talk.agent.clone())
    }

    /// The card's click: its conversation in the page's place, or, shown already, the page again.
    pub(super) fn show_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        if self.talk_agent().as_ref() == Some(&agent) {
            return self.leave_talk(cx);
        }
        let name = (self.task_cards.iter().find(|card| card.agent == agent))
            .map_or("the agent", |card| card.cli.name());
        let placeholder = self.palette.text(FORM_PLACEHOLDER);
        let reply = cx.new(|cx| {
            TextInput::new(format!("Reply to {name}…"), cx).placeholder_color(placeholder)
        });
        let from = match self.talk.take() {
            Some(talk) => talk.from,
            None => self.selected,
        };
        // check.sh reads this line.
        log!("browser: task {agent} shown");
        self.talk = Some(Talk {
            agent: agent.clone(),
            space: self.space,
            from,
            since: Instant::now(),
            leaving: None,
            shown: None,
            reply,
            pictures: Vec::new(),
        });
        self.browser.show_task(agent, cx);
        self.browser.focus_ui(cx);
        self.focus_talk = true;
        cx.notify();
    }

    /// The conversation goes back into its card, and the tab shown before shows again.
    pub(super) fn leave_talk(&mut self, cx: &mut Context<Self>) {
        let Some(talk) = self.talk.as_mut().filter(|talk| talk.leaving.is_none()) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: task {} left", talk.agent);
        talk.leaving = Some(Instant::now());
        if cx.reduce_motion() {
            self.end_talk(cx);
        }
        cx.notify();
    }

    fn end_talk(&mut self, cx: &mut Context<Self>) {
        let Some(talk) = self.talk.take() else { return };
        if talk.from != 0 && self.space == talk.space {
            self.select(talk.from, cx);
        }
    }

    /// A page or another Space shows: the card is the page's again.
    pub(super) fn page_ends_talk(&mut self) {
        if self.talk.as_ref().is_some_and(|talk| self.selected != 0 || self.space != talk.space) {
            self.talk = None;
        }
    }

    /// Ends the motion out once it's done, and draws the next frame of either while one runs.
    pub(super) fn step_talk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        if talk.leaving.is_some_and(|since| since.elapsed().as_secs_f32() >= TIME) {
            return self.end_talk(cx);
        }
        let since = talk.leaving.unwrap_or(talk.since);
        if since.elapsed().as_secs_f32() < TIME && !cx.reduce_motion() {
            window.request_animation_frame();
        }
    }

    /// The Agents' conversation of the task shown, as it changed. Its pages are pictured again, as
    /// what it's doing changed, and only while it shows.
    pub fn set_conversation(&mut self, conversation: Option<Conversation>, cx: &mut Context<Self>) {
        let Some(talk) = self.talk.as_mut().filter(|talk| talk.leaving.is_none()) else {
            return;
        };
        if talk.shown == conversation {
            return;
        }
        let Some(conversation) = conversation.filter(|shown| shown.agent == talk.agent) else {
            return self.leave_talk(cx);
        };
        let moved = talk.shown.as_ref().is_none_or(|shown| {
            (&shown.doing, &shown.tabs, shown.lines.len())
                != (&conversation.doing, &conversation.tabs, conversation.lines.len())
        });
        if moved {
            let mut pictures = Vec::new();
            for (tab, _) in &conversation.tabs {
                let old = talk.pictures.iter_mut().find(|(each, ..)| each == tab);
                let request = next_request();
                pictures.push((*tab, request, old.and_then(|(_, _, picture)| picture.take())));
                self.browser.task_picture(*tab, request, cx);
            }
            if let Some(talk) = self.talk.as_mut() {
                talk.pictures = pictures;
            }
        }
        if let Some(talk) = self.talk.as_mut() {
            talk.shown = Some(conversation);
        }
        cx.notify();
    }

    /// A picture the conversation asked for came: whether it was one of its own.
    pub(super) fn talk_pictured(
        &mut self,
        request: u64,
        picture: &Option<PagePicture>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(talk) = self.talk.as_mut() else { return false };
        let Some(entry) = talk.pictures.iter_mut().find(|(_, each, _)| *each == request) else {
            return false;
        };
        if picture.is_some() {
            entry.2 = picture.clone();
            cx.notify();
        }
        true
    }

    /// A picture's click: lane 2's page mode takes it from here, the task's page in the card.
    pub(super) fn show_task_page(&mut self, agent: SharedString, tab: u64, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!("browser: task {agent} page {tab}");
        cx.notify();
    }

    fn send_reply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        let input = talk.reply.read(cx);
        if input.composing() {
            return;
        }
        let text = input.text().trim().to_string();
        if text.is_empty() {
            return;
        }
        let agent = talk.agent.clone();
        talk.reply.update(cx, |input, cx| input.reset(SharedString::default(), window, cx));
        self.browser.task(agent, TaskAction::Reply(text.into()), cx);
    }

    fn stop_task(&mut self, cx: &mut Context<Self>) {
        let Some(talk) = &self.talk else { return };
        // check.sh reads this line.
        log!("browser: command stop-task via conversation");
        self.browser.task(talk.agent.clone(), TaskAction::Stop, cx);
    }

    /// Focuses the reply field once it's drawn, after the card's click.
    pub(super) fn focus_talk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.focus_talk)
            && let Some(talk) = &self.talk
        {
            window.focus(&talk.reply.focus_handle(cx), cx);
        }
    }

    /// The conversation on the card, coming in from the card's side or going back.
    pub(super) fn talk_view(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let talk = self.talk.as_ref().filter(|talk| talk.space == self.space)?;
        if self.selected != 0 {
            return None;
        }
        let shown = match (talk.leaving, cx.reduce_motion()) {
            (_, true) => 1.,
            (Some(since), false) => fade_at(1., 0., since.elapsed().as_secs_f32(), TIME),
            (None, false) => fade_at(0., 1., talk.since.elapsed().as_secs_f32(), TIME),
        };
        let (frame, _) = self.page_frame(self.viewport);
        let column = div()
            .relative()
            .left(px(-SLIDE * (1. - shown)))
            .opacity(shown)
            .size_full()
            .max_w(px(COLUMN))
            .flex()
            .flex_col()
            .text_size(px(text::BODY))
            .text_color(self.palette.text(TITLE))
            .child(self.talk_header(talk, cx))
            .children(self.talk_pictures(talk, cx))
            .child(
                div()
                    .id("conversation-lines")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(LINE_GAP))
                    .pt(px(LINE_GAP))
                    .children(self.talk_lines(talk)),
            )
            .child(self.talk_reply(talk, cx));
        Some(
            div()
                .id("conversation")
                .key_context(KEY_CONTEXT)
                .on_action(cx.listener(|shell, _: &SendReply, window, cx| {
                    shell.send_reply(window, cx)
                }))
                .on_action(cx.listener(|shell, _: &LeaveConversation, _, cx| shell.leave_talk(cx)))
                .on_action(cx.listener(|shell, _: &StopTask, _, cx| shell.stop_task(cx)))
                .absolute()
                .left(frame.left())
                .top(frame.top())
                .w(frame.size.width)
                .h(frame.size.height)
                .px(px(INSET))
                .pt(px(TOP))
                .pb(px(REPLY_BOTTOM))
                .flex()
                .justify_center()
                // A press here is the conversation's, not the window background's, which moves the
                // window.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(column)
                .into_any_element(),
        )
    }

    /// The agent's name and the task's title, with Stop ⌘. while it works or waits on the user.
    fn talk_header(&self, talk: &Talk, cx: &mut Context<Self>) -> gpui::Div {
        let shown = talk.shown.as_ref();
        let name = shown.map_or(SharedString::default(), |shown| {
            format!("{} · {}", shown.name, shown.title).into()
        });
        let stop = shown.filter(|shown| shown.stoppable).map(|_| {
            card_button(&self.palette, "conversation-stop", "Stop ⌘.")
                .on_click(cx.listener(|shell, _, _, cx| shell.stop_task(cx)))
        });
        div()
            .flex_none()
            .h(px(HEADER))
            .flex()
            .items_center()
            .justify_between()
            .text_color(self.palette.text(SECONDARY))
            .child(div().min_w_0().truncate().child(name))
            .children(stop)
    }

    /// Its pages' pictures, each over its site, in a row; a click shows the page.
    fn talk_pictures(&self, talk: &Talk, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let shown = talk.shown.as_ref().filter(|shown| !shown.tabs.is_empty())?;
        let pictures = shown.tabs.iter().map(|(tab, site)| {
            let tab = *tab;
            let image = (talk.pictures.iter().find(|(each, ..)| *each == tab))
                .and_then(|(_, _, picture)| picture.as_ref().map(PagePicture::image));
            let picture = div()
                .relative()
                .w(px(PICTURE_WIDTH))
                .h(px(PICTURE_HEIGHT))
                .rounded(px(PICTURE_RADIUS))
                .bg(self.palette.plate(rgba(KEYS_FILL).a))
                .overflow_hidden()
                .children(image.map(|image| {
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let corners = Corners::all(px(PICTURE_RADIUS));
                            window.paint_image(bounds, bounds, corners, image.clone(), 0, false).ok();
                        },
                    )
                    .absolute()
                    .size_full()
                }));
            let agent = talk.agent.clone();
            div()
                .id(("conversation-page", tab))
                .flex_none()
                .w(px(PICTURE_WIDTH))
                .cursor_pointer()
                .child(picture)
                .child(
                    div()
                        .h(px(SITE))
                        .truncate()
                        .text_size(px(text::SMALL))
                        .text_color(self.palette.text(SECONDARY))
                        .child(site.clone()),
                )
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.show_task_page(agent.clone(), tab, cx)
                }))
        });
        Some(div().flex_none().flex().gap(px(PICTURE_GAP)).pt(px(LINE_GAP)).children(pictures))
    }

    /// What was asked, with its page, and the lines after it, then what it's doing now.
    fn talk_lines(&self, talk: &Talk) -> Vec<AnyElement> {
        let Some(shown) = &talk.shown else { return Vec::new() };
        let quiet = |text: SharedString| {
            div()
                .text_size(px(text::SMALL))
                .text_color(self.palette.text(SECONDARY))
                .child(text)
        };
        let you = |text: SharedString| {
            div()
                .flex()
                .justify_end()
                .child(
                    div()
                        .max_w(px(COLUMN * 0.75))
                        .px(px(BUBBLE_X))
                        .py(px(BUBBLE_Y))
                        .rounded(px(RADIUS))
                        .bg(self.palette.plate(rgba(KEYS_FILL).a))
                        .child(text),
                )
                .into_any_element()
        };
        let mut lines = vec![you(shown.asked.clone())];
        if let Some(page) = &shown.page {
            let title = if page.title.is_empty() { &page.url } else { &page.title };
            lines.push(div().flex().justify_end().child(quiet(title.clone())).into_any_element());
        }
        for line in &shown.lines {
            lines.push(match line {
                Line::You(text) => you(text.clone()),
                Line::Said(text) => div().child(text.clone()).into_any_element(),
                Line::Steps { last, count } => {
                    let steps = match count {
                        1 => "1 step".to_string(),
                        count => format!("{count} steps"),
                    };
                    quiet(format!("{last} · {steps}").into()).into_any_element()
                }
            });
        }
        if let Some(doing) = &shown.doing {
            let dot = ringed_dot(STATUS_AGENT, STATUS_DOT).opacity(self.breath.strength());
            lines.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(BUBBLE_X))
                    .child(dot)
                    .child(quiet(doing.clone()))
                    .into_any_element(),
            );
        }
        lines
    }

    /// The reply field, the TextInput, so IME and the field's keys are a text field's.
    fn talk_reply(&self, talk: &Talk, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let reply = talk.reply.clone();
        div()
            .id("conversation-reply")
            .flex_none()
            .mt(px(LINE_GAP))
            .h(px(REPLY_HEIGHT))
            .px(px(REPLY_INSET))
            .flex()
            .items_center()
            .rounded(px(RADIUS))
            .bg(self.palette.plate(rgba(KEYS_FILL).a))
            .font_weight(FontWeight::NORMAL)
            .on_click(cx.listener(move |_, _, window, cx| {
                window.focus(&reply.focus_handle(cx), cx);
            }))
            .child(div().flex_1().child(talk.reply.clone()))
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

    use super::TIME;
    use crate::agents::runner::Cli;
    use crate::agents::task::{Card, Conversation, Dot, Line};
    use crate::shell::{Command, Folders, Shell, Slot, recorder::Recorder};

    fn card() -> Card {
        Card {
            agent: "task-1".into(),
            home: 1,
            cli: Cli::Claude,
            title: "Flight to Lisbon".into(),
            status: "Searching flights".into(),
            dot: Dot::Working,
            answers: Vec::new(),
            question: None,
            open: None,
            retry: false,
            command: None,
        }
    }

    fn conversation(doing: &str) -> Conversation {
        Conversation {
            agent: "task-1".into(),
            name: "Claude",
            title: "Flight to Lisbon".into(),
            asked: "a flight to Lisbon next Friday".into(),
            page: None,
            lines: vec![Line::Steps { last: "Opened a page · skyscanner.net".into(), count: 3 }],
            doing: Some(doing.to_string().into()),
            tabs: vec![(20, "skyscanner.net".into())],
            stoppable: true,
        }
    }

    /// A window with tab 10 shown and a task's card at the top of Today, its conversation shown by
    /// a click on the card, as the core then shows no page.
    fn shown(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        cx.update(|cx| {
            crate::shell::commands::bind_keys(cx);
            crate::text_input::bind_keys(cx);
        });
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        shell.update(cx, |shell, cx| {
            let space = (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10, 11], Vec::new(), Folders::default(), cx);
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
            shell.set_task_cards(vec![card()], cx);
            shell.slides.clear();
        });
        cx.run_until_parked();
        let at = shell.read_with(cx, |shell, _| {
            (shell.drawn_list_top() + shell.layout()[&Slot::Task(1)] + 12., shell.width() / 2.)
        });
        host.take();
        cx.simulate_click(point(px(at.1), px(at.0)), Modifiers::none());
        shell.update(cx, |shell, cx| shell.set_selected(0, cx));
        cx.run_until_parked();
        (shell, host, cx)
    }

    /// A click on the card shows its conversation in the page's place and gives its reply field the
    /// keyboard; its pages are pictured only while it shows, and again as what it does changes.
    #[gpui::test]
    fn the_card_shows_its_conversation(cx: &mut TestAppContext) {
        let (shell, host, cx) = shown(cx);
        assert!(host.take().contains(&"show task task-1".to_string()));
        shell.read_with(cx, |shell, _| assert_eq!(shell.talk_agent().as_deref(), Some("task-1")));

        shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Reading")), cx));
        assert_eq!(host.take(), ["task picture 20"]);
        shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Reading")), cx));
        assert!(host.take().is_empty(), "pictured again with nothing changed");
        shell.update(cx, |shell, cx| shell.set_conversation(Some(conversation("Clicked")), cx));
        assert_eq!(host.take(), ["task picture 20"]);

        cx.run_until_parked();
        cx.simulate_input("Ryanair");
        cx.simulate_keystrokes("enter");
        assert!(host.take().contains(&r#"task task-1 Reply("Ryanair")"#.to_string()));
        cx.simulate_keystrokes("cmd-.");
        assert!(host.take().contains(&"task task-1 Stop".to_string()));
    }

    /// Esc sends it back into its card, and the tab shown before shows again once it has gone.
    #[gpui::test]
    fn escape_leaves_for_the_tab_shown_before(cx: &mut TestAppContext) {
        let (shell, host, cx) = shown(cx);
        host.take();
        cx.simulate_keystrokes("escape");
        assert!(!host.take().iter().any(|line| line.starts_with("task ")), "Esc stopped it");
        shell.update(cx, |shell, _| {
            assert_eq!(shell.talk_agent(), None);
            let talk = shell.talk.as_mut().expect("it went at once");
            talk.leaving = Some(Instant::now() - Duration::from_secs_f32(TIME));
        });
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert!(host.take().contains(&"select 10".to_string()));
        shell.read_with(cx, |shell, _| assert!(shell.talk.is_none()));
    }

    /// ⌘W dismisses the task whose conversation shows, and a tab shown ends the conversation.
    #[gpui::test]
    fn command_w_dismisses_and_a_tab_ends_it(cx: &mut TestAppContext) {
        let (shell, host, cx) = shown(cx);
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "ui", cx));
        assert!(host.take().contains(&"task task-1 Dismiss".to_string()));
        shell.read_with(cx, |shell, _| assert_eq!(shell.talk_agent(), None));

        shell.update(cx, |shell, cx| {
            shell.show_task("task-1".into(), cx);
            shell.set_selected(11, cx);
            assert!(shell.talk.is_none());
        });
    }
}
