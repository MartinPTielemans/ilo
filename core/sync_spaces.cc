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
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/uuid.h"
#include "browser_core/favorites.h"
#include "browser_core/profiles.h"
#include "browser_core/session.h"
#include "browser_core/space_default.h"
#include "browser_core/space.h"
#include "browser_core/sync_other_tabs.h"
#include "browser_core/sync_spaces.pb.h"
#include "browser_core/sync_spaces_newer.h"
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
#include "url/gurl.h"
#include "url/url_constants.h"

namespace browser_core::sync_spaces {
namespace {

using Entity = sync_spaces_pb::SpaceEntity;

// Long enough that a run of changes, such as ten links opened at once or a redirect chain, is one
// look and one commit, and short enough that the other devices have it within about 2 s.
constexpr base::TimeDelta kSettle = base::Seconds(1);

// How far on a Today tab's last showing must be from what the account has before it's sent: about
// once an hour a tab, never on every switch, close enough for auto-archive's 6 hours or more.
constexpr int64_t kSeenStep = 60 * 60;

std::string Key(const base::Uuid& id) {
  return id.AsLowercaseString();
}

// Before a Today tab's node id in its entity's id and client tag. ilo 0.1.13 and older take a kind
// they don't know for SPACE, proto2's default, so they'd make a Space of each Today tab and write
// it back as one over every device's copy. Their Parse takes only a bare lowercase UUID, so they
// drop these instead (#2108).
constexpr char kTodayPrefix[] = "today:";

std::string TodayKey(const base::Uuid& id) {
  return kTodayPrefix + Key(id);
}

// The node id in |key|, an entity's id: a Today tab's after its prefix, which Index() goes by, as
// a tab keeps its node as it's pinned or unpinned.
std::string NodeKey(const std::string& key) {
  return base::StartsWith(key, kTodayPrefix) ? key.substr(sizeof(kTodayPrefix) - 1) : key;
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
      base::StartsWith(entity.id(), kTodayPrefix) != (entity.kind() == Entity::TODAY) ||
      !base::Uuid::ParseLowercase(NodeKey(entity.id())).is_valid()) {
    return std::nullopt;
  }
  return entity;
}

// A change of what a newer ilo wrote.
bool Newer(const std::unique_ptr<syncer::EntityChange>& change) {
  if (change->type() == syncer::EntityChange::ACTION_DELETE) {
    return false;
  }
  const std::optional<Entity> entity = Parse(change->data());
  return entity && WrittenByNewer(*entity);
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

// Whether a Today tab at |url| syncs its page: not one that can't open on another computer, or that
// holds the page itself, such as a data: URL.
bool Syncs(const GURL& url) {
  return url.is_valid() && !url.SchemeIs(url::kDataScheme) && !url.SchemeIsBlob() &&
         !url.SchemeIs(url::kJavaScriptScheme) && !url.SchemeIsFileSystem() &&
         !url.SchemeIs(url::kAboutScheme);
}

// When |row|, a Today tab, was last shown here, in seconds: now while a window shows it.
int64_t SeenHere(bc_id row, const Tab& tab) {
  return (Window::Showing(row) ? base::Time::Now() : tab.last_seen()).ToTimeT();
}

// Every Space, pinned tab, folder and Today tab as sync knows them now, by id. A Today tab's last
// showing is |known|'s unless it's an hour on from it, and one with no page that syncs, as a tab
// yet to start loading or one on a data: URL, is as |known| has it, if at all.
std::map<std::string, Entity> Current(const std::map<std::string, Entity>& known) {
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
    for (bc_id row : space->today()) {
      const Space::Node* node = space->NodeOf(row);
      Tab* tab = Tab::Get(row);
      if (!node || !node->id.is_valid() || !tab) {
        continue;
      }
      const std::string key = TodayKey(node->id);
      auto was = known.find(key);
      const GURL url = tab->Url();
      if (!Syncs(url)) {
        if (was != known.end()) {
          all[key] = was->second;
        }
        continue;
      }
      Entity& each = all[key];
      each.set_id(key);
      each.set_kind(Entity::TODAY);
      each.set_space(space_key);
      each.set_position(Bytes(node->position));
      each.set_url(url.spec());
      each.set_name(tab->name());
      each.set_title(tab->Title());
      each.set_icon(tab->icon());
      const int64_t here = SeenHere(row, *tab);
      const bool sent = was != known.end() && was->second.has_seen();
      each.set_seen(sent && here < was->second.seen() + kSeenStep ? was->second.seen() : here);
    }
  }
  return all;
}

// Whether |row| is one of |space|'s Today tabs.
bool IsToday(const Space& space, bc_id row) {
  return row && std::ranges::contains(space.today(), row);
}

// Whether |a| and |b| differ only in their title, which rides along with the next change rather
// than being sent alone, as a page's title changes as it loads and after.
bool OnlyTitle(const Entity& a, const Entity& b) {
  Entity a_untitled = a;
  Entity b_untitled = b;
  a_untitled.clear_title();
  b_untitled.clear_title();
  return a_untitled.SerializeAsString() == b_untitled.SerializeAsString();
}

// Where a Space, pinned tab, folder or Today tab is on this device.
struct Local {
  Space* space = nullptr;
  // The pinned tab, folder or Today tab, or 0 for the Space itself.
  bc_id row = 0;
};

std::map<std::string, Local> Index() {
  std::map<std::string, Local> index;
  for (Space* space : Space::All()) {
    if (!Synced(space)) {
      continue;
    }
    index[Key(space->node().id)] = {space, 0};
    for (const std::vector<bc_id>* rows : {&space->pinned_rows(), &space->today()}) {
      for (bc_id row : *rows) {
        if (const Space::Node* node = space->NodeOf(row)) {
          index[Key(node->id)] = {space, row};
        }
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
    if (store_ && !paused_ && !timer_.IsRunning()) {
      timer_.Start(FROM_HERE, kSettle, base::BindOnce(&Bridge::Look, base::Unretained(this)));
    }
  }

  bool paused() const { return paused_; }

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
    if (!error && !paused_ && untouched && account_has_spaces) {
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
    // What a newer ilo wrote can't go in the sidebar, or be written again from it, without losing
    // what this version can't read, so the Spaces stop syncing here until ilo is updated. The
    // batch's metadata isn't kept, so the account sends it again to the updated browser.
    if (paused_ || std::ranges::any_of(changes, Newer)) {
      if (!paused_) {
        LOG(ERROR) << "browser: sync spaces paused, as a newer ilo wrote them";
      }
      paused_ = true;
      timer_.Stop();
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
    pages_.clear();
    restoring_ = false;
    paused_ = false;
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
    if (!store_ || paused_ || !change_processor()->IsTrackingMetadata() || Space::All().empty()) {
      return;
    }
    // The store writes as each change goes; the session a while after. So the first look after a
    // launch puts back what the store has and the session missed, such as a Space that came from
    // another device just before the browser went, rather than deleting it from the account.
    if (restoring_) {
      restoring_ = false;
      const std::map<std::string, Entity> current = Current(known_);
      std::vector<Entity> missed;
      for (const auto& [key, entity] : known_) {
        if (!current.contains(key)) {
          missed.push_back(entity);
        }
      }
      Apply(std::move(missed), {});
    }
    std::map<std::string, Entity> current = Current(known_);
    std::unique_ptr<syncer::DataTypeStore::WriteBatch> batch = store_->CreateWriteBatch();
    std::map<std::string, std::string> pages;
    size_t sent = 0;
    size_t gone = 0;
    size_t bytes = 0;
    for (auto& [key, entity] : current) {
      auto it = known_.find(key);
      if (entity.kind() == Entity::TODAY) {
        // Its page is sent only once it navigated here. One another device's navigation reached
        // while it was shown here keeps its page, unloaded nowhere, and isn't sent back.
        const std::string here = entity.url();
        auto was = pages_.find(key);
        if (it != known_.end() && it->second.kind() == Entity::TODAY &&
            (was == pages_.end() || was->second == here)) {
          entity.set_url(it->second.url());
        }
        pages[key] = here;
        if (it != known_.end() && OnlyTitle(it->second, entity)) {
          continue;
        }
      }
      if (it != known_.end() && it->second.SerializeAsString() == entity.SerializeAsString()) {
        continue;
      }
      change_processor()->Put(key, DataOf(entity), batch->GetMetadataChangeList());
      batch->WriteData(key, entity.SerializeAsString());
      known_[key] = entity;
      ++sent;
      bytes += entity.ByteSizeLong();
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
      ++gone;
    }
    store_->CommitWriteBatch(std::move(batch),
                             base::BindOnce(&Bridge::OnCommit, weak_factory_.GetWeakPtr()));
    pages_ = std::move(pages);
    if (sent || gone) {
      // check.sh counts these lines, for what a busy session sends.
      LOG(ERROR) << "browser: sync spaces sent " << sent << " deleted " << gone << " bytes "
                 << bytes;
    }
  }

  // Makes the sidebar match what came from the account: |put| new or changed, |deleted| gone.
  void Apply(std::vector<Entity> put, const std::vector<std::string>& deleted) {
    std::map<std::string, Local> index = Index();
    // A tab pinned or unpinned on another device is deleted under the one id and put under the
    // other, so it moves rather than going: with the put in this batch, or the row here already the
    // other kind, as the put came first.
    std::set<std::string> putting;
    for (const Entity& entity : put) {
      putting.insert(NodeKey(entity.id()));
    }
    for (const std::string& key : deleted) {
      auto it = index.find(NodeKey(key));
      if (it == index.end() || (it->second.row && putting.contains(it->first)) ||
          (it->second.row && IsToday(*it->second.space, it->second.row) != (key != NodeKey(key)))) {
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
    // A tab now in a Space here leaves the command bar's list of another computer's.
    RefreshOtherTabs();
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
      // check.sh waits for this line.
      LOG(ERROR) << "browser: synced close " << local.row;
      Window::CloseTab(local.row);
    }
  }

  void Put(const Entity& entity, const std::map<std::string, Local>& index) {
    const std::string node_key = NodeKey(entity.id());
    const Space::Node node{base::Uuid::ParseLowercase(node_key), PositionOf(entity)};
    auto here = index.find(node_key);
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
    // A Today tab moved to another Space keeps its id, so it moves here too, as it did there: into
    // another profile's Space as a new tab of that profile's, which takes the id.
    if (here != index.end() && here->second.space != space && entity.kind() == Entity::TODAY &&
        IsToday(*here->second.space, here->second.row)) {
      Window::MoveToSpace(here->second.row, space->id());
      const std::map<std::string, Local> moved = Index();
      if (auto now = moved.find(node_key); now != moved.end() && now->second.space == space) {
        PutToday(entity, space, now->second.row);
        return;
      }
    }
    // A pinned tab or folder moved to another Space has a new id there, so one found in another
    // Space was deleted and made again with its id, which only happens to an id the core made up
    // itself.
    if (here != index.end() && here->second.space != space) {
      Remove(here->second);
      here = index.end();
    }
    bc_id row = here != index.end() ? here->second.row : 0;
    if (entity.kind() == Entity::TODAY) {
      PutToday(entity, space, row);
      return;
    }
    // Where its place puts it among the rows directly in its folder, without it.
    size_t at = 0;
    for (bc_id sibling : space->pinned_rows()) {
      const Space::Node* each = space->NodeOf(sibling);
      if (sibling != row && space->FolderOf(sibling) == folder && each &&
          each->position.LessThan(node.position)) {
        ++at;
      }
    }
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
      // In the Space's profile, waiting for it if it's still loading.
      row = Tab::Restore(space->profile(), saved);
      if (!row) {
        return;
      }
      space->AddPinned(row, folder, at, url);
    } else {
      const Space::Node* was = space->NodeOf(row);
      // A Today tab pinned on another device is pinned here, at the URL it was pinned at there.
      const bool today = IsToday(*space, row);
      if (today || space->FolderOf(row) != folder || !was || !was->position.Equals(node.position)) {
        space->Move(row, /*pinned=*/true, folder, at, [&](bc_id) { return url; });
      }
      if (space->IsFolder(row)) {
        space->RenameFolder(row, entity.name());
        space->SetFolderIcon(row, entity.icon());
      } else if (url != space->PinnedUrl(row)) {
        space->SetPinnedUrl(row, url);
      }
    }
    space->RestoreNodes(space->node(), {{row, node}});
  }

  // Puts the Today tab |entity| in |space| at its place, as |row| if it's here already. A new one
  // has no page until it's clicked, as one the session brings back, so sync never loads a page. One
  // another device navigated goes there without its page too, unless a window here shows it, which
  // keeps its page; and its last showing is the account's if that's later than here.
  void PutToday(const Entity& entity, Space* space, bc_id row) {
    const Space::Node node{base::Uuid::ParseLowercase(NodeKey(entity.id())), PositionOf(entity)};
    const GURL url(entity.url());
    const std::u16string title = base::UTF8ToUTF16(entity.title());
    // Where its place puts it among the Today tabs, without it.
    size_t at = 0;
    for (bc_id sibling : space->today()) {
      const Space::Node* each = space->NodeOf(sibling);
      if (sibling != row && each && each->position.LessThan(node.position)) {
        ++at;
      }
    }
    const base::Time seen =
        entity.has_seen() ? base::Time::FromTimeT(entity.seen()) : base::Time::Now();
    if (!row) {
      if (!Syncs(url)) {
        return;
      }
      SavedTab saved;
      sessions::SerializedNavigationEntry& page = saved.history.emplace_back();
      page.set_index(0);
      page.set_virtual_url(url);
      page.set_original_request_url(url);
      page.set_title(title);
      page.set_transition_type(ui::PAGE_TRANSITION_LINK);
      page.set_timestamp(base::Time::Now());
      saved.name = entity.name();
      saved.icon = entity.icon();
      saved.last_seen = seen;
      // In the Space's profile, waiting for it if it's still loading.
      row = Tab::Restore(space->profile(), saved);
      if (!row) {
        return;
      }
      space->AddAt(row, at);
    } else {
      const Space::Node* was = space->NodeOf(row);
      if (!IsToday(*space, row) || !was || !was->position.Equals(node.position)) {
        // A pinned tab unpinned on another device is unpinned here.
        space->Move(row, /*pinned=*/false, 0, at, [](bc_id) { return GURL(); });
      }
      Tab* tab = Tab::Get(row);
      if (tab && Syncs(url) && url != tab->Url() && !Window::Showing(row)) {
        // Its page here goes, keeping its history, which goes on to where the other device went.
        if (tab->web_contents()) {
          tab->Unload();
        }
        tab->Repoint(url, title);
      }
      if (tab && seen > tab->last_seen()) {
        tab->set_last_seen(seen);
      }
    }
    space->RestoreNodes(space->node(), {{row, node}});
    // check.sh waits for this line.
    const Tab* tab = Tab::Get(row);
    LOG(ERROR) << "browser: synced today " << row << " in space " << space->id() << " at " << at
               << (tab && tab->web_contents() ? " loaded " : " unloaded ") << url.spec();
  }

  base::ScopedObservation<Profile, ProfileObserver> profile_observation_{this};
  std::unique_ptr<syncer::DataTypeStore> store_;
  // What sync last had of each Space, pinned tab and folder, as the store keeps it, by id.
  std::map<std::string, Entity> known_;
  // Those that came before their Space.
  std::set<std::string> waiting_;
  // Each Today tab's page here at the last look, by id, which tells a navigation here from one
  // another device made.
  std::map<std::string, std::string> pages_;
  // Until the first look after the store is read.
  bool restoring_ = true;
  // A newer ilo wrote the account's Spaces, so this device stopped syncing them, until it signs
  // out or the browser starts again.
  bool paused_ = false;
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

std::unique_ptr<syncer::DataTypeController> Controller(Profile* profile) {
  Bridge*& bridge = Bridges()[profile];
  if (!bridge) {
    bridge = new Bridge(profile);
  }
  syncer::DataTypeControllerDelegate* delegate =
      bridge->change_processor()->GetControllerDelegate().get();
  // The same in transport mode, as the account is always signed in that way.
  return std::make_unique<syncer::DataTypeController>(
      syncer::ILO_SPACE,
      std::make_unique<syncer::ForwardingDataTypeControllerDelegate>(delegate),
      std::make_unique<syncer::ForwardingDataTypeControllerDelegate>(delegate));
}

void Changed() {
  for (const auto& [profile, bridge] : Bridges()) {
    bridge->Changed();
  }
}

bool NeedsUpdate() {
  return std::ranges::any_of(Bridges(), [](const auto& each) { return each.second->paused(); });
}

}  // namespace browser_core::sync_spaces
