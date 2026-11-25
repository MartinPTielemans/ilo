//! Settings › Agents' Your assistant (#4100): its rows, each with its Model under it, an
//! assistant's ⋯ menu and the ask before Sign Out; and Notifications' Notify.

use super::*;
use browser::agents::task::{MODEL, model_default};
use browser::needs_you::{Notify, WHEN_TO_NOTIFY};

impl Settings {
    /// Your assistant: a row for each CLI tasks from ⇥ in ⌘T run on, which signs in with its own
    /// sign-in and has the mark that picks it.
    pub(super) fn assistants(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        // Each assistant's Model, under its row once it's signed in.
        let providers = agents.read(cx).providers();
        let models: Vec<Option<(gpui::Div, f32)>> = (providers.iter())
            .map(|&(cli, _, signed_in)| signed_in.then(|| self.model_row(agents, cli, cx)))
            .collect();
        let model = agents.read(cx);
        let marked = model.default_task_agent();
        let assistants = providers.into_iter().map(|(cli, shown, _)| {
            let act = {
                let (settings, agents) = (cx.weak_entity(), agents.clone());
                move |act: Act, window: &mut Window, cx: &mut App| {
                    let agents = agents.clone();
                    match act {
                        Act::Get => {
                            log!("browser: assistant install page {}", cli.id());
                            open_notice(cli.install_page(), cx)
                        }
                        Act::SignIn => agents.update(cx, |agents, cx| agents.sign_in(cli, cx)),
                        Act::Cancel => agents.update(cx, |agents, _| agents.cancel_sign_in(cli)),
                        Act::Mark => {
                            log!("browser: tasks run on {}", cli.id());
                            agents.update(cx, |agents, cx| agents.set_default_task_agent(cli, cx))
                        }
                        Act::Menu => {
                            let menu = |settings: &mut Settings, cx: &mut Context<Settings>| {
                                settings.assistant_menu(&agents, cli, window, cx)
                            };
                            settings.update(cx, menu).ok();
                        }
                    }
                }
            };
            let row = assistant_row(
                look,
                cli,
                &shown,
                marked == Some(cli),
                self.a11y,
                self.menu_at(Popup::Assistant(cli)),
                Rc::new(act),
            );
            (row, ROW_X)
        });
        let assistants: Vec<_> = assistants.collect();
        let assistants = (assistants.into_iter().zip(models))
            .flat_map(|(row, model)| std::iter::once(row).chain(model));
        // Another app is Apps that connect's, two groups down, so it's not here as well.
        group_lined(look, assistants)
    }

