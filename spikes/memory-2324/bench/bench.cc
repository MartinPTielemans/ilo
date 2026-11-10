// The memory store's design outside Chromium, to measure it on real page text: the same schema,
// tokenizer, terms, sealing and search as core/memory_index.cc, on the system's SQLite, ICU,
// OpenSSL and zlib instead of Chromium's. Common Crawl's WET files hold the text crawled pages
// showed, boilerplate included, which is close to what PageText reads from a tab.
//
//   bench <file.warc.wet.gz> <db> <pages> [prefix=1]
//
// Prints the store's size at 1,000, 5,000 and 10,000 pages, the time to index each page, and the
// time to answer queries made from words on the stored pages, in a warm and a fresh connection.

#include <openssl/evp.h>
#include <openssl/hmac.h>
#include <openssl/rand.h>
#include <sqlite3.h>
#include <sys/stat.h>
#include <unicode/brkiter.h>
#include <unicode/unistr.h>
#include <zlib.h>

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <memory>
#include <random>
#include <set>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace {

using Clock = std::chrono::steady_clock;

double Ms(Clock::duration d) {
  return std::chrono::duration<double, std::milli>(d).count();
}

// As kMaxText in the core: a page's text past 64 KB is neither kept nor indexed.
constexpr size_t kMaxText = 64 * 1024;
// Words longer than this are hashes, tokens and base64, not something anyone types.
constexpr int32_t kMaxWord = 40;
constexpr size_t kCandidates = 50;
constexpr size_t kMatches = 5;

struct Page {
  std::string url;
  std::string title;
  std::string text;
};

// Reads the next conversion record of a WET file.
bool NextPage(gzFile file, Page& page) {
  char line[1 << 16];
  while (gzgets(file, line, sizeof line)) {
    if (strncmp(line, "WARC-Type: conversion", 21) != 0) {
      continue;
    }
    page = {};
    size_t length = 0;
    while (gzgets(file, line, sizeof line)) {
      if (strncmp(line, "WARC-Target-URI: ", 17) == 0) {
        page.url.assign(line + 17);
        page.url.erase(page.url.find_last_not_of("\r\n") + 1);
      } else if (strncmp(line, "Content-Length: ", 16) == 0) {
        length = strtoul(line + 16, nullptr, 10);
      } else if (line[0] == '\r' || line[0] == '\n') {
        break;
      }
    }
    page.text.resize(length);
    size_t got = 0;
    while (got < length) {
      int n = gzread(file, page.text.data() + got, length - got);
      if (n <= 0) {
        return false;
      }
      got += n;
    }
    size_t end = page.text.find('\n');
    page.title = page.text.substr(0, std::min<size_t>(end, 200));
    return true;
  }
  return false;
}

// BENCH_TERM_BYTES=4 keeps 4 of the HMAC's bytes, which SQLite stores in 4 bytes instead of 8.
int TermBytes() {
  static int bytes = getenv("BENCH_TERM_BYTES") ? atoi(getenv("BENCH_TERM_BYTES")) : 8;
  return bytes;
}

class Terms {
 public:
  explicit Terms(const std::string& key) : key_(key) {}

  // The first 8 bytes of HMAC-SHA256(key, tag word), so the file says nothing of the words without
  // the key, and a word and a prefix of the same letters never share a term.
  int64_t Of(char tag, const std::string& word) const {
    std::string input(1, tag);
    input += word;
    unsigned char mac[32];
    unsigned int length = 0;
    HMAC(EVP_sha256(), key_.data(), key_.size(), reinterpret_cast<const unsigned char*>(input.data()),
         input.size(), mac, &length);
    if (TermBytes() == 4) {
      int32_t term;
      memcpy(&term, mac, sizeof term);
      return term;
    }
    int64_t term;
    memcpy(&term, mac, sizeof term);
    return term;
  }

 private:
  std::string key_;
};

// The words of |text|, lowercased, as ICU's word breaks find them, which for Chinese and Japanese
// uses its dictionary.
std::vector<icu::UnicodeString> Words(const std::string& text) {
  static icu::BreakIterator* breaks = [] {
    UErrorCode status = U_ZERO_ERROR;
    return icu::BreakIterator::createWordInstance(icu::Locale::getRoot(), status);
  }();
  icu::UnicodeString unicode = icu::UnicodeString::fromUTF8(text);
  breaks->setText(unicode);
  std::vector<icu::UnicodeString> words;
  int32_t start = breaks->first();
  for (int32_t end = breaks->next(); end != icu::BreakIterator::DONE;
       start = end, end = breaks->next()) {
    if (breaks->getRuleStatus() == UBRK_WORD_NONE || end - start > kMaxWord) {
      continue;
    }
    icu::UnicodeString word(unicode, start, end - start);
    word.toLower(icu::Locale::getRoot());
    words.push_back(word);
  }
  return words;
}

