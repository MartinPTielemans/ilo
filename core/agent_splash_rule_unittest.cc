#include "browser_core/agent_splash_rule.h"

#include <string>
#include <utility>
#include <vector>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

base::DictValue Flag(const std::string& name) {
  return base::DictValue().Set("name", name).Set(
      "value", base::DictValue().Set("type", "booleanOrUndefined").Set("value", true));
}

// A node of a DevTools tree with |role| and, set, the boolean properties |flags|.
base::DictValue Node(const std::string& role, std::vector<std::string> flags = {}) {
  base::ListValue properties;
  for (const std::string& flag : flags) {
    properties.Append(Flag(flag));
  }
  return base::DictValue()
      .Set("role", base::DictValue().Set("type", "role").Set("value", role))
      .Set("properties", std::move(properties));
}

// A document's tree: the document, which takes focus, then |nodes|.
template <typename... Nodes>
base::ListValue Tree(Nodes... nodes) {
  base::ListValue tree;
  tree.Append(Node("RootWebArea", {"focusable"}));
  (tree.Append(std::move(nodes)), ...);
  return tree;
}

std::vector<base::ListValue> Trees(base::ListValue main, base::ListValue frame) {
  std::vector<base::ListValue> trees;
  trees.push_back(std::move(main));
  trees.push_back(std::move(frame));
  return trees;
}

std::vector<base::ListValue> Page(base::ListValue main) {
  std::vector<base::ListValue> trees;
  trees.push_back(std::move(main));
  return trees;
}

TEST(AgentSplashRuleTest, AProgressBarWithNothingToActOnIsASplash) {
  EXPECT_TRUE(IsSplash(Page(Tree(Node("StaticText"), Node("progressbar")))));
}

TEST(AgentSplashRuleTest, ANodeMarkedBusyIsASplash) {
  EXPECT_TRUE(IsSplash(Page(Tree(Node("generic", {"busy"}), Node("StaticText")))));
}

// The app is a frame of the page, which shows the splash; the page's own document holds the frame.
TEST(AgentSplashRuleTest, AProgressBarInAFrameIsASplash) {
  EXPECT_TRUE(IsSplash(Trees(Tree(Node("StaticText"), Node("Iframe", {"focusable"})),
                             Tree(Node("progressbar")))));
}

TEST(AgentSplashRuleTest, SomethingToActOnMeansTheContentIsThere) {
  EXPECT_FALSE(IsSplash(Page(Tree(Node("progressbar"), Node("button", {"focusable"})))));
}

// A page whose list is still loading below its menu isn't held for it.
TEST(AgentSplashRuleTest, ALinkInAFrameMeansTheContentIsThere) {
  EXPECT_FALSE(IsSplash(Trees(Tree(Node("Iframe", {"focusable"})),
                              Tree(Node("progressbar"), Node("link", {"focusable"})))));
}

TEST(AgentSplashRuleTest, AProgressBarWithAValueIsntASplash) {
  base::DictValue bar = Node("progressbar");
  bar.Set("value", base::DictValue().Set("type", "number").Set("value", 40));
  EXPECT_FALSE(IsSplash(Page(Tree(std::move(bar)))));
}

TEST(AgentSplashRuleTest, APageWithNothingLoadingIsntASplash) {
  EXPECT_FALSE(IsSplash(Page(Tree(Node("heading"), Node("StaticText")))));
}

TEST(AgentSplashRuleTest, AnIgnoredProgressBarDoesntCount) {
  base::DictValue bar = Node("progressbar");
  bar.Set("ignored", true);
  EXPECT_FALSE(IsSplash(Page(Tree(std::move(bar), Node("StaticText")))));
}

TEST(AgentSplashRuleTest, AnUnreadFrameIsNoSplash) {
  EXPECT_FALSE(IsSplash(Trees(Tree(Node("Iframe", {"focusable"})), base::ListValue())));
}

}  // namespace
}  // namespace browser_core
