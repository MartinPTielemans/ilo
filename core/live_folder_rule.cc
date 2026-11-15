#include "browser_core/live_folder_rule.h"

#include <string_view>
#include <tuple>
#include <utility>

#include "base/json/json_reader.h"
#include "base/strings/escape.h"
#include "base/strings/strcat.h"
#include "base/strings/string_util.h"
#include "base/values.h"

namespace browser_core::live_folder {

namespace {

// An owner or a repository name as GitHub allows them: letters, digits, '-', '_' and '.'.
bool IsName(std::string_view name) {
  if (name.empty() || name == "." || name == "..") {
    return false;
  }
  for (char c : name) {
    if (!base::IsAsciiAlphaNumeric(c) && c != '-' && c != '_' && c != '.') {
      return false;
    }
  }
  return true;
}

// The character a reference such as "&amp;" or "&#39;" stands for, at the start of |text|, and
// how long the reference is; nullopt if it isn't one the site writes.
std::optional<std::pair<std::string, size_t>> Reference(std::string_view text) {
  static constexpr std::pair<std::string_view, std::string_view> kNamed[] = {
      {"&amp;", "&"}, {"&lt;", "<"},   {"&gt;", ">"},   {"&quot;", "\""},
      {"&#39;", "'"}, {"&#x27;", "'"}, {"&#x2F;", "/"}, {"&nbsp;", " "},
  };
  for (const auto& [reference, character] : kNamed) {
    if (base::StartsWith(text, reference, base::CompareCase::INSENSITIVE_ASCII)) {
      return std::make_pair(std::string(character), reference.size());
    }
  }
  return std::nullopt;
}

std::optional<Item> Row(const base::Value& value, List list) {
  const base::DictValue* row = value.GetIfDict();
  if (!row) {
    return std::nullopt;
  }
  const std::optional<int> number = row->FindInt("number");
  const std::string* title = row->FindString("hl_title");
  const std::string* owner = row->FindStringByDottedPath("repo.repository.owner_login");
  const std::string* name = row->FindStringByDottedPath("repo.repository.name");
  const std::string* state = row->FindString("state");
  if (!number || *number <= 0 || !title || !owner || !name || !state || !IsName(*owner) ||
      !IsName(*name) || owner->size() + 1 + name->size() > kMaxRepo) {
    return std::nullopt;
  }
  Item item{.list = list,
            .repo = *owner + "/" + *name,
            .number = static_cast<uint32_t>(*number),
            .title = PlainTitle(*title)};
  if (row->FindBool("merged").value_or(false)) {
    item.state = State::kMerged;
  } else if (*state == "closed") {
    item.state = State::kClosed;
  } else if (*state != "open") {
    return std::nullopt;
  } else if (const std::string* reviewable = row->FindString("reviewable_state");
             reviewable && *reviewable == "draft") {
    item.state = State::kDraft;
  }
  return item;
}

}  // namespace

GURL ListUrl(List list) {
  // Open ones alone, not archived, most recently updated first: the page the user would read.
  const std::string_view whose =
      list == List::kReviewRequests ? "review-requested:@me" : "author:@me";
  const std::string query =
      base::StrCat({"is:pr is:open archived:false ", whose, " sort:updated-desc"});
  return GURL(base::StrCat(
      {kSite, "/search?q=", base::EscapeQueryParamValue(query, true), "&type=pullrequests"}));
}

std::optional<Answer> Parse(std::string_view body, List list) {
  if (body.size() > kMaxBytes) {
    return std::nullopt;
  }
  std::optional<base::DictValue> answer =
      base::JSONReader::ReadDict(body, base::JSON_PARSE_RFC, kMaxDepth);
  const base::DictValue* search =
      answer ? answer->FindDictByDottedPath("payload.blackbirdSearchRoute") : nullptr;
  const std::optional<bool> signed_in = search ? search->FindBool("logged_in") : std::nullopt;
  if (!signed_in) {
    return std::nullopt;
  }
  Answer parsed{.signed_in = *signed_in};
  if (!parsed.signed_in) {
    return parsed;
  }
  const base::ListValue* rows = search->FindList("results");
  if (!rows) {
    return std::nullopt;
  }
  for (const base::Value& row : *rows) {
    if (parsed.items.size() == kMaxRows) {
      break;
    }
    if (std::optional<Item> item = Row(row, list)) {
      parsed.items.push_back(std::move(*item));
    }
  }
  return parsed;
}

std::string PlainTitle(std::string_view title) {
  std::string plain;
  bool space = false;
  for (size_t i = 0; i < title.size();) {
    // Markup, such as the <em> the site puts round a match, goes: a title's own '<' comes as &lt;.
    if (title[i] == '<') {
      const size_t end = title.find('>', i);
      i = end == std::string_view::npos ? title.size() : end + 1;
      continue;
    }
    std::string character(1, title[i]);
    size_t length = 1;
    if (title[i] == '&') {
      if (auto reference = Reference(title.substr(i))) {
        std::tie(character, length) = *std::move(reference);
      }
    }
    i += length;
    if (character.size() == 1 &&
        (base::IsAsciiWhitespace(character[0]) || base::IsAsciiControl(character[0]))) {
      space = !plain.empty();
      continue;
    }
    if (space) {
      plain += ' ';
      space = false;
    }
    plain += character;
  }
  // JSONReader gives valid UTF-8, which the cut keeps whole.
  base::TruncateUTF8ToByteSize(plain, kMaxTitle, &plain);
  return plain;
}

}  // namespace browser_core::live_folder
