#include "browser_core/system_stores.h"

#include <set>
#include <string>
#include <vector>

#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/files/scoped_temp_dir.h"
#include "components/prefs/testing_pref_service.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// Writes a profile folder |name| in |folder| whose Preferences hold |json|.
void WriteProfile(const base::FilePath& folder, const char* name, const std::string& json) {
  const base::FilePath profile = folder.AppendASCII(name);
  ASSERT_TRUE(base::CreateDirectory(profile));
  ASSERT_TRUE(base::WriteFile(profile.AppendASCII("Preferences"), json));
}

TEST(SystemStoresTest, ProfilesOnDiskNameTheirStores) {
  base::ScopedTempDir folder;
  ASSERT_TRUE(folder.CreateUniqueTempDir());
  WriteProfile(folder.GetPath(), "Default",
               R"({"browser_core": {"system_engine": {"store": "a", "sites": ["x.com"]}}})");
  WriteProfile(folder.GetPath(), "Profile 1", R"({"browser_core": {"system_engine": {}}})");
  WriteProfile(folder.GetPath(), "Profile 2",
               R"({"browser_core": {"system_engine": {"store": "b"}}})");
  // Not a profile's Preferences: a folder without, and a file that isn't JSON.
  ASSERT_TRUE(base::CreateDirectory(folder.GetPath().AppendASCII("Crashpad")));
  WriteProfile(folder.GetPath(), "Profile 3", "{");
  EXPECT_EQ(StoresNamedOnDisk(folder.GetPath()), (std::set<std::string>{"a", "b"}));
  EXPECT_EQ(StoreNamedOnDisk(folder.GetPath().AppendASCII("Default")), "a");
  EXPECT_EQ(StoreNamedOnDisk(folder.GetPath().AppendASCII("Profile 1")), "");
}

TEST(SystemStoresTest, AFolderRemovedByHandTakesItsStoresName) {
  base::ScopedTempDir folder;
  ASSERT_TRUE(folder.CreateUniqueTempDir());
  WriteProfile(folder.GetPath(), "Default",
               R"({"browser_core": {"system_engine": {"store": "old"}}})");
  const std::vector<std::string> given = {"old"};
  EXPECT_TRUE(StoresUnnamed(given, StoresNamedOnDisk(folder.GetPath())).empty());
  // The user removes the profile's folder, and a profile made at its path names a new store.
  ASSERT_TRUE(base::DeletePathRecursively(folder.GetPath().AppendASCII("Default")));
  WriteProfile(folder.GetPath(), "Default",
               R"({"browser_core": {"system_engine": {"store": "new"}}})");
  EXPECT_EQ(StoresUnnamed({"old", "new"}, StoresNamedOnDisk(folder.GetPath())),
            std::vector<std::string>{"old"});
}

// A data folder's own Local State, which lists the stores its profiles were given.
class LocalState : public TestingPrefServiceSimple {
 public:
  LocalState() { RegisterGivenStores(registry()); }
};

TEST(SystemStoresTest, AFolderMovedAsideKeepsItsStoresFromACleanOneAtItsPath) {
  base::ScopedTempDir parent;
  ASSERT_TRUE(parent.CreateUniqueTempDir());
  const base::FilePath folder = parent.GetPath().AppendASCII("ilo");
  const base::FilePath aside = parent.GetPath().AppendASCII("ilo aside");
  WriteProfile(folder, "Default", R"({"browser_core": {"system_engine": {"store": "a"}}})");
  WriteProfile(folder, "Profile 1", R"({"browser_core": {"system_engine": {"store": "b"}}})");
  LocalState old_state;
  SetGivenStores(old_state, {"a", "b"});
  // The user moves the data folder aside, its Local State with it, and the app starts a clean
  // one at its path: its list is empty, so its first launch deletes none of the other's stores.
  ASSERT_TRUE(base::Move(folder, aside));
  LocalState clean_state;
  WriteProfile(folder, "Default", R"({"browser_core": {"system_engine": {}}})");
  EXPECT_TRUE(GivenStores(clean_state).empty());
  EXPECT_TRUE(StoresUnnamed(GivenStores(clean_state), StoresNamedOnDisk(folder)).empty());
  // Put back, its profiles name every store its list holds, and none is deleted.
  ASSERT_TRUE(base::DeletePathRecursively(folder));
  ASSERT_TRUE(base::Move(aside, folder));
  EXPECT_EQ(GivenStores(old_state), (std::vector<std::string>{"a", "b"}));
  EXPECT_TRUE(StoresUnnamed(GivenStores(old_state), StoresNamedOnDisk(folder)).empty());
}

