//! Sensitive sites (#4369): a bank's, a tax office's, a health record's, any site the user signs
//! in to with a bank or government ID, which agents use only at Full access. No list of banks
//! knows them all, so a site is known by what happens there, all in the browser: the user's own
//! mark, and a sign-in that passes through an identity provider's pages. A form that moves money
//! is an order wherever it is (page.rs), whatever the site.

use std::collections::HashMap;
use std::sync::LazyLock;

use gpui::{App, Context};
use serde_json::{Map, Value};

use super::fence;
use super::tier::Why;

/// The list in Settings, beside Off limits: the words table's SS3, SS5 and SS6.
pub const HEADING: &str = "Sensitive sites";
pub const NONE: &str = "No sites";
pub const NONE_LINE: &str =
    "Sites you mark, and sites you sign in to with a bank or government ID, show here.";
pub const FOOT: &str = "Agents use these sites only at Full access.";

/// A row of the list, SS7: the site, and its profile with two or more.
pub fn row(site: &str, profile: Option<&str>) -> String {
    match profile {
        Some(profile) => format!("{site} · {profile}"),
        None => site.to_string(),
    }
}

/// The identity providers a bank or government ID sign-in passes through, each a site whole, as
/// `fence::entry_for` matches one: the national eIDs, the banks' shared IDs, and the brokers that
/// show them. A sign-in shown on the site's own page, as a QR code to scan, passes through none.
pub const IDENTITY: &[&str] = &[
    // Denmark: MitID, its NemLog-in, and Nets' broker.
    "mitid.dk",
    "nemlog-in.dk",
    "netseidbroker.dk",
    // Norway: BankID and ID-porten.
    "bankid.no",
    "idporten.no",
    // Sweden: BankID and Freja.
    "bankid.com",
    "frejaeid.com",
    // Finland: Suomi.fi's identification.
    "suomi.fi",
    // Estonia, Latvia and Lithuania: Smart-ID, Estonia's TARA, Latvia's eParaksts.
    "smart-id.com",
    "tara.ria.ee",
    "eparaksts.lv",
    // Iceland.
    "innskraning.island.is",
    // Belgium: itsme.
    "itsme.be",
    "itsme-id.com",
    // The Netherlands: DigiD.
    "digid.nl",
    // Switzerland: SwissID.
    "swissid.ch",
    // Austria: ID Austria.
    "eid.oesterreich.gv.at",
    // France: FranceConnect.
    "franceconnect.gouv.fr",
    // Spain: Cl@ve.
    "clave.gob.es",
    // Portugal: autenticacao.gov.
    "autenticacao.gov.pt",
    // Italy: CIE's.
    "idserver.servizicie.interno.gov.it",
    // The United Kingdom: GOV.UK One Login.
    "account.gov.uk",
    // Brokers that show these IDs for a site.
    "signicat.com",
    "criipto.id",
];

/// Whether `host` is an identity provider's (`IDENTITY`).
pub fn identity(host: &str) -> bool {
    static IDENTITIES: LazyLock<Vec<String>> =
        LazyLock::new(|| IDENTITY.iter().map(|each| each.to_string()).collect());
    fence::entry_for(&IDENTITIES, host).is_some()
}

/// The sites each profile keeps as sensitive, the user's marks and the sign-ins', and each tab's
/// way through an identity provider, which marks the site it comes back to.
#[derive(Default)]
pub struct Sensitive {
    /// Each profile's sites, in order.
    marked: HashMap<String, Vec<String>>,
    /// The tabs at an identity provider's page, and each sign-in tab's opener, whose site the
    /// sign-in is for.
    through: HashMap<u64, bool>,
    openers: HashMap<u64, u64>,
}

impl Sensitive {
    /// Whether `host` is sensitive in `profile`: marked, or under a marked site.
    pub fn of(&self, profile: &str, host: &str) -> bool {
        let marked = self.marked.get(profile);
        marked.is_some_and(|sites| fence::entry_for(sites, host).is_some())
    }

    /// `profile`'s sensitive sites, in order.
    pub fn listed(&self, profile: &str) -> Vec<String> {
        self.marked.get(profile).cloned().unwrap_or_default()
    }

    /// Every profile with a sensitive site.
    pub fn profiles(&self) -> Vec<String> {
        let mut profiles: Vec<String> = self.marked.keys().cloned().collect();
        profiles.sort();
        profiles
    }

    /// Marks `host` sensitive in `profile`, or with `on` false takes it off. True if it changed.
    pub fn mark(&mut self, profile: &str, host: &str, on: bool) -> bool {
        let Some(host) = fence::host_of(host) else {
            return false;
        };
        match on {
            true => {
                let sites = self.marked.entry(profile.to_string()).or_default();
                let Err(at) = sites.binary_search(&host) else {
                    return false;
                };
                sites.insert(at, host);
                true
            }
            false => {
                let Some(sites) = self.marked.get_mut(profile) else {
                    return false;
                };
                let Ok(at) = sites.binary_search(&host) else {
                    return false;
                };
                sites.remove(at);
                if sites.is_empty() {
                    self.marked.remove(profile);
                }
                true
            }
        }
    }

    /// `tab` was opened by `opener`'s page, as a sign-in's window is.
    pub fn opened(&mut self, tab: u64, opener: u64) {
        self.openers.insert(tab, opener);
    }

