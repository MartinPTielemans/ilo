//! The sites each profile has Developer Mode on for, which Settings › Agents lists beside Off limits
//! with the way to turn each off (#4377). It's the user's alone: an agent's page_log reads a page's
//! log, and no tool lists these sites or changes them.

use gpui::{ClickEvent, SharedString, Window, div, prelude::*, px};

use crate::settings_rows::{Drawn, Look, drawn, end, frame, group, stop_button, text};
use crate::tokens::settings::POPUP_WIDTH;

/// The list's heading.
pub const TITLE: &str = "Sites in Developer Mode";
/// A row's button.
pub const TURN_OFF: &str = "Turn Off";

/// What the list reads of the core.
pub trait Host {
    /// The loaded profiles, each with its id and name, the one the browser started with first.
    fn profiles(&self) -> Vec<(u64, SharedString)>;
    /// `profile`'s sites in Developer Mode (bc_developer_mode_sites).
    fn sites(&self, profile: u64) -> Vec<SharedString>;
    /// Turns Developer Mode off for `site` in `profile` (bc_developer_mode_turn_off_site).
    fn turn_off(&self, profile: u64, site: &str);
}

/// A site in Developer Mode, as the list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Site {
    pub profile: u64,
    /// The profile's name, shown only where there are two or more profiles.
    pub profile_name: Option<SharedString>,
    pub site: SharedString,
}