// A data folder removed whole takes its list: its stores are left, unreachable, rather than a
// list outside it deleting a moved-aside folder's.
TEST(SystemStoresTest, AWholeDataFolderRemovedTakesItsList) {
  base::ScopedTempDir parent;
  ASSERT_TRUE(parent.CreateUniqueTempDir());
  const base::FilePath folder = parent.GetPath().AppendASCII("ilo");
  ASSERT_TRUE(base::CreateDirectory(folder));
  LocalState clean_state;
  EXPECT_TRUE(StoresUnnamed(GivenStores(clean_state), StoresNamedOnDisk(folder)).empty());
}

// The old ids of a data folder's profiles: Default and Profile 1 to 3, as Local State counts 4.
class OldStoresTest : public testing::Test {
 protected:
  const base::FilePath folder{FILE_PATH_LITERAL("/Users/a/Library/Application Support/ilo")};
  const int count = 4;
  std::string Old(const char* name) const { return OldStoreIdFor(folder.AppendASCII(name)); }
  std::vector<std::string> Gone(const std::vector<std::string>& listed,
                                const std::set<std::string>& named = {},
                                const std::set<base::FilePath>& present = {}) const {
    return OldStoresUnnamed(listed, folder, count, named, present);
  }
};

TEST_F(OldStoresTest, IdsAreTheOnesAnOldVersionGave) {
  // StoreIdFor's before #2664, worked out apart from it: the SHA-1 of the path, as version 5.
  EXPECT_EQ(OldStoreIdFor(base::FilePath(
                FILE_PATH_LITERAL("/Users/a/Library/Application Support/ilo/Default"))),
            "c1186153-1520-5a7b-beca-0bcc5edb8688");
  EXPECT_NE(Old("Default"), Old("Profile 1"));
  EXPECT_EQ(OldProfilePaths(folder, count),
            (std::vector<base::FilePath>{
                folder.AppendASCII("Default"), folder.AppendASCII("Profile 1"),
                folder.AppendASCII("Profile 2"), folder.AppendASCII("Profile 3")}));
  EXPECT_EQ(OldProfilePaths(folder, 1), std::vector<base::FilePath>{folder.AppendASCII("Default")});
}

// Rule 1: only a store on disk is asked for.
TEST_F(OldStoresTest, AStoreNotListedIsntDeleted) {
  EXPECT_TRUE(Gone({}).empty());
  EXPECT_EQ(Gone({Old("Profile 2")}), std::vector<std::string>{Old("Profile 2")});
}

// Rule 2: only an old id of this folder's own profiles, up to Local State's count.
TEST_F(OldStoresTest, OnlyThisFoldersOldIdsGo) {
  const std::string other =
      OldStoreIdFor(base::FilePath(FILE_PATH_LITERAL("/Users/a/ilo-dev")).AppendASCII("Profile 1"));
  const std::string random = "3f2a8c1e-7b4d-4e9a-a1c2-5d6e7f8a9b0c";
  EXPECT_EQ(Gone({other, random, Old("Profile 4"), Old("Profile 3")}),
            std::vector<std::string>{Old("Profile 3")});
  EXPECT_TRUE(OldStoresUnnamed({Old("Profile 1")}, folder, 1, {}, {}).empty());
}

// Rule 3: a store a profile names stays.
TEST_F(OldStoresTest, ANamedStoreStays) {
  EXPECT_EQ(Gone({Old("Profile 1"), Old("Profile 2")}, {Old("Profile 1")}),
            std::vector<std::string>{Old("Profile 2")});
}

// Rule 4: a store whose path has a profile folder stays, even named by nothing, as a profile from
// before #2664 whose Preferences were never written.
TEST_F(OldStoresTest, AStoreWhosePathHasAFolderStays) {
  EXPECT_EQ(Gone({Old("Default"), Old("Profile 1")}, {}, {folder.AppendASCII("Default")}),
            std::vector<std::string>{Old("Profile 1")});
}

// A new data folder never had an old store, so it's marked and never swept: a folder set aside
// and put back later keeps the stores of its profiles the new one hasn't.
TEST(OldStoresSweepTest, ANewDataFolderIsMarkedWithoutASweep) {
  EXPECT_EQ(OldStoresFor(/*new_local_state=*/true, /*swept=*/false), OldStores::kMark);
  EXPECT_EQ(OldStoresFor(false, true), OldStores::kNothing);
  EXPECT_EQ(OldStoresFor(false, false), OldStores::kSweep);
}

}  // namespace
}  // namespace browser_core
