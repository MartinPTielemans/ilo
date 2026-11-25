// What sync sends of a tab the user handed an agent (handed.h), apart from the bridge so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SYNC_SPACES_HANDED_H_
#define BROWSER_CORE_SYNC_SPACES_HANDED_H_

#include <map>
#include <string>

#include "browser_core/sync_spaces.pb.h"

namespace browser_core::sync_spaces {

// A handed tab is in no Space sync keeps for as long as the agent has it. |sent|, what's sent of
// the account's Spaces now, takes it as |known| has it, what the account was last sent, under
// |key|, its entity's id there: a Today tab as it was, or a pinned tab with the address it's
// pinned at, its name, its folder and its place. So the account's other devices see it neither
// close nor go where the agent takes it, a pin stays the pin it was, and nothing synced says it's
// handed. Nothing of one |known| hasn't: it's sent once it's back, as it is then.
void SendAsKnown(const std::string& key,
                 const std::map<std::string, sync_spaces_pb::SpaceEntity>& known,
                 std::map<std::string, sync_spaces_pb::SpaceEntity>& sent);

}  // namespace browser_core::sync_spaces

#endif  // BROWSER_CORE_SYNC_SPACES_HANDED_H_
