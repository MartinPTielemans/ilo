// What the device takes from the account's server: Chromium's own DataTypeWorker, as the engine
// builds it with our series, given updates as a server would send them, with the core's look at
// each update (KeepSyncUpdate) and without it.

#include <memory>
#include <string>

#include "base/memory/raw_ptr.h"
#include "base/test/task_environment.h"
#include "browser_core/sync_guard.h"
#include "components/sync/base/client_tag_hash.h"
#include "components/sync/base/data_type.h"
#include "components/sync/base/passphrase_enums.h"
#include "components/sync/engine/cancelation_signal.h"
#include "components/sync/engine/cycle/status_controller.h"
#include "components/sync/engine/data_type_worker.h"
#include "components/sync/protocol/data_type_state.pb.h"
#include "components/sync/protocol/entity_specifics.pb.h"
#include "components/sync/protocol/sync_entity.pb.h"
#include "components/sync/test/fake_cryptographer.h"
#include "components/sync/test/mock_data_type_processor.h"
#include "components/sync/test/mock_nudge_handler.h"
#include "components/sync/test/single_type_mock_server.h"
#include "testing/gtest/include/gtest/gtest.h"

// engine/patches/ours/chromium-sync-hooks.patch defines it in data_type_worker.cc; core/sync.cc
// sets it, which this test does in its place.
extern bool (*g_browser_core_sync_keep_update)(syncer::DataType type,
                                               bool encrypted,
                                               const sync_pb::SyncEntity& update);

namespace browser_core {
namespace {

constexpr char kKey[] = "passphrase";

sync_pb::EntitySpecifics Bookmark() {
  sync_pb::EntitySpecifics specifics;
  specifics.mutable_bookmark()->set_url("https://example.test/");
  specifics.mutable_bookmark()->set_full_title("Example");
  return specifics;
}

sync_pb::EntitySpecifics Extension() {
  sync_pb::EntitySpecifics specifics;
  specifics.mutable_extension()->set_id(std::string(32, 'a'));
  specifics.mutable_extension()->set_enabled(true);
  return specifics;
}

// A device of the account with its passphrase, which encrypts |type|, as every type Chromium can
// encrypt is once the passphrase is set.
class SyncUpdatesTest : public testing::Test {
 protected:
  void Start(syncer::DataType type) {
    type_ = type;
    sync_pb::DataTypeState state;
    state.mutable_progress_marker()->set_data_type_id(
        syncer::GetSpecificsFieldNumberFromDataType(type));
    state.mutable_progress_marker()->set_token("token");
    state.set_initial_sync_state(sync_pb::DataTypeState_InitialSyncState_INITIAL_SYNC_DONE);
    server_ = std::make_unique<syncer::SingleTypeMockServer>(type);
    worker_ = std::make_unique<syncer::DataTypeWorker>(
        type, state, &cryptographer_, /*encryption_enabled=*/false,
        syncer::PassphraseType::kCustomPassphrase, &nudge_handler_, &cancelation_signal_);
    auto processor = std::make_unique<syncer::MockDataTypeProcessor>();
    processor_ = processor.get();
    worker_->ConnectSync(std::move(processor));
    cryptographer_.AddEncryptionKey(kKey);
    cryptographer_.SelectDefaultEncryptionKey(kKey);
    worker_->EnableEncryption();
  }

  void TearDown() override { g_browser_core_sync_keep_update = nullptr; }

  sync_pb::SyncEntity Update(const std::string& tag, const sync_pb::EntitySpecifics& specifics) {
    return server_->UpdateFromServer(10, syncer::ClientTagHash::FromUnhashed(type_, tag),
                                     specifics);
  }

  // As a device of the account commits it: ciphertext beside the type's own field, empty.
  sync_pb::EntitySpecifics Encrypt(const sync_pb::EntitySpecifics& specifics) {
    sync_pb::EntitySpecifics encrypted;
    syncer::AddDefaultFieldValue(type_, &encrypted);
    EXPECT_TRUE(
        cryptographer_.EncryptString(specifics.SerializeAsString(), encrypted.mutable_encrypted()));
    return encrypted;
  }

  // Whether the device's model was given |update| once the server sent it.
  bool Takes(const sync_pb::SyncEntity& update) {
    worker_->ProcessGetUpdatesResponse(server_->GetProgress(), server_->GetContext(), {&update},
                                       &status_);
    worker_->ApplyUpdates(&status_, /*cycle_done=*/true);
    for (size_t n = 0; n < processor_->GetNumUpdateResponses(); ++n) {
      for (const syncer::UpdateResponseData* data : processor_->GetNthUpdateResponse(n)) {
        if (data->entity.id == update.id_string()) {
          return true;
        }
      }
    }
    return false;
  }

