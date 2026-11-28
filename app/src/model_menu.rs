//! The model menu's content: which assistants and models the composer's model choice offers, what
//! each row picks, and what its trigger says. The row it sits in is the command bar's (#4094); this
//! is only what the menu says (#4394).

use gpui::SharedString;

use crate::shell::{MakerMark, MenuEntry};

/// An agent ⌘T's ask lists (Agent task).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskAgent {
    pub id: SharedString,
    pub name: SharedString,
    /// Its glyph, Settings › Agents' own.
    pub glyph: &'static str,
    /// The model it runs tasks on, by name, or None for its own default.
    pub model: Option<SharedString>,
    /// That model by id, empty for its own default: Settings' Model row.
    pub model_id: String,
    /// Settings' name for its default, `{assistant}'s default` (Y1), which the model trigger shows
    /// while it's picked, and which model that is where it said. The menu's row is `Default` (D3).
    pub default_row: SharedString,
    pub default_model: Option<SharedString>,
    /// The models it offers after its default, as id and name: the one list Settings' Model row
    /// is built from.
    pub models: Vec<(String, String)>,
    /// When its plan's limit resets, if it's at it.
    pub resets: Option<SharedString>,
}

/// What ⌘T's ask offers, as the browser last learned it: the agents signed in, the default first,
/// and whether it has learned which are yet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskAgents {
    pub agents: Vec<TaskAgent>,
    pub known: bool,
    /// The assistants installed but not signed in, by id and name, which the composer's model
    /// menu offers to sign in.
    pub signed_out: Vec<(SharedString, SharedString)>,
}

/// The mark before the model trigger's words: the maker's of `agent`'s models, as the menu's rows
/// have it (#4394), none for an agent that isn't one of the assistants.
pub fn mark(agent: &TaskAgent) -> Option<MakerMark> {
    crate::agents::provider::caps(&agent.id).map(|caps| caps.mark)
}

/// The model the next task on `agent` starts on, by id, empty for its own default: the pick for
/// it, or Settings'. A pick it no longer offers is its default, as in Settings.
pub fn next_model(picked: Option<&(SharedString, String)>, agent: &TaskAgent) -> String {
    let picked = picked.filter(|(on, _)| *on == agent.id).map(|(_, id)| id);
    let id = picked.unwrap_or(&agent.model_id);
    match agent.models.iter().any(|(each, _)| each == id) {
        true => id.clone(),
        false => String::new(),
    }
}

/// What the assistant's chip says: its name and the model the next task starts on, or its Model
/// menu's first row for its own default.
pub fn model_words(agent: &TaskAgent, id: &str) -> String {
    match agent.models.iter().find(|(each, _)| each == id) {
        Some((_, name)) => format!("{} · {name}", agent.name),
        None => agent.default_row.to_string(),
    }
}

/// The assistant a thread starts on: the one `picked` names, while it isn't at its limit, else the
/// first ready.
pub fn starts_on<'a>(
    agents: &'a TaskAgents,
    picked: Option<&(SharedString, String)>,
) -> Option<&'a TaskAgent> {
    let picked = picked.map(|(id, _)| id);
    let mut ready = agents.agents.iter().filter(|agent| agent.resets.is_none());
    let first = ready.clone().next();
    ready.find(|agent| Some(&agent.id) == picked).or(first)
}

