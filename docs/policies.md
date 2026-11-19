# Managed policies

ilo takes Chromium's [enterprise policies](https://chromeenterprise.google/policies/), such as `URLBlocklist` or `ExtensionInstallForcelist`, set where ilo reads them on each system, under ilo's own names rather than every Chromium's. A user sees those in force at chrome://policy.

| System | Mandatory | Recommended |
|---|---|---|
| macOS | `/Library/Managed Preferences/app.ilo.browser.plist`, from a configuration profile whose payload type is `app.ilo.browser` | `defaults write app.ilo.browser <policy> …` |
| Linux | JSON files in `/etc/ilo/policies/managed/` | `/etc/ilo/policies/recommended/` |
| Linux, the Flatpak | an extension of `app.ilo.browser.Extension` (version 1) with `policies/managed/`, such as an unmanaged one: `/var/lib/flatpak/extension/app.ilo.browser.Extension.<name>/<arch>/1/policies/managed/` | the same with `policies/recommended/` |
| Windows | `HKLM` or `HKCU\SOFTWARE\Policies\ilo` | `…\ilo\Recommended` |

- **The Flatpak** has no view of the host's `/etc`, so its policies come through Flatpak's extension point, as Flathub's Chromium's do. An unmanaged extension's version folder may be a symlink to a folder holding `policies/`, never to a policy file itself, which Flatpak can't bind.
- **uBlock Origin**, built in, takes its settings as Chromium's 3rd-party policy for its extension id, `oghkpboejbohceocapipmnjifoklonij`, as `3rdparty.extensions.<id>` beside the others. ilo's own settings for it, Settings › Ad Blocking's lists and My filters under the key `ilo`, rank below an administrator's: a `toOverwrite` that sets `filterLists` or `filters` locks those in Settings, which shows them as the policy sets them.
- **Not here:** Chrome Browser Cloud Management, enrolment tokens and Google-account policies, none of which ungoogled-chromium has.

`app/check/policies.sh` checks each: `URLBlocklist` set the system's way blocks a page, and Chromium's own names, set beside it, don't apply.
