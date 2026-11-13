// How an agent's typing goes to a page as DevTools key events (agent_page.cc), apart so
// browser_core_unittests tests it alone.

#ifndef BROWSER_CORE_TYPING_KEYS_H_
#define BROWSER_CORE_TYPING_KEYS_H_

#include <string>
#include <vector>

#include "base/values.h"

namespace browser_core {

// The DevTools key events that type |text|, as a keyboard types each character, its keys down
// and up, so a field's own key handlers filter and format it as they do a user's typing. A line
// break isn't Enter, which would send a chat's message or submit a form: it goes in as text with
// the character next to it, as insertText put it, in a keypress alone. Blink inserts no control
// character a key event has alone, and an event holds 3 UTF-16 units at most.
std::vector<base::DictValue> TypingKeys(const std::u16string& text);

}  // namespace browser_core

#endif  // BROWSER_CORE_TYPING_KEYS_H_
