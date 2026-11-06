#ifndef BROWSER_CORE_AUTOFILL_MENU_H_
#define BROWSER_CORE_AUTOFILL_MENU_H_

#include "browser_core/public/browser_core.h"

namespace content {
class WebContents;
}

namespace browser_core {

// Edit › AutoFill: Chrome's dropdown at the field of a tab's page that has the keyboard. Chrome's
// autofill keeps no focused field the browser can ask for, so each tab notes the field its page's
// focus last went to, as autofill hears of it from the renderer.
class AutofillMenu {
 public:
  // Starts noting where |contents|' page's focus goes. The tab calls it as it takes its page.
  static void Watch(content::WebContents* contents);
  // See bc_tab_show_autofill.
  static bool Show(bc_id tab, bool logins);
};

}  // namespace browser_core

#endif  // BROWSER_CORE_AUTOFILL_MENU_H_
