// What a page in an agent's Space is refused, apart so browser_core_unittests tests the rule alone.
// The answer is where the page is and nothing else: no page and no agent's call says which.

#ifndef BROWSER_CORE_AGENT_SPACE_RULE_H_
#define BROWSER_CORE_AGENT_SPACE_RULE_H_

namespace browser_core::agent_space_rule {

// The tab a page is in when it asks.
struct Tab {
  // In an agent's Space: a page of the agent's own, or a tab of the user's handed to it.
  bool agents = false;
  // The user holds it from the agent, as at a handoff or once they take it over.
  bool held = false;
};

// What a page asks of the browser that is the user's to give.
enum class Ask {
  // The clipboard's content, by a paste.
  kPaste,
  // The picker for a file or a folder of the user's.
  kFilePicker,
  // A USB, HID, serial or Bluetooth device the user gave the page's site.
  kDevice,
  // A link of the page's opened in a preview over the user's window: by a click that leaves a
  // pinned tab's site, a click with ⇧, or the row of the page's menu.
  kLinkPreview,
};

// Whether the browser refuses |ask| from a page in |tab|. Nothing in a tab of the user's. In an
// agent's Space the picker and a device always, and a paste and a link preview unless the user
// holds the tab, so their own paste and their own click work at a step they do themselves.
bool Refuses(Ask ask, const Tab& tab);

// Whether the page's menu has its row that opens a link in a preview, for a link that could open
// in one (|web_link|): never where the click itself would open none.
bool MenuHasLinkPreviewRow(bool web_link, const Tab& tab);

}  // namespace browser_core::agent_space_rule

#endif  // BROWSER_CORE_AGENT_SPACE_RULE_H_
