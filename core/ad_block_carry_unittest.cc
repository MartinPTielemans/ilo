#include "browser_core/ad_block_carry.h"

#include <memory>
#include <string>

#include "base/files/file_util.h"
#include "base/files/scoped_temp_dir.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "third_party/leveldatabase/env_chromium.h"
#include "third_party/leveldatabase/src/include/leveldb/db.h"

namespace browser_core::ad_block_carry {
namespace {

class AdBlockCarry : public testing::Test {
 protected:
  void SetUp() override { ASSERT_TRUE(dir_.CreateUniqueTempDir()); }

  base::FilePath Store() const { return dir_.GetPath().AppendASCII("store"); }

  // Keeps |json| under |key| in the store, as chrome.storage.local keeps a value.
  void Keep(const std::string& key, const std::string& json) {
    leveldb_env::Options options;
    options.create_if_missing = true;
    std::unique_ptr<leveldb::DB> db;
    ASSERT_TRUE(leveldb_env::OpenDB(options, Store().AsUTF8Unsafe(), &db).ok());
    ASSERT_TRUE(db->Put(leveldb::WriteOptions(), key, json).ok());
  }

 private:
  base::ScopedTempDir dir_;
};

TEST_F(AdBlockCarry, ReadsMyFiltersUBlockOriginKept) {
  Keep("user-filters", R"("||ads.example^\n/ad.js")");
  Keep("netWhitelist", R"("example.com")");
  EXPECT_EQ(OldFilters(Store()), "||ads.example^\n/ad.js");
  EXPECT_EQ(OldFilters(Store()), "||ads.example^\n/ad.js") << "reading it changes nothing";
}

TEST_F(AdBlockCarry, NoStoreOrNoFiltersIsNone) {
  EXPECT_EQ(OldFilters(Store()), "") << "a profile uBlock Origin never ran in";
  EXPECT_FALSE(base::DirectoryExists(Store())) << "the read makes no store";
  Keep("netWhitelist", R"("example.com")");
  EXPECT_EQ(OldFilters(Store()), "") << "a store with no filters in it";
}

TEST_F(AdBlockCarry, AStoreItCantReadIsLeftAsItIs) {
  Keep("user-filters", "{}");
  EXPECT_EQ(OldFilters(Store()), std::nullopt) << "a value that isn't text";

  // A store whose files aren't LevelDB's, as a broken disk leaves one: LevelDB's own repair would
  // drop what it can't read, and value_store's would delete the store.
  ASSERT_TRUE(base::DeletePathRecursively(Store()));
  ASSERT_TRUE(base::CreateDirectory(Store()));
  ASSERT_TRUE(base::WriteFile(Store().AppendASCII("CURRENT"), "MANIFEST-000009\n"));
  ASSERT_TRUE(base::WriteFile(Store().AppendASCII("MANIFEST-000009"), "not a manifest"));
  EXPECT_EQ(OldFilters(Store()), std::nullopt);
  std::string manifest;
  ASSERT_TRUE(base::ReadFileToString(Store().AppendASCII("MANIFEST-000009"), &manifest));
  EXPECT_EQ(manifest, "not a manifest") << "the store is left for the way back";
}

}  // namespace
}  // namespace browser_core::ad_block_carry
