#include "browser_core/agent_page_rule.h"

#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

constexpr char kWeb[] = "https://example.org/";
constexpr char kBoard[] = "chrome://board/0123456789abcdef0123456789abcdef";

const char* Refusal(const char* page, const char* visible = "", const char* pinned = "") {
  return AgentPageRefusal(GURL(page), GURL(visible), GURL(pinned));
}

TEST(AgentPageRule, AWebPageIsTheAgents) {
  EXPECT_EQ(Refusal(kWeb, kWeb), nullptr);
  EXPECT_EQ(Refusal("http://example.org/a", "http://example.org/a", kWeb), nullptr);
}

// However it's reached: shown, typed and loading, or pinned there before it loads.
TEST(AgentPageRule, ABoardIsNeverTheAgents) {
  EXPECT_STREQ(Refusal(kBoard, kBoard), kNotAgentsTab);
  EXPECT_STREQ(Refusal(kWeb, kBoard), kNotAgentsTab);
  EXPECT_STREQ(Refusal("", "", kBoard), kNotAgentsTab);
  EXPECT_STREQ(Refusal(kWeb, kWeb, kBoard), kNotAgentsTab);
}

TEST(AgentPageRule, APageOffTheWebIsRefused) {
  for (const char* page : {"chrome://settings/passwords", "file:///notes/plan.txt",
                           "chrome-extension://abcdefghijklmnopabcdefghijklmnop/popup.html",
                           "data:text/html,words", "about:srcdoc"}) {
    EXPECT_STREQ(Refusal(page, page), kNotWebPage) << page;
    // Typed into the agent's tab and loading, the page it's leaving still shown.
    EXPECT_STREQ(Refusal(kWeb, page), kNotWebPage) << page;
    // Leaving it for a web page, whose page action waits for that page and checks it then.
    EXPECT_EQ(Refusal(page, kWeb), nullptr) << page;
  }
}

// The empty page a tab or popup starts on: an agent's own new tab before its first page commits,
// which its page action waits past.
TEST(AgentPageRule, ATabYetToCommitItsFirstPageIsTheAgents) {
  EXPECT_EQ(Refusal("", ""), nullptr);
  EXPECT_EQ(Refusal("", kWeb), nullptr);
  EXPECT_EQ(Refusal("about:blank", "about:blank"), nullptr);
  EXPECT_EQ(Refusal("about:blank#top", kWeb), nullptr);
}

TEST(AgentPageRule, ATabYetToLoadIsHeldToItsPinnedPage) {
  EXPECT_EQ(Refusal("", "", kWeb), nullptr);
  EXPECT_STREQ(Refusal("", "", "chrome://settings"), kNotWebPage);
  // Loaded, it's held to what it shows.
  EXPECT_EQ(Refusal(kWeb, kWeb, "chrome://settings"), nullptr);
}

}  // namespace
}  // namespace browser_core
