#include "browser_core/other_tabs.h"

#include <memory>
#include <string>
#include <vector>

#include "base/strings/utf_string_conversions.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "components/sessions/core/session_types.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"

namespace browser_core {
namespace {

// A tab that went to each of |pages|, as "url title", and shows the one at |shown|.
std::unique_ptr<sessions::SessionTab> TabOf(const std::vector<std::string>& pages, int shown) {
  auto tab = std::make_unique<sessions::SessionTab>();
  for (const std::string& page : pages) {
    sessions::SerializedNavigationEntry entry;
    const size_t space = page.find(' ');
    entry.set_virtual_url(GURL(page.substr(0, space)));
    entry.set_title(base::UTF8ToUTF16(space == std::string::npos ? "" : page.substr(space + 1)));
    tab->navigations.push_back(entry);
  }
  tab->current_navigation_index = shown;
  return tab;
}

TEST(OtherTabsTest, EachComputersTabsComeInItsOrderWithThePageEachShows) {
  auto docs = TabOf({"https://example.com/a A", "https://example.com/b B"}, 1);
  auto news = TabOf({"https://news.example/ News"}, 0);
  auto mail = TabOf({"https://mail.example/ Mail"}, 0);
  const base::Time now = base::Time::Now();
  const std::vector<OtherComputer> computers = {
      {.name = "ilo on Mac (MacBookPro18,3)",
       .os = "macOS",
       .seen = now,
       .tabs = {docs.get(), news.get()}},
      {.name = "ilo on Linux", .os = "Linux", .seen = now - base::Days(2), .tabs = {mail.get()}},
  };
  const std::vector<OtherTab> tabs = OtherTabsOf(computers);
  ASSERT_EQ(tabs.size(), 3u);
  EXPECT_EQ(tabs[0],
            (OtherTab{"https://example.com/b", "B", "ilo on Mac (MacBookPro18,3)", "macOS", now}));
  EXPECT_EQ(tabs[1].url, "https://news.example/");
  EXPECT_EQ(tabs[2], (OtherTab{"https://mail.example/", "Mail", "ilo on Linux", "Linux",
                               now - base::Days(2)}));
}

TEST(OtherTabsTest, OnlyPagesThatOpenHereAreListed) {
  auto settings = TabOf({"chrome://settings/ Settings"}, 0);
  auto file = TabOf({"file:///home/me/notes.txt notes.txt"}, 0);
  auto none = std::make_unique<sessions::SessionTab>();
  // An index past the end, as a new tab page after the last saved page leaves, shows the last.
  auto past = TabOf({"http://example.org/ Example"}, 3);
  const std::vector<OtherComputer> computers = {
      {.name = "ilo on Windows",
       .os = "Windows",
       .tabs = {settings.get(), file.get(), none.get(), past.get(), nullptr}}};
  const std::vector<OtherTab> tabs = OtherTabsOf(computers);
  ASSERT_EQ(tabs.size(), 1u);
  EXPECT_EQ(tabs[0].url, "http://example.org/");
  EXPECT_EQ(tabs[0].title, "Example");
}

}  // namespace
}  // namespace browser_core
