// Whether a newer ilo wrote one of the account's Spaces entities, so this version would lose what
// it can't read by writing it again (sync_spaces.cc). Apart from the bridge, so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SYNC_SPACES_NEWER_H_
#define BROWSER_CORE_SYNC_SPACES_NEWER_H_

#include "browser_core/sync_spaces.pb.h"

namespace browser_core::sync_spaces {

// Whether |entity| holds a field this version doesn't know, or a kind it doesn't: proto2 keeps an
// enum value it doesn't know with the fields it doesn't, and reads the field as its first value,
// so an unknown kind would otherwise read as a Space.
bool WrittenByNewer(const sync_spaces_pb::SpaceEntity& entity);

}  // namespace browser_core::sync_spaces

#endif  // BROWSER_CORE_SYNC_SPACES_NEWER_H_
