#ifndef BROWSER_CORE_DIALOG_KEYS_H_
#define BROWSER_CORE_DIALOG_KEYS_H_

#include "build/build_config.h"
#include "ui/gfx/native_ui_types.h"

namespace content {
class WebContents;
}
namespace views {
class Widget;
}

// The keys typed in a tab's dialog of Chromium's, such as a site's sign-in or Chromium 157's
// warning before http, which holds the keyboard over its page: the app is asked about them as it is
// about a page's (#2416). In Chrome its shortcuts are the browser window's accelerators, which reach
// a tab's dialog too; ilo's are the app's, which only a page's keys reached.
namespace browser_core::dialog_keys {

// Asks the app about the keys of each tab dialog that browser window |window| shows, from now until
// the dialog goes.
void Watch(views::Widget* window);

// Gives the keyboard back to the tab dialog over |page| when the page takes it, as a click on the
// page beside the dialog gives it on the Mac: the page drops every key while its dialog shows, so
// ilo's shortcuts would be lost (#2625).
void PageFocused(content::WebContents* page);

#if !BUILDFLAG(IS_MAC)
// The same, for the page whose view holds |view|, or is held by it, which took aura's focus. A page
// whose tab ignores input, as it does under a dialog, doesn't tell its tab it was focused, and its
// view doesn't take the focus, so the platform layer, which hears aura's focus, says it (#2625).
void PageViewFocused(gfx::NativeView view);
#endif

}  // namespace browser_core::dialog_keys

#endif  // BROWSER_CORE_DIALOG_KEYS_H_
