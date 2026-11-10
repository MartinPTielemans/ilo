#include "browser_core/sync_settings.h"

#include <map>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "browser_core/ad_block.h"
#include "browser_core/auto_archive.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "browser_core/session.h"
#include "browser_core/sync_guard.h"
#include "browser_core/sync_settings.pb.h"
#include "browser_core/webrtc.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "chrome/browser/sync/data_type_store_service_factory.h"
#include "chrome/common/channel_info.h"
#include "components/sync/base/data_type.h"
#include "components/sync/base/report_unrecoverable_error.h"
#include "components/sync/model/client_tag_based_data_type_processor.h"
#include "components/sync/model/data_type_store.h"
#include "components/sync/model/data_type_store_service.h"
#include "components/sync/model/data_type_sync_bridge.h"
#include "components/sync/model/entity_change.h"
#include "components/sync/model/forwarding_data_type_controller_delegate.h"
#include "components/sync/model/in_memory_metadata_change_list.h"
#include "components/sync/model/metadata_batch.h"
#include "components/sync/model/mutable_data_batch.h"
#include "components/sync/protocol/entity_data.h"
#include "components/sync/service/data_type_controller.h"

namespace browser_core::sync_settings {
namespace {

using Entity = sync_settings_pb::SettingEntity;

// As the Spaces' bridge waits, so a run of changes is one look.
constexpr base::TimeDelta kSettle = base::Seconds(1);

// The switches that follow the account, by their entities' names, which never change.
struct Switch {
  const char* name;
  bool Settings::* field;
};
constexpr Switch kSwitches[] = {
    {"little_windows", &Settings::little_windows},
    {"warn_before_quitting", &Settings::warn_before_quitting},
    {"restore_windows", &Settings::restore_windows},
    {"warn_moving_profiles", &Settings::warn_moving_profiles},
    {"ad_blocking", &Settings::ad_blocking},
    {"cookie_notices_hidden", &Settings::cookie_notices_hidden},
    {"relay_calls", &Settings::relay_calls},
    {"link_previews", &Settings::link_previews},
    {"auto_picture_in_picture", &Settings::auto_picture_in_picture},
    {"ask_where_to_save", &Settings::ask_where_to_save},
    {"notification_prompts", &Settings::notification_prompts},
    {"space_header", &Settings::space_header},
};
// And the numbers: seconds, bc_hover_kinds' four bits and bc_appearance's choice.
constexpr char kLittleArchiveAfter[] = "little_archive_after";
constexpr char kHoverKinds[] = "hover_kinds";
constexpr char kAppearance[] = "appearance";

// Whether the session has read the settings: before then, the device's are the defaults.
bool g_read = false;

// Each setting that follows the account, as its entity, by name.
std::map<std::string, Entity> Current() {
  const Settings& settings = UserSettings();
  std::map<std::string, Entity> all;
  const auto add = [&all](const char* name) -> Entity& {
    Entity& entity = all[name];
    entity.set_name(name);
    return entity;
  };
  for (const Switch& each : kSwitches) {
    add(each.name).set_on(settings.*each.field);
  }
  add(kLittleArchiveAfter).set_number(settings.little_archive_after.InSecondsF());
  add(kHoverKinds).set_number(settings.hover_kinds);
  add(kAppearance).set_number(settings.appearance);
  return all;
}

// Sets |entity|'s setting, if it's one this version knows, with a value it takes.
void Set(const Entity& entity, Settings& settings) {
  for (const Switch& each : kSwitches) {
    if (entity.name() == each.name && entity.has_on()) {
      settings.*each.field = entity.on();
    }
  }
  const double number = entity.number();
  if (entity.name() == kLittleArchiveAfter && number > 0) {
    settings.little_archive_after = base::Seconds(number);
  }
  if (entity.name() == kHoverKinds && entity.has_number() && number >= 0 && number <= 15 &&
      number == static_cast<int>(number)) {
    settings.hover_kinds = static_cast<int>(number);
  }
  if (entity.name() == kAppearance && entity.has_number() &&
      (number == 0 || number == 1 || number == 2)) {
    settings.appearance = static_cast<int>(number);
  }
}

// Gives the device the settings that came from the account, and does what each changed asks, as
// the C API's setters do.
void Apply(const std::vector<Entity>& entities) {
  const std::map<std::string, Entity> before = Current();
  for (const Entity& entity : entities) {
    Set(entity, UserSettings());
  }
  const std::map<std::string, Entity> after = Current();
  const auto changed = [&](const std::string& name) {
    return before.at(name).SerializeAsString() != after.at(name).SerializeAsString();
  };
  bool any = false;
  for (const auto& [name, entity] : after) {
    if (!changed(name)) {
      continue;
    }
    any = true;
    // check.sh reads it.
    if (entity.has_on()) {
      LOG(ERROR) << "browser: sync settings took " << name << (entity.on() ? " on" : " off");
    } else {
      LOG(ERROR) << "browser: sync settings took " << name << " " << entity.number();
    }
  }
  if (!any) {
    return;
  }
  if (changed("ad_blocking")) {
    ApplyAdBlocking();
  }
  if (changed("cookie_notices_hidden")) {
    ApplyCookieNotices();
  }
  if (changed("relay_calls")) {
    ApplyWebRtc();
  }
  if (changed("auto_picture_in_picture")) {
    ApplyAutoPictureInPicture(/*changed=*/true);
  }
  if (changed("ask_where_to_save")) {
    ApplyEveryday(Everyday::kAskWhereToSave);
  }
  if (changed("notification_prompts")) {
    ApplyEveryday(Everyday::kNotificationPrompts);
  }
  if (changed(kLittleArchiveAfter)) {
    ScheduleAutoArchive();
  }
  if (changed(kAppearance)) {
    ApplyAppearance();
  }
  SessionChanged();
  if (Host().settings_synced) {
    Host().settings_synced(Host().context);
  }
}

std::optional<Entity> Parse(const syncer::EntityData& data) {
  Entity entity;
  if (!data.specifics.has_ilo_setting() ||
      !entity.ParseFromString(data.specifics.ilo_setting().entity()) || entity.name().empty()) {
    return std::nullopt;
  }
  return entity;
}

std::unique_ptr<syncer::EntityData> DataOf(const Entity& entity) {
  auto data = std::make_unique<syncer::EntityData>();
  data->specifics.mutable_ilo_setting()->set_entity(entity.SerializeAsString());
  data->name = entity.name();
  return data;
}

class Bridge;
std::map<Profile*, Bridge*>& Bridges();

class Bridge : public syncer::DataTypeSyncBridge, public ProfileObserver {
 public:
  explicit Bridge(Profile* profile)
      : syncer::DataTypeSyncBridge(std::make_unique<syncer::ClientTagBasedDataTypeProcessor>(
            syncer::ILO_SETTING,
            base::BindRepeating(&syncer::ReportUnrecoverableError, chrome::GetChannel()))) {
    profile_observation_.Observe(profile);
    DataTypeStoreServiceFactory::GetForProfile(profile)->GetStoreFactory().Run(
        syncer::ILO_SETTING,
        base::BindOnce(&Bridge::OnStoreCreated, weak_factory_.GetWeakPtr()));
  }