/// The model menu, as its items and what each picks: each assistant by its own name, then the
/// models it offers, its default first and the one a thread starts on (`picked`, else Settings')
/// checked; one at its limit says when it resets, and one not signed in has Sign In, which starts
/// its own sign-in as Settings' does; then Settings › Agents while `to_settings`. Each assistant's
/// heading is its name as Settings' page has it. Over them all a field narrows them as the user
/// types: an assistant with no model matching goes, heading and all, as Sign In does, and Settings
/// stays whatever is typed; a pick is still one of these rows. The composer and Settings'
/// `A new thread starts with` both open it.
pub fn model_choices(
    agents: &TaskAgents,
    picked: Option<&(SharedString, String)>,
    to_settings: bool,
) -> (Vec<MenuEntry>, Vec<ModelPick>) {
    let (on, chosen) = match starts_on(agents, picked) {
        Some(agent) => (Some(agent.id.clone()), Some(next_model(picked, agent))),
        None => (None, None),
    };
    let mut items = vec![MenuEntry::search(SEARCH_MODELS)];
    let mut picks: Vec<ModelPick> = vec![ModelPick::Nothing];
    // Settings' name for the assistant, so the two never say it differently.
    let label = |id: &SharedString, name: &SharedString| -> SharedString {
        match crate::agents::runner::Cli::from_id(id) {
            Some(cli) => crate::settings_rows::assistant_name(cli).into(),
            None => name.clone(),
        }
    };
    for agent in &agents.agents {
        items.push(heading(label(&agent.id, &agent.name)));
        picks.push(ModelPick::Nothing);
        let at_limit = agent
            .resets
            .clone()
            .map(|resets| format!("Limit resets {resets}"));
        let checked = chosen.as_deref().filter(|_| on.as_ref() == Some(&agent.id));
        let default = agent.default_model.as_deref();
        let (ids, entries) = crate::agents::task::model_menu(
            checked.unwrap_or("\0"),
            DEFAULT_ROW,
            default,
            agent.models.clone(),
        );
        for ((id, _), entry) in ids.into_iter().zip(entries) {
            let entry = MenuEntry {
                enabled: at_limit.is_none(),
                detail: at_limit.clone().map(Into::into).or(entry.detail.clone()),
                over: false,
                ..entry
            };
            items.push(entry);
            picks.push(ModelPick::Model(agent.id.clone(), id));
        }
    }
    for (id, name) in &agents.signed_out {
        items.push(heading(label(id, name)));
        picks.push(ModelPick::Nothing);
        items.push(MenuEntry::named(SIGN_IN, true).offer().out());
        picks.push(ModelPick::SignIn(id.clone()));
    }
    if to_settings {
        items.push(MenuEntry::line());
        picks.push(ModelPick::Nothing);
        items.push(MenuEntry::named(SETTINGS, true).kept().out());
        picks.push(ModelPick::Settings);
    }
    (items, picks)
}

/// A heading in a menu named as written, such as an assistant's name over its models.
fn heading(title: SharedString) -> MenuEntry {
    MenuEntry {
        header: true,
        ..MenuEntry::named(title, false)
    }
}

/// A pick on the composer's model menu.
#[derive(Clone, Debug, PartialEq)]
pub enum ModelPick {
    /// A header or a line, which is never chosen.
    Nothing,
    /// A model of an assistant, by the assistant's id and the model's, empty for its default.
    Model(SharedString, String),
    /// Sign In under an assistant not signed in, by its id.
    SignIn(SharedString),
    Settings,
}

/// Words on the composer's choices, from the words table (Z9, A5).
pub(crate) const SIGN_IN: &str = "Sign In";
const SETTINGS: &str = "Settings";
/// The model menu's search field (Z9).
const SEARCH_MODELS: &str = "Search models";
/// The model menu's first row in each assistant's group (D3); Settings' picker keeps Y1's
/// `{assistant}'s default`.
pub const DEFAULT_ROW: &str = "Default";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::provider::{PROVIDERS, caps};

    /// The model trigger leads with the same mark as the menu's rows, its provider's, and none
    /// for an agent that isn't one of the providers.
    #[test]
    fn the_triggers_mark_is_its_makers_as_the_menus_rows_have_it() {
        let agent = |id: &str| TaskAgent {
            id: id.to_string().into(),
            ..TaskAgent::default()
        };
        for each in PROVIDERS {
            assert_eq!(mark(&agent(each.id)), Some(each.mark), "{}", each.id);
        }
        assert_eq!(mark(&agent("someone-else")), None);
        assert!(caps("someone-else").is_none());
    }
}
