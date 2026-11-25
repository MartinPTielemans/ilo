// A profile's Always use secure connections, and Chrome's warning before http kept out of an
// agent's tab. The answers are secure_connections_rule.h's for the tab the page is in
// (agent_space_page.h): nothing a page or an agent's call says changes one.

#ifndef BROWSER_CORE_SECURE_CONNECTIONS_H_
#define BROWSER_CORE_SECURE_CONNECTIONS_H_

#include "browser_core/public/browser_core.h"

class GURL;
class Profile;

namespace content {
class WebContents;
}  // namespace content

namespace browser_core {

// bc_profile_secure_connections and bc_profile_set_secure_connections, for a profile that is
// loaded.
bool SecureConnections(Profile* profile, bool* on);
bool SetSecureConnections(Profile* profile, bool on);

// g_browser_core_no_http_warning: whether Chrome shows no warning for |url|, which answered only
// over http and was stopped in |contents|. In an agent's tab it shows none: the core puts the tab
// back, to the page it had or closed if it had none, after one activity entry that names |url|'s
// host, both after this call, since a page can't be left from inside its own load's end and the
// app takes no entry inside a call of its own into the core.
bool TakesHttpWarning(content::WebContents* contents, const GURL& url);

// Whether Chrome's warning before http is up in |tab|, unanswered: only ever in a tab that is the
// user's to answer in.
bool HasHttpWarning(bc_id tab);

// The user handed |tab| back while Chrome's warning was up in it, unanswered: the warning closes
// and the tab is put back, as if the page had been stopped in the agent's tab it is again.
void LeaveNoHttpWarning(bc_id tab);

}  // namespace browser_core

#endif  // BROWSER_CORE_SECURE_CONNECTIONS_H_