  void Changed() {
    if (store_ && !timer_.IsRunning()) {
      timer_.Start(FROM_HERE, kSettle, base::BindOnce(&Bridge::Look, base::Unretained(this)));
    }
  }

  // The session has read the settings: those that came before it did go in now.
  void Read() {
    std::vector<Entity> came;
    for (const std::string& name : early_) {
      if (auto it = known_.find(name); it != known_.end()) {
        came.push_back(it->second);
      }
    }
    early_.clear();
    Apply(came);
    Changed();
  }

  // syncer::DataTypeSyncBridge:
  std::unique_ptr<syncer::MetadataChangeList> CreateMetadataChangeList() override {
    return std::make_unique<syncer::InMemoryMetadataChangeList>();
  }

  // A device turning Settings on takes the account's, as the user chose them on the devices
  // already syncing, and gives it those the account hasn't, as on its first device.
  std::optional<syncer::ModelError> MergeFullSyncData(
      std::unique_ptr<syncer::MetadataChangeList> metadata_changes,
      syncer::EntityChangeList changes) override {
    return ApplyIncrementalSyncChanges(std::move(metadata_changes), std::move(changes));
  }

  std::optional<syncer::ModelError> ApplyIncrementalSyncChanges(
      std::unique_ptr<syncer::MetadataChangeList> metadata_changes,
      syncer::EntityChangeList changes) override {
    if (!store_) {
      return std::nullopt;
    }
    std::unique_ptr<syncer::DataTypeStore::WriteBatch> batch =
        store_->CreateWriteBatch(std::move(metadata_changes));
    std::vector<Entity> put;
    for (const std::unique_ptr<syncer::EntityChange>& change : changes) {
      const std::string& key = change->storage_key();
      // A setting gone from the account stays as it is here, and goes up again.
      if (change->type() == syncer::EntityChange::ACTION_DELETE) {
        known_.erase(key);
        batch->DeleteData(key);
        continue;
      }
      std::optional<Entity> entity = Parse(change->data());
      if (!entity) {
        continue;
      }
      known_[key] = *entity;
      batch->WriteData(key, entity->SerializeAsString());
      put.push_back(*std::move(entity));
    }
    store_->CommitWriteBatch(std::move(batch),
                             base::BindOnce(&Bridge::OnCommit, weak_factory_.GetWeakPtr()));
    if (g_read) {
      Apply(put);
    } else {
      for (const Entity& entity : put) {
        early_.insert(entity.name());
      }
    }
    // Sends what the device has that the account hasn't, or a value this device doesn't take.
    Changed();
    return std::nullopt;
  }

