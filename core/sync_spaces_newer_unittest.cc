#include "browser_core/sync_spaces_newer.h"

#include <string>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::sync_spaces {
namespace {

using Entity = sync_spaces_pb::SpaceEntity;

std::string Written() {
  Entity entity;
  entity.set_id("8a6f0c1e-2b3d-4e5f-8a9b-0c1d2e3f4a5b");
  entity.set_kind(Entity::PIN);
  entity.set_space("1b2c3d4e-5f6a-4b7c-8d9e-0f1a2b3c4d5e");
  entity.set_url("https://example.org/");
  entity.set_title("Example");
  return entity.SerializeAsString();
}

Entity Read(const std::string& bytes) {
  Entity entity;
  EXPECT_TRUE(entity.ParseFromString(bytes));
  return entity;
}

TEST(SyncSpacesNewerTest, WhatThisVersionWritesIsItsOwn) {
  EXPECT_FALSE(WrittenByNewer(Read(Written())));
  EXPECT_FALSE(WrittenByNewer(Read(std::string())));
}

// A field this version lacks, as a newer ilo adds one: field 1000, a varint.
TEST(SyncSpacesNewerTest, AFieldItLacksIsNewer) {
  EXPECT_TRUE(WrittenByNewer(Read(Written() + "\xc0\x3e\x01")));
}

// A kind this version lacks, such as kind 9, reads as a Space.
TEST(SyncSpacesNewerTest, AKindItLacksIsNewer) {
  Entity written;
  written.set_id("8a6f0c1e-2b3d-4e5f-8a9b-0c1d2e3f4a5b");
  written.set_url("https://example.org/");
  const Entity entity = Read(written.SerializeAsString() + "\x10\x09");
  EXPECT_EQ(entity.kind(), Entity::SPACE);
  EXPECT_TRUE(WrittenByNewer(entity));
}

}  // namespace
}  // namespace browser_core::sync_spaces
