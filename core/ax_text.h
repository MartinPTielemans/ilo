#ifndef BROWSER_CORE_AX_TEXT_H_
#define BROWSER_CORE_AX_TEXT_H_

#include <functional>
#include <optional>
#include <set>
#include <string>
#include <string_view>

#include "base/values.h"

namespace ui {
struct AXTreeUpdate;
}  // namespace ui

namespace browser_core {

// The page's text from |update|, a line for each block, from the documents whose URL is in
// |readable| alone: those of the frames an agent may read (IsWebFrame). The snapshot names no frame
// but by its document's URL.
std::string TextOf(const ui::AXTreeUpdate& update,
                   const std::set<std::string, std::less<>>& readable);

// An element's text as TextOf reads a page's, and whether it held what that leaves out: a field
// holding a value, or a frame an agent may not read.
struct ElementText {
  std::string text;
  bool withheld = false;
};

// The text of the element whose backend node id is |node|, as TextOf reads a page's, from |nodes|:
// DevTools' accessibility tree of the frame it's in (Accessibility.getFullAXTree), the one the
// agent's outline reads. A frame inside it is another tree, so none of its text goes. Chromium
// 157's snapshot numbers its nodes anew, so it can't find an element by its node id.
ElementText TextUnder(const base::ListValue& nodes, int node);

// Of a node of DevTools' accessibility tree, as the outline and TextUnder read one:

// Whether the node is a field, whose value is the user's: a text field of any kind, a select, a
// date, time or number input, or an editable region such as a mail's body.
bool IsField(const base::DictValue& node);

// Whether the node has the boolean |property|, such as "selected", and it's true.
bool Has(const base::DictValue& node, std::string_view property);

// What a node of the tree holds, as text: a field's value, or a number field's number.
std::optional<std::string> ValueOf(const base::DictValue& node);

}  // namespace browser_core

#endif  // BROWSER_CORE_AX_TEXT_H_
