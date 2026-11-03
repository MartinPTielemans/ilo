#include "browser_core/sync_pairing.h"

#include <string>

#include "base/base64url.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

PairingKey Filled(uint8_t byte) {
  PairingKey key;
  key.fill(byte);
  return key;
}

std::string Base64Url(const PairingKey& key) {
  std::string out;
  base::Base64UrlEncode(key, base::Base64UrlEncodePolicy::OMIT_PADDING, &out);
  return out;
}

// ilo-server's docs/protocol.md, Pairing: its worked example, end to end.
TEST(SyncPairingTest, WorkedExample) {
  PairingRequest request(Filled(0x11));
  PairingAnswer answer(request.commitment(), Filled(0x22));
  EXPECT_EQ(Base64Url(request.public_key()), "e06Qm75__kTEZaIgA31gjuNYl9Me-XLwf3SJLLD3PxM");
  EXPECT_EQ(Base64Url(answer.public_key()), "D6poTtKIZ7l_Smot7l34zpdOdrcBjj8iocTPJnhXDyA");
  EXPECT_EQ(Base64Url(request.commitment()), "iQWWm0uPaU4SyelWA0dwWNlsMHC39SNJ4PjHEuJhCMo");

  ASSERT_TRUE(request.Accept(answer.public_key()));
  ASSERT_TRUE(answer.Reveal(request.public_key()));
  EXPECT_EQ(request.code(), "312568");
  EXPECT_EQ(answer.code(), "312568");

  const SyncSecret secret = Filled(0x33);
  const std::optional<std::vector<uint8_t>> sealed = answer.Seal("example", secret);
  ASSERT_TRUE(sealed);
  std::string encoded;
  base::Base64UrlEncode(*sealed, base::Base64UrlEncodePolicy::OMIT_PADDING, &encoded);
  EXPECT_EQ(encoded, "MIbD9AQ11WmirOWxkJoDGTitoTFomGYAyLFIQ20_tWtXiV2OFNqWo46897wVJd9I");
  EXPECT_EQ(request.Open("example", *sealed), secret);
  EXPECT_FALSE(request.Open("another", *sealed));
}

// browser#638: the relay accepts with a key of its own, sees the key the requester reveals and
// the code the real approver shows, then sends a second pair_accept with a key it found to make
// the requester's code the same. The requester must refuse it and keep its first code.
TEST(SyncPairingTest, RequesterTakesOneAccept) {
  PairingRequest request;
  const PairingKey first = PairingKeys().public_key;
  ASSERT_TRUE(request.Accept(first));
  const std::string code = request.code();
  EXPECT_EQ(code, PairingCode(request.public_key(), first));

  EXPECT_FALSE(request.Accept(PairingKeys().public_key));
  EXPECT_EQ(request.code(), code);
  EXPECT_FALSE(request.Accept(first));
  EXPECT_EQ(request.code(), code);
}

// The approver takes only the key the request committed to, and only once.
TEST(SyncPairingTest, ApproverTakesOneReveal) {
  PairingRequest request;
  PairingAnswer swapped(request.commitment());
  EXPECT_FALSE(swapped.Reveal(PairingKeys().public_key));
  EXPECT_TRUE(swapped.code().empty());
  EXPECT_FALSE(swapped.Reveal(request.public_key()));
  EXPECT_FALSE(swapped.Seal("p", Filled(0x33)));

  PairingAnswer answer(request.commitment());
  ASSERT_TRUE(answer.Reveal(request.public_key()));
  const std::string code = answer.code();
  EXPECT_FALSE(answer.Reveal(request.public_key()));
  EXPECT_EQ(answer.code(), code);
}

// Nothing opens or seals before a key was taken.
TEST(SyncPairingTest, NothingBeforeTheKey) {
  PairingRequest request;
  PairingAnswer answer(request.commitment());
  EXPECT_FALSE(answer.Seal("p", Filled(0x33)));
  EXPECT_FALSE(request.Open("p", std::vector<uint8_t>(48)));
  EXPECT_TRUE(request.code().empty());
}

}  // namespace
}  // namespace browser_core