 private:
  base::test::SingleThreadTaskEnvironment task_environment_;
  syncer::DataType type_ = syncer::BOOKMARKS;
  syncer::FakeCryptographer cryptographer_;
  syncer::MockNudgeHandler nudge_handler_;
  syncer::CancelationSignal cancelation_signal_;
  syncer::StatusController status_;
  std::unique_ptr<syncer::SingleTypeMockServer> server_;
  std::unique_ptr<syncer::DataTypeWorker> worker_;
  // The worker's, which it outlives.
  raw_ptr<syncer::MockDataTypeProcessor> processor_ = nullptr;
};

// Chromium's worker on its own takes a plain update of a type the account encrypts
// (PopulateUpdateResponseData, "No encryption"), which none of the account's devices wrote.
TEST_F(SyncUpdatesTest, ChromiumTakesAPlainUpdateOfAnEncryptedType) {
  Start(syncer::BOOKMARKS);
  EXPECT_TRUE(Takes(Update("plain", Bookmark())));
}

// With the core's look, the device takes only what the account's devices write.
TEST_F(SyncUpdatesTest, TakesOnlyWhatTheAccountsDevicesWrite) {
  g_browser_core_sync_keep_update = &KeepSyncUpdate;
  Start(syncer::BOOKMARKS);
  EXPECT_FALSE(Takes(Update("plain", Bookmark())));
  EXPECT_TRUE(Takes(Update("encrypted", Encrypt(Bookmark()))));

  // The server's own folders, which it makes with nothing in them, and can't encrypt.
  sync_pb::SyncEntity folder = Update("bookmark_bar", sync_pb::EntitySpecifics());
  syncer::AddDefaultFieldValue(syncer::BOOKMARKS, folder.mutable_specifics());
  folder.set_server_defined_unique_tag("bookmark_bar");
  EXPECT_TRUE(Takes(folder));
  sync_pb::SyncEntity filled = Update("other_bookmarks", Bookmark());
  filled.set_server_defined_unique_tag("other_bookmarks");
  EXPECT_FALSE(Takes(filled));
}

// A device acts on an extension it's given: it installs it from the store.
TEST_F(SyncUpdatesTest, TakesNoPlainExtension) {
  g_browser_core_sync_keep_update = &KeepSyncUpdate;
  Start(syncer::EXTENSIONS);
  EXPECT_FALSE(Takes(Update("plain", Extension())));
  EXPECT_TRUE(Takes(Update("encrypted", Encrypt(Extension()))));
}

TEST(SyncUpdatesRuleTest, KeepsDeletionsAndWhatIsntEncrypted) {
  sync_pb::SyncEntity deletion;
  deletion.set_deleted(true);
  EXPECT_TRUE(KeepSyncUpdate(syncer::BOOKMARKS, true, deletion));

  // A type the account doesn't encrypt, such as its devices' list, comes as it is.
  sync_pb::SyncEntity device;
  device.mutable_specifics()->mutable_device_info()->set_cache_guid("guid");
  EXPECT_TRUE(KeepSyncUpdate(syncer::DEVICE_INFO, false, device));

  sync_pb::SyncEntity plain;
  *plain.mutable_specifics() = Bookmark();
  EXPECT_FALSE(KeepSyncUpdate(syncer::BOOKMARKS, true, plain));
  EXPECT_TRUE(KeepSyncUpdate(syncer::BOOKMARKS, false, plain));

  // Ciphertext with the plain fields beside it, or with no ciphertext in it, isn't what a device
  // writes.
  sync_pb::SyncEntity both;
  *both.mutable_specifics() = Bookmark();
  both.mutable_specifics()->mutable_encrypted()->set_key_name(kKey);
  both.mutable_specifics()->mutable_encrypted()->set_blob("ciphertext");
  EXPECT_FALSE(KeepSyncUpdate(syncer::BOOKMARKS, true, both));
  sync_pb::SyncEntity hollow;
  hollow.mutable_specifics()->mutable_encrypted()->set_key_name(kKey);
  EXPECT_FALSE(KeepSyncUpdate(syncer::BOOKMARKS, true, hollow));

  sync_pb::SyncEntity password;
  password.mutable_specifics()->mutable_password()->mutable_encrypted()->set_key_name(kKey);
  password.mutable_specifics()->mutable_password()->mutable_encrypted()->set_blob("ciphertext");
  EXPECT_TRUE(KeepSyncUpdate(syncer::PASSWORDS, true, password));
  password.mutable_specifics()->mutable_password()->mutable_unencrypted_metadata()->set_url(
      "https://example.test/");
  EXPECT_FALSE(KeepSyncUpdate(syncer::PASSWORDS, true, password));
}

}  // namespace
}  // namespace browser_core