std::string Utf8(const icu::UnicodeString& word) {
  std::string out;
  word.toUTF8String(out);
  return out;
}

// The prefix a query's last word is looked up by, as its user is still typing it.
constexpr int32_t kPrefix = 3;

std::vector<int64_t> PageTerms(const Terms& terms, const std::string& text, bool prefixes) {
  std::unordered_set<std::string> words;
  std::unordered_set<std::string> starts;
  for (const icu::UnicodeString& word : Words(text)) {
    words.insert(Utf8(word));
    if (prefixes && word.countChar32() >= kPrefix) {
      starts.insert(Utf8(icu::UnicodeString(word, 0, word.moveIndex32(0, kPrefix))));
    }
  }
  std::vector<int64_t> out;
  out.reserve(words.size() + starts.size());
  for (const std::string& word : words) {
    out.push_back(terms.Of('w', word));
  }
  for (const std::string& start : starts) {
    out.push_back(terms.Of('p', start));
  }
  std::sort(out.begin(), out.end());
  out.erase(std::unique(out.begin(), out.end()), out.end());
  return out;
}

// What the core's Seal does: zlib, then the OS-held key's AES-256-GCM, which is what Chromium's
// os_crypt_async Encryptor uses on every OS.
class Sealer {
 public:
  Sealer() { RAND_bytes(key_, sizeof key_); }

  std::string Seal(const std::string& plain) const {
    uLongf size = compressBound(plain.size());
    std::string packed(size, '\0');
    compress2(reinterpret_cast<Bytef*>(packed.data()), &size,
              reinterpret_cast<const Bytef*>(plain.data()), plain.size(), 6);
    packed.resize(size);
    std::string out(12 + packed.size() + 16 + 4, '\0');
    uint32_t length = plain.size();
    memcpy(out.data(), &length, 4);
    unsigned char* nonce = reinterpret_cast<unsigned char*>(out.data() + 4);
    RAND_bytes(nonce, 12);
    EVP_CIPHER_CTX* context = EVP_CIPHER_CTX_new();
    EVP_EncryptInit_ex(context, EVP_aes_256_gcm(), nullptr, key_, nonce);
    int n = 0;
    EVP_EncryptUpdate(context, reinterpret_cast<unsigned char*>(out.data() + 16), &n,
                      reinterpret_cast<const unsigned char*>(packed.data()), packed.size());
    int m = 0;
    EVP_EncryptFinal_ex(context, reinterpret_cast<unsigned char*>(out.data() + 16 + n), &m);
    EVP_CIPHER_CTX_ctrl(context, EVP_CTRL_GCM_GET_TAG, 16, out.data() + 16 + packed.size());
    EVP_CIPHER_CTX_free(context);
    return out;
  }

  std::string Open(const void* data, size_t size) const {
    const char* sealed = static_cast<const char*>(data);
    uint32_t length;
    memcpy(&length, sealed, 4);
    size_t packed_size = size - 16 - 16;
    std::string packed(packed_size, '\0');
    EVP_CIPHER_CTX* context = EVP_CIPHER_CTX_new();
    EVP_DecryptInit_ex(context, EVP_aes_256_gcm(), nullptr, key_,
                       reinterpret_cast<const unsigned char*>(sealed + 4));
    int n = 0;
    EVP_DecryptUpdate(context, reinterpret_cast<unsigned char*>(packed.data()), &n,
                      reinterpret_cast<const unsigned char*>(sealed + 16), packed_size);
    EVP_CIPHER_CTX_ctrl(context, EVP_CTRL_GCM_SET_TAG, 16,
                        const_cast<char*>(sealed + 16 + packed_size));
    int m = 0;
    bool ok = EVP_DecryptFinal_ex(context, reinterpret_cast<unsigned char*>(packed.data() + n), &m);
    EVP_CIPHER_CTX_free(context);
    if (!ok) {
      return {};
    }
    std::string plain(length, '\0');
    uLongf plain_size = length;
    uncompress(reinterpret_cast<Bytef*>(plain.data()), &plain_size,
               reinterpret_cast<const Bytef*>(packed.data()), packed.size());
    return plain;
  }

 private:
  unsigned char key_[32];
};

void Exec(sqlite3* db, const char* sql) {
  char* error = nullptr;
  if (sqlite3_exec(db, sql, nullptr, nullptr, &error) != SQLITE_OK) {
    fprintf(stderr, "%s: %s\n", sql, error);
    exit(1);
  }
}

