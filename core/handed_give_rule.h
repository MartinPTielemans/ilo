// Which of the user's tabs may be handed to an agent (bc_tab_give_to_agent), apart so
// browser_core_unittests tests the rule alone. The app asks the same of a tab before it gives it,
// and this is the answer that holds whoever asks.

#ifndef BROWSER_CORE_HANDED_GIVE_RULE_H_
#define BROWSER_CORE_HANDED_GIVE_RULE_H_

namespace browser_core::handed_give_rule {

// A tab as the give finds it.
struct Tab {
  // In a Space of the user's that the Spaces list has: not an agent's, and not a window's own, as a
  // private window's is.
  bool users = false;
  // The Space it's given to is an agent's, in the tab's own profile, as the page goes as it is.
  bool to_agent = false;
  bool same_profile = false;
  // A pinned tab is kept where it's pinned, and an agent may navigate its pages away.
  bool pinned = false;
  // Shown with others in a split view.
  bool split = false;
  // At an http or https page in Chromium's engine, the only pages an agent may act in.
  bool web_page = false;
  // Its page is using the camera, the microphone, or a screen's, a window's or a tab's picture: a
  // call isn't handed over by a slip.
  bool camera = false;
  bool microphone = false;
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
  // Its page has a USB, HID, serial or Bluetooth device open, scans for Bluetooth ones, or is being
  // told where the user is: what a page has running stays the user's.
  bool device_or_location = false;
  // Its page holds a file or a folder of the user's.
  bool files = false;
  // Its page has a Picture in Picture window, Chrome's floating one for a video or one for a
  // document: no page in an agent's Space has one (agent_space_rule.h), and closing the user's is
  // theirs to do. A video shown in a media card isn't one: the give ends that, and it plays on in
  // its page.
  bool picture_in_picture = false;
};

// Whether |tab| is given. Each of them alone refuses it.
bool MayGive(const Tab& tab);

}  // namespace browser_core::handed_give_rule

#endif  // BROWSER_CORE_HANDED_GIVE_RULE_H_
