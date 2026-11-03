#include "browser_core/sync_guard.h"

#include <algorithm>
#include <array>
#include <string_view>

#include "base/base64url.h"
#include "base/no_destructor.h"
#include "base/synchronization/lock.h"
#include "build/build_config.h"
#include "components/sync/protocol/device_info_specifics.pb.h"
#include "components/sync/protocol/entity_data.h"
#include "components/sync/protocol/entity_specifics.pb.h"
#include "components/sync/protocol/password_specifics.pb.h"
#include "crypto/hash.h"
#include "crypto/hmac.h"
#include "crypto/kdf.h"
#include "crypto/random.h"

namespace browser_core {

namespace {

// Whether |specifics| hold nothing but what the passphrase encrypted.
bool Ciphertext(syncer::DataType type, const sync_pb::EntitySpecifics& specifics) {
  sync_pb::EntitySpecifics rest = specifics;
  if (type == syncer::PASSWORDS) {
    // Passwords encrypt into their own specifics, beside a backup of their notes.
    if (!specifics.password().has_encrypted()) {
      return false;
    }
    sync_pb::PasswordSpecifics& password = *rest.mutable_password();
    password.clear_encrypted();
    password.clear_encrypted_notes_backup();
    if (password.ByteSizeLong() != 0) {
      return false;
    }
    rest.clear_password();
  } else {
    if (!specifics.has_encrypted()) {
      return false;
    }
    rest.clear_encrypted();
  }
  return rest.ByteSizeLong() == 0;
}

struct TagKey {
  base::Lock lock;
  std::optional<std::array<uint8_t, 32>> key;
  // For when there's no secret.
  const std::array<uint8_t, 32> random = crypto::RandBytesAsArray<32>();
};

TagKey& Key() {
  static base::NoDestructor<TagKey> key;
  return *key;
}

}  // namespace

bool HoldSyncCommit(syncer::DataType type, bool encrypted, syncer::PassphraseType passphrase) {
  // Sharing a password goes to another user, whom only Google's servers can find.
  if (type == syncer::OUTGOING_PASSWORD_SHARING_INVITATION) {
    return true;
  }
  return syncer::EncryptableUserTypes().Has(type) &&
         !(encrypted && syncer::IsExplicitPassphrase(passphrase));
}

std::string SyncDeviceLabel(const std::string& model) {
#if BUILDFLAG(IS_MAC)
  std::string label = "ilo on Mac";
#elif BUILDFLAG(IS_WIN)
  std::string label = "ilo on Windows";
#else
  std::string label = "ilo on Linux";
#endif
  return model.empty() ? label : label + " (" + model + ")";
}

void TrimSyncCommit(syncer::DataType type, syncer::CommitRequestDataList& commit) {
  if (type != syncer::DEVICE_INFO) {
    return;
  }
  for (const std::unique_ptr<syncer::CommitRequestData>& change : commit) {
    syncer::EntityData& entity = *change->entity;
    if (entity.is_deleted()) {
      continue;
    }
    const sync_pb::DeviceInfoSpecifics& full = entity.specifics.device_info();
    sync_pb::DeviceInfoSpecifics trimmed;
    trimmed.set_cache_guid(full.cache_guid());
    trimmed.set_client_name(SyncDeviceLabel(full.model()));
    trimmed.set_device_type(full.device_type());
    trimmed.set_os_type(full.os_type());
    trimmed.set_device_form_factor(full.device_form_factor());
    trimmed.set_last_updated_timestamp(full.last_updated_timestamp());
    trimmed.set_pulse_interval_in_minutes(full.pulse_interval_in_minutes());
    *entity.specifics.mutable_device_info() = std::move(trimmed);
    entity.name = entity.specifics.device_info().client_name();
  }
}

bool SyncCommitIsCiphertext(syncer::DataType type, const syncer::CommitRequestDataList& commit) {
  if (!syncer::EncryptableUserTypes().Has(type)) {
    return true;
  }
  return std::ranges::all_of(commit, [type](const std::unique_ptr<syncer::CommitRequestData>& c) {
    const syncer::EntityData& entity = *c->entity;
    // A password's deletion has no name, and no deletion has specifics.
    return (entity.name.empty() || entity.name == "encrypted") &&
           (entity.is_deleted() || Ciphertext(type, entity.specifics));
  });
}

void SetSyncPasswordTagKey(const std::optional<SyncSecret>& secret) {
  std::optional<std::array<uint8_t, 32>> key;
  if (secret) {
    constexpr std::string_view kInfo = "ilo sync client tags v1";
    key = crypto::kdf::Hkdf<32>(crypto::hash::kSha256, *secret, base::span<const uint8_t>(),
                                base::as_byte_span(kInfo));
  }
  TagKey& tag_key = Key();
  base::AutoLock lock(tag_key.lock);
  tag_key.key = key;
}

std::string SyncPasswordTag(const std::string& tag) {
  std::array<uint8_t, 32> key;
  {
    TagKey& tag_key = Key();
    base::AutoLock lock(tag_key.lock);
    key = tag_key.key.value_or(tag_key.random);
  }
  std::string out;
  base::Base64UrlEncode(crypto::hmac::SignSha256(key, base::as_byte_span(tag)),
                        base::Base64UrlEncodePolicy::OMIT_PADDING, &out);
  return out;
}

}  // namespace browser_core
