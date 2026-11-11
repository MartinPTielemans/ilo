#ifndef BROWSER_CORE_AX_TEXT_H_
#define BROWSER_CORE_AX_TEXT_H_

#include <functional>
#include <set>
#include <string>

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

// The text of the element whose DOM node id, DevTools' backend node id, is |node| in |update|.
// Empty, and withheld, for one inside a frame an agent may not read.
ElementText TextUnder(const ui::AXTreeUpdate& update,
                      const std::set<std::string, std::less<>>& readable,
                      int node);

}  // namespace browser_core

#endif  // BROWSER_CORE_AX_TEXT_H_
