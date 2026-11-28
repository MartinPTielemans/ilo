#ifndef BROWSER_CORE_PAGE_IN_FIELD_RULE_H_
#define BROWSER_CORE_PAGE_IN_FIELD_RULE_H_

#include <optional>

namespace browser_core {

// Whether an agent's tab's page has the user's keyboard in a field, which the app parks the
// agent's next act on that tab for (#4462). Said only as it changes, and false as the tab closes or
// stops being an agent's. Apart from content so browser_core_unittests tests it alone.
class PageInField {
 public:
  struct Now {
    // The tab is in an agent's Space: a user's tab never says anything.
    bool agents = false;
    // The page's focused element takes text, as a text field does.
    bool editable = false;
    // The page, not the app's UI, has its window's keyboard.
    bool focused = false;
    // Its window is the key window.
    bool key = false;
  };

  // What the app is told for |now|, or nothing when it's what it was told last.
  std::optional<bool> Update(const Now& now);
  // The tab is closing: false if the app was last told true.
  std::optional<bool> Closed();

 private:
  bool said_ = false;
};

// The tabs' Space hook (tab.cc), called after every Space change as PageLogsSpacesChanged is: a tab
// kept from an agent's Space lets its field go.
void PageInFieldsSpacesChanged();

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_IN_FIELD_RULE_H_
