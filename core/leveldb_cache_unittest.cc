// The engine's patch to leveldb's LRU cache, chromium-leveldb-release-evicts.patch (#2316).
#include <cstddef>
#include <memory>

#include "testing/gtest/include/gtest/gtest.h"
#include "third_party/leveldatabase/src/include/leveldb/cache.h"

namespace browser_core {
namespace {

// leveldb's cache has 16 shards, each holding a 16th of its capacity.
constexpr size_t kShard = 100;

int deleted = 0;
void Delete(const leveldb::Slice&, void*) {
  ++deleted;
}

class LevelDbCache : public testing::Test {
 protected:
  void SetUp() override { deleted = 0; }

  std::unique_ptr<leveldb::Cache> cache_{leveldb::NewLRUCache(16 * kShard)};
};

TEST_F(LevelDbCache, AReleasedEntryLargerThanItsShardIsEvicted) {
  cache_->Release(cache_->Insert("block", nullptr, 4 * kShard, &Delete));
  leveldb::Cache::Handle* found = cache_->Lookup("block");
  EXPECT_EQ(found, nullptr);
  if (found) {
    cache_->Release(found);
  }
  EXPECT_EQ(cache_->TotalCharge(), 0u);
  EXPECT_EQ(deleted, 1);
}

TEST_F(LevelDbCache, AnEntryLargerThanItsShardStaysWhileHeld) {
  leveldb::Cache::Handle* held = cache_->Insert("block", nullptr, 4 * kShard, &Delete);
  // A second hold, released, leaves the first.
  leveldb::Cache::Handle* again = cache_->Lookup("block");
  ASSERT_NE(again, nullptr);
  cache_->Release(again);
  again = cache_->Lookup("block");
  ASSERT_NE(again, nullptr);
  cache_->Release(again);
  EXPECT_EQ(cache_->TotalCharge(), 4 * kShard);
  EXPECT_EQ(deleted, 0);

  cache_->Release(held);
  EXPECT_EQ(cache_->TotalCharge(), 0u);
  EXPECT_EQ(deleted, 1);
}

// The eviction is only past the shard's capacity: a released entry that fits stays cached.
TEST_F(LevelDbCache, AReleasedEntryThatFitsItsShardStays) {
  cache_->Release(cache_->Insert("block", nullptr, kShard / 2, &Delete));
  leveldb::Cache::Handle* found = cache_->Lookup("block");
  EXPECT_NE(found, nullptr);
  if (found) {
    cache_->Release(found);
  }
  EXPECT_EQ(deleted, 0);
}

}  // namespace
}  // namespace browser_core
