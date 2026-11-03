#include "browser_core/sync_spaces.h"

#include <algorithm>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/strings/utf_string_conversions.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/uuid.h"
#include "browser_core/favorites.h"
#include "browser_core/hooks.h"
#include "browser_core/profiles.h"
#include "browser_core/session.h"
#include "browser_core/space_default.h"
#include "browser_core/space.h"
#include "browser_core/sync_spaces.pb.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "chrome/browser/sync/data_type_store_service_factory.h"
#include "chrome/common/channel_info.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "components/sync/base/data_type.h"
#include "components/sync/base/deletion_origin.h"
#include "components/sync/base/report_unrecoverable_error.h"
#include "components/sync/base/unique_position.h"
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
#include "components/sync/protocol/unique_position.pb.h"
#include "components/sync/service/data_type_controller.h"
#include "ui/base/page_transition_types.h"

namespace browser_core::sync_spaces {
namespace {

using Entity = sync_spaces_pb::SpaceEntity;

// Long enough that a run of changes, such as a page's title settling as it loads, is one look.
constexpr base::TimeDelta kSettle = base::Seconds(1);

std::string Key(const base::Uuid& id) {
  return id.AsLowercaseString();
}

std::string Bytes(const syncer::UniquePosition& position) {
  std::string bytes;
  position.SerializeToString(&bytes);
  return bytes;
}

syncer::UniquePosition PositionOf(const Entity& entity) {
  sync_pb::UniquePosition proto;
  return proto.ParseFromString(entity.position()) ? syncer::UniquePosition::FromProto(proto)
                                                  : syncer::UniquePosition();
}

std::optional<Entity> Parse(const syncer::EntityData& data) {
  Entity entity;
  if (!data.specifics.has_ilo_space() ||
      !entity.ParseFromString(data.specifics.ilo_space().entity()) ||
      !base::Uuid::ParseLowercase(entity.id()).is_valid()) {
    return std::nullopt;
  }
  return entity;
}

std::unique_ptr<syncer::EntityData> DataOf(const Entity& entity) {
  auto data = std::make_unique<syncer::EntityData>();
  data->specifics.mutable_ilo_space()->set_entity(entity.SerializeAsString());
  data->name = entity.id();
  return data;
}

// The Spaces sync keeps, the user's: an agent's goes with its agent.
bool Synced(const Space* space) {
  return !space->agent() && space->node().id.is_valid();
}

// The device's own first Space, made because a browser needs one, while it gives way to the
// account's (space_default.h): its only one, which the user hasn't made theirs. Else null.
Space* UntouchedDefault() {
  std::vector<Space*> own = Space::All();
  std::erase_if(own, [](Space* space) { return !Synced(space); });
  if (own.size() != 1) {
    return nullptr;
  }
  Space* space = own.front();
  const FirstSpace first = {
      .named = space->touched(),
      .coloured = space->color().has_value() || !space->theme().empty(),
      .today_tabs = space->today().size(),
      .pinned_rows = space->pinned_rows().size(),
      .favorites = Favorites::In(*space).size(),
  };
  return GivesWay(first) ? space : nullptr;
}

// Every Space, pinned tab and folder as sync knows them now, by id.
std::map<std::string, Entity> Current() {
  std::map<std::string, Entity> all;
  for (Space* space : Space::All()) {
    if (!Synced(space)) {
      continue;
    }
    const std::string space_key = Key(space->node().id);
    Entity& entity = all[space_key];
    entity.set_id(space_key);
    entity.set_kind(Entity::SPACE);
    entity.set_position(Bytes(space->node().position));
    entity.set_name(space->name());
    if (space->color()) {
      entity.set_color(*space->color());
    }
    entity.set_icon(space->icon());
    entity.set_theme(space->theme());
    for (bc_id row : space->pinned_rows()) {
      const Space::Node* node = space->NodeOf(row);
      if (!node || !node->id.is_valid()) {
        continue;
      }
      Entity& each = all[Key(node->id)];
      each.set_id(Key(node->id));
      each.set_space(space_key);
      if (const Space::Node* folder = space->NodeOf(space->FolderOf(row))) {
        each.set_parent(Key(folder->id));
      }
      each.set_position(Bytes(node->position));
      if (space->IsFolder(row)) {
        each.set_kind(Entity::FOLDER);
        each.set_name(space->FolderName(row));
        each.set_icon(space->FolderIcon(row));
      } else {
        each.set_kind(Entity::PIN);
        each.set_url(space->PinnedUrl(row).spec());
        if (Tab* tab = Tab::Get(row)) {
          each.set_name(tab->name());
          each.set_title(tab->Title());
          each.set_icon(tab->icon());
        }
      }
    }
  }
  return all;
}

// Where a Space, pinned tab or folder is on this device.
struct Local {
  Space* space = nullptr;
  // The pinned tab or folder, or 0 for the Space itself.
  bc_id row = 0;
};

std::map<std::string, Local> Index() {
  std::map<std::string, Local> index;
  for (Space* space : Space::All()) {
    if (!Synced(space)) {
      continue;
    }
    index[Key(space->node().id)] = {space, 0};
    for (bc_id row : space->pinned_rows()) {
      if (const Space::Node* node = space->NodeOf(row)) {
        index[Key(node->id)] = {space, row};
      }
    }
  }
  return index;
}

class Bridge;
std::map<Profile*, Bridge*>& Bridges();

class Bridge : public syncer::DataTypeSyncBridge, public ProfileObserver {
 public:
  explicit Bridge(Profile* profile)
      : syncer::DataTypeSyncBridge(std::make_unique<syncer::ClientTagBasedDataTypeProcessor>(
            syncer::ILO_SPACE,
            base::BindRepeating(&syncer::ReportUnrecoverableError, chrome::GetChannel()))) {
    profile_observation_.Observe(profile);
    DataTypeStoreServiceFactory::GetForProfile(profile)->GetStoreFactory().Run(
        syncer::ILO_SPACE,
        base::BindOnce(&Bridge::OnStoreCreated, weak_factory_.GetWeakPtr()));
  }