// Chromium's sql::Database: 4 KB pages, exclusive locking, and secure delete, built in.
sqlite3* OpenDb(const char* path) {
  sqlite3* db;
  sqlite3_open(path, &db);
  Exec(db, "PRAGMA page_size=4096");
  Exec(db, "PRAGMA locking_mode=EXCLUSIVE");
  Exec(db, "PRAGMA secure_delete=ON");
  Exec(db, "PRAGMA journal_mode=TRUNCATE");
  // BENCH_SYNC=OFF leaves out the fsyncs, to time the work apart from the disk's flushes.
  if (getenv("BENCH_SYNC") && strcmp(getenv("BENCH_SYNC"), "OFF") == 0) {
    Exec(db, "PRAGMA synchronous=OFF");
  }
  Exec(db, "PRAGMA cache_size=-2048");
  return db;
}

constexpr char kSchema[] =
    "CREATE TABLE IF NOT EXISTS pages(id INTEGER PRIMARY KEY, url INTEGER NOT NULL,"
    " site INTEGER NOT NULL, visited INTEGER NOT NULL, sealed BLOB NOT NULL);"
    "CREATE UNIQUE INDEX IF NOT EXISTS pages_url ON pages(url);"
    "CREATE INDEX IF NOT EXISTS pages_site ON pages(site);"
    "CREATE TABLE IF NOT EXISTS words(term INTEGER NOT NULL, page INTEGER NOT NULL,"
    " PRIMARY KEY(term, page)) WITHOUT ROWID;";

std::string Site(const std::string& url) {
  size_t start = url.find("://");
  start = start == std::string::npos ? 0 : start + 3;
  size_t end = url.find_first_of("/:?#", start);
  return url.substr(start, end == std::string::npos ? std::string::npos : end - start);
}

long FileSize(const char* path) {
  struct stat info;
  return stat(path, &info) == 0 ? info.st_size : -1;
}

struct Stats {
  std::vector<double> values;
  void Add(double v) { values.push_back(v); }
  double At(double q) {
    std::sort(values.begin(), values.end());
    return values.empty() ? 0 : values[std::min(values.size() - 1, size_t(q * values.size()))];
  }
  double Mean() {
    double s = 0;
    for (double v : values) s += v;
    return values.empty() ? 0 : s / values.size();
  }
};

struct Search {
  const Terms& terms;
  const Sealer& sealer;

  // Pages whose words hold every word of |query|, its last as a prefix, newest first.
  double sql_ms = 0;
  double open_ms = 0;

  // Whether the page at |want| is among the matches. As the core's Search: the last word whole
  // first, then by its first three letters, each pass opening the newest kCandidates.
  bool Run(sqlite3* db, const std::string& query, const std::string& want, size_t* candidates) {
    std::vector<icu::UnicodeString> words = Words(query);
    *candidates = 0;
    if (words.empty()) {
      return false;
    }
    bool typing = words.back().countChar32() >= kPrefix;
    std::set<int64_t> seen;
    size_t matches = 0;
    bool found_want = false;
    for (int pass = 0; pass < (typing ? 2 : 1) && matches < kMatches; ++pass) {
      std::vector<int64_t> wanted;
      for (size_t i = 0; i < words.size(); ++i) {
        bool last = i + 1 == words.size();
        if (last && pass == 1) {
          wanted.push_back(terms.Of(
              'p', Utf8(icu::UnicodeString(words[i], 0, words[i].moveIndex32(0, kPrefix)))));
        } else {
          wanted.push_back(terms.Of('w', Utf8(words[i])));
        }
      }
      std::string sql = "SELECT id, sealed FROM pages WHERE id IN (";
      for (size_t i = 0; i < wanted.size(); ++i) {
        sql += i ? " INTERSECT " : "";
        sql += "SELECT page FROM words WHERE term=?";
      }
      sql += ") ORDER BY visited DESC LIMIT " + std::to_string(kCandidates);
      sqlite3_stmt* statement;
      sqlite3_prepare_v2(db, sql.c_str(), -1, &statement, nullptr);
      for (size_t i = 0; i < wanted.size(); ++i) {
        sqlite3_bind_int64(statement, i + 1, wanted[i]);
      }
      while (true) {
        Clock::time_point stepped = Clock::now();
        bool row = matches < kMatches && sqlite3_step(statement) == SQLITE_ROW;
        Clock::time_point opened = Clock::now();
        sql_ms += Ms(opened - stepped);
        if (!row) break;
        if (!seen.insert(sqlite3_column_int64(statement, 0)).second) continue;
        ++*candidates;
        std::string plain = sealer.Open(sqlite3_column_blob(statement, 1),
                                        sqlite3_column_bytes(statement, 1));
        size_t url_end = plain.find('\n');
        size_t text = plain.find('\n', url_end + 1) + 1;
        // A term can stand for another word with the same 4 bytes, and a prefix for many words,
        // so a candidate counts only once its own words hold the query's.
        std::vector<icu::UnicodeString> page = Words(plain.substr(text));
        bool all = true;
        for (size_t i = 0; i < words.size() && all; ++i) {
          bool last = i + 1 == words.size();
          bool found = false;
          for (const icu::UnicodeString& word : page) {
            if (last ? word.startsWith(words[i]) : word == words[i]) {
              found = true;
              break;
            }
          }
          all = found;
        }
        matches += all;
        found_want |= all && plain.compare(0, url_end, want) == 0;
        open_ms += Ms(Clock::now() - opened);
      }
      sqlite3_finalize(statement);
    }
    return found_want;
  }
};

}  // namespace

