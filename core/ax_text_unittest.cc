#include "browser_core/ax_text.h"

#include <set>
#include <string>
#include <vector>

#include "base/strings/string_number_conversions.h"
#include "base/values.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_node_data.h"
#include "ui/accessibility/ax_tree_update.h"

namespace browser_core {
namespace {

constexpr char kPage[] = "https://fieldnotes.example/";

// A node of DevTools' accessibility tree, as Accessibility.getFullAXTree gives one.
base::DictValue Node(int id, const std::string& role, std::vector<int> children) {
  base::ListValue child_ids;
  for (int child : children) {
    child_ids.Append(base::NumberToString(child));
  }
  return base::DictValue()
      .Set("nodeId", base::NumberToString(id))
      .Set("ignored", false)
      .Set("role", base::DictValue().Set("type", "role").Set("value", role))
      .Set("childIds", std::move(child_ids));
}

base::DictValue Element(int id, int backend, const std::string& role, std::vector<int> children) {
  return Node(id, role, std::move(children)).Set("backendDOMNodeId", backend);
}

base::DictValue Text(int id, const std::string& words) {
  return Node(id, "StaticText", {})
      .Set("name", base::DictValue().Set("type", "computedString").Set("value", words));
}

base::DictValue Field(int id, const std::string& value, std::vector<int> children) {
  return Node(id, "textbox", std::move(children))
      .Set("value", base::DictValue().Set("type", "string").Set("value", value));
}

base::DictValue WithProperty(base::DictValue node, const std::string& name, base::Value value) {
  base::ListValue* properties = node.EnsureList("properties");
  properties->Append(base::DictValue()
                         .Set("name", name)
                         .Set("value", base::DictValue().Set("value", std::move(value))));
  return node;
}

// A sign-in section the user pointed at, an email and a password filled in it as autofill leaves
// them, and a section with no field.
base::ListValue SignIn() {
  return base::ListValue()
      .Append(Element(1, 1, "RootWebArea", {2, 8}))
      .Append(Element(2, 42, "section", {3, 4, 6, 7}))
      .Append(Text(3, "Sign in"))
      // Chromium puts what's typed in a field in its subtree too.
      .Append(Field(4, "ada@example.com", {5}))
      .Append(Text(5, "ada@example.com"))
      .Append(Field(6, "•••••••", {}))
      .Append(Text(7, "Forgot your password?"))
      .Append(Element(8, 43, "section", {9}))
      .Append(Text(9, "Trending"));
}

TEST(AxTextTest, AnElementsTextHasNoFieldsValueAndSaysItHeldOne) {
  const ElementText section = TextUnder(SignIn(), 42);
  EXPECT_EQ(section.text, "Sign in Forgot your password?");
  EXPECT_EQ(section.text.find("ada@example.com"), std::string::npos);
  EXPECT_EQ(section.text.find("•"), std::string::npos);
  EXPECT_TRUE(section.withheld);

  const ElementText plain = TextUnder(SignIn(), 43);
  EXPECT_EQ(plain.text, "Trending");
  EXPECT_FALSE(plain.withheld);

  // An element the tree doesn't hold gives nothing.
  const ElementText gone = TextUnder(SignIn(), 99);
  EXPECT_TRUE(gone.text.empty());
  EXPECT_FALSE(gone.withheld);
}

TEST(AxTextTest, AnEmptyFieldWithholdsNothing) {
  const base::ListValue nodes = base::ListValue()
                                    .Append(Element(1, 42, "form", {2, 3}))
                                    .Append(Text(2, "Search"))
                                    .Append(Field(3, "", {}));
  const ElementText text = TextUnder(nodes, 42);
  EXPECT_EQ(text.text, "Search");
  EXPECT_FALSE(text.withheld);
}

TEST(AxTextTest, AnEditableRegionIsAField) {
  // A mail's body: no field's role, but editable, so what's written in it is the user's.
  base::DictValue body = WithProperty(Node(3, "generic", {4}), "editable", base::Value("richtext"));
  body.Set("value", base::DictValue().Set("type", "string").Set("value", "Dear Ada"));
  const base::ListValue nodes = base::ListValue()
                                    .Append(Element(1, 42, "main", {2, 3}))
                                    .Append(Text(2, "New message"))
                                    .Append(std::move(body))
                                    .Append(Text(4, "Dear Ada"));
  const ElementText text = TextUnder(nodes, 42);
  EXPECT_EQ(text.text, "New message");
  EXPECT_TRUE(text.withheld);
}

TEST(AxTextTest, BlocksStandOnLinesOfTheirOwn) {
  const base::ListValue nodes = base::ListValue()
                                    .Append(Element(1, 42, "article", {2, 4}))
                                    .Append(Node(2, "heading", {3}))
                                    .Append(Text(3, "Field notes"))
                                    .Append(Node(4, "paragraph", {5, 6}))
                                    .Append(Text(5, "Birds at"))
                                    .Append(Text(6, "dawn"));
  EXPECT_EQ(TextUnder(nodes, 42).text, "Field notes\nBirds at dawn");
}

TEST(AxTextTest, NothingOfWhatTheUserCantSeeOrAnExtensionMade) {
  base::DictValue hidden = Node(3, "generic", {4});
  hidden.Set("ignored", true);
  hidden.Set("ignoredReasons",
             base::ListValue().Append(base::DictValue().Set("name", "notRendered")));
  const base::ListValue nodes =
      base::ListValue()
          .Append(Element(1, 42, "section", {2, 3, 5, 7}))
          .Append(Text(2, "Shown"))
          .Append(std::move(hidden))
          .Append(Text(4, "Hidden"))
          .Append(WithProperty(Node(5, "generic", {6}), "fromExtension", base::Value(true)))
          .Append(Text(6, "An extension's words"))
          // A frame's document is another tree, which this one's doesn't hold.
          .Append(Node(7, "Iframe", {}));
  const ElementText text = TextUnder(nodes, 42);
  EXPECT_EQ(text.text, "Shown");
  EXPECT_FALSE(text.withheld);
}

TEST(AxTextTest, ThePagesTextLeavesOutAFrameAnAgentMayNotRead) {
  ui::AXTreeUpdate update;
  update.root_id = 1;
  ui::AXNodeData page;
  page.id = 1;
  page.role = ax::mojom::Role::kRootWebArea;
  page.AddStringAttribute(ax::mojom::StringAttribute::kUrl, kPage);
  page.child_ids = {2};
  ui::AXNodeData frame;
  frame.id = 2;
  frame.role = ax::mojom::Role::kRootWebArea;
  frame.AddStringAttribute(ax::mojom::StringAttribute::kUrl, "chrome-extension://abc/panel.html");
  frame.child_ids = {3};
  ui::AXNodeData words;
  words.id = 3;
  words.role = ax::mojom::Role::kStaticText;
  words.AddStringAttribute(ax::mojom::StringAttribute::kName, "An extension's words");
  update.nodes = {page, frame, words};
  EXPECT_EQ(TextOf(update, {kPage}), "");
}

}  // namespace
}  // namespace browser_core
