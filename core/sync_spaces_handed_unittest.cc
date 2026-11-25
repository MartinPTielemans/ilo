#include "browser_core/sync_spaces_handed.h"

#include <map>
#include <string>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::sync_spaces {
namespace {

using Entity = sync_spaces_pb::SpaceEntity;
using Entities = std::map<std::string, Entity>;

constexpr char kPin[] = "8a6f0c1e-2b3d-4e5f-8a9b-0c1d2e3f4a5b";
constexpr char kToday[] = "today:3c4d5e6f-7a8b-4c9d-8e0f-1a2b3c4d5e6f";
constexpr char kSpace[] = "1b2c3d4e-5f6a-4b7c-8d9e-0f1a2b3c4d5e";
constexpr char kFolder[] = "5e6f7a8b-9c0d-4e1f-8a2b-3c4d5e6f7a8b";

// A pinned tab in a folder, as the account has it.
Entity Pin() {
  Entity entity;
  entity.set_id(kPin);
  entity.set_kind(Entity::PIN);
  entity.set_space(kSpace);
  entity.set_parent(kFolder);
  entity.set_position("place");
  entity.set_url("https://example.org/pinned");
  entity.set_name("Mine");
  entity.set_title("Example");
  return entity;
}

Entity Today() {
  Entity entity;
  entity.set_id(kToday);
  entity.set_kind(Entity::TODAY);
  entity.set_space(kSpace);
  entity.set_position("place");
  entity.set_url("https://example.org/today");
  entity.set_title("Example");
  return entity;
}

// A handed pin is in no Space sync keeps, so nothing would be sent of it and the account's other
// devices would lose it.
TEST(SyncSpacesHandedTest, AHandedPinIsSentAsThePinTheAccountHas) {
  const Entities known = {{kPin, Pin()}};
  Entities sent;
  SendAsKnown(kPin, known, sent);
  ASSERT_TRUE(sent.contains(kPin));
  EXPECT_EQ(sent[kPin].SerializeAsString(), Pin().SerializeAsString());
}

TEST(SyncSpacesHandedTest, AHandedTodayTabIsSentAsTheAccountHasIt) {
  const Entities known = {{kToday, Today()}, {kPin, Pin()}};
  Entities sent;
  SendAsKnown(kToday, known, sent);
  ASSERT_EQ(sent.size(), 1u);
  EXPECT_EQ(sent[kToday].SerializeAsString(), Today().SerializeAsString());
}

// Where the agent took it, and what its page is called there, is never what's sent of it: not its
// address, its title, its folder or its place.
TEST(SyncSpacesHandedTest, NothingTheAgentMadeOfItIsSent) {
  const Entities known = {{kPin, Pin()}};
  Entity taken = Pin();
  taken.set_url("https://elsewhere.example/");
  taken.set_title("Elsewhere");
  taken.clear_parent();
  taken.set_position("another");
  Entities sent = {{kPin, taken}};
  SendAsKnown(kPin, known, sent);
  EXPECT_EQ(sent[kPin].SerializeAsString(), Pin().SerializeAsString());
}

// One handed before the account was ever sent it stays unsent until it's back.
TEST(SyncSpacesHandedTest, NothingIsSentOfOneTheAccountNeverHad) {
  Entities sent;
  SendAsKnown(kPin, {}, sent);
  EXPECT_TRUE(sent.empty());
  sent = {{kPin, Pin()}};
  SendAsKnown(kPin, {}, sent);
  EXPECT_TRUE(sent.empty());
}

}  // namespace
}  // namespace browser_core::sync_spaces
