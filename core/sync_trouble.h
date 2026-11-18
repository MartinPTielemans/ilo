// What a sync cycle came to, and whether a trouble has lasted long enough for Settings to say so
// (#3435): three failed cycles in a row, or two minutes of failed cycles with none going through,
// while sync kept trying, decided as each cycle ends, with no timer. Apart from the account
// (sync.cc), so browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SYNC_TROUBLE_H_
#define BROWSER_CORE_SYNC_TROUBLE_H_

#include <string_view>

#include "base/time/time.h"
#include "browser_core/public/browser_core.h"
#include "components/sync/engine/syncer_error.h"

namespace browser_core::sync_trouble {

// The BC_SYNC_KIND_ value Settings names a change by, from the field of its EntitySpecifics that
// says its type, such as "bookmark".
int KindOfField(std::string_view field);

// What the server refused of a cycle's changes, as the commit responses say: how many, and of
// what kind, BC_SYNC_KIND_OTHER once they're of more than one.
class Refused {
 public:
  void Add(int kind);
  int count() const { return count_; }
  int kind() const { return kind_; }

 private:
  int count_ = 0;
  int kind_ = BC_SYNC_KIND_OTHER;
};

// What the cycle whose downloading ended with |download| and committing with |commit| came to: a
// BC_SYNC_TROUBLE_ value, NONE for a cycle that went through. A conflict isn't a trouble: the next
// cycle downloads what the server has and commits again. |refused| counts the changes the server
// refused, which tells a refusal from its error.
int Classify(const syncer::SyncerError& download,
             const syncer::SyncerError& commit,
             const Refused& refused);

// The troubles of the cycles as they end, and the one Settings shows. Not "Run", which in a
// gtest TEST names testing::Test::Run.
class Cycles {
 public:
  // A cycle with |trouble| that started at |started| ended at |ended|, with |refused| of its
  // changes refused. Returns whether what Read gives changed.
  bool Ended(int trouble, const Refused& refused, base::Time started, base::Time ended);
  // Signed out, or signed in again: a run of the account before starts nothing.
  void Reset();
  // The trouble shown, and with it since when and what was refused, into |activity|'s fields.
  void Read(bc_sync_activity* activity) const;

 private:
  // The cycles that failed in a row, and when the first and the last of them ended.
  int failed_ = 0;
  base::Time first_;
  base::Time last_;
  int shown_ = BC_SYNC_TROUBLE_NONE;
  base::Time since_;
  Refused refused_;
};

}  // namespace browser_core::sync_trouble

#endif  // BROWSER_CORE_SYNC_TROUBLE_H_
