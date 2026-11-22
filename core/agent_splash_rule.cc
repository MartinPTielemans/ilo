#include "browser_core/agent_splash_rule.h"

#include <optional>
#include <string>
#include <string_view>

namespace browser_core {
namespace {

// Whether |node| has the boolean property |name|, set.
bool Has(const base::DictValue& node, std::string_view name) {
  const base::ListValue* properties = node.FindList("properties");
  if (!properties) {
    return false;
  }
  for (const base::Value& property : *properties) {
    const base::DictValue* dict = property.GetIfDict();
    const std::string* each = dict ? dict->FindString("name") : nullptr;
    if (each && *each == name) {
      return dict->FindBoolByDottedPath("value.value").value_or(false);
    }
  }
  return false;
}

}  // namespace

bool IsSplash(const std::vector<base::ListValue>& trees) {
  bool loading = false;
  for (const base::ListValue& tree : trees) {
    for (const base::Value& node : tree) {
      const base::DictValue* dict = node.GetIfDict();
      if (!dict || dict->FindBool("ignored").value_or(false)) {
        continue;
      }
      const std::string* role = dict->FindStringByDottedPath("role.value");
      const std::string_view kind = role ? std::string_view(*role) : std::string_view();
      // Each document takes focus, the page's and each frame's, and holds the rest.
      if (Has(*dict, "focusable") && kind != "RootWebArea" && kind != "Iframe") {
        return false;
      }
      loading |= Has(*dict, "busy") || (kind == "progressbar" && !dict->Find("value"));
    }
  }
  return loading;
}

}  // namespace browser_core
