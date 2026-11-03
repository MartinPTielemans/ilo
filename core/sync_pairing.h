// Pairing's cryptography (ilo-server's docs/protocol.md, Pairing): a device that holds the account
// secret seals it to a new device through the relay, which can't read it, and both show a match
// code the user compares, which the relay can't forge, as the new device commits to its key first.

#ifndef BROWSER_CORE_SYNC_PAIRING_H_
#define BROWSER_CORE_SYNC_PAIRING_H_

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <vector>

#include "browser_core/sync_words.h"

namespace browser_core {

using PairingKey = std::array<uint8_t, 32>;

// An X25519 key pair made for one pairing and dropped after it.
struct PairingKeys {
  PairingKeys();
  // Made from |private_key|, for the protocol's worked example.
  explicit PairingKeys(const PairingKey& private_key);
  ~PairingKeys();

  PairingKey private_key;
  PairingKey public_key;
};

// SHA-256("ilo pairing v1 commitment" || requester's key).
std::array<uint8_t, 32> PairingCommitment(const PairingKey& requester);

// The 6 digits both devices show, from the requester's key and the approver's.
std::string PairingCode(const PairingKey& requester, const PairingKey& approver);

// The secret sealed by the approver to the requester, the ciphertext and its tag, for |pairing|.
// None if |requester| is a low-order key.
std::optional<std::vector<uint8_t>> SealPairing(const PairingKeys& approver,
                                                const PairingKey& requester,
                                                const std::string& pairing,
                                                const SyncSecret& secret);

// The secret the approver sealed, opened by the requester, or none if it doesn't open.
std::optional<SyncSecret> OpenPairing(const PairingKeys& requester,
                                      const PairingKey& approver,
                                      const std::string& pairing,
                                      const std::vector<uint8_t>& sealed);

// This device asking another, which holds the secret, for it (the requester), in one pairing.
// Each side takes one key from the other per pairing (protocol.md, Steps): another is someone
// between the devices, such as the relay, trying keys until the match codes agree.
class PairingRequest {
 public:
  PairingRequest();
  // Made with |private_key|, for the protocol's worked example.
  explicit PairingRequest(const PairingKey& private_key);
  ~PairingRequest();

  // This device's key, which pair_reveal sends, and the commitment to it pair_request sends.
  const PairingKey& public_key() const { return keys_.public_key; }
  std::array<uint8_t, 32> commitment() const;

  // The approver's key, from its pair_accept. True for the first; then this device reveals its
  // key and code() is the match code. False for any other, and the pairing must be cancelled.
  bool Accept(const PairingKey& approver);
  // The match code, or empty before Accept.
  const std::string& code() const { return code_; }

  // The secret the approver sealed for |pairing|, or none if it doesn't open or nothing was
  // accepted.
  std::optional<SyncSecret> Open(const std::string& pairing,
                                 const std::vector<uint8_t>& sealed) const;

 private:
  PairingKeys keys_;
  std::optional<PairingKey> approver_;
  std::string code_;
};

// This device, which holds the secret, answering a request that committed to |commitment| (the
// approver), in one pairing.
class PairingAnswer {
 public:
  explicit PairingAnswer(const std::array<uint8_t, 32>& commitment);
  // Made with |private_key|, for the protocol's worked example.
  PairingAnswer(const std::array<uint8_t, 32>& commitment, const PairingKey& private_key);
  ~PairingAnswer();

  // This device's key, which pair_accept sends.
  const PairingKey& public_key() const { return keys_.public_key; }

  // The requester's key, from its pair_reveal. True for the first, if it's the key the request
  // committed to; then code() is the match code. False otherwise, and the pairing must be
  // cancelled.
  bool Reveal(const PairingKey& requester);
  // The match code, or empty before Reveal.
  const std::string& code() const { return code_; }

  // |secret| sealed to the revealed key for |pairing|, or none before Reveal or for a low-order
  // key.
  std::optional<std::vector<uint8_t>> Seal(const std::string& pairing,
                                           const SyncSecret& secret) const;

 private:
  const std::array<uint8_t, 32> commitment_;
  PairingKeys keys_;
  // Whether a pair_reveal came, if it was refused too.
  bool revealed_ = false;
  std::optional<PairingKey> requester_;
  std::string code_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SYNC_PAIRING_H_
