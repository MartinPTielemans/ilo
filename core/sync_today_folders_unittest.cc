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

Entity Tab(const std::string& parent) {
  Entity tab;
  tab.set_id(std::string(kTodayPrefix) + kTab);
  tab.set_kind(Entity::TODAY);
  tab.set_space(kSpace);
  tab.set_parent(parent);
  tab.set_url("https://example.org/");
  return tab;
}

std::map<std::string, Entity> Known(const Entity& folder) {
  return {{folder.id(), folder}};
}

TEST(SyncTodayFoldersTest, ATabGoesInTheTodaysFolderItNames) {
  const Entity folder = Folder(Entity::TODAY_FOLDER, kSpace);
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), Known(folder)), folder.id());
}

TEST(SyncTodayFoldersTest, ATabWithNoParentStaysLoose) {
  EXPECT_EQ(TodayFolderOf(Tab(""), Known(Folder(Entity::TODAY_FOLDER, kSpace))), "");
}

// One the account doesn't have, as its folder's entity hasn't come or never will.
TEST(SyncTodayFoldersTest, AnUnknownParentLeavesATabLoose) {
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), {}), "");
  EXPECT_EQ(TodayFolderOf(Tab("5f6a7b8c-9d0e-4f1a-8b3c-4d5e6f7a8b9c"),
                          Known(Folder(Entity::TODAY_FOLDER, kSpace))),
            "");
}

// A Today tab never goes in a pinned folder, whatever its parent says.
TEST(SyncTodayFoldersTest, APinnedFolderTakesNoTodayTab) {
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), Known(Folder(Entity::FOLDER, kSpace))), "");
}

// Nor in a folder of another Space, such as one this device doesn't sync, as an agent's isn't.
TEST(SyncTodayFoldersTest, AnotherSpacesFolderTakesNoTodayTab) {
  EXPECT_EQ(TodayFolderOf(Tab(kFolder), Known(Folder(Entity::TODAY_FOLDER, kOtherSpace))), "");
}

TEST(SyncTodayFoldersTest, OnlyATodayTabGoesInOne) {
  Entity pinned = Tab(kFolder);
  pinned.set_kind(Entity::PIN);
  pinned.set_id(kTab);
  EXPECT_EQ(TodayFolderOf(pinned, Known(Folder(Entity::TODAY_FOLDER, kSpace))), "");
}

}  // namespace
}  // namespace browser_core::sync_spaces
