// What a saved read (bc_profile_read, #3820) reads and refuses, apart so browser_core_unittests
// tests the rules alone. saved_read.cc does the reading.

#ifndef BROWSER_CORE_SAVED_READ_RULE_H_
#define BROWSER_CORE_SAVED_READ_RULE_H_

#include <cstddef>
#include <cstdint>
#include <string>

class GURL;

namespace browser_core {

// The most of an answer a read keeps: a page's HTML can pass a mebibyte.
inline constexpr size_t kSavedReadMaxBytes = 4 << 20;
// The longest a read waits, redirects included: whoever asked is waiting on it.
inline constexpr int64_t kSavedReadMaxMilliseconds = 30'000;

// Why |url| can't be read with |max_bytes| and |timeout_ms| on a profile that's |off_the_record|,
// or empty if it can. Only an http(s) address with no user name, password or fragment, since what
// the core sends is the address alone, and never on a private window's session.
std::string SavedReadRefusal(const GURL& url,
                             bool off_the_record,
                             size_t max_bytes,
                             int64_t timeout_ms);

// The limits a read keeps to: what was asked, but no more than the most.
size_t SavedReadBytes(size_t max_bytes);
int64_t SavedReadMilliseconds(int64_t timeout_ms);

// Whether a read of |asked| follows a redirect to |to|: only on |asked|'s own origin, so an
// http to https upgrade, or another host of the same site, is told rather than followed.
bool SavedReadFollows(const GURL& asked, const GURL& to);

}  // namespace browser_core

#endif  // BROWSER_CORE_SAVED_READ_RULE_H_