  std::unique_ptr<syncer::DataBatch> GetDataForCommit(StorageKeyList keys) override {
    auto batch = std::make_unique<syncer::MutableDataBatch>();
    for (const std::string& key : keys) {
      if (auto it = known_.find(key); it != known_.end()) {
        batch->Put(key, DataOf(it->second));
      }
    }
    return batch;
  }

  std::unique_ptr<syncer::DataBatch> GetAllDataForDebugging() override {
    auto batch = std::make_unique<syncer::MutableDataBatch>();
    for (const auto& [key, entity] : known_) {
      batch->Put(key, DataOf(entity));
    }
    return batch;
  }

  // Keyed with the account's secret, as a password's is (SyncPasswordTag), so the server can't
  // tell which setting changed, or when.
  std::string GetClientTag(const syncer::EntityData& data) const override {
    std::optional<Entity> entity = Parse(data);
    return entity ? SyncPasswordTag("ilo setting " + entity->name()) : std::string();
  }

  std::string GetStorageKey(const syncer::EntityData& data) const override {
    std::optional<Entity> entity = Parse(data);
    return entity ? entity->name() : std::string();
  }

  // Turned off or signed out: the settings stay as they are, and the device forgets the account's.
  void ApplyDisableSyncChanges(
      std::unique_ptr<syncer::MetadataChangeList> metadata_changes) override {
    timer_.Stop();
    known_.clear();
    early_.clear();
    if (!store_) {
      return;
    }
    store_->DeleteAllDataAndMetadata(std::move(metadata_changes), base::DoNothing());
  }

  bool IsEntityDataValid(const syncer::EntityData& data) const override {
    return Parse(data).has_value();
  }

  // The specifics' one field is the core's own message, which this version reads all of.
  sync_pb::EntitySpecifics TrimAllSupportedFieldsFromRemoteSpecifics(
      const sync_pb::EntitySpecifics& specifics) const override {
    return sync_pb::EntitySpecifics();
  }

  // ProfileObserver: the browser is quitting, and nothing here changes any more.
  void OnProfileWillBeDestroyed(Profile* profile) override {
    timer_.Stop();
    profile_observation_.Reset();
    store_.reset();
    Bridges().erase(profile);
  }

