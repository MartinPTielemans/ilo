#include "browser_core/select_choices.h"

#include <string>
#include <vector>

#include "base/values.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// A node of an accessibility tree as DevTools gives it.
base::Value Node(std::string id,
                 std::string parent,
                 std::string role,
                 std::string name,
                 std::vector<std::string> children = {}) {
  base::ListValue child_ids;
  for (std::string& child : children) {
    child_ids.Append(std::move(child));
  }
  base::DictValue node;
  node.Set("nodeId", std::move(id));
  if (!parent.empty()) {
    node.Set("parentId", std::move(parent));
  }
  node.Set("role", base::DictValue().Set("value", std::move(role)));
  node.Set("name", base::DictValue().Set("value", std::move(name)));
  node.Set("childIds", std::move(child_ids));
  return base::Value(std::move(node));
}

TEST(SelectChoicesTest, ASelectsOptionsComeInThePagesOrderGroupsTooWithoutWhichIsChosen) {
  base::ListValue nodes;
  nodes.Append(Node("1", "", "RootWebArea", "Form", {"2"}));
  nodes.Append(Node("2", "1", "combobox", "Answer", {"3"}));
  nodes.Append(Node("3", "2", "MenuListPopup", "", {"4", "5", "6"}));
  nodes.Append(Node("4", "3", "option", "  Yes  "));
  nodes.Append(Node("5", "3", "group", "Other", {"7", "8"}));
  nodes.Append(Node("6", "3", "option", "Maybe"));
  nodes.Append(Node("7", "5", "option", "No"));
  nodes.Append(Node("8", "5", "option", ""));
  // The option the user chose says so in its properties, which the choices never read.
  nodes[4].GetDict().Set(
      "properties",
      base::ListValue().Append(base::DictValue().Set("name", "selected").Set(
          "value", base::DictValue().Set("type", "boolean").Set("value", true))));
  const auto choices = SelectChoices(nodes);
  ASSERT_EQ(choices.size(), 1u);
  EXPECT_EQ(choices.at("2"), (std::vector<std::string>{"Yes", "No", "Maybe"}));
}

TEST(SelectChoicesTest, WhatsUnderAnotherFieldIsNeverAChoice) {
  base::ListValue nodes;
  nodes.Append(Node("1", "", "RootWebArea", "Form", {"2", "4"}));
  // A field's text, as Chromium puts what was typed or filled under it.
  nodes.Append(Node("2", "1", "textbox", "Email", {"3"}));
  nodes.Append(Node("3", "2", "StaticText", "ada@example.com"));
  // A list of the page's own, its options already in the outline.
  nodes.Append(Node("4", "1", "listbox", "Size", {"5"}));
  nodes.Append(Node("5", "4", "option", "M"));
  EXPECT_TRUE(SelectChoices(nodes).empty());
}

}  // namespace
}  // namespace browser_core
