#include "browser_core/ax_text.h"

#include <set>
#include <string>
#include <vector>

#include "testing/gtest/include/gtest/gtest.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_node_data.h"
#include "ui/accessibility/ax_tree_update.h"

namespace browser_core {
namespace {

constexpr char kPage[] = "https://fieldnotes.example/";

ui::AXNodeData Node(ui::AXNodeID id, ax::mojom::Role role, std::vector<ui::AXNodeID> children) {
  ui::AXNodeData node;
  node.id = id;
  node.role = role;
  node.child_ids = std::move(children);
  return node;
}

ui::AXNodeData Text(ui::AXNodeID id, const std::string& words) {
  ui::AXNodeData node = Node(id, ax::mojom::Role::kStaticText, {});
  node.AddStringAttribute(ax::mojom::StringAttribute::kName, words);
  return node;
}

ui::AXNodeData Document(ui::AXNodeID id,
                        const std::string& url,
                        std::vector<ui::AXNodeID> children) {
  ui::AXNodeData node = Node(id, ax::mojom::Role::kRootWebArea, std::move(children));
  node.AddStringAttribute(ax::mojom::StringAttribute::kUrl, url);
  return node;
}

ui::AXNodeData Field(ui::AXNodeID id,
                     const std::string& value,
                     std::vector<ui::AXNodeID> children) {
  ui::AXNodeData node = Node(id, ax::mojom::Role::kTextField, std::move(children));
  node.AddStringAttribute(ax::mojom::StringAttribute::kValue, value);
  return node;
}

// A page with a sign-in section the user pointed at, an email and a password filled in it, as
// autofill would leave them, and a section with no field.
ui::AXTreeUpdate SignIn() {
  ui::AXTreeUpdate update;
  update.root_id = 1;
  ui::AXNodeData section = Node(2, ax::mojom::Role::kSection, {3, 4, 6, 7});
  section.AddIntAttribute(ax::mojom::IntAttribute::kDOMNodeId, 42);
  ui::AXNodeData password = Field(6, "hunter2", {});
  password.AddState(ax::mojom::State::kProtected);
  ui::AXNodeData plain = Node(8, ax::mojom::Role::kSection, {9});
  plain.AddIntAttribute(ax::mojom::IntAttribute::kDOMNodeId, 43);
  update.nodes = {
      Document(1, kPage, {2, 8}),
      section,
      Text(3, "Sign in"),
      // Chromium puts what's typed in a field in its subtree too.
      Field(4, "ada@example.com", {5}),
      Text(5, "ada@example.com"),
      password,
      Text(7, "Forgot your password?"),
      plain,
      Text(9, "Trending"),
  };
  return update;
}

TEST(AxTextTest, AnElementsTextHasNoFieldsValueAndSaysItHeldOne) {
  const std::set<std::string, std::less<>> readable = {kPage};
  const ElementText section = TextUnder(SignIn(), readable, 42);
  EXPECT_EQ(section.text, "Sign in Forgot your password?");
  EXPECT_EQ(section.text.find("ada@example.com"), std::string::npos);
  EXPECT_EQ(section.text.find("hunter2"), std::string::npos);
  EXPECT_TRUE(section.withheld);

  const ElementText plain = TextUnder(SignIn(), readable, 43);
  EXPECT_EQ(plain.text, "Trending");
  EXPECT_FALSE(plain.withheld);

  // An element the snapshot doesn't hold gives nothing.
  const ElementText gone = TextUnder(SignIn(), readable, 99);
  EXPECT_TRUE(gone.text.empty());
  EXPECT_FALSE(gone.withheld);
}

TEST(AxTextTest, AnEmptyFieldWithholdsNothing) {
  ui::AXTreeUpdate update;
  update.root_id = 1;
  ui::AXNodeData form = Node(2, ax::mojom::Role::kForm, {3, 4});
  form.AddIntAttribute(ax::mojom::IntAttribute::kDOMNodeId, 42);
  update.nodes = {Document(1, kPage, {2}), form, Text(3, "Search"), Field(4, "", {})};
  const ElementText text = TextUnder(update, {kPage}, 42);
  EXPECT_EQ(text.text, "Search");
  EXPECT_FALSE(text.withheld);
}

TEST(AxTextTest, NothingOfAnElementInAFrameAnAgentMayNotRead) {
  ui::AXTreeUpdate update;
  update.root_id = 1;
  ui::AXNodeData inside = Node(4, ax::mojom::Role::kGenericContainer, {5});
  inside.AddIntAttribute(ax::mojom::IntAttribute::kDOMNodeId, 77);
  ui::AXNodeData frame = Node(2, ax::mojom::Role::kIframe, {3});
  frame.AddIntAttribute(ax::mojom::IntAttribute::kDOMNodeId, 76);
  update.nodes = {
      Document(1, kPage, {2}),
      frame,
      Document(3, "chrome-extension://abc/panel.html", {4}),
      inside,
      Text(5, "An extension's words"),
  };
  const std::set<std::string, std::less<>> readable = {kPage};

  const ElementText text = TextUnder(update, readable, 77);
  EXPECT_TRUE(text.text.empty());
  EXPECT_TRUE(text.withheld);

  // The frame's own element, pointed at whole, goes without the frame's document.
  const ElementText around = TextUnder(update, readable, 76);
  EXPECT_TRUE(around.text.empty());
  EXPECT_TRUE(around.withheld);

  // The page's text leaves it out too, as it did.
  EXPECT_EQ(TextOf(update, readable), "");
}

}  // namespace
}  // namespace browser_core
