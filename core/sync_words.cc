#include "browser_core/sync_words.h"

#include <algorithm>
#include <cstring>
#include <vector>

#include "base/strings/string_split.h"
#include "base/strings/string_util.h"
#include "crypto/hash.h"

namespace browser_core {

namespace {

// The secret's 256 bits and the 8 of its checksum, as 24 words take them.
std::array<uint8_t, 33> WithChecksum(const SyncSecret& secret) {
  std::array<uint8_t, 33> bits;
  std::ranges::copy(secret, bits.begin());
  bits[32] = crypto::hash::Sha256(secret)[0];
  return bits;
}

// The 11 bits from bit |at| on, most significant first.
int WordAt(const std::array<uint8_t, 33>& bits, size_t at) {
  int word = 0;
  for (size_t bit = at; bit < at + 11; ++bit) {
    word = (word << 1) | ((bits[bit / 8] >> (7 - bit % 8)) & 1);
  }
  return word;
}

}  // namespace

std::string SyncWordsOf(const SyncSecret& secret) {
  const std::array<uint8_t, 33> bits = WithChecksum(secret);
  std::string words;
  for (size_t i = 0; i < 24; ++i) {
    if (i > 0) {
      words += ' ';
    }
    words += kSyncWords[WordAt(bits, i * 11)];
  }
  return words;
}

std::optional<SyncSecret> SyncSecretOf(std::string_view words) {
  const std::vector<std::string> typed = base::SplitString(
      base::ToLowerASCII(words), base::kWhitespaceASCII, base::TRIM_WHITESPACE,
      base::SPLIT_WANT_NONEMPTY);
  if (typed.size() != 24) {
    return std::nullopt;
  }
  std::array<uint8_t, 33> bits = {};
  size_t at = 0;
  for (const std::string& word : typed) {
    const char* const* end = kSyncWords + std::size(kSyncWords);
    const char* const* found = std::lower_bound(
        kSyncWords, end, word,
        [](const char* listed, const std::string& typed_word) {
          return std::strcmp(listed, typed_word.c_str()) < 0;
        });
    if (found == end || word != *found) {
      return std::nullopt;
    }
    const int index = static_cast<int>(found - kSyncWords);
    for (int bit = 10; bit >= 0; --bit, ++at) {
      bits[at / 8] |= ((index >> bit) & 1) << (7 - at % 8);
    }
  }
  SyncSecret secret;
  std::copy_n(bits.begin(), secret.size(), secret.begin());
  if (WithChecksum(secret)[32] != bits[32]) {
    return std::nullopt;
  }
  return secret;
}

}  // namespace browser_core
