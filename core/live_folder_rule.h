#ifndef BROWSER_CORE_LIVE_FOLDER_RULE_H_
#define BROWSER_CORE_LIVE_FOLDER_RULE_H_

#include <stddef.h>
#include <stdint.h>

#include <optional>
#include <string>
#include <string_view>
#include <vector>

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
