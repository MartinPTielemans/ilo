//! Settings › Agents' Your assistant (#4100): its rows, an assistant's ⋯ menu, and the ask
//! before Sign Out.

use super::*;

impl Settings {
    /// Your assistant: a row for each CLI tasks from ⇥ in ⌘T run on, which signs in with its own
    /// sign-in and has the mark that picks it, then a row for another agent, which connects.
    pub(super) fn assistants(&self, agents: &Entity<Agents>, cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let model = agents.read(cx);
        let marked = model.default_task_agent();
        let rows = Cli::ALL.map(|cli| {
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
            assistant_row(
                look,
                cli,
                &model.assistant_row(cli),
                marked == Some(cli),
                self.a11y,
                self.menu_at(Popup::Assistant(cli)),
                Rc::new(act),
            )
        });
        let how = cx.listener(|settings, _: &(), window, cx| {
            settings.show_found(Section::Agents, APPS_THAT_CONNECT.into(), window, cx)
        });
        let another = another_agent_row(look, self.a11y, move |window, cx| how(&(), window, cx));
        group(look, rows.into_iter().chain([another]))
    }

    /// Opens a signed-in assistant's ⋯ menu: Model, with its own default first then its models,
    /// and Sign Out, which asks first.
    fn assistant_menu(
        &mut self,
        agents: &Entity<Agents>,
        cli: Cli,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let (models, model_items) = agents.read(cx).task_model_menu(cli);
        let mut items = vec![MenuEntry::new("Model", true)];
        items.extend(model_items.into_iter().map(|item| MenuEntry {
            parent: Some(0),
            // A submenu opens beside its item, not over it.
            over: false,
            ..item
        }));
        items.push(MenuEntry::line());
        let sign_out = items.len();
        items.push(MenuEntry::new("Sign Out…", true));
        let (settings, agents) = (cx.weak_entity(), agents.clone());
        let picked = Box::new(move |picked: Option<usize>, cx: &mut App| {
            match picked {
                Some(at) if at == sign_out => {
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
                Some(at) => {
                    if let Some((id, _)) = at.checked_sub(1).and_then(|index| models.get(index)) {
                        // check.sh waits for this line.
                        log!(
                            "browser: task model {} {}",
                            cli.id(),
                            if id.is_empty() { "default" } else { id }
                        );
                        agents.update(cx, |agents, cx| agents.set_task_model(cli, id, cx));
                    }
                }
                None => {}
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
