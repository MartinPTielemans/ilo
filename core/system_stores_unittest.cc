#include "browser_core/system_stores.h"

#include <string>

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

}  // namespace
}  // namespace browser_core
