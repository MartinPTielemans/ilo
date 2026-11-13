#ifndef BROWSER_CORE_DIALOG_KEYS_H_
#define BROWSER_CORE_DIALOG_KEYS_H_

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

}  // namespace browser_core::dialog_keys

#endif  // BROWSER_CORE_DIALOG_KEYS_H_