    /// `cli`'s Model, under its row and set in from it as a feature's own settings are, with the
    /// hairline from where it's set in: what its new tasks start with.
    fn model_row(
        &self,
        agents: &Entity<Agents>,
        cli: Cli,
        cx: &mut Context<Self>,
    ) -> (gpui::Div, f32) {
        let label: SharedString = match agents.read(cx).task_model_name(cli) {
            Some(model) => model,
            None => model_default(cli).into(),
        };
        let agents = agents.clone();
        let popup = self.popup(
            Popup::Model(cli),
            label,
            POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| {
                settings.choose_model(&agents, cli, window, cx)
            }),
        );
        let x = ROW_X + GLYPH_PLATE + GLYPH_PLATE_GAP;
        let controls = vec![
            popup
                .child(self.drawn(format!("model {}", cli.id())))
                .into_any_element(),
        ];
        (line_row(&self.look, MODEL, None, controls).pl(px(x)), x)
    }

    /// `cli`'s Model pop-up: its own default first, saying which model it is where the CLI says,
    /// then the models it lists. A model is set only from that list.
    fn choose_model(
        &mut self,
        agents: &Entity<Agents>,
        cli: Cli,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let (models, items) = agents.read(cx).task_model_menu(cli);
        let (settings, agents) = (cx.weak_entity(), agents.clone());
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some((id, _)) = picked.and_then(|at| models.get(at)) {
                // check.sh waits for this line.
                log!(
                    "browser: task model {} {}",
                    cli.id(),
                    if id.is_empty() { "default" } else { id }
                );
                agents.update(cx, |agents, cx| agents.set_task_model(cli, id, cx));
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::Model(cli), items, picked, window, cx);
    }

    /// Notifications' Notify: when a task that finishes or needs the user posts its
    /// notification.
    pub(super) fn when_to_notify(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> Row {
        let chosen = agents.read(cx).notify();
        let agents = agents.clone();
        let popup = self.popup(
            Popup::Notify,
            chosen.title().into(),
            POPUP_WIDTH,
            cx.listener(move |settings, _, window, cx| {
                settings.choose_notify(&agents, chosen, window, cx)
            }),
        );
        setting(
            &self.look,
            WHEN_TO_NOTIFY,
            None,
            vec![
                popup
                    .child(self.drawn("when to notify".into()))
                    .into_any_element(),
            ],
        )
    }

    fn choose_notify(
        &mut self,
        agents: &Entity<Agents>,
        chosen: Notify,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items = (Notify::ALL.iter())
            .map(|&each| MenuEntry {
                checked: each == chosen,
                over: each == chosen,
                ..MenuEntry::new(each.title(), true)
            })
            .collect();
        let (settings, agents) = (cx.weak_entity(), agents.clone());
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if let Some(&notify) = picked.and_then(|at| Notify::ALL.get(at)) {
                // check.sh waits for this line.
                log!("browser: notify {}", notify.id());
                agents.update(cx, |agents, cx| agents.set_notify(notify, cx));
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::Notify, items, picked, window, cx);
    }

    /// Opens a signed-in assistant's ⋯ menu: Sign Out, which asks first. Its Model is its own row.
    fn assistant_menu(
        &mut self,
        agents: &Entity<Agents>,
        cli: Cli,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let items = vec![MenuEntry::new("Sign Out…", true)];
        let (settings, agents) = (cx.weak_entity(), agents.clone());
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            if picked == Some(0) {
                let agents = agents.clone();
                // Asked once the menu is gone, in the window it was in.
                cx.defer(move |cx| {
                    let Some(window) = cx.try_global::<SettingsWindow>().map(|w| w.0) else {
                        return;
                    };
                    let ask = |settings: &mut Settings,
                               window: &mut Window,
                               cx: &mut Context<Settings>| {
                        settings.ask_sign_out(&agents, cli, window, cx)
                    };
                    window.update(cx, ask).ok();
                });
            }
            settings.update(cx, |settings, cx| settings.chose(cx)).ok();
        });
        self.pop_up(Popup::Assistant(cli), items, picked, window, cx);
    }

    /// Asks before Sign Out, which signs the assistant out everywhere it's used, as Delete asks
    /// for a profile: in an alert, or on Linux in Settings' own dialog.
    fn ask_sign_out(
        &mut self,
        agents: &Entity<Agents>,
        cli: Cli,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = cli.assistant();
        let message = format!("Sign out of {name}?");
        let detail = format!(
            "This signs {name} out everywhere you use it, not just in {}.",
            env!("APP_NAME")
        );
        let agents = agents.clone();
        let sign_out = move |cx: &mut App| {
            log!("browser: assistant sign-out {}", cli.id());
            agents.update(cx, |agents, cx| agents.sign_out(cli, cx))
        };
        #[cfg(target_os = "linux")]
        dialog::ask_in(
            &self.dialog,
            message,
            detail,
            "Sign Out",
            // It reaches past this device, so ↩ doesn't choose it.
            dialog::Return::Nothing,
            Box::new(sign_out),
            window,
            cx,
        );
        #[cfg(not(target_os = "linux"))]
        {
            let _ = window;
            cx.spawn(async move |_, cx| {
                if crate::ask(&message, &detail, &["Sign Out", "Cancel"], false) == 0 {
                    cx.update(sign_out);
                }
            })
            .detach();
        }
    }
}
