#ifndef BROWSER_CORE_MEMORY_INDEX_H_
#define BROWSER_CORE_MEMORY_INDEX_H_

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/containers/span.h"
#include "base/files/file_path.h"
#include "base/time/time.h"
#include "sql/database.h"

namespace browser_core {

// Why a site's pages stay out of a profile's memory (bc_memory_kept_out). The page's own
// no-store keeps that page out alone, and isn't listed.
enum class KeptOut : int {
  kUser = 0,     // The user listed it.
  kBank = 1,     // It said it's a bank or another financial service (schema.org).
  kHealth = 2,   // It said it's a medical page or a medical organization (schema.org).
  kDomain = 3,   // Its domain is .bank or .insurance, whose registries check who holds them.
};

// The pages the user's Spaces show are kept under their site, the registrable domain
// (example.co.uk for docs.example.co.uk), which forgetting and keeping out go by.

// A page found for a query, newest first.
struct MemoryMatch {
  std::string url;
  std::string title;
  // A line of the page around the first word of the query, as it reads on the page.
  std::string snippet;
  base::Time visited;
};

// The pages a profile kept, their text found by its words, in one SQLite file. Chromium's SQLite
// has neither full-text search nor virtual tables, so the words are a table of their own: each
// word of a page, and the first three letters of each, as a term, which is 4 bytes of an HMAC
// keyed by the store's own key. A page's URL, title and text are sealed with the profile's
// OS-held key, as its saved passwords are, and the key the terms are made with is too. So the file
// alone says how many pages there are and how many words each has, and nothing of what they are.
//
// Made and used on one sequence that may block. Indexing a page reads and writes the disk, so it
// never runs on the UI thread.
class MemoryIndex {
 public:
  // Seals and opens what the file keeps; the core's uses the profile's os_crypt Encryptor.
  class Sealer {
   public:
    virtual ~Sealer() = default;
    virtual std::optional<std::vector<uint8_t>> Seal(std::string_view plain) const = 0;
    virtual std::optional<std::string> Open(base::span<const uint8_t> sealed) const = 0;
  };

  explicit MemoryIndex(std::unique_ptr<Sealer> sealer);
  MemoryIndex(const MemoryIndex&) = delete;
  MemoryIndex& operator=(const MemoryIndex&) = delete;
  ~MemoryIndex();

  // Opens the file at |path|, making it if there's none. False if it can't, or if its key can't
  // be opened with this Sealer: then nothing is kept, and the file is left as it is.
  bool Open(const base::FilePath& path);
  bool OpenInMemoryForTesting();

  // Keeps |url|'s page, under |site|, in place of what was kept of it before. Its text past
  // kMaxText is cut.
  bool Add(std::string_view url, std::string_view site, std::string_view title,
           std::string_view text, base::Time visited);

  // The pages, newest first and at most |limit|, whose words hold every word of |query|, the last
  // as the start of one, as the user is still typing it.
  std::vector<MemoryMatch> Search(std::string_view query, size_t limit);

  // Forgets every page of |site|.
  void ForgetSite(std::string_view site);
  // Forgets the pages at |urls|, and every page visited from |begin| to |end| when begin < end.
  void ForgetPages(const std::vector<std::string>& urls, base::Time begin, base::Time end);
  // Forgets every page and the key; SQLite's secure delete zeroes what they took in the file.
  void ForgetAll();

  // The sites kept out, with why.
  std::vector<std::pair<std::string, KeptOut>> SitesKeptOut();
  // Keeps |site| out, forgetting its pages, or lets it in again with |out| false.
  void KeepOut(std::string_view site, KeptOut why, bool out);
  // Whether |site| is kept out.
  bool IsKeptOut(std::string_view site);

  int64_t PageCount();

  // A page's text past this is neither kept nor indexed: 64 KB is past 95 % of pages' text.
  static constexpr size_t kMaxText = 64 * 1024;

 private:
  bool SetUp();
  // The term a word of a page is found by; |prefix| for the first letters of one.
  int32_t Term(const std::u16string& word, bool prefix) const;
  // A URL's or a site's, in 8 bytes, as two must never share one.
  int64_t Name(char tag, std::string_view name) const;
  std::vector<int32_t> TermsOf(std::string_view text) const;
  std::optional<int64_t> PageOf(std::string_view url);
  void ForgetPage(int64_t id);
  // The page |id|'s url, title and text, opened.
  struct Kept {
    std::string url;
    std::string title;
    std::string text;
  };
  std::optional<Kept> Unseal(int64_t id);

  std::unique_ptr<Sealer> sealer_;
  sql::Database db_;
  std::vector<uint8_t> key_;
};

// The words of |text| as the store keeps them: ICU's word breaks, which for Chinese and Japanese
// use its dictionary, lowercased, and none longer than 40 characters, which are hashes and tokens
// rather than anything anyone types.
std::vector<std::u16string> MemoryWords(std::u16string_view text);

}  // namespace browser_core

#endif  // BROWSER_CORE_MEMORY_INDEX_H_
