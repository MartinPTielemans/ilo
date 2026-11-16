#ifndef BROWSER_CORE_LIVE_FOLDER_RULE_H_
#define BROWSER_CORE_LIVE_FOLDER_RULE_H_

#include <stddef.h>
#include <stdint.h>

#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/functional/callback.h"
#include "base/time/time.h"
#include "url/gurl.h"

// What a Live Folder asks GitHub and what it takes from the answer (#2979), apart from the network
// so browser_core_unittests tests it alone. The site and the questions are fixed here: the app
// names a list, never an address.
namespace browser_core::live_folder {

// The site asked, on the Space's profile's own session. An answer from anywhere else fails.
inline constexpr char kSite[] = "https://github.com";

// The most of one list's answer read. A page of the site's search, ten rows, is about 10 KB; an
// answer past this isn't the page the core asked for, and fails rather than being read in part.
inline constexpr size_t kMaxBytes = 256 * 1024;
// The most rows taken from one list: a page has ten, and the sidebar shows what a glance takes in.
inline constexpr size_t kMaxRows = 25;
// The most of a title kept, in bytes, cut at a character: a row shows a line of it.
inline constexpr size_t kMaxTitle = 300;
// The most an owner/name may be, GitHub's own limits (39 and 100) with room.
inline constexpr size_t kMaxRepo = 160;
// The least time between two asks of the site for one profile's lists, whoever asks: an ask inside
// it is answered what the site last gave (#3074).
inline constexpr base::TimeDelta kFloor = base::Minutes(2);
// How deep the answer's JSON may nest: a row's repository is seven containers in, and JSONReader
// counts the top level once more.
inline constexpr size_t kMaxDepth = 9;

// The lists, as the header's BC_LIVE_REVIEW_REQUESTS and BC_LIVE_CREATED number them.
enum class List { kReviewRequests = 0, kCreated = 1 };
// A row's state, as the header's BC_LIVE_OPEN and BC_LIVE_DRAFT number them. The lists ask for open
// pull requests alone, so a merged or closed one leaves them rather than coming with its state.
enum class State { kOpen = 0, kDraft = 1 };

struct Item {
  List list = List::kReviewRequests;
  // "owner/name".
  std::string repo;
  uint32_t number = 0;
  // Plain text.
  std::string title;
  State state = State::kOpen;

  bool operator==(const Item&) const = default;
};

struct Answer {
  // Whether someone is signed in to the site; with nobody, there are no items.
  bool signed_in = false;
  std::vector<Item> items;
};

// What a fetch reports, as the header's BC_LIVE_ANSWERED, BC_LIVE_SIGNED_OUT and BC_LIVE_FAILED
// number them.
enum class Kind { kAnswered = 0, kSignedOut = 1, kFailed = 2 };

struct Outcome {
  Kind kind = Kind::kFailed;
  // Both lists' rows, in the lists' order; only with kAnswered.
  std::vector<Item> items;
};

// What the site last gave for one profile's lists.
struct Kept {
  // Each list's answer, in the lists' order, nullopt for one that failed.
  std::vector<std::optional<Answer>> answers;
  // When the fetch ended.
  base::Time answered;
};

// The site's fetches, one per profile and source, so the site is asked at most once a kFloor for
// each, however many ask and however often: a Live Folder now, a pinned tab's card next (#3074).
// Its clock is base's, which a test's TaskEnvironment moves.
class SharedFetches {
 public:
  // A profile's directory and a BC_LIVE_ source.
  using Key = std::pair<std::string, int>;
  using Done = base::OnceCallback<void(const Kept&)>;

  SharedFetches();
  SharedFetches(const SharedFetches&) = delete;
  SharedFetches& operator=(const SharedFetches&) = delete;
  ~SharedFetches();

  // Answers |done| with |key|'s lists, never inside this call. A fetch under way is joined, even
  // by a |refresh|. Otherwise, inside kFloor of the last fetch's end, |done| gets what that gave
  // a task later, but a |refresh|, the user's own ask, doesn't wait for kFloor. Otherwise |start|
  // runs now, asking the site, and its fetch ends with Ended.
  void Ask(const Key& key, bool refresh, base::OnceClosure start, Done done);

  // |key|'s fetch ended with |answers|: keeps them, and answers each who waited on it.
  void Ended(const Key& key, std::vector<std::optional<Answer>> answers);

 private:
  struct Shared {
    Shared();
    Shared(Shared&&);
    ~Shared();

    bool under_way = false;
    std::vector<Done> waiting;
    std::optional<Kept> last;
    base::TimeTicks ended;
  };
  std::map<Key, Shared> shared_;
};

// What a fetch reports from its lists' answers, each nullopt for one that failed. It was asked
// with the profile |asked|, and the folder's Space has |now| as it answers, empty while that
// profile isn't loaded. Rows asked with one
// profile's session never reach a Space on another, so a folder moved to another profile's Space
// while it waited gets kFailed: one answer to the fetch, and the core doesn't ask again on its own.
// No |answers| is kFailed too: nothing was asked.
Outcome OutcomeOf(const std::vector<std::optional<Answer>>& answers,
                  std::string_view asked,
                  std::string_view now);

// What the core asks for |list|: the site's search for the signed-in user's open pull requests,
// those waiting on their review or their own, most recently updated first.
GURL ListUrl(List list);

// The rows of the site's answer for |list|, or nullopt for an answer that isn't the site's search:
// not JSON, too big or too deep, or without whether anyone is signed in. A row missing a part, or
// with a repository or number that can't be one, or that isn't open, is left out.
std::optional<Answer> Parse(std::string_view body, List list);

// |title| as plain text: the site's highlight markup taken out, its character references read,
// control characters and runs of spaces made one space, and cut to kMaxTitle.
std::string PlainTitle(std::string_view title);

}  // namespace browser_core::live_folder

#endif  // BROWSER_CORE_LIVE_FOLDER_RULE_H_
