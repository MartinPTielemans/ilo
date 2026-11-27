// Reads what a held tab's page has that's the user's to do, a password, one-time-code or CAPTCHA
// step, and tells the app (held_page), for it to end an agent's hand-off once the page has moved
// past the sign-in (#4361). Only from what the browser reads of a page already: the forms the
// password manager sees rendered as it loads, those autofill parsed, and the names in its
// accessibility tree. No script and no timer: each report follows a load or a form's parse.

#ifndef BROWSER_CORE_HELD_PAGE_H_
#define BROWSER_CORE_HELD_PAGE_H_

#include <vector>

#include "browser_core/public/browser_core.h"

namespace autofill {
class FormData;
}  // namespace autofill

namespace content {
class RenderFrameHost;
}  // namespace content

namespace browser_core::held_page {

// The core holds |tab| from now: reads each page it commits after this one, until Unwatch. A tab
// watched already keeps the pages it counted.
void Watch(bc_id tab);

// The core holds |tab| no more.
void Unwatch(bc_id tab);

// g_browser_core_password_forms_rendered: |frame| loaded, with |forms| rendered.
void FormsRendered(content::RenderFrameHost* frame, const std::vector<autofill::FormData>& forms);

}  // namespace browser_core::held_page

#endif  // BROWSER_CORE_HELD_PAGE_H_
