//! Settings › Overview (S1): the default browser's notice, Loose ends and the six tiles, from what
//! the app already holds (browser::settings_overview). Drawing it asks the OS nothing and runs
//! nothing of an assistant's.

use super::*;
use browser::settings_overview::{
    self as overview, ELSEWHERE, Facts, LEFT_FROM_SETUP, LOOSE_ENDS, LooseEnd, MAKE_DEFAULT,
    SHOW_DISMISSED,
};
use browser::tokens::settings::overview::NOTICE;

impl Settings {
    /// Overview's page: the notice while the product isn't the default, the loose ends the app
    /// knows of, then the tiles and the line that points at the search.
    pub(super) fn overview(&mut self, cx: &mut Context<Self>) -> Page {
        let put_away =
            (self.put_away).get_or_insert_with(|| overview::PutAway::load(crate::profile_dir()));
        let ends = match agents::global(cx) {
            Some(agents) => overview::loose_ends(agents.read(cx), put_away),
            None => Vec::new(),
        };
        let dismissed = put_away.any();
        let look = &self.look;
        let mut blocks = Vec::new();
        let default = self.default_browser.read(cx);
        if overview::notice_shows(default) {
            let offers = default.offers();
            let line: SharedString = match offers {
                true => ELSEWHERE.into(),
                false => format!("{ELSEWHERE} {}", default.detail()).into(),
            };
            // The user's click alone asks the OS, as General's Make Default does.
            let make = offers.then(|| {
                let make = cx.listener(|settings: &mut Self, _, _, cx| {
                    log!("browser: make default browser");
                    settings.default_browser.read(cx).make_default();
                });
                browser::settings_rows::primary(look, "overview-make-default", MAKE_DEFAULT)
                    .when(self.a11y, |make| named_button(make, MAKE_DEFAULT))
                    .tab_index(0)
                    .focus_from_keys_only()
                    .focus_visible(ring(look.ink))
                    .relative()
                    .child(self.drawn("overview make default".into()))
                    .on_click(make)
                    .into_any_element()
            });
            let notice = mark_row(
                look,
                overview::not_default(),
                Some(line),
                make.into_iter().collect(),
            )
            .min_h(px(NOTICE));
            blocks.push(Block::Group(group(look, [notice])));
        }
        if !ends.is_empty() {
            let rows = ends.into_iter().enumerate().map(|(index, end)| {
                let act = Rc::new(move |_: &mut Window, cx: &mut App| match end {
                    LooseEnd::SignIn(cli) => {
                        let Some(agents) = agents::global(cx) else {
                            return;
                        };
                        // As the assistant's row does: its maker's page while it's missing.
                        if agents.read(cx).found(cli) == agents::ready::Ready::NotFound {
                            log!("browser: assistant install page {}", cli.id());
                            open_notice(cli.install_page(), cx);
                        } else {
                            agents.update(cx, |agents, cx| agents.sign_in(cli, cx));
                        }
                    }
                });
                let settings = cx.weak_entity();
                let away = Rc::new(move |_: &mut Window, cx: &mut App| {
                    log!("browser: loose end put away");
                    let put = |settings: &mut Settings, cx: &mut Context<Settings>| {
                        if let Some(put_away) = &mut settings.put_away {
                            put_away.put_away(&end.key());
                        }
                        cx.notify();
                    };
                    settings.update(cx, put).ok();
                });
                overview::loose_end_row(look, index, end, self.a11y, act, away)
            });
            let left = div()
                .text_size(px(text::SMALL))
                .line_height(px(DETAIL_LINE))
                .text_color(look.detail)
                .child(LEFT_FROM_SETUP)
                .into_any_element();
            blocks.push(Block::Scoped(LOOSE_ENDS.into(), left));
            blocks.push(Block::Group(group(look, rows)));
        }
        // The way back for every loose end its ✕ put away, as ⌘Z is for the last (#4409).
        if dismissed {
            let show =
                browser::settings_rows::button(look, "overview-show-dismissed", SHOW_DISMISSED)
                    .when(self.a11y, |show| named_button(show, SHOW_DISMISSED))
                    .tab_index(0)
                    .focus_from_keys_only()
                    .focus_visible(ring(look.ink))
                    .child(self.drawn("overview show dismissed".into()))
                    .on_click(cx.listener(|settings: &mut Self, _, _, cx| {
                        log!("browser: loose ends brought back");
                        if let Some(put_away) = &mut settings.put_away {
                            put_away.bring_back_all();
                        }
                        cx.notify();
                    }));
            blocks.push(Block::View(
                div().flex().justify_end().child(show).into_any_element(),
            ));
        }
        let facts = self.overview_facts(cx);
        let cards = (overview::tiles(&facts).into_iter()).map(|tile| {
            let settings = cx.weak_entity();
            let section = tile.section;
            let open = Rc::new(move |window: &mut Window, cx: &mut App| {
                let show = |settings: &mut Settings, cx: &mut Context<Settings>| {
                    settings.show_section(section, window, cx)
                };
                settings.update(cx, show).ok();
            });
            overview::tile_card(look, &tile, self.a11y, open)
                .child(self.drawn(format!("overview tile {}", section.name())))
        });
        blocks.push(Block::Heading(overview::set_up().into()));
        blocks.push(Block::View(
            overview::tiles_grid(cards.collect())
                .child(overview::note(look, &shell::keystroke_label("cmd-f")))
                .into_any_element(),
        ));
        Page::new(Section::Overview, "", blocks)
    }

    /// What the tiles say, each read from what the app already holds: the core's settings, and
    /// the logins and devices only once their pages had them read.
    fn overview_facts(&self, cx: &mut Context<Self>) -> Facts {
        let profiles = profiles();
        let after = unsafe { bc::bc_archive_after() };
        let level = agents::global(cx).map(|agents| {
            let space = unsafe { bc::bc_window_space(crate::front().0) };
            let tier = agents.read(cx).composer_last(space).tier;
            SharedString::from(tier.label())
        });
        let collector = crash_reports_collector();
        let crash_sent = !collector.is_empty() && unsafe { bc::bc_send_crash_reports() };
        let count_sent = crate::daily_count_host::named() && crate::daily_count_host::on();
        let profile = self.passwords_profile(&profiles);
        let saved = profile.and_then(|profile| {
            let mut offers = false;
            let what = bc::BC_SAVE_PASSWORDS as i32;
            unsafe { bc::bc_profile_offers_to_save(profile, what, &mut offers) }.then_some(offers)
        });
        let logins = profile.and_then(|profile| {
            let name = profiles
                .iter()
                .find(|each| each.id == profile)?
                .name
                .clone();
            Some((self.logins.get(&profile)?.len(), name))
        });
        let account = self.account.read(cx);
        Facts {
            edge: !off(NO_EDGE_REVEAL),
            zoom: Some(unsafe { bc::bc_page_zoom() }),
            archive: (after != bc::BC_ARCHIVE_NEVER as f64).then(archive_after_words),
            profiles: profiles.len(),
            spaces: profiles.iter().map(|profile| profile.spaces).sum(),
            level,
            ads_blocked: unsafe { bc::bc_ad_blocking() },
            nothing_sent: !crash_sent && !count_sent,
            saved_and_filled: saved == Some(true),
            logins,
            signed_in: account.state().status == account::Status::On,
            signed_out: matches!(
                account.state().status,
                account::Status::SignedOut | account::Status::SigningIn
            ),
            devices: account.device_count(),
        }
    }
}
