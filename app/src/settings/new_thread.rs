//! Settings › Threads' `A new thread starts with` (S8): Access and Model, which open the composer's
//! own menus and keep a pick where the composer keeps its last, in the front window's profile, so
//! the two are one choice and never two.

use super::*;
use browser::agents::task::MODEL;
use browser::agents::tier::{ACCESS, Tier};
use browser::command_bar::{self, EFFORT_DEFAULT, EFFORT_NAME, ModelPick};

/// The group's heading, as drawn (S8), and the line under it, as the table has it (ST1).
const NEW_THREAD: &str = "A new thread starts with";
const NEW_THREAD_FOOT: &str = "Pick differently for one thread where you write it.";

impl Settings {
    /// The group, while threads are on: Access, Model once an assistant is signed in, and Effort
    /// while the assistant a new thread starts on has a program that takes one.
    pub(super) fn new_thread(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Vec<Block> {
        let look = &self.look;
        let space = crate::command_bar_host::front_space(cx);
        let last = agents.read(cx).composer_last(space);
        let offered = crate::command_bar_host::task_agents_of(agents.read(cx));
        let access = {
            let (agents, tier) = (agents.clone(), last.tier);
            let choose = cx.listener(move |settings, _, window, cx| {
                settings.choose_access(&agents, tier, window, cx)
            });
            self.popup(
                Popup::NewThreadAccess,
                tier.label().into(),
                POPUP_WIDTH,
                choose,
            )
            .child(self.drawn("new thread access".into()))
            .into_any_element()
        };
        let mut rows = vec![setting(look, ACCESS, None, vec![access])];
        let picked =
            (last.model.as_ref()).map(|(cli, id)| (SharedString::from(cli.id()), id.clone()));
        if let Some(agent) = command_bar::starts_on(&offered, picked.as_ref()) {
            let model = command_bar::next_model(picked.as_ref(), agent);
            let words = command_bar::model_words(agent, &model);
            let choosing = agents.clone();
            let choose = cx.listener(move |settings, _, window, cx| {
                settings.choose_new_model(&choosing, window, cx)
            });
            let model = self
                .popup(Popup::NewThreadModel, words.into(), POPUP_WIDTH, choose)
                .child(self.drawn("new thread model".into()))
                .into_any_element();
            rows.push(setting(look, MODEL, None, vec![model]));
            let levels = command_bar::efforts_of(&agent.id);
            if !levels.is_empty() {
                let effort = last.effort.filter(|level| levels.contains(level));
                let words = effort.map_or(EFFORT_DEFAULT.into(), command_bar::effort_name);
                let agents = agents.clone();
                let choose = cx.listener(move |settings, _, window, cx| {
                    settings.choose_effort(&agents, levels, effort, window, cx)
                });
                let effort = self
                    .popup(Popup::NewThreadEffort, words.into(), POPUP_WIDTH, choose)
                    .child(self.drawn("new thread effort".into()))
                    .into_any_element();
                rows.push(setting(look, EFFORT_NAME, None, vec![effort]));
            }
        }
        vec![
            Block::Heading(NEW_THREAD.into()),
            Block::Rows(rows),
            Block::Under(foot(look, NEW_THREAD_FOOT)),
        ]
    }

    /// Access's menu: the composer's level menu, without Full access, which is never where a
    /// thread starts.
    fn choose_access(
        &mut self,
        agents: &Entity<Agents>,
        current: Tier,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let (tiers, items) = command_bar::level_choices(current, false);
        let (settings, agents) = (cx.weak_entity(), agents.clone());
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some(&tier) = picked.and_then(|at| tiers.get(at)) {
                log!("browser: new thread level {}", tier.label());
                let space = crate::command_bar_host::front_space(cx);
                agents.update(cx, |agents, cx| {
                    let last = agents.composer_last(space);
                    agents.remember_composer(space, tier, last.model);
                    cx.notify();
                });
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::NewThreadAccess, items, picked, window, cx);
    }

    /// Effort's menu: the composer's effort menu, for the program of the assistant a new thread
    /// starts on.
    fn choose_effort(
        &mut self,
        agents: &Entity<Agents>,
        levels: &'static [&'static str],
        current: Option<&'static str>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let (picks, items) = command_bar::effort_choices(levels, current);
        let (settings, agents) = (cx.weak_entity(), agents.clone());
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some(&effort) = picked.and_then(|at| picks.get(at)) {
                log!("browser: new thread effort {}", effort.unwrap_or("default"));
                let space = crate::command_bar_host::front_space(cx);
                agents.update(cx, |agents, cx| {
                    agents.set_new_thread_effort(space, effort);
                    cx.notify();
                });
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::NewThreadEffort, items, picked, window, cx);
    }

    /// Model's menu: the composer's model menu, without the way to Settings it's already in.
    fn choose_new_model(
        &mut self,
        agents: &Entity<Agents>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let space = crate::command_bar_host::front_space(cx);
        let last = agents.read(cx).composer_last(space);
        let offered = crate::command_bar_host::task_agents_of(agents.read(cx));
        let picked =
            (last.model.as_ref()).map(|(cli, id)| (SharedString::from(cli.id()), id.clone()));
        let (items, picks) = command_bar::model_choices(&offered, picked.as_ref(), false);
        let (settings, agents) = (cx.weak_entity(), agents.clone());
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            match picked.and_then(|at| picks.get(at)).cloned() {
                Some(ModelPick::Model(id, model)) => {
                    if let Some(cli) = Cli::from_id(&id) {
                        log!("browser: new thread model {}", cli.id());
                        let space = crate::command_bar_host::front_space(cx);
                        agents.update(cx, |agents, cx| {
                            let last = agents.composer_last(space);
                            agents.remember_composer(space, last.tier, Some((cli, model)));
                            cx.notify();
                        });
                    }
                }
                Some(ModelPick::SignIn(id)) => {
                    if let Some(cli) = Cli::from_id(&id) {
                        agents.update(cx, |agents, cx| agents.sign_in(cli, cx));
                    }
                }
                Some(ModelPick::Nothing | ModelPick::Settings) | None => {}
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::NewThreadModel, items, picked, window, cx);
    }
}