int main(int argc, char** argv) {
  if (argc < 4) {
    fprintf(stderr, "bench <wet.gz> <db> <pages> [prefix=1]\n");
    return 2;
  }
  const char* db_path = argv[2];
  size_t count = strtoul(argv[3], nullptr, 10);
  bool prefixes = argc < 5 || argv[4][0] != '0';
  remove(db_path);
  std::string journal = std::string(db_path) + "-journal";

  std::string key(32, '\0');
  RAND_bytes(reinterpret_cast<unsigned char*>(key.data()), key.size());
  Terms terms(key);
  Sealer sealer;
  sqlite3* db = OpenDb(db_path);
  Exec(db, kSchema);

  gzFile file = gzopen(argv[1], "rb");
  gzbuffer(file, 1 << 20);
  Stats words_time, seal_time, write_time, total_time, term_count, text_size, sealed_size;
  std::vector<Page> kept;  // Every 10th page, for queries.
  std::unordered_map<std::string, int> pages_with;  // How many pages hold each word.
  std::mt19937 random(2324);
  sqlite3_stmt* add_page;
  sqlite3_prepare_v2(db, "INSERT OR REPLACE INTO pages(url, site, visited, sealed) VALUES(?,?,?,?)",
                     -1, &add_page, nullptr);
  sqlite3_stmt* add_word;
  sqlite3_prepare_v2(db, "INSERT OR IGNORE INTO words(term, page) VALUES(?,?)", -1, &add_word,
                     nullptr);
  Page page;
  size_t added = 0;
  size_t skipped = 0;
  while (added < count && NextPage(file, page)) {
    if (page.text.size() < 200) {
      ++skipped;  // Empty pages and redirects: PageText would read almost nothing.
      continue;
    }
    if (page.text.size() > kMaxText) {
      page.text.resize(kMaxText);
      // Never cut a UTF-8 sequence in two.
      while (!page.text.empty() && (page.text.back() & 0xC0) == 0x80) page.text.pop_back();
      if (!page.text.empty() && (page.text.back() & 0x80)) page.text.pop_back();
    }
    Clock::time_point start = Clock::now();
    std::vector<int64_t> page_terms = PageTerms(terms, page.text, prefixes);
    Clock::time_point worded = Clock::now();
    std::string sealed = sealer.Seal(page.url + "\n" + page.title + "\n" + page.text);
    Clock::time_point sealed_at = Clock::now();
    Exec(db, "BEGIN");
    sqlite3_bind_int64(add_page, 1, terms.Of('u', page.url));
    sqlite3_bind_int64(add_page, 2, terms.Of('s', Site(page.url)));
    sqlite3_bind_int64(add_page, 3, int64_t(added));
    sqlite3_bind_blob(add_page, 4, sealed.data(), sealed.size(), SQLITE_STATIC);
    sqlite3_step(add_page);
    sqlite3_reset(add_page);
    int64_t id = sqlite3_last_insert_rowid(db);
    for (int64_t term : page_terms) {
      sqlite3_bind_int64(add_word, 1, term);
      sqlite3_bind_int64(add_word, 2, id);
      sqlite3_step(add_word);
      sqlite3_reset(add_word);
    }
    Exec(db, "COMMIT");
    Clock::time_point done = Clock::now();
    words_time.Add(Ms(worded - start));
    seal_time.Add(Ms(sealed_at - worded));
    write_time.Add(Ms(done - sealed_at));
    total_time.Add(Ms(done - start));
    term_count.Add(page_terms.size());
    text_size.Add(page.text.size());
    sealed_size.Add(sealed.size());
    ++added;
    {
      std::unordered_set<std::string> unique;
      for (const icu::UnicodeString& word : Words(page.text)) unique.insert(Utf8(word));
      for (const std::string& word : unique) ++pages_with[word];
    }
    if (added % 10 == 0) {
      kept.push_back(page);
    }
    if (added == 1000 || added == 5000 || added == 10000 || added == count) {
      printf("pages %zu: file %.1f MB (%.1f MB per 1,000 pages)\n", added,
             FileSize(db_path) / 1e6, FileSize(db_path) / 1e6 / (added / 1000.0));
      fflush(stdout);
    }
  }
  sqlite3_finalize(add_page);
  sqlite3_finalize(add_word);
  printf("skipped %zu pages under 200 bytes of text\n", skipped);
  printf("text per page: mean %.1f KB, median %.1f KB, p95 %.1f KB; sealed mean %.1f KB\n",
         text_size.Mean() / 1e3, text_size.At(0.5) / 1e3, text_size.At(0.95) / 1e3,
         sealed_size.Mean() / 1e3);
  printf("terms per page: mean %.0f, median %.0f, p95 %.0f\n", term_count.Mean(),
         term_count.At(0.5), term_count.At(0.95));
  printf("index a page, ms: median %.2f, p95 %.2f, p99 %.2f, max %.2f (words %.2f, seal %.2f, "
         "write %.2f, medians)\n",
         total_time.At(0.5), total_time.At(0.95), total_time.At(0.99), total_time.At(1.0),
         words_time.At(0.5), seal_time.At(0.5), write_time.At(0.5));

  // Queries a person might type for a page they read, the last word cut short as if still
  // typing. "adjacent": one, two or three neighbouring words of the page, common ones too.
  // "distinctive": two of its words, in its order, that fewer than 1 % of the pages hold, as
  // a person remembers a page by what set it apart.
  auto cut = [&](icu::UnicodeString word) {
    if (word.countChar32() > 4) {
      word.truncate(word.moveIndex32(0, 2 + random() % (word.countChar32() - 2)));
    }
    return Utf8(word);
  };
  std::vector<std::pair<std::string, std::string>> adjacent, distinctive;
  for (const Page& source : kept) {
    std::vector<icu::UnicodeString> words = Words(source.text);
    if (words.size() < 10) continue;
    size_t n = 1 + random() % 3;
    size_t at = random() % (words.size() - n);
    std::string query;
    for (size_t i = 0; i < n; ++i) {
      query += (i ? " " : "") + (i + 1 == n ? cut(words[at + i]) : Utf8(words[at + i]));
    }
    adjacent.emplace_back(query, source.url);
    std::vector<icu::UnicodeString> rare;
    for (const icu::UnicodeString& word : words) {
      if (word.countChar32() >= 3 && pages_with[Utf8(word)] * 100 < int(added) &&
          std::find(rare.begin(), rare.end(), word) == rare.end()) {
        rare.push_back(word);
      }
    }
    if (rare.size() >= 2) {
      size_t first = random() % (rare.size() - 1);
      size_t second = first + 1 + random() % (rare.size() - first - 1);
      distinctive.emplace_back(Utf8(rare[first]) + " " + cut(rare[second]), source.url);
    }
  }
  Search search{terms, sealer};
  for (int round = 0; round < 2; ++round) {
    if (round == 1) {
      // A fresh connection, as at launch: SQLite's cache is empty, the OS's may not be.
      sqlite3_close(db);
      db = OpenDb(db_path);
    }
    for (int set = 0; set < 2; ++set) {
      const auto& queries = set ? distinctive : adjacent;
      Stats query_time, candidate_count;
      search.sql_ms = search.open_ms = 0;
      size_t found = 0;
      for (const auto& [query, want] : queries) {
        size_t candidates = 0;
        Clock::time_point start = Clock::now();
        found += search.Run(db, query, want, &candidates);
        query_time.Add(Ms(Clock::now() - start));
        candidate_count.Add(candidates);
      }
      printf("%s, %s, %zu queries at %zu pages, ms: median %.2f, p95 %.2f, p99 %.2f, max %.2f; "
             "%zu found the page they came from among the 5 shown; candidates opened, mean %.1f\n",
             round ? "fresh connection" : "warm", set ? "distinctive" : "adjacent",
             queries.size(), added, query_time.At(0.5), query_time.At(0.95), query_time.At(0.99),
             query_time.At(1.0), found, candidate_count.Mean());
      printf("  of which SQLite %.2f ms and opening candidates %.2f ms a query, mean\n",
             search.sql_ms / queries.size(), search.open_ms / queries.size());
    }
  }
  sqlite3_close(db);
  return 0;
}
