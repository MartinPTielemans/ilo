// How an agent's typing goes to a page as DevTools key events (agent_page.cc), apart so
// browser_core_unittests tests it alone.

#ifndef BROWSER_CORE_TYPING_KEYS_H_
#define BROWSER_CORE_TYPING_KEYS_H_

#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/values.h"

namespace browser_core {

// The DevTools key events that type |text|, as a keyboard types each character, its keys down
// and up, so a field's own key handlers filter and format it as they do a user's typing. A line
// break isn't Enter, which would send a chat's message or submit a form: it goes in as text with
// the character next to it, as insertText put it, in a keypress alone. Blink inserts no control
// character a key event has alone, and an event holds 3 UTF-16 units at most.
std::vector<base::DictValue> TypingKeys(const std::u16string& text);

// Whether |text| is in the HTML value form of a date, time, datetime-local, month or week input,
// such as 2026-11-25 or 14:30, the only text whose field's segments are read (DateFieldKeys).
bool IsDateValueForm(std::string_view text);

// The keys that fill each segment of a date or time input of |type|, in the order of |segments|,
// the pseudo names Blink gives the parts it shows, as -webkit-datetime-edit-month-field. Blink's
// parts take digits in the locale's order, so 2026-11-25 typed as keys gives a wrong date
// (#4281): each part of |text| goes into its own segment instead, as digits padded to the part's
// width, and AM or PM as Backspace, which empties the segment, then ArrowUp once for AM or twice
// for PM, as the segment's labels are the locale's. Nothing when |text| isn't exactly |type|'s
// value form, or |segments| aren't exactly its parts, as one with a seconds segment for a value
// without seconds: the text is then typed as keys, as into any field.
std::optional<std::vector<std::vector<base::DictValue>>> DateFieldKeys(
    std::string_view type,
    std::string_view text,
    const std::vector<std::string>& segments);

}  // namespace browser_core

#endif  // BROWSER_CORE_TYPING_KEYS_H_