  void Changed() {
    if (store_ && !timer_.IsRunning()) {
      timer_.Start(FROM_HERE, kSettle, base::BindOnce(&Bridge::Look, base::Unretained(this)));
    }
  }

  // syncer::DataTypeSyncBridge:
  std::unique_ptr<syncer::MetadataChangeList> CreateMetadataChangeList() override {
    return std::make_unique<syncer::InMemoryMetadataChangeList>();
  }

  // The first sync of this device's Spaces with the account's: each side keeps its own and gets the
  // other's, as a first sync of bookmarks does, since neither knows which the user meant to keep.
  // But where the account has Spaces, the device's own first one goes if the user never touched it,
  // as Arc's does on a new device, rather than adding an empty Space to every device; its Today
  // tabs move to the account's first Space, so nothing the user had open goes.
  std::optional<syncer::ModelError> MergeFullSyncData(
      std::unique_ptr<syncer::MetadataChangeList> metadata_changes,
      syncer::EntityChangeList changes) override {
    restoring_ = false;
    Space* untouched = UntouchedDefault();
    const bool account_has_spaces =
        std::ranges::any_of(changes, [](const std::unique_ptr<syncer::EntityChange>& change) {
          if (change->type() == syncer::EntityChange::ACTION_DELETE) {
            return false;
          }
          const std::optional<Entity> entity = Parse(change->data());
          return entity && entity->kind() == Entity::SPACE;
        });
    std::optional<syncer::ModelError> error =
        ApplyIncrementalSyncChanges(std::move(metadata_changes), std::move(changes));
    if (!error && untouched && account_has_spaces) {
      Drop(untouched->id());
    }
    return error;
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
    std::vector<std::string> deleted;
    for (const std::unique_ptr<syncer::EntityChange>& change : changes) {
      const std::string& key = change->storage_key();
      if (change->type() == syncer::EntityChange::ACTION_DELETE) {
        known_.erase(key);
        waiting_.erase(key);
        batch->DeleteData(key);
        deleted.push_back(key);
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
    Apply(std::move(put), deleted);
    // Sends what applying them changed here, such as a row given a new place among its siblings.
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

  std::string GetClientTag(const syncer::EntityData& data) const override {
    std::optional<Entity> entity = Parse(data);
    return entity ? entity->id() : std::string();
  }

  std::string GetStorageKey(const syncer::EntityData& data) const override {
    return GetClientTag(data);
  }

  // Signed out: the Spaces stay on the device, which forgets what the account had.
  void ApplyDisableSyncChanges(
      std::unique_ptr<syncer::MetadataChangeList> metadata_changes) override {
    timer_.Stop();
    known_.clear();
    waiting_.clear();
    restoring_ = false;
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

  // Sends what changed here since sync last heard: each Space, pinned tab and folder that's new or
  // different, and each that's gone.
  void Look() {
    // Before the session is back there are no Spaces, which isn't the user deleting them all.
    if (!store_ || !change_processor()->IsTrackingMetadata() || Space::All().empty()) {
      return;
    }
    // The store writes as each change goes; the session a while after. So the first look after a
    // launch puts back what the store has and the session missed, such as a Space that came from
    // another device just before the browser went, rather than deleting it from the account.
    if (restoring_) {
      restoring_ = false;
      const std::map<std::string, Entity> current = Current();
      std::vector<Entity> missed;
      for (const auto& [key, entity] : known_) {
        if (!current.contains(key)) {
          missed.push_back(entity);
        }
      }
      Apply(std::move(missed), {});
    }
    const std::map<std::string, Entity> current = Current();
    std::unique_ptr<syncer::DataTypeStore::WriteBatch> batch = store_->CreateWriteBatch();
    for (const auto& [key, entity] : current) {
      auto it = known_.find(key);
      if (it != known_.end() && it->second.SerializeAsString() == entity.SerializeAsString()) {
        continue;
      }
      change_processor()->Put(key, DataOf(entity), batch->GetMetadataChangeList());
      batch->WriteData(key, entity.SerializeAsString());
      known_[key] = entity;
    }
    for (auto it = known_.begin(); it != known_.end();) {
      // One waiting for its Space or folder to come isn't here yet, rather than gone.
      if (current.contains(it->first) || waiting_.contains(it->first)) {
        ++it;
        continue;
      }
      change_processor()->Delete(it->first, syncer::DeletionOrigin::Unspecified(),
                                 batch->GetMetadataChangeList());
      batch->DeleteData(it->first);
      it = known_.erase(it);
    }
    store_->CommitWriteBatch(std::move(batch),
                             base::BindOnce(&Bridge::OnCommit, weak_factory_.GetWeakPtr()));
  }

  // Makes the sidebar match what came from the account: |put| new or changed, |deleted| gone.
  void Apply(std::vector<Entity> put, const std::vector<std::string>& deleted) {
    std::map<std::string, Local> index = Index();
    for (const std::string& key : deleted) {
      auto it = index.find(key);
      if (it == index.end()) {
        continue;
      }
      Remove(it->second);
      index = Index();
    }
    // Those still waiting for their Space or folder may be able to go in now.
    for (const std::string& key : waiting_) {
      if (auto it = known_.find(key); it != known_.end()) {
        put.push_back(it->second);
      }
    }
    waiting_.clear();
    // Spaces first, in their order, then each row once its Space and folder are here, so each goes
    // in where its place puts it among its siblings.
    std::ranges::stable_sort(put, [](const Entity& a, const Entity& b) {
      if ((a.kind() == Entity::SPACE) != (b.kind() == Entity::SPACE)) {
        return a.kind() == Entity::SPACE;
      }
      return PositionOf(a).LessThan(PositionOf(b));
    });
    for (bool progress = true; progress && !put.empty();) {
      progress = false;
      for (auto it = put.begin(); it != put.end();) {
        const bool ready =
            it->kind() == Entity::SPACE ||
            (index.contains(it->space()) && (it->parent().empty() || index.contains(it->parent())));
        if (!ready) {
          ++it;
          continue;
        }
        Put(*it, index);
        index = Index();
        it = put.erase(it);
        progress = true;
      }
    }
    // A row whose folder never came goes in its Space, outside any folder; one whose Space never
    // came waits for it.
    for (Entity& entity : put) {
      if (index.contains(entity.space())) {
        entity.clear_parent();
        Put(entity, index);
        index = Index();
      } else {
        waiting_.insert(entity.id());
      }
    }
  }

  // Deletes |space|, which sync never had, once its Today tabs are in the first of the Spaces sync
  // has, in their order.
  void Drop(bc_id space) {
    Space* dropped = Space::FromId(space);
    std::vector<Space*> others = Space::All();
    std::erase_if(others, [&](Space* each) { return each == dropped || !Synced(each); });
    if (!dropped || others.empty()) {
      return;
    }
    // Each goes to the top of the Today tabs there, so the bottom one goes first.
    const std::vector<bc_id> today = dropped->today();
    for (auto it = today.rbegin(); it != today.rend(); ++it) {
      Window::MoveToSpace(*it, others.front()->id());
    }
    Window::DeleteSpace(space);
  }

  void Remove(const Local& local) {
    if (!local.row) {
      Window::DeleteSpace(local.space->id());
    } else if (local.space->IsFolder(local.row)) {
      // What was in it is deleted, or moves, on its own.
      local.space->DeleteFolder(local.row);
    } else {
      Window::CloseTab(local.row);
    }
  }

  void Put(const Entity& entity, const std::map<std::string, Local>& index) {
    const Space::Node node{base::Uuid::ParseLowercase(entity.id()), PositionOf(entity)};
    auto here = index.find(entity.id());
    if (entity.kind() == Entity::SPACE) {
      Space* space = here != index.end() ? here->second.space : nullptr;
      if (!space) {
        space = Space::Create(entity.name());
        space->SetProfile(DirOf(StartProfile()));
      }
      space->SetName(entity.name());
      space->SetColor(entity.has_color() ? std::optional(entity.color()) : std::nullopt);
      space->SetIcon(entity.icon());
      space->SetTheme(entity.theme());
      space->RestoreNodes(node, {});
      return;
    }
    Space* space = index.at(entity.space()).space;
    const bc_id folder = entity.parent().empty() ? 0 : index.at(entity.parent()).row;
    // A row moved to another Space has a new id there, so one found in another Space was deleted
    // and made again with its id, which only happens to an id the core made up itself.
    if (here != index.end() && here->second.space != space) {
      Remove(here->second);
      here = index.end();
    }
    bc_id row = here != index.end() ? here->second.row : 0;
    // Where its place puts it among the rows directly in its folder, without it.
    size_t at = 0;
    for (bc_id sibling : space->pinned_rows()) {
      const Space::Node* each = space->NodeOf(sibling);
      if (sibling != row && space->FolderOf(sibling) == folder && each &&
          each->position.LessThan(node.position)) {
        ++at;
      }
    }
    const auto no_url = [](bc_id) { return GURL(); };
    const GURL url(entity.url());
    if (!row && entity.kind() == Entity::FOLDER) {
      row = space->AddFolder(entity.name(), folder, at);
      space->SetFolderIcon(row, entity.icon());
    } else if (!row) {
      // Without its page until it's first shown, as a pinned tab the session brings back.
      SavedTab saved;
      sessions::SerializedNavigationEntry& page = saved.history.emplace_back();
      page.set_index(0);
      page.set_virtual_url(url);
      page.set_original_request_url(url);
      page.set_title(base::UTF8ToUTF16(entity.title()));
      page.set_transition_type(ui::PAGE_TRANSITION_AUTO_BOOKMARK);
      page.set_timestamp(base::Time::Now());
      saved.name = entity.name();
      saved.icon = entity.icon();
      row = Tab::Restore(ProfileFor(space->profile()), saved);
      if (!row) {
        return;
      }
      space->AddPinned(row, folder, at, url);
    } else {
      if (space->IsFolder(row)) {
        space->RenameFolder(row, entity.name());
        space->SetFolderIcon(row, entity.icon());
      } else if (url != space->PinnedUrl(row)) {
        space->SetPinnedUrl(row, url);
      }
      const Space::Node* was = space->NodeOf(row);
      if (space->FolderOf(row) != folder || !was || !was->position.Equals(node.position)) {
        space->Move(row, /*pinned=*/true, folder, at, no_url);
      }
    }
    space->RestoreNodes(space->node(), {{row, node}});
  }

  base::ScopedObservation<Profile, ProfileObserver> profile_observation_{this};
  std::unique_ptr<syncer::DataTypeStore> store_;
  // What sync last had of each Space, pinned tab and folder, as the store keeps it, by id.
  std::map<std::string, Entity> known_;
  // Those that came before their Space.
  std::set<std::string> waiting_;
  // Until the first look after the store is read.
  bool restoring_ = true;
  base::OneShotTimer timer_;
  base::WeakPtrFactory<Bridge> weak_factory_{this};
};

// Each profile's whose sync was made, as Chromium makes it while the profile loads, before the
// core knows its start profile. Only the start profile's sync has the browser's account, so only
// its bridge ever tracks the account's Spaces; the others wait, doing nothing. Never destroyed:
// sync's controller calls its bridge until the profile's sync has shut down, after the profile
// says it's going.
std::map<Profile*, Bridge*>& Bridges() {
  static base::NoDestructor<std::map<Profile*, Bridge*>> bridges;
  return *bridges;
}

}  // namespace

void Install() {
  g_browser_core_sync_controllers = [](Profile* profile, syncer::SyncService* sync_service) {
    std::vector<std::unique_ptr<syncer::DataTypeController>> controllers;
    Bridge*& bridge = Bridges()[profile];
    if (!bridge) {
      bridge = new Bridge(profile);
    }
    syncer::DataTypeControllerDelegate* delegate =
        bridge->change_processor()->GetControllerDelegate().get();
    // The same in transport mode, as the account is always signed in that way.
    controllers.push_back(std::make_unique<syncer::DataTypeController>(
        syncer::ILO_SPACE,
        std::make_unique<syncer::ForwardingDataTypeControllerDelegate>(delegate),
        std::make_unique<syncer::ForwardingDataTypeControllerDelegate>(delegate)));
    return controllers;
  };
}

void Changed() {
  for (const auto& [profile, bridge] : Bridges()) {
    bridge->Changed();
  }
}

}  // namespace browser_core::sync_spaces
