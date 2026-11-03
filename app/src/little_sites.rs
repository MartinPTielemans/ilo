//! The profile Little ilo opens each site in: the one the user last picked for it in Little ilo's
//! bar, as Arc's Little Arc remembers a domain's (docs/ux-spec.md › Little Arc › Profile). The
//! app keeps them in its profile directory; they're the user's, so no agent path reads or sets
//! them.

/// Each site, by its host less a leading "www.", with the name of the profile picked for it.
#[derive(Default, Debug, PartialEq)]
pub struct Sites {
    sites: Vec<(String, String)>,
}

impl Sites {
    /// Reads the file: a line a site, its host, a tab, then its profile's name.
    pub fn parse(text: &str) -> Self {
        let sites = text
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .filter(|(host, profile)| !host.is_empty() && !profile.is_empty())
            .map(|(host, profile)| (host.to_string(), profile.to_string()))
            .collect();
        Sites { sites }
    }

    pub fn to_text(&self) -> String {
        self.sites.iter().map(|(host, profile)| format!("{host}\t{profile}\n")).collect()
    }

    /// The profile picked for `url`'s site, if one was.
    pub fn profile(&self, url: &str) -> Option<&str> {
        let site = site(url)?;
        self.sites.iter().find(|(host, _)| *host == site).map(|(_, profile)| profile.as_str())
    }

    /// Keeps `profile` as the one `url`'s site opens in, and says whether that changed anything.
    pub fn remember(&mut self, url: &str, profile: &str) -> bool {
        let Some(site) = site(url) else {
            return false;
        };
        match self.sites.iter_mut().find(|(host, _)| *host == site) {
            Some((_, kept)) if kept == profile => false,
            Some((_, kept)) => {
                *kept = profile.to_string();
                true
            }
            None => {
                self.sites.push((site, profile.to_string()));
                true
            }
        }
    }

    /// The profile `from` is now named `to`, and its sites follow it.
    pub fn rename(&mut self, from: &str, to: &str) -> bool {
        let mut changed = false;
        for (_, profile) in self.sites.iter_mut().filter(|(_, profile)| profile == from) {
            *profile = to.to_string();
            changed = true;
        }
        changed
    }

    /// Forgets the sites of profiles not in `names`, which were deleted.
    pub fn keep_profiles(&mut self, names: &[&str]) -> bool {
        let before = self.sites.len();
        self.sites.retain(|(_, profile)| names.contains(&profile.as_str()));
        self.sites.len() != before
    }
}

/// A web page's site: its host, lowercased, less a leading "www.", or None for any other URL.
pub fn site(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    // A port follows the last colon, but for an IPv6 address's own, inside its brackets.
    let host = match host.rfind(':') {
        Some(at) if !host[at..].contains(']') => &host[..at],
        _ => host,
    };
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    (!host.is_empty()).then(|| host.to_string())
}

#[cfg(test)]
mod tests {
    use super::{Sites, site};

    #[test]
    fn a_site_is_its_host_without_www() {
        assert_eq!(site("https://www.Example.com/a?b#c").as_deref(), Some("example.com"));
        assert_eq!(site("http://user@mail.example.com:8080/").as_deref(), Some("mail.example.com"));
        assert_eq!(site("https://[::1]:443/").as_deref(), Some("[::1]"));
        assert_eq!(site("file:///tmp/a.html"), None);
        assert_eq!(site("about:blank"), None);
    }

    /// A pick is kept for its site, survives the file, follows a rename, and goes with its profile.
    #[test]
    fn the_profile_picked_for_a_site_is_kept() {
        let mut sites = Sites::default();
        assert!(sites.remember("https://www.example.com/a", "Work"));
        assert!(!sites.remember("https://example.com/b", "Work"));
        assert_eq!(sites.profile("https://example.com/c"), Some("Work"));
        assert_eq!(sites.profile("https://other.example/"), None);
        assert!(sites.remember("https://example.com/", "Default"));
        assert!(!sites.remember("about:blank", "Work"));
        sites.remember("https://news.example/", "Work");

        let read = Sites::parse(&sites.to_text());
        assert_eq!(read, sites);
        assert_eq!(Sites::parse("bad line\n\tWork\nhost\t\n"), Sites::default());

        let mut renamed = read;
        assert!(renamed.rename("Work", "Office"));
        assert_eq!(renamed.profile("https://news.example/"), Some("Office"));
        assert!(renamed.keep_profiles(&["Default"]));
        assert_eq!(renamed.profile("https://news.example/"), None);
        assert_eq!(renamed.profile("https://example.com/"), Some("Default"));
    }
}
