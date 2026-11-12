#include "browser_core/select_choices.h"

#include <utility>

#include "base/containers/adapters.h"
#include "base/strings/string_util.h"

namespace browser_core {

namespace {

bool IsRole(const base::DictValue& node, std::string_view role) {
  const std::string* value = node.FindStringByDottedPath("role.value");
  return value && *value == role;
}

}  // namespace

std::map<std::string, std::vector<std::string>> SelectChoices(const base::ListValue& nodes) {
  std::map<std::string, const base::DictValue*> by_id;
  for (const base::Value& node : nodes) {
    const base::DictValue* dict = node.GetIfDict();
    const std::string* id = dict ? dict->FindString("nodeId") : nullptr;
    if (id) {
      by_id[*id] = dict;
    }
  }
  std::map<std::string, std::vector<std::string>> choices;
  for (const base::Value& node : nodes) {
    // Chromium gives a select's options a popup of their own, which no other field has.
    const base::DictValue* popup = node.GetIfDict();
    const std::string* select = popup ? popup->FindString("parentId") : nullptr;
    if (!select || !IsRole(*popup, "MenuListPopup")) {
      continue;
    }
    std::vector<std::string>& names = choices[*select];
    std::vector<const base::DictValue*> steps = {popup};
    while (!steps.empty()) {
      const base::DictValue* each = steps.back();
      steps.pop_back();
      if (IsRole(*each, "option")) {
        const std::string* name = each->FindStringByDottedPath("name.value");
        std::string shown =
            name ? base::CollapseWhitespaceASCII(*name, /*trim_sequences_with_line_breaks=*/true)
                 : std::string();
        if (!shown.empty()) {
          names.push_back(std::move(shown));
        }
        continue;
      }
      if (const base::ListValue* children = each->FindList("childIds")) {
        for (const base::Value& child : base::Reversed(*children)) {
          const std::string* id = child.GetIfString();
          const auto found = id ? by_id.find(*id) : by_id.end();
          if (found != by_id.end()) {
            steps.push_back(found->second);
          }
        }
      }
    }
  }
  return choices;
}

}  // namespace browser_core
