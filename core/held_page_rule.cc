#include "browser_core/held_page_rule.h"

#include <algorithm>
#include <string>

#include "base/strings/string_util.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_node_data.h"
#include "ui/accessibility/ax_tree_update.h"

namespace browser_core::held_page_rule {

Facts FactsOf(const std::vector<Field>& fields) {
  return {
      .password = std::ranges::any_of(
          fields, [](const Field& field) { return field.password && field.focusable; }),
      .one_time_code = std::ranges::any_of(fields, &Field::one_time_code),
  };
}

bool HoldsWord(std::string_view name, std::string_view words) {
  const std::string lower = base::ToLowerASCII(name);
  while (!words.empty()) {
    const size_t end = std::min(words.find('\n'), words.size());
    const std::string_view word = words.substr(0, end);
    if (!word.empty() && lower.find(word) != std::string::npos) {
      return true;
    }
    words.remove_prefix(std::min(end + 1, words.size()));
  }
  return false;
}

bool CaptchaIn(const ui::AXTreeUpdate& update, std::string_view words) {
  return std::ranges::any_of(update.nodes, [&](const ui::AXNodeData& node) {
    // A text node's name is the text itself, and so is that of an element named from its contents.
    if (node.IsIgnored() || node.HasState(ax::mojom::State::kInvisible) ||
        node.role == ax::mojom::Role::kStaticText || node.role == ax::mojom::Role::kInlineTextBox ||
        node.GetNameFrom() == ax::mojom::NameFrom::kContents) {
      return false;
    }
    return HoldsWord(node.GetStringAttribute(ax::mojom::StringAttribute::kName), words);
  });
}

bool Add(Facts& known, const Facts& more) {
  const Facts before = known;
  known.password |= more.password;
  known.one_time_code |= more.one_time_code;
  known.captcha |= more.captcha;
  return known != before;
}

}  // namespace browser_core::held_page_rule
