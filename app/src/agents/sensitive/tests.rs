//! The marks and the sign-ins that make them, on their own.

use super::*;

/// The identity providers are each a site whole: their own pages, and none whose name only
/// ends the same.
#[test]
fn identity_providers_are_sites_whole() {
    for host in [
        "www.mitid.dk",
        "nemlog-in.mitid.dk",
        "auth.bankid.no",
        "account.gov.uk",
    ] {
        assert!(identity(host), "{host}");
    }
    for host in ["notmitid.dk", "mitid.dk.example", "bank.example", ""] {
        assert!(!identity(host), "{host}");
    }
}

/// A mark covers the site's subdomains, in its profile alone, and comes back as it was saved.
#[test]
fn marks_are_each_profiles_and_kept() {
    let mut sensitive = Sensitive::default();
    assert!(sensitive.mark("Default", "https://www.Bank.example/login", true));
    assert!(!sensitive.mark("Default", "bank.example", true));
    assert!(sensitive.of("Default", "netbank.bank.example"));
    assert!(!sensitive.of("Default", "notbank.example"));
    assert!(!sensitive.of("Profile 2", "bank.example"));
    let mut back = Sensitive::default();
    back.load(Some(&sensitive.saved()));
    assert_eq!(back.listed("Default"), ["bank.example"]);
    assert!(back.mark("Default", "bank.example", false));
    assert!(back.profiles().is_empty());
}

/// Only a tab back from an identity provider's page signs in to a site; one that only visits
/// either doesn't, and a closed tab forgets its way.
#[test]
fn a_sign_in_is_a_way_through_an_identity_provider() {
    let mut sensitive = Sensitive::default();
    assert_eq!(sensitive.at(1, "bank.example"), None);
    assert_eq!(sensitive.at(1, "www.mitid.dk"), None);
    let back = sensitive.at(1, "netbank.bank.example");
    assert_eq!(back, Some((1, Some("netbank.bank.example".into()))));
    assert_eq!(sensitive.at(1, "bank.example"), None);
    sensitive.at(2, "www.mitid.dk");
    sensitive.closed(2);
    assert_eq!(sensitive.at(2, "bank.example"), None);
    sensitive.opened(3, 1);
    assert_eq!(sensitive.at(3, "www.mitid.dk"), Some((1, None)));
}
