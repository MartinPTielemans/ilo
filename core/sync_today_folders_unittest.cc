#include "browser_core/sync_today_folders.h"

#include <map>
#include <string>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::sync_spaces {
namespace {

using Entity = sync_spaces_pb::SpaceEntity;

constexpr char kSpace[] = "1b2c3d4e-5f6a-4b7c-8d9e-0f1a2b3c4d5e";
constexpr char kOtherSpace[] = "2c3d4e5f-6a7b-4c8d-9e0f-1a2b3c4d5e6f";
constexpr char kFolder[] = "3d4e5f6a-7b8c-4d9e-8f1a-2b3c4d5e6f7a";
constexpr char kTab[] = "4e5f6a7b-8c9d-4e0f-9a2b-3c4d5e6f7a8b";

Entity Folder(Entity::Kind kind, const std::string& space) {
  Entity folder;
  folder.set_id(kind == Entity::TODAY_FOLDER ? std::string(kTodayPrefix) + kFolder : kFolder);
  folder.set_kind(kind);
  folder.set_space(space);
  folder.set_name("Trip");
  return folder;
}

// A Today tab as the bridge writes one, in the today's folder |folder| names, if any.
Entity Tab(const std::string& folder) {
  Entity tab;
  tab.set_id(std::string(kTodayPrefix) + kTab);
  tab.set_kind(Entity::TODAY);
  tab.set_space(kSpace);
  SetTodayFolder(tab, folder);
  tab.set_url("https://example.org/");
  tab.set_title("Example");
  tab.set_seen(1764000000);
  return tab;
}

std::map<std::string, Entity> Known(const Entity& folder) {
  return {{folder.id(), folder}};
}

TEST(SyncTodayFoldersTest, ATabGoesInTheTodaysFolderItNames) {
  const Entity folder = Folder(Entity::TODAY_FOLDER, kSpace);
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), Known(folder)), folder.id());
}

TEST(SyncTodayFoldersTest, ATabInNoFolderStaysLoose) {
  EXPECT_EQ(TodayFolderOf(Tab(""), Known(Folder(Entity::TODAY_FOLDER, kSpace))), "");
}

// One the account doesn't have, as its folder's entity hasn't come or never will.
TEST(SyncTodayFoldersTest, AnUnknownFolderLeavesATabLoose) {
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), {}), "");
  EXPECT_EQ(TodayFolderOf(Tab("5f6a7b8c-9d0e-4f1a-8b3c-4d5e6f7a8b9c"),
                          Known(Folder(Entity::TODAY_FOLDER, kSpace))),
            "");
}

// A Today tab never goes in a pinned folder, whatever it names.
TEST(SyncTodayFoldersTest, APinnedFolderTakesNoTodayTab) {
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), Known(Folder(Entity::FOLDER, kSpace))), "");
}

// Nor in a folder of another Space, such as one this device doesn't sync, as an agent's isn't.
TEST(SyncTodayFoldersTest, AnotherSpacesFolderTakesNoTodayTab) {
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), Known(Folder(Entity::TODAY_FOLDER, kOtherSpace))), "");
}

// The field a pinned tab's folder is in puts a Today tab in no folder, and one carrying both goes
// by its today_folder alone.
TEST(SyncTodayFoldersTest, ATodayTabsParentIsNothingToIt) {
  Entity tab = Tab("");
  tab.set_parent(kFolder);
  EXPECT_EQ(TodayFolderOf(tab, Known(Folder(Entity::TODAY_FOLDER, kSpace))), "");

  constexpr char kPinnedFolder[] = "6a7b8c9d-0e1f-4a2b-9c3d-5e6f7a8b9c0d";
  Entity both = Tab(kFolder);
  both.set_parent(kPinnedFolder);
  std::map<std::string, Entity> known = Known(Folder(Entity::TODAY_FOLDER, kSpace));
  Entity pinned = Folder(Entity::FOLDER, kSpace);
  pinned.set_id(kPinnedFolder);
  known[pinned.id()] = pinned;
  EXPECT_EQ(TodayFolderOf(both, known), std::string(kTodayPrefix) + kFolder);
}

// A tab in no today's folder is written byte for byte as before today's folders, so an account
// without one never stops an older ilo syncing. One in a folder adds that field alone, which an
// older ilo lacks, so it stops rather than write the tab back loose (sync_spaces_newer.h).
TEST(SyncTodayFoldersTest, OnlyATabInATodaysFolderCarriesIt) {
  const Entity loose = Tab("");
  EXPECT_FALSE(loose.has_today_folder());
  EXPECT_FALSE(loose.has_parent());

  const std::string before = loose.SerializeAsString();
  const std::string after = Tab(kFolder).SerializeAsString();
  ASSERT_GT(after.size(), before.size());
  EXPECT_EQ(after.substr(0, before.size()), before);
  // Field 13, length-delimited, then the folder's id: the last field, so the rest is as before.
  EXPECT_EQ(after.substr(before.size()), std::string("\x6a\x24") + kFolder);
}

TEST(SyncTodayFoldersTest, OnlyATodayTabGoesInOne) {
  Entity pinned = Tab(kFolder);
  pinned.set_kind(Entity::PIN);
  pinned.set_id(kTab);
  EXPECT_EQ(TodayFolderOf(pinned, Known(Folder(Entity::TODAY_FOLDER, kSpace))), "");
}

}  // namespace
}  // namespace browser_core::sync_spaces
