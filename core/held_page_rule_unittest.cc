#include "browser_core/held_page_rule.h"

#include <string>
#include <vector>

#include "browser_core/public/browser_core.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "ui/accessibility/ax_enums.mojom.h"
#include "ui/accessibility/ax_node_data.h"
#include "ui/accessibility/ax_tree_update.h"

namespace browser_core::held_page_rule {
namespace {

constexpr Field kUsername{.focusable = true};
constexpr Field kPassword{.password = true, .focusable = true};

ui::AXNodeData Named(int id,
                     ax::mojom::Role role,
                     const std::string& name,
                     ax::mojom::NameFrom from) {
  ui::AXNodeData node;
  node.id = id;
  node.role = role;
  node.AddStringAttribute(ax::mojom::StringAttribute::kName, name);
  node.SetNameFrom(from);
  return node;
}

ui::AXNodeData Frame(int id, const std::string& title) {
  return Named(id, ax::mojom::Role::kIframe, title, ax::mojom::NameFrom::kTitle);
}

ui::AXTreeUpdate Page(std::vector<ui::AXNodeData> nodes) {
  ui::AXTreeUpdate update;
  ui::AXNodeData root =
      Named(1, ax::mojom::Role::kRootWebArea, "Sign in", ax::mojom::NameFrom::kRelatedElement);
  for (const ui::AXNodeData& node : nodes) {
    root.child_ids.push_back(node.id);
  }
  update.root_id = root.id;
  update.nodes.push_back(std::move(root));
  for (ui::AXNodeData& node : nodes) {
    update.nodes.push_back(std::move(node));
  }
  return update;
}

TEST(HeldPageRuleTest, ASignInsSecondStepHasAPassword) {
  EXPECT_TRUE(FactsOf({kPassword}).password);
  EXPECT_TRUE(FactsOf({kUsername, kPassword}).password);
}

// The password field a sign-in's first step hides for the password manager is none of the user's.
TEST(HeldPageRuleTest, AHiddenPasswordFieldIsNoStep) {
  EXPECT_FALSE(FactsOf({kUsername, {.password = true, .focusable = false}}).password);
}

TEST(HeldPageRuleTest, APageWithNoneOfThemHasNothing) {
  EXPECT_EQ(FactsOf({}), Facts());
  EXPECT_EQ(FactsOf({kUsername}), Facts());
  EXPECT_FALSE(CaptchaIn(Page({}), BC_CAPTCHA_WORDS));
  EXPECT_FALSE(CaptchaIn(Page({Frame(2, "Payment")}), BC_CAPTCHA_WORDS));
}

TEST(HeldPageRuleTest, AOneTimeCodeFieldIsAStep) {
  EXPECT_EQ(FactsOf({{.focusable = true, .one_time_code = true}}), Facts{.one_time_code = true});
}

TEST(HeldPageRuleTest, ACaptchasFrameIsFoundByItsTitle) {
  EXPECT_TRUE(CaptchaIn(Page({Frame(2, "reCAPTCHA")}), BC_CAPTCHA_WORDS));
  EXPECT_TRUE(
      CaptchaIn(Page({Frame(2, "Widget containing checkbox for hCaptcha security challenge")}),
                BC_CAPTCHA_WORDS));
  EXPECT_TRUE(CaptchaIn(Page({Frame(2, "Widget containing a Cloudflare security challenge")}),
                        BC_CAPTCHA_WORDS));
  EXPECT_TRUE(CaptchaIn(Page({Named(2, ax::mojom::Role::kCheckBox, "I'm not a robot",
                                    ax::mojom::NameFrom::kAttribute)}),
                        BC_CAPTCHA_WORDS));
}

// A sign-in's page often says it's protected by a CAPTCHA that asks the user nothing.
TEST(HeldPageRuleTest, TextAboutACaptchaIsNone) {
  EXPECT_FALSE(
      CaptchaIn(Page({Named(2, ax::mojom::Role::kStaticText, "This site is protected by reCAPTCHA",
                            ax::mojom::NameFrom::kContents)}),
                BC_CAPTCHA_WORDS));
  EXPECT_FALSE(CaptchaIn(
      Page({Named(2, ax::mojom::Role::kLink, "reCAPTCHA terms", ax::mojom::NameFrom::kContents)}),
      BC_CAPTCHA_WORDS));
}

TEST(HeldPageRuleTest, AHiddenCaptchaIsNone) {
  ui::AXNodeData hidden = Frame(2, "reCAPTCHA");
  hidden.AddState(ax::mojom::State::kInvisible);
  EXPECT_FALSE(CaptchaIn(Page({hidden}), BC_CAPTCHA_WORDS));
  ui::AXNodeData ignored = Frame(2, "reCAPTCHA");
  ignored.AddState(ax::mojom::State::kIgnored);
  EXPECT_FALSE(CaptchaIn(Page({ignored}), BC_CAPTCHA_WORDS));
}

TEST(HeldPageRuleTest, TheWordsMatchInAnyCase) {
  EXPECT_TRUE(HoldsWord("VERIFY YOU ARE HUMAN", BC_CAPTCHA_WORDS));
  EXPECT_TRUE(HoldsWord("Just a moment: security challenge", BC_CAPTCHA_WORDS));
  EXPECT_FALSE(HoldsWord("Verify your email", BC_CAPTCHA_WORDS));
  EXPECT_FALSE(HoldsWord("", BC_CAPTCHA_WORDS));
}

// A later form or frame adds what it turns true, once; what the page's load had stays.
TEST(HeldPageRuleTest, ALaterFactAddsOnce) {
  Facts known{.password = true};
  EXPECT_FALSE(Add(known, {.password = true}));
  EXPECT_FALSE(Add(known, {}));
  EXPECT_TRUE(known.password);
  EXPECT_TRUE(Add(known, {.captcha = true}));
  EXPECT_EQ(known, (Facts{.password = true, .captcha = true}));
  EXPECT_FALSE(Add(known, {.captcha = true}));
}

}  // namespace
}  // namespace browser_core::held_page_rule