/// Every loaded profile's sites, the first profile's first, each profile's as the core lists them.
pub fn listed(host: &dyn Host) -> Vec<Site> {
    let profiles = host.profiles();
    let several = profiles.len() > 1;
    (profiles.into_iter())
        .flat_map(|(profile, name)| {
            let profile_name = several.then_some(name);
            (host.sites(profile).into_iter())
                .filter(|site| !site.is_empty())
                .map(|site| Site {
                    profile,
                    profile_name: profile_name.clone(),
                    site,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The user's Turn Off on `site`.
pub fn turn_off(host: &dyn Host, site: &Site) {
    host.turn_off(site.profile, &site.site);
}

/// What a row's button does when it's pressed.
pub type Press = Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App)>;

/// The list's rows, a group, each site with its Turn Off and, where there are several profiles, its
/// profile's name. None for no site, so a profile with none has no list. `press` gives the press of
/// the button of the `index`th site.
pub fn rows(
    look: &Look,
    a11y: bool,
    sites: &[Site],
    logged: &Drawn,
    press: impl Fn(usize) -> Press,
) -> Option<gpui::Div> {
    if sites.is_empty() {
        return None;
    }
    let rows = sites.iter().enumerate().map(|(index, each)| {
        let id = gpui::ElementId::Name(format!("developer-mode-off-{index}").into());
        let turn_off = stop_button(look, id, TURN_OFF, a11y)
            .relative()
            .child(drawn(
                format!("developer mode off {}", each.site),
                logged.clone(),
            ))
            .on_click(press(index));
        let named = (each.profile_name.clone()).map(|name| {
            let name = div().max_w(px(POPUP_WIDTH)).truncate().child(name);
            name.text_color(look.detail).into_any_element()
        });
        let controls = named.into_iter().chain([turn_off.into_any_element()]);
        frame()
            .child(text(look, each.site.clone(), None))
            .child(end(controls))
    });
    Some(group(look, rows))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Entity, IntoElement, Render, TestAppContext, VisualTestContext};

    use super::*;
    use crate::settings_rows::spoken::{press, pressable, stops};

    /// The core's profiles and their sites, and each Turn Off asked of it.
    #[derive(Default)]
    struct Core {
        profiles: RefCell<Vec<(u64, SharedString)>>,
        sites: RefCell<Vec<(u64, Vec<SharedString>)>>,
        turned_off: RefCell<Vec<(u64, String)>>,
    }

    impl Core {
        fn with(profiles: &[(u64, &str, &[&str])]) -> Rc<Core> {
            let core = Core::default();
            for &(id, name, sites) in profiles {
                core.profiles
                    .borrow_mut()
                    .push((id, name.to_string().into()));
                let sites = sites.iter().map(|site| site.to_string().into()).collect();
                core.sites.borrow_mut().push((id, sites));
            }
            Rc::new(core)
        }
    }

    impl Host for Core {
        fn profiles(&self) -> Vec<(u64, SharedString)> {
            self.profiles.borrow().clone()
        }

        fn sites(&self, profile: u64) -> Vec<SharedString> {
            let sites = self.sites.borrow();
            (sites.iter().find(|(id, _)| *id == profile))
                .map_or(Vec::new(), |(_, sites)| sites.clone())
        }

        fn turn_off(&self, profile: u64, site: &str) {
            self.turned_off.borrow_mut().push((profile, site.into()));
            // As the core does: the site is no longer in the profile's list.
            for (id, sites) in self.sites.borrow_mut().iter_mut() {
                if *id == profile {
                    sites.retain(|each| each != site);
                }
            }
        }
    }

    /// The list drawn as Settings draws it, on what the core says now.
    struct Shown {
        core: Rc<Core>,
    }

    impl Render for Shown {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let look = Look::settings(true);
            let sites = listed(&*self.core);
            let core = self.core.clone();
            let each = sites.clone();
            let list = rows(&look, true, &sites, &Drawn::default(), move |index| {
                let (core, site) = (core.clone(), each[index].clone());
                let press = cx.listener(move |_, _: &ClickEvent, _, cx| {
                    turn_off(&*core, &site);
                    cx.notify();
                });
                Box::new(press)
            });
            div().children(list)
        }
    }

    fn shown<'a>(
        core: &Rc<Core>,
        cx: &'a mut TestAppContext,
    ) -> (Entity<Shown>, &'a mut VisualTestContext) {
        let core = core.clone();
        let (view, cx) = cx.add_window_view(|_, _| Shown { core });
        cx.run_until_parked();
        (view, cx)
    }

    /// The buttons an assistive app can press, by name.
    fn names(cx: &mut VisualTestContext) -> Vec<String> {
        (pressable(cx).into_iter())
            .map(|(_, name, _)| name)
            .collect()
    }

    /// One profile lists its sites as they came, with no profile's name; two or more list each
    /// profile's, the first's first, each with its profile; and a profile with none lists none.
    #[test]
    fn every_profile_lists_its_sites_and_names_itself_only_among_several() {
        let one = Core::with(&[(1, "Default", &["localhost", "example.com"])]);
        let sites: Vec<_> = (listed(&*one).into_iter())
            .map(|each| (each.profile, each.profile_name, each.site.to_string()))
            .collect();
        assert_eq!(
            sites,
            [
                (1, None, "localhost".into()),
                (1, None, "example.com".into())
            ]
        );

        let two = Core::with(&[
            (1, "Default", &["localhost"]),
            (2, "Work", &[]),
            (3, "Play", &["example.com"]),
        ]);
        let sites: Vec<_> = (listed(&*two).into_iter())
            .map(|each| {
                let name = each.profile_name.map(|name| name.to_string());
                (each.profile, name, each.site.to_string())
            })
            .collect();
        assert_eq!(
            sites,
            [
                (1, Some("Default".into()), "localhost".into()),
                (3, Some("Play".into()), "example.com".into()),
            ]
        );
    }

    /// Each row's Turn Off asks the core to turn Developer Mode off for that profile and site, and
    /// the row goes with it; the last one's goes with the list, which isn't drawn empty.
    #[gpui::test]
    fn turn_off_asks_the_core_for_that_profiles_site_and_an_empty_list_is_not_drawn(
        cx: &mut TestAppContext,
    ) {
        let core = Core::with(&[
            (1, "Default", &["localhost"]),
            (3, "Play", &["example.com"]),
        ]);
        let (_view, cx) = shown(&core, cx);
        assert_eq!(names(cx), [TURN_OFF, TURN_OFF]);

        // ⇥ reaches each row's button, and Return presses it.
        let stops = stops(cx);
        assert_eq!(stops.len(), 2, "{stops:?}");
        press("enter", cx);
        assert_eq!(*core.turned_off.borrow(), [(1, "localhost".to_string())]);
        assert_eq!(names(cx), [TURN_OFF]);

        cx.update(|window, cx| window.focus_next(cx));
        press("enter", cx);
        assert_eq!(
            *core.turned_off.borrow(),
            [(1, "localhost".to_string()), (3, "example.com".to_string())]
        );
        assert_eq!(names(cx), Vec::<String>::new());
        assert!(
            rows(
                &Look::settings(true),
                true,
                &listed(&*core),
                &Drawn::default(),
                |_| { Box::new(|_, _, _| {}) }
            )
            .is_none()
        );
    }
}
