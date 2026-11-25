#include "browser_core/keychain_item.h"

#include <map>
#include <optional>
#include <string>
#include <string_view>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Plist = std::map<std::string, std::string, std::less<>>;

std::optional<KeychainItem> ItemFor(const Plist& plist) {
  return KeychainItemFor([&](std::string_view key) {
    auto found = plist.find(key);
    return found == plist.end() ? std::string() : found->second;
  });
}

TEST(KeychainItem, IsNamedForTheProfilesFolder) {
  std::optional<KeychainItem> item = ItemFor({{"CrProductDirName", "ilo"},
                                              {"CFBundleName", "ilo"},
                                              {"CFBundleDisplayName", "ilo"}});
  ASSERT_TRUE(item);
  EXPECT_EQ(item->service, "ilo Safe Storage");
  EXPECT_EQ(item->account, "ilo");
}

TEST(KeychainItem, KeepsItsNameWhenTheShownNameChanges) {
  std::optional<KeychainItem> item = ItemFor({{"CrProductDirName", "ilo"},
                                              {"CFBundleName", "woven"},
                                              {"CFBundleDisplayName", "woven"}});
  ASSERT_TRUE(item);
  EXPECT_EQ(item->service, "ilo Safe Storage");
  EXPECT_EQ(item->account, "ilo");
}

TEST(KeychainItem, FollowsTheFolderNotTheShownName) {
  std::optional<KeychainItem> item =
      ItemFor({{"CrProductDirName", "Fork Browser"}, {"CFBundleName", "ilo"}});
  ASSERT_TRUE(item);
  EXPECT_EQ(item->service, "Fork Browser Safe Storage");
  EXPECT_EQ(item->account, "Fork Browser");
}

TEST(KeychainItem, IsChromiumsOwnWithoutTheFolderKey) {
  EXPECT_FALSE(ItemFor({{"CFBundleName", "ilo"}, {"CFBundleDisplayName", "ilo"}}));
  EXPECT_FALSE(ItemFor({{"CrProductDirName", ""}, {"CFBundleName", "ilo"}}));
  EXPECT_FALSE(ItemFor({}));
}

}  // namespace
}  // namespace browser_core
