#ifndef BROWSER_CORE_WEBRTC_H_
#define BROWSER_CORE_WEBRTC_H_

class Profile;

namespace browser_core {

// WebRTC gives a page no address its HTTP doesn't (decision 10). It gathers on the default route's
// public interface alone, and only through the proxy and TURN relays while the profile uses a proxy
// or the user has Relay Calls on (UserSettings().relay_calls). That's the profile's default for
// webrtc.ip_handling_policy, so a value a policy, an extension or the user set there wins. Until
// the settings are read and the profile's proxy is known, it stays ungoogled-chromium's default,
// relays only.

// Before the first profile is made.
void RegisterWebRtc();

// Follows |profile|'s proxy from now on, as the core uses it, for its default.
void FollowProxy(Profile* profile);

// Gives each profile followed its default, once the settings are read and as the user changes
// them.
void ApplyWebRtc();

}  // namespace browser_core

#endif  // BROWSER_CORE_WEBRTC_H_
