// When a tab's dialog takes the keyboard back from the page under it, apart so
// browser_core_unittests tests the rule alone (#3212).

#ifndef BROWSER_CORE_DIALOG_TAKE_BACK_RULE_H_
#define BROWSER_CORE_DIALOG_TAKE_BACK_RULE_H_

#include <cstdint>

namespace browser_core::dialog_take_back_rule {

// Counts the pages that took the keyboard, in any window. A tab's dialog takes it back from its
// page a turn after the page took it, and only if no page has taken it since. On the Mac a press
// in the pane beside the dialog makes the window key, which focuses the window's shown page, the
// dialog's, before the press focuses the page pressed: the keyboard stays lent to that one, as
// Windows leaves it with the pane that still has it on the next turn (#3443). The dialog's page
// pressed or shown again takes it afresh, and its dialog then takes it back.
class Turns {
 public:
  // A page took the keyboard: its turn.
  uint64_t Took() { return ++last_; }
  // Whether the page that took it at |turn| still has it.
  bool Holds(uint64_t turn) const { return turn == last_; }

 private:
  uint64_t last_ = 0;
};

}  // namespace browser_core::dialog_take_back_rule

#endif  // BROWSER_CORE_DIALOG_TAKE_BACK_RULE_H_
