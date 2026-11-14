#include "browser_core/ad_block_carry.h"

#include <memory>

#include "base/files/file_util.h"
#include "base/json/json_reader.h"
#include "base/values.h"
#include "third_party/leveldatabase/env_chromium.h"
#include "third_party/leveldatabase/src/include/leveldb/db.h"

namespace browser_core::ad_block_carry {

namespace {

// Where uBlock Origin keeps My filters in its chrome.storage.local (its js/assets.js).
constexpr char kKey[] = "user-filters";

}  // namespace

std::optional<std::string> OldFilters(const base::FilePath& folder) {
  if (!base::DirectoryExists(folder)) {
    return std::string();
  }
  // Not value_store's LeveldbValueStore, which repairs a corrupt store and deletes one it can't.
  leveldb_env::Options options;
  options.create_if_missing = false;
  options.paranoid_checks = true;
  std::unique_ptr<leveldb::DB> db;
  if (!leveldb_env::OpenDB(options, folder.AsUTF8Unsafe(), &db).ok()) {
    return std::nullopt;
  }
  std::string json;
  const leveldb::Status read = db->Get(leveldb::ReadOptions(), kKey, &json);
  if (read.IsNotFound()) {
    return std::string();
  }
  // chrome.storage keeps each value as JSON.
  std::optional<base::Value> value =
      read.ok() ? base::JSONReader::Read(json, base::JSON_PARSE_RFC) : std::nullopt;
  if (!value || !value->is_string()) {
    return std::nullopt;
  }
  return value->GetString();
}

}  // namespace browser_core::ad_block_carry
