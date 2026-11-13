#include "browser_core/system_stores.h"

#include <string>

#include "base/files/file_util.h"
#include "base/files/scoped_temp_dir.h"
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

TEST(SystemStoresTest, AWholeDataFolderRemovedNamesNone) {
  base::ScopedTempDir folder;
  ASSERT_TRUE(folder.CreateUniqueTempDir());
  const base::FilePath gone = folder.GetPath().AppendASCII("ilo");
  EXPECT_TRUE(StoresNamedOnDisk(gone).empty());
  EXPECT_EQ(StoresUnnamed({"a", "b"}, StoresNamedOnDisk(gone)),
            (std::vector<std::string>{"a", "b"}));
}

}  // namespace
}  // namespace browser_core
