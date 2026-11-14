#ifndef BROWSER_CORE_DIALOG_KEYS_H_
#define BROWSER_CORE_DIALOG_KEYS_H_

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

}  // namespace browser_core::dialog_keys

#endif  // BROWSER_CORE_DIALOG_KEYS_H_