    /// `tab` is at `host`, a web page's: the site a tab comes back to from an identity provider
    /// is the one it signed in to, and a sign-in tab another opened signs in its opener's. Gives
    /// the tab whose site is then signed in to, and the host when it's this tab's own.
    pub fn at(&mut self, tab: u64, host: &str) -> Option<(u64, Option<String>)> {
        if identity(host) {
            self.through.insert(tab, true);
            return self.openers.get(&tab).map(|opener| (*opener, None));
        }
        self.through.remove(&tab)?;
        Some((tab, Some(host.to_string())))
    }

    /// `tab` closed.
    pub fn closed(&mut self, tab: u64) {
        self.through.remove(&tab);
        self.openers.remove(&tab);
    }

    /// The marks as the Agents' file keeps them: each profile's sites.
    pub fn saved(&self) -> Value {
        let profiles = (self.marked.iter())
            .map(|(profile, sites)| (profile.clone(), Value::from(sites.clone())));
        Value::Object(profiles.collect::<Map<_, _>>())
    }

    /// Takes back `saved`, as `saved` gave it.
    pub fn load(&mut self, saved: Option<&Value>) {
        self.marked.clear();
        let profiles = saved.and_then(Value::as_object).into_iter().flatten();
        for (profile, sites) in profiles {
            let sites = sites.as_array().into_iter().flatten();
            for site in sites.filter_map(Value::as_str) {
                self.mark(profile, site, true);
            }
        }
    }
}

impl super::Agents {
    /// The profile `act` acts in: its tab's, or for open_tab its task's Space's, else its
    /// agent's.
    fn act_profile(&self, act: &super::Act, cx: &App) -> String {
        let space = match act.tab {
            0 => (self.judged_task(act))
                .map_or_else(|| self.browser.agent_space(&act.agent), |task| task.space),
            tab => self.browser.space_of(tab, cx),
        };
        self.browser.space_profile(space)
    }

    /// What acting on `host` for `act` is, if it's on a sensitive site: an identity provider's
    /// page is a sign-in, and a site marked in its profile is one to use.
    pub(super) fn sensitive_why(&self, act: &super::Act, host: &str, cx: &App) -> Option<Why> {
        if identity(host) {
            return Some(Why::SignInSensitive);
        }
        let marked = self.sensitive.of(&self.act_profile(act, cx), host);
        marked.then_some(Why::Sensitive)
    }

    /// Whether `host` is sensitive in `profile`, as Site Controls' mark shows it.
    pub fn is_sensitive(&self, profile: &str, host: &str) -> bool {
        self.sensitive.of(profile, host)
    }

    /// Whether the site of `tab`'s page is sensitive in its profile, none for a page that isn't
    /// http(s), as Site Controls' mark shows it.
    pub fn tab_sensitive(&self, tab: u64, cx: &App) -> Option<bool> {
        let (profile, host) = self.tab_site(tab, cx)?;
        Some(self.sensitive.of(&profile, &host))
    }

    /// Marks the site of `tab`'s page sensitive in its profile, or takes it off, as Site
    /// Controls' mark does.
    pub fn mark_tab_sensitive(&mut self, tab: u64, on: bool, cx: &mut Context<Self>) {
        if let Some((profile, host)) = self.tab_site(tab, cx) {
            self.mark_sensitive(&profile, &host, on, cx);
        }
    }

    /// The profile and site of `tab`'s page, none for one that isn't http(s).
    fn tab_site(&self, tab: u64, cx: &App) -> Option<(String, String)> {
        let url = self.browser.tab(tab, cx).url;
        let host = super::is_web(&url).then(|| super::site(&url))?;
        Some((
            self.browser.space_profile(self.browser.space_of(tab, cx)),
            host,
        ))
    }

    /// `profile`'s sensitive sites, as Settings lists them.
    pub fn sensitive_sites(&self, profile: &str) -> Vec<String> {
        self.sensitive.listed(profile)
    }

    /// Every profile with a sensitive site.
    pub fn sensitive_profiles(&self) -> Vec<String> {
        self.sensitive.profiles()
    }

    /// Marks `host` sensitive in `profile`, or with `on` false takes it off, as the user's mark in
    /// Site Controls and Settings' Remove do.
    pub fn mark_sensitive(&mut self, profile: &str, host: &str, on: bool, cx: &mut Context<Self>) {
        self.marked(profile, host, on);
        cx.notify();
    }

    fn marked(&mut self, profile: &str, host: &str, on: bool) {
        if self.sensitive.mark(profile, host, on) {
            self.save();
            log!(
                "browser: agents sensitive {} {host}",
                if on { "marked" } else { "unmarked" }
            );
        }
    }

    /// `tab` is at `url`: a tab back from an identity provider's pages marks the site it came
    /// back to, in its profile, and a sign-in tab another opened marks its opener's.
    pub(super) fn sensitive_at(&mut self, tab: u64, url: &str, cx: &App) {
        if !super::is_web(url) {
            return;
        }
        let Some((signed, host)) = self.sensitive.at(tab, &super::site(url)) else {
            return;
        };
        let host = host.unwrap_or_else(|| super::site(&self.browser.tab(signed, cx).url));
        if host.is_empty() || identity(&host) {
            return;
        }
        let profile = self
            .browser
            .space_profile(self.browser.space_of(signed, cx));
        self.marked(&profile, &host, true);
    }

    /// `tab`, which `opener`'s page opened, may be its sign-in's.
    pub(super) fn sensitive_opened(&mut self, tab: u64, opener: u64) {
        self.sensitive.opened(tab, opener);
    }

    /// `tab` closed.
    pub(super) fn sensitive_closed(&mut self, tab: u64) {
        self.sensitive.closed(tab);
    }
}

#[cfg(test)]
mod tests;
