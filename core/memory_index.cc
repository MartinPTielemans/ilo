#include "browser_core/memory_index.h"

#include <algorithm>
#include <array>
#include <functional>
#include <set>
#include <tuple>
#include <utility>

#include "base/containers/span.h"
#include "base/i18n/break_iterator.h"
#include "base/i18n/case_conversion.h"
#include "base/numerics/byte_conversions.h"
#include "base/strings/cstring_view.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "crypto/hmac.h"
#include "crypto/random.h"
#include "sql/statement.h"
#include "sql/transaction.h"
#include "third_party/icu/source/common/unicode/utf16.h"
#include "third_party/zlib/google/compression_utils.h"

namespace browser_core {

namespace {

constexpr size_t kMaxWord = 40;
// A query's last word is found by its first three letters until it's whole, then checked
// against each page's own words.
constexpr size_t kPrefix = 3;
// The newest pages holding every term of a query that are opened to check them.
constexpr size_t kCandidates = 50;
constexpr size_t kSnippet = 160;
constexpr size_t kKeyBytes = 32;

// One table of pages, sealed, and one of the terms each page's words make, which a query
// intersects. WITHOUT ROWID keeps a term's pages together, in term order. One statement each, as
// Chromium means sql::Database::Execute to take one.
constexpr base::cstring_view kSchema[] = {
    "CREATE TABLE IF NOT EXISTS meta(name TEXT PRIMARY KEY, value BLOB NOT NULL)",
    "CREATE TABLE IF NOT EXISTS pages(id INTEGER PRIMARY KEY, url INTEGER NOT NULL,"
    " site INTEGER NOT NULL, visited INTEGER NOT NULL, sealed BLOB NOT NULL)",
    "CREATE UNIQUE INDEX IF NOT EXISTS pages_url ON pages(url)",
    "CREATE INDEX IF NOT EXISTS pages_site ON pages(site)",
    "CREATE INDEX IF NOT EXISTS pages_visited ON pages(visited)",
    "CREATE TABLE IF NOT EXISTS words(term INTEGER NOT NULL, page INTEGER NOT NULL,"
    " PRIMARY KEY(term, page)) WITHOUT ROWID",
    "CREATE TABLE IF NOT EXISTS kept_out(site INTEGER PRIMARY KEY, why INTEGER NOT NULL,"
    " sealed BLOB NOT NULL)",
};

size_t CodePoints(const std::u16string& word) {
  size_t count = 0;
  for (size_t i = 0; i < word.size(); ++i) {
    count += !U16_IS_TRAIL(word[i]);
  }
  return count;
}

// The first |count| code points of |word|.
std::u16string Start(const std::u16string& word, size_t count) {
  size_t end = 0;
  for (size_t seen = 0; end < word.size(); ++end) {
    if (!U16_IS_TRAIL(word[end]) && seen++ == count) {
      break;
    }
  }
  return word.substr(0, end);
}

// Never cuts a UTF-8 sequence in two.
std::string_view Cut(std::string_view text, size_t max) {
  if (text.size() <= max) {
    return text;
  }
  size_t end = max;
  while (end > 0 && (static_cast<uint8_t>(text[end]) & 0xC0) == 0x80) {
    --end;
  }
  return text.substr(0, end);
}

struct Word {
  std::u16string text;
  size_t at;  // In the text it came from, in UTF-16 units.
};

std::vector<Word> WordsAt(std::u16string_view text) {
  std::vector<Word> words;
  base::i18n::BreakIterator breaks(text, base::i18n::BreakIterator::BREAK_WORD);
  if (!breaks.Init()) {
    return words;
  }
  while (breaks.Advance()) {
    if (!breaks.IsWord() || breaks.pos() - breaks.prev() > kMaxWord) {
      continue;
    }
    words.push_back({base::i18n::ToLower(breaks.GetString()), breaks.prev()});
  }
  return words;
}

// Where in |text| a page holds every one of |query|'s words, the last as the start of one: the
// first word's place, or nullopt. Terms are 4 bytes of a hash and the last word's is its first
// three letters, so a page they find is a candidate until its own words say so.
std::optional<size_t> Holds(std::u16string_view text, const std::vector<std::u16string>& query) {
  std::vector<Word> words = WordsAt(text);
  std::optional<size_t> first;
  for (size_t i = 0; i < query.size(); ++i) {
    bool last = i + 1 == query.size();
    auto found = std::ranges::find_if(words, [&](const Word& word) {
      return last ? word.text.starts_with(query[i]) : word.text == query[i];
    });
    if (found == words.end()) {
      return std::nullopt;
    }
    if (i == 0) {
      first = found->at;
    }
  }
  return first;
}

// The line of |text| at |at|, cut to kSnippet around it.
std::string Snippet(std::u16string_view text, size_t at) {
  size_t start = text.rfind(u'\n', at);
  start = start == std::u16string_view::npos ? 0 : start + 1;
  size_t end = text.find(u'\n', at);
  end = end == std::u16string_view::npos ? text.size() : end;
  if (end - start > kSnippet) {
    start = std::max(start, at > kSnippet / 4 ? at - kSnippet / 4 : 0);
    end = std::min(end, start + kSnippet);
  }
  return base::UTF16ToUTF8(base::TrimWhitespace(text.substr(start, end - start), base::TRIM_ALL));
}

}  // namespace

std::vector<std::u16string> MemoryWords(std::u16string_view text) {
  std::vector<std::u16string> words;
  for (Word& word : WordsAt(text)) {
    words.push_back(std::move(word.text));
  }
  return words;
}

MemoryIndex::MemoryIndex(std::unique_ptr<Sealer> sealer)
    : sealer_(std::move(sealer)),
      // Chromium's metrics take only the tags its histograms list. A slice adds "Memory" in an
      // engine patch; the spike borrows the closest, Chrome's own store of page passages.
      db_(sql::DatabaseOptions().set_page_size(4096).set_cache_size(512),
          sql::Database::Tag("HistoryEmbeddings")) {}

MemoryIndex::~MemoryIndex() = default;

bool MemoryIndex::Open(const base::FilePath& path) {
  return db_.Open(path) && SetUp();
}

bool MemoryIndex::OpenInMemoryForTesting() {
  return db_.OpenInMemory() && SetUp();
}

bool MemoryIndex::SetUp() {
  for (base::cstring_view statement : kSchema) {
    if (!db_.Execute(statement)) {
      return false;
    }
  }
  sql::Statement read(db_.GetUniqueStatement("SELECT value FROM meta WHERE name='key'"));
  if (read.Step()) {
    std::optional<std::string> key = sealer_->Open(read.ColumnBlob(0));
    if (!key || key->size() != kKeyBytes) {
      // Another OS key sealed it, as after the keychain's item was deleted: nothing it holds can
      // be read, and the user forgetting everything starts it again.
      return false;
    }
    key_.assign(key->begin(), key->end());
    return true;
  }
  key_ = crypto::RandBytesAsVector(kKeyBytes);
  std::optional<std::vector<uint8_t>> sealed =
      sealer_->Seal(std::string_view(reinterpret_cast<const char*>(key_.data()), key_.size()));
  if (!sealed) {
    return false;
  }
  sql::Statement write(db_.GetUniqueStatement("INSERT INTO meta(name, value) VALUES('key', ?)"));
  write.BindBlob(0, std::move(*sealed));
  return write.Run();
}

int32_t MemoryIndex::Term(const std::u16string& word, bool prefix) const {
  std::string input(prefix ? "p" : "w");
  input += base::UTF16ToUTF8(word);
  std::array<uint8_t, 32> mac = crypto::hmac::SignSha256(key_, base::as_byte_span(input));
  return base::I32FromLittleEndian(base::span(mac).first<4u>());
}

int64_t MemoryIndex::Name(char tag, std::string_view name) const {
  std::string input(1, tag);
  input += name;
  std::array<uint8_t, 32> mac = crypto::hmac::SignSha256(key_, base::as_byte_span(input));
  return base::I64FromLittleEndian(base::span(mac).first<8u>());
}

std::vector<int32_t> MemoryIndex::TermsOf(std::string_view text) const {
  std::vector<int32_t> terms;
  for (const std::u16string& word : MemoryWords(base::UTF8ToUTF16(text))) {
    terms.push_back(Term(word, false));
    if (CodePoints(word) >= kPrefix) {
      terms.push_back(Term(Start(word, kPrefix), true));
    }
  }
  std::ranges::sort(terms);
  terms.erase(std::ranges::unique(terms).begin(), terms.end());
  return terms;
}

bool MemoryIndex::Add(std::string_view url, std::string_view site, std::string_view title,
                      std::string_view text, base::Time visited) {
  if (key_.empty()) {
    return false;
  }
  text = Cut(text, kMaxText);
  std::string line(title);
  base::ReplaceChars(line, "\n", " ", &line);
  std::string plain = base::StrCat({url, "\n", line, "\n", text});
  std::string packed;
  if (!compression::GzipCompress(base::span(plain), &packed)) {
    return false;
  }
  std::optional<std::vector<uint8_t>> sealed = sealer_->Seal(packed);
  if (!sealed) {
    return false;
  }
  std::vector<int32_t> terms = TermsOf(text);
  // One transaction a page: one disk flush, and a crash keeps the page whole or not at all.
  sql::Transaction transaction(&db_);
  if (!transaction.Begin()) {
    return false;
  }
  if (std::optional<int64_t> old = PageOf(url)) {
    ForgetPage(*old);
  }
  sql::Statement page(db_.GetCachedStatement(
      SQL_FROM_HERE, "INSERT INTO pages(url, site, visited, sealed) VALUES(?,?,?,?)"));
  page.BindInt64(0, Name('u', url));
  page.BindInt64(1, Name('s', site));
  page.BindTime(2, visited);
  page.BindBlob(3, std::move(*sealed));
  if (!page.Run()) {
    return false;
  }
  int64_t id = db_.GetLastInsertRowId();
  sql::Statement word(
      db_.GetCachedStatement(SQL_FROM_HERE, "INSERT OR IGNORE INTO words(term, page) VALUES(?,?)"));
  for (int32_t term : terms) {
    word.BindInt(0, term);
    word.BindInt64(1, id);
    if (!word.Run()) {
      return false;
    }
    word.Reset(true);
  }
  return transaction.Commit();
}

std::optional<int64_t> MemoryIndex::PageOf(std::string_view url) {
  sql::Statement find(db_.GetCachedStatement(SQL_FROM_HERE, "SELECT id FROM pages WHERE url=?"));
  find.BindInt64(0, Name('u', url));
  if (!find.Step()) {
    return std::nullopt;
  }
  return find.ColumnInt64(0);
}

std::optional<MemoryIndex::Kept> MemoryIndex::Unseal(int64_t id) {
  sql::Statement read(
      db_.GetCachedStatement(SQL_FROM_HERE, "SELECT sealed FROM pages WHERE id=?"));
  read.BindInt64(0, id);
  if (!read.Step()) {
    return std::nullopt;
  }
  std::optional<std::string> packed = sealer_->Open(read.ColumnBlob(0));
  std::string plain;
  if (!packed || !compression::GzipUncompress(*packed, &plain)) {
    return std::nullopt;
  }
  size_t url_end = plain.find('\n');
  size_t title_end = url_end == std::string::npos ? url_end : plain.find('\n', url_end + 1);
  if (title_end == std::string::npos) {
    return std::nullopt;
  }
  return Kept{plain.substr(0, url_end), plain.substr(url_end + 1, title_end - url_end - 1),
              plain.substr(title_end + 1)};
}

void MemoryIndex::ForgetPage(int64_t id) {
  // A page's terms come from its own text, opened again, so the file keeps no list of them.
  if (std::optional<Kept> kept = Unseal(id)) {
    sql::Statement word(
        db_.GetCachedStatement(SQL_FROM_HERE, "DELETE FROM words WHERE term=? AND page=?"));
    for (int32_t term : TermsOf(kept->text)) {
      word.BindInt(0, term);
      word.BindInt64(1, id);
      word.Run();
      word.Reset(true);
    }
  } else {
    // One that can't be opened can't say its terms: find its rows the slow way.
    sql::Statement words(
        db_.GetCachedStatement(SQL_FROM_HERE, "DELETE FROM words WHERE page=?"));
    words.BindInt64(0, id);
    words.Run();
  }
  sql::Statement page(db_.GetCachedStatement(SQL_FROM_HERE, "DELETE FROM pages WHERE id=?"));
  page.BindInt64(0, id);
  page.Run();
}

std::vector<MemoryMatch> MemoryIndex::Search(std::string_view query, size_t limit) {
  std::vector<MemoryMatch> matches;
  std::vector<std::u16string> words = MemoryWords(base::UTF8ToUTF16(query));
  if (key_.empty() || words.empty() || limit == 0) {
    return matches;
  }
  // The last word as a whole word first, then by its first letters as the user may still be
  // typing it: those find many more pages, of which only the newest kCandidates are opened, so a
  // rare word typed whole would be missed among them.
  bool typing = CodePoints(words.back()) >= kPrefix;
  std::set<int64_t> opened;
  for (bool start : {false, true}) {
    if (start && !typing) {
      break;
    }
    std::vector<int32_t> terms;
    for (size_t i = 0; i < words.size(); ++i) {
      bool last = i + 1 == words.size();
      terms.push_back(last && start ? Term(Start(words[i], kPrefix), true)
                                    : Term(words[i], false));
    }
    std::string sql = "SELECT id, visited FROM pages WHERE id IN (";
    for (size_t i = 0; i < terms.size(); ++i) {
      sql += i ? " INTERSECT SELECT page FROM words WHERE term=?"
               : "SELECT page FROM words WHERE term=?";
    }
    sql += ") ORDER BY visited DESC LIMIT " + base::NumberToString(kCandidates);
    sql::Statement candidates(db_.GetUniqueStatement(sql));
    for (size_t i = 0; i < terms.size(); ++i) {
      candidates.BindInt(i, terms[i]);
    }
    while (matches.size() < limit && candidates.Step()) {
      int64_t id = candidates.ColumnInt64(0);
      if (!opened.insert(id).second) {
        continue;
      }
      std::optional<Kept> kept = Unseal(id);
      if (!kept) {
        continue;
      }
      std::u16string text = base::UTF8ToUTF16(kept->text);
      if (std::optional<size_t> at = Holds(text, words)) {
        matches.push_back({std::move(kept->url), std::move(kept->title), Snippet(text, *at),
                           candidates.ColumnTime(1)});
      }
    }
  }
  // The whole-word pass's pages come first; the reader wants the newest.
  std::ranges::stable_sort(matches, std::ranges::greater(), &MemoryMatch::visited);
  return matches;
}

void MemoryIndex::ForgetSite(std::string_view site) {
  if (key_.empty()) {
    return;
  }
  sql::Transaction transaction(&db_);
  if (!transaction.Begin()) {
    return;
  }
  std::vector<int64_t> ids;
  sql::Statement find(db_.GetCachedStatement(SQL_FROM_HERE, "SELECT id FROM pages WHERE site=?"));
  find.BindInt64(0, Name('s', site));
  while (find.Step()) {
    ids.push_back(find.ColumnInt64(0));
  }
  for (int64_t id : ids) {
    ForgetPage(id);
  }
  transaction.Commit();
}

void MemoryIndex::ForgetPages(const std::vector<std::string>& urls, base::Time begin,
                              base::Time end) {
  if (key_.empty()) {
    return;
  }
  sql::Transaction transaction(&db_);
  if (!transaction.Begin()) {
    return;
  }
  std::vector<int64_t> ids;
  for (const std::string& url : urls) {
    if (std::optional<int64_t> id = PageOf(url)) {
      ids.push_back(*id);
    }
  }
  if (begin < end) {
    sql::Statement find(db_.GetCachedStatement(
        SQL_FROM_HERE, "SELECT id FROM pages WHERE visited >= ? AND visited < ?"));
    find.BindTime(0, begin);
    find.BindTime(1, end);
    while (find.Step()) {
      ids.push_back(find.ColumnInt64(0));
    }
  }
  std::ranges::sort(ids);
  ids.erase(std::ranges::unique(ids).begin(), ids.end());
  for (int64_t id : ids) {
    ForgetPage(id);
  }
  transaction.Commit();
}

void MemoryIndex::ForgetAll() {
  // Chromium builds SQLite with secure delete, so the freed pages are zeroed; VACUUM then gives
  // them back to the disk. The sites kept out are a setting, and stay.
  if (db_.Execute("DELETE FROM words") && db_.Execute("DELETE FROM pages")) {
    std::ignore = db_.Execute("VACUUM");
  }
}

std::vector<std::pair<std::string, KeptOut>> MemoryIndex::SitesKeptOut() {
  std::vector<std::pair<std::string, KeptOut>> sites;
  sql::Statement read(db_.GetUniqueStatement("SELECT why, sealed FROM kept_out"));
  while (read.Step()) {
    if (std::optional<std::string> site = sealer_->Open(read.ColumnBlob(1))) {
      sites.emplace_back(std::move(*site), static_cast<KeptOut>(read.ColumnInt(0)));
    }
  }
  std::ranges::sort(sites);
  return sites;
}

void MemoryIndex::KeepOut(std::string_view site, KeptOut why, bool out) {
  if (key_.empty()) {
    return;
  }
  if (!out) {
    sql::Statement remove(
        db_.GetCachedStatement(SQL_FROM_HERE, "DELETE FROM kept_out WHERE site=?"));
    remove.BindInt64(0, Name('s', site));
    remove.Run();
    return;
  }
  std::optional<std::vector<uint8_t>> sealed = sealer_->Seal(site);
  if (!sealed) {
    return;
  }
  sql::Statement add(db_.GetCachedStatement(
      SQL_FROM_HERE, "INSERT OR REPLACE INTO kept_out(site, why, sealed) VALUES(?,?,?)"));
  add.BindInt64(0, Name('s', site));
  add.BindInt(1, static_cast<int>(why));
  add.BindBlob(2, std::move(*sealed));
  if (add.Run()) {
    ForgetSite(site);
  }
}

bool MemoryIndex::IsKeptOut(std::string_view site) {
  if (key_.empty()) {
    return true;
  }
  sql::Statement find(
      db_.GetCachedStatement(SQL_FROM_HERE, "SELECT 1 FROM kept_out WHERE site=?"));
  find.BindInt64(0, Name('s', site));
  return find.Step();
}

int64_t MemoryIndex::PageCount() {
  sql::Statement count(db_.GetUniqueStatement("SELECT COUNT(*) FROM pages"));
  return count.Step() ? count.ColumnInt64(0) : 0;
}

}  // namespace browser_core