 private:
  void OnStoreCreated(const std::optional<syncer::ModelError>& error,
                      std::unique_ptr<syncer::DataTypeStore> store) {
    if (error) {
      change_processor()->ReportError(*error);
      return;
    }
    store_ = std::move(store);
    store_->ReadAllData(base::BindOnce(&Bridge::OnReadAllData, weak_factory_.GetWeakPtr()));
  }

  void OnReadAllData(const std::optional<syncer::ModelError>& error,
                     std::unique_ptr<syncer::DataTypeStore::RecordList> records) {
    if (error) {
      change_processor()->ReportError(*error);
      return;
    }
    for (const syncer::DataTypeStore::Record& record : *records) {
      Entity entity;
      if (entity.ParseFromString(record.value)) {
        known_[record.id] = std::move(entity);
      }
    }
    store_->ReadAllMetadata(base::BindOnce(&Bridge::OnReadAllMetadata, weak_factory_.GetWeakPtr()));
  }

  void OnReadAllMetadata(const std::optional<syncer::ModelError>& error,
                         std::unique_ptr<syncer::MetadataBatch> metadata) {
    if (error) {
      change_processor()->ReportError(*error);
      return;
    }
    change_processor()->ModelReadyToSync(std::move(metadata));
    Changed();
  }

  void OnCommit(const std::optional<syncer::ModelError>& error) {
    if (error) {
      change_processor()->ReportError(*error);
    }
  }

  // Sends each setting that's new or different here since sync last heard. The session saves as
  // each changes and once more at a quit, so after a launch the device's settings are the newest
  // it had: one changed just before a quit, too soon for this to send it, goes up now.
  void Look() {
    if (!g_read || !store_ || !change_processor()->IsTrackingMetadata()) {
      return;
    }
    std::unique_ptr<syncer::DataTypeStore::WriteBatch> batch = store_->CreateWriteBatch();
    for (const auto& [name, entity] : Current()) {
      auto it = known_.find(name);
      if (it != known_.end() && it->second.SerializeAsString() == entity.SerializeAsString()) {
        continue;
      }
      change_processor()->Put(name, DataOf(entity), batch->GetMetadataChangeList());
      batch->WriteData(name, entity.SerializeAsString());
      known_[name] = entity;
    }
    store_->CommitWriteBatch(std::move(batch),
                             base::BindOnce(&Bridge::OnCommit, weak_factory_.GetWeakPtr()));
  }

  base::ScopedObservation<Profile, ProfileObserver> profile_observation_{this};
  std::unique_ptr<syncer::DataTypeStore> store_;
  // What sync last had of each setting, as the store keeps it, by name, with those this version
  // doesn't know.
  std::map<std::string, Entity> known_;
  // Those that came before the session read the settings.
  std::set<std::string> early_;
  base::OneShotTimer timer_;
  base::WeakPtrFactory<Bridge> weak_factory_{this};
};

// As the Spaces' bridges are kept (sync_spaces.cc): only the start profile's ever syncs, and none
// is destroyed, as sync's controller calls it until the profile's sync has shut down.
std::map<Profile*, Bridge*>& Bridges() {
  static base::NoDestructor<std::map<Profile*, Bridge*>> bridges;
  return *bridges;
}

}  // namespace

std::unique_ptr<syncer::DataTypeController> Controller(Profile* profile) {
  Bridge*& bridge = Bridges()[profile];
  if (!bridge) {
    bridge = new Bridge(profile);
  }
  syncer::DataTypeControllerDelegate* delegate =
      bridge->change_processor()->GetControllerDelegate().get();
  // The same in transport mode, as the account is always signed in that way.
  return std::make_unique<syncer::DataTypeController>(
      syncer::ILO_SETTING,
      std::make_unique<syncer::ForwardingDataTypeControllerDelegate>(delegate),
      std::make_unique<syncer::ForwardingDataTypeControllerDelegate>(delegate));
}

void Read() {
  g_read = true;
  for (const auto& [profile, bridge] : Bridges()) {
    bridge->Read();
  }
}

void Changed() {
  for (const auto& [profile, bridge] : Bridges()) {
    bridge->Changed();
  }
}

}  // namespace browser_core::sync_settings
