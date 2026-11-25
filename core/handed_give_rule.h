// Which of the user's tabs may be handed to an agent (bc_tab_give_to_agent), apart so
// browser_core_unittests tests the rule alone. The app asks the same of a tab before it gives it,
// and this is the answer that holds whoever asks.

#ifndef BROWSER_CORE_HANDED_GIVE_RULE_H_
#define BROWSER_CORE_HANDED_GIVE_RULE_H_

namespace browser_core::handed_give_rule {

// A tab as the give finds it. A tab in a split view is given and leaves the split, a pinned tab is
// given and its pin is kept for its way back (handed.h), one with a Picture in Picture window is
// given and the window closes (bc_tab_give_to_agent), and one whose page is being told the location
// is given as it is. None of those is asked here.
struct Tab {
  // In a Space of the user's that the Spaces list has: not an agent's, and not a window's own, as a
  // private window's is.
  bool users = false;
  // The Space it's given to is an agent's, in the tab's own profile, as the page goes as it is.
  bool to_agent = false;
  bool same_profile = false;
  // In a Live Folder, or in a folder inside one: such a folder fills itself and replaces its tabs,
  // so the tab's place wouldn't be the user's to come back to.
  bool live_folder = false;
  // At an http or https page in Chromium's engine, the only pages an agent may act in.
  bool web_page = false;
  // Its page is using the camera or the microphone: a call isn't handed over by a slip.
  bool camera = false;
  bool microphone = false;
  // Its page is sharing the screen, a window or another tab: what's on the user's screen isn't an
  // agent's to send on. Once it stops sharing it may be given.
  bool screen = false;
  // Its page asks the user for a permission, shown or kept for when the tab is: an allow given
  // after the give would start the camera or the microphone in an agent's tab.
  bool permission_asked = false;
  // One of its page's dialogs is up, such as an alert or the question whether to leave it: none is
  // drawn over an agent's pages.
  bool dialog = false;
  // The profile has a saved login for a sign-in form on its page, which Chrome may have filled in
  // as it loaded: an agent would sign in with it by pressing the form's button.
  bool saved_login = false;
  // Its page has a USB, HID, serial or Bluetooth device open, or scans for Bluetooth ones: an agent
  // would act on the device through the page.
  bool device = false;
  // Its page holds a file or a folder of the user's.
  bool files = false;
};

// Whether |tab| is given. Each of them alone refuses it.
bool MayGive(const Tab& tab);

}  // namespace browser_core::handed_give_rule

#endif  // BROWSER_CORE_HANDED_GIVE_RULE_H_
