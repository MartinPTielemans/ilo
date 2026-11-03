// The account secret as 24 recovery words: BIP-39's English encoding of its 256 bits and the first
// 8 bits of their SHA-256, 11 bits a word (ilo-server's docs/protocol.md, The account secret). The
// words are also the passphrase Chromium's sync is given.

#ifndef BROWSER_CORE_SYNC_WORDS_H_
#define BROWSER_CORE_SYNC_WORDS_H_

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

namespace browser_core {

using SyncSecret = std::array<uint8_t, 32>;

// BIP-39's English list, in its order, which is alphabetical (sync_words_list.cc).
extern const char* const kSyncWords[2048];

// |secret| as words, lowercase, one space between them.
std::string SyncWordsOf(const SyncSecret& secret);

// The secret |words| encode, in any case and spacing, or none unless they're 24 words of the list
// with their checksum.
std::optional<SyncSecret> SyncSecretOf(std::string_view words);

}  // namespace browser_core

#endif  // BROWSER_CORE_SYNC_WORDS_H_
