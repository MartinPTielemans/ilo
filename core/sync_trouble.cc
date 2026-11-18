#include "browser_core/sync_trouble.h"

#include "components/sync/engine/polling_constants.h"

namespace browser_core::sync_trouble {

namespace {

// Long enough that a network change or a wake from sleep, which fails a cycle or two, is over.
constexpr base::TimeDelta kLasting = base::Minutes(2);
constexpr int kLastingCycles = 3;
// The longest sync waits between two tries once they fail: syncer::kMaxBackoffTime, 10 minutes,
// which BackoffDelayProvider clamps every wait to (backoff_delay_provider.cc). A cycle that starts
// longer than that after the failed one before it didn't follow it, as when the Mac slept between
// them, so the failures of a cycle before and one after a sleep aren't two minutes of them.
constexpr base::TimeDelta kLongestRetryWait = syncer::kMaxBackoffTime;

int Trouble(const syncer::SyncerError& error, const Refused& refused) {
  using Type = syncer::SyncerError::Type;
  switch (error.type()) {
    case Type::kSuccess:
      return BC_SYNC_TROUBLE_NONE;
    case Type::kNetworkError:
      return BC_SYNC_TROUBLE_OFFLINE;
    case Type::kHttpError:
      return error.GetHttpErrorOrDie() == net::HTTP_UNAUTHORIZED ? BC_SYNC_TROUBLE_SIGN_IN
                                                                 : BC_SYNC_TROUBLE_SERVER;
    case Type::kProtocolError:
      switch (error.GetProtocolErrorOrDie()) {
        case syncer::THROTTLED:
          return BC_SYNC_TROUBLE_THROTTLED;
        case syncer::CONFLICT:
          return BC_SYNC_TROUBLE_NONE;
        // What a commit with a refused change comes to, and the second also what a server that
        // can't answer now says of the whole request, which refuses nothing in particular.
        case syncer::INVALID_MESSAGE:
        case syncer::TRANSIENT_ERROR:
          return refused.count() ? BC_SYNC_TROUBLE_REFUSED : BC_SYNC_TROUBLE_SERVER;
        default:
          return BC_SYNC_TROUBLE_SERVER;
      }
    case Type::kProtocolViolationError:
      return BC_SYNC_TROUBLE_SERVER;
  }
  return BC_SYNC_TROUBLE_SERVER;
}

}  // namespace

int KindOfField(std::string_view field) {
  if (field == "ilo_space") {
    return BC_SYNC_KIND_SPACES;
  }
  if (field == "bookmark") {
    return BC_SYNC_KIND_BOOKMARKS;
  }
  if (field == "password" || field == "incoming_password_sharing_invitation" ||
      field == "outgoing_password_sharing_invitation") {
    return BC_SYNC_KIND_PASSWORDS;
  }
  if (field == "session") {
    return BC_SYNC_KIND_OPEN_TABS;
  }
  if (field == "ilo_setting" || field == "preference" || field == "priority_preference") {
    return BC_SYNC_KIND_SETTINGS;
  }
  if (field == "extension" || field == "extension_setting") {
    return BC_SYNC_KIND_EXTENSIONS;
  }
  return BC_SYNC_KIND_OTHER;
}

void Refused::Add(int kind) {
  kind_ = count_ == 0 || kind == kind_ ? kind : BC_SYNC_KIND_OTHER;
  ++count_;
}

int Classify(const syncer::SyncerError& download,
             const syncer::SyncerError& commit,
             const Refused& refused) {
  // A cycle that couldn't download commits nothing, so its download says what stopped it.
  const int downloaded = Trouble(download, refused);
  return downloaded != BC_SYNC_TROUBLE_NONE ? downloaded : Trouble(commit, refused);
}

bool Run::Ended(int trouble, const Refused& refused, base::Time started, base::Time ended) {
  const int was = shown_;
  const base::Time was_since = since_;
  const Refused was_refused = refused_;
  if (trouble == BC_SYNC_TROUBLE_NONE || trouble == BC_SYNC_TROUBLE_THROTTLED) {
    // The server answered: a run of failures is over, and being told to wait shows at once.
    failed_ = 0;
    shown_ = trouble;
    since_ = base::Time();
    refused_ = Refused();
  } else {
    // A run of failures is one only while sync kept trying: otherwise it starts again here.
    if (failed_ > 0 && started - last_ > kLongestRetryWait) {
      failed_ = 0;
    }
    if (failed_++ == 0) {
      first_ = ended;
    }
    last_ = ended;
    if (failed_ >= kLastingCycles || ended - first_ >= kLasting) {
      shown_ = trouble;
      since_ = first_;
      refused_ = trouble == BC_SYNC_TROUBLE_REFUSED ? refused : Refused();
    }
  }
  return shown_ != was || since_ != was_since || refused_.count() != was_refused.count() ||
         refused_.kind() != was_refused.kind();
}

void Run::Reset() {
  *this = Run();
}

void Run::Read(bc_sync_activity* activity) const {
  activity->trouble = shown_;
  activity->since = since_.is_null() ? 0 : since_.InMillisecondsFSinceUnixEpoch();
  activity->refused = shown_ == BC_SYNC_TROUBLE_REFUSED ? refused_.count() : 0;
  activity->refused_kind = shown_ == BC_SYNC_TROUBLE_REFUSED ? refused_.kind() : 0;
}

}  // namespace browser_core::sync_trouble
