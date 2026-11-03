#include "browser_core/sync_pairing.h"

#include <string_view>

#include "base/containers/span.h"
#include "base/strings/stringprintf.h"
#include "crypto/aead.h"
#include "crypto/hash.h"
#include "crypto/kdf.h"
#include "third_party/boringssl/src/include/openssl/curve25519.h"

namespace browser_core {

namespace {

// SHA-256 of |label|, an ASCII label without its terminator, and then |keys|.
std::array<uint8_t, 32> Labelled(std::string_view label,
                                 std::initializer_list<const PairingKey*> keys) {
  std::vector<uint8_t> data(label.begin(), label.end());
  for (const PairingKey* key : keys) {
    data.insert(data.end(), key->begin(), key->end());
  }
  return crypto::hash::Sha256(data);
}

// The key the secret is sealed with: HKDF-SHA256 of the X25519 secret the two keys share. None for
// a low-order key, whose shared secret is all zeros.
std::optional<std::array<uint8_t, 32>> SealKey(const PairingKeys& mine,
                                               const PairingKey& theirs,
                                               const PairingKey& requester,
                                               const PairingKey& approver) {
  std::array<uint8_t, 32> shared;
  if (!X25519(shared.data(), mine.private_key.data(), theirs.data())) {
    return std::nullopt;
  }
  const std::array<uint8_t, 32> salt = Labelled("ilo pairing v1 keys", {&requester, &approver});
  constexpr std::string_view kInfo = "ilo pairing v1 seal";
  return crypto::kdf::Hkdf<32>(crypto::hash::kSha256, shared, salt,
                               base::as_byte_span(kInfo));
}

// Each key seals once, so the nonce can be all zeros.
constexpr std::array<uint8_t, 12> kNonce = {};

}  // namespace

PairingKeys::PairingKeys() {
  X25519_keypair(public_key.data(), private_key.data());
}

PairingKeys::PairingKeys(const PairingKey& key) : private_key(key) {
  X25519_public_from_private(public_key.data(), private_key.data());
}

PairingKeys::~PairingKeys() {
  private_key.fill(0);
}

std::array<uint8_t, 32> PairingCommitment(const PairingKey& requester) {
  return Labelled("ilo pairing v1 commitment", {&requester});
}

std::string PairingCode(const PairingKey& requester, const PairingKey& approver) {
  const std::array<uint8_t, 32> hash = Labelled("ilo pairing v1 code", {&requester, &approver});
  uint64_t value = 0;
  for (size_t i = 0; i < 8; ++i) {
    value = (value << 8) | hash[i];
  }
  return base::StringPrintf("%06u", static_cast<unsigned>(value % 1000000));
}

std::optional<std::vector<uint8_t>> SealPairing(const PairingKeys& approver,
                                                const PairingKey& requester,
                                                const std::string& pairing,
                                                const SyncSecret& secret) {
  const std::optional<std::array<uint8_t, 32>> key =
      SealKey(approver, requester, requester, approver.public_key);
  if (!key) {
    return std::nullopt;
  }
  return crypto::aead::Seal(crypto::aead::CHACHA20_POLY1305, *key, secret, kNonce,
                            base::as_byte_span(pairing));
}

std::optional<SyncSecret> OpenPairing(const PairingKeys& requester,
                                      const PairingKey& approver,
                                      const std::string& pairing,
                                      const std::vector<uint8_t>& sealed) {
  const std::optional<std::array<uint8_t, 32>> key =
      SealKey(requester, approver, requester.public_key, approver);
  if (!key) {
    return std::nullopt;
  }
  const std::optional<std::vector<uint8_t>> opened = crypto::aead::Open(
      crypto::aead::CHACHA20_POLY1305, *key, sealed, kNonce, base::as_byte_span(pairing));
  if (!opened || opened->size() != SyncSecret().size()) {
    return std::nullopt;
  }
  SyncSecret secret;
  std::ranges::copy(*opened, secret.begin());
  return secret;
}

PairingRequest::PairingRequest() = default;

PairingRequest::PairingRequest(const PairingKey& private_key) : keys_(private_key) {}

PairingRequest::~PairingRequest() = default;

std::array<uint8_t, 32> PairingRequest::commitment() const {
  return PairingCommitment(keys_.public_key);
}

bool PairingRequest::Accept(const PairingKey& approver) {
  if (approver_) {
    return false;
  }
  approver_ = approver;
  code_ = PairingCode(keys_.public_key, approver);
  return true;
}

std::optional<SyncSecret> PairingRequest::Open(const std::string& pairing,
                                               const std::vector<uint8_t>& sealed) const {
  return approver_ ? OpenPairing(keys_, *approver_, pairing, sealed) : std::nullopt;
}

PairingAnswer::PairingAnswer(const std::array<uint8_t, 32>& commitment)
    : commitment_(commitment) {}

PairingAnswer::PairingAnswer(const std::array<uint8_t, 32>& commitment,
                             const PairingKey& private_key)
    : commitment_(commitment), keys_(private_key) {}

PairingAnswer::~PairingAnswer() = default;

bool PairingAnswer::Reveal(const PairingKey& requester) {
  if (revealed_) {
    return false;
  }
  revealed_ = true;
  if (PairingCommitment(requester) != commitment_) {
    return false;
  }
  requester_ = requester;
  code_ = PairingCode(requester, keys_.public_key);
  return true;
}

std::optional<std::vector<uint8_t>> PairingAnswer::Seal(const std::string& pairing,
                                                        const SyncSecret& secret) const {
  return requester_ ? SealPairing(keys_, *requester_, pairing, secret) : std::nullopt;
}

}  // namespace browser_core
