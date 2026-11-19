#include "browser_core/share.h"

#include <algorithm>
#include <array>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/base64.h"
#include "base/base64url.h"
#include "base/functional/bind.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/logging.h"
#include "base/memory/raw_ref.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/strings/strcat.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/uuid.h"
#include "base/values.h"
#include "browser_core/host.h"
#include "browser_core/network_refuser.h"
#include "browser_core/session.h"
#include "browser_core/share_snapshot.h"
#include "browser_core/space.h"
#include "browser_core/sync.h"
#include "browser_core/tab.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "content/public/browser/browser_context.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/navigation_throttle.h"
#include "content/public/browser/navigation_throttle_registry.h"
#include "content/public/browser/storage_partition.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "crypto/aead.h"
#include "crypto/random.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "net/base/isolation_info.h"
#include "net/base/load_flags.h"
#include "net/base/net_errors.h"
#include "net/http/http_request_headers.h"
#include "net/http/http_response_headers.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "net/url_request/redirect_info.h"
#include "net/url_request/referrer_policy.h"
#include "services/network/public/cpp/constants.h"
#include "services/network/public/cpp/originating_process_id.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/mojom/network_context.mojom.h"
#include "services/network/public/mojom/url_loader_factory.mojom.h"
#include "services/network/public/mojom/url_response_head.mojom.h"
#include "third_party/skia/include/core/SkBitmap.h"
#include "ui/base/page_transition_types.h"
#include "ui/gfx/codec/png_codec.h"
#include "ui/gfx/image/image.h"
#include "ui/gfx/image/image_skia.h"
#include "ui/gfx/image/image_skia_rep.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

constexpr net::NetworkTrafficAnnotationTag kShareData =
    net::DefineNetworkTrafficAnnotation("browser_core_share_data", R"(
      semantics {
        sender: "Share links"
        description:
          "Asks a shared link's server for the link's sealed snapshot, which the browser opens "
          "with the key in the link's fragment and shows in a page of its own, in place of the "
          "server's page, without the user's cookies, client certificates or sign-ins."
        trigger:
          "The user opens a link to a share, whose server answered with Ilo-Share: v1."
        data:
          "The link's id, with the browser's user agent. Never the key, cookies, referrer, client "
          "certificate or credentials."
        destination: OTHER
        destination_other: "The ilo server the link names."
      }
      policy {
        cookies_allowed: NO
        setting: "None: it's asked only for a link the user opens."
        policy_exception_justification: "Asked only as the user opens a shared link."
      })");

// Each of the account's links has an entry on its server: what the list shows, and the link's key,
// so another of its devices can copy the link. Entries are sealed under a key derived from the
// account's secret, which the server never has, so any of its devices lists them and the server
// reads none, nor the key in them.
constexpr std::string_view kListInfo = "ilo share v1 list";
constexpr uint8_t kEntryVersion = 1;
constexpr size_t kIvSize = 12;
constexpr size_t kIdLength = 22;
// A share's /data, every hop: no redirect is followed, so it's one.
constexpr base::TimeDelta kTimeout = base::Seconds(10);

uint64_t g_last_request = 0;

// One of the account's links, from its sealed entry.
struct Listed {
  std::string id;
  std::array<uint8_t, 32> key{};
  std::string node;
  int kind = BC_SHARE_FOLDER;
  std::string title;
  size_t tabs = 0;
  size_t folders = 0;
  std::optional<uint32_t> color;
  std::string icon;
  double shared_at = 0;
};

std::vector<Listed>& Cache() {
  static base::NoDestructor<std::vector<Listed>> cache;
  return *cache;
}

std::string Base64Url(base::span<const uint8_t> bytes) {
  std::string out;
  base::Base64UrlEncode(bytes, base::Base64UrlEncodePolicy::OMIT_PADDING, &out);
  return out;
}

std::string EntryData(std::string_view id) {
  return base::StrCat({"ilo share list v1 ", id});
}

std::optional<std::string> SealEntry(const Listed& listed) {
  const std::optional<std::array<uint8_t, 32>> key = sync::ShareKey(kListInfo);
  if (!key) {
    return std::nullopt;
  }
  base::DictValue dict;
  dict.Set("v", static_cast<int>(kEntryVersion));
  dict.Set("key", Base64Url(listed.key));
  if (!listed.node.empty()) {
    dict.Set("node", listed.node);
  }
  dict.Set("kind", listed.kind);
  dict.Set("title", listed.title);
  dict.Set("tabs", static_cast<double>(listed.tabs));
  dict.Set("folders", static_cast<double>(listed.folders));
  if (listed.color) {
    dict.Set("color", static_cast<double>(*listed.color));
  }
  dict.Set("icon", listed.icon);
  dict.Set("shared", listed.shared_at);
  const std::string json = base::WriteJson(dict).value_or(std::string());
  std::array<uint8_t, kIvSize> iv;
  crypto::RandBytes(iv);
  const std::vector<uint8_t> sealed =
      crypto::aead::Seal(crypto::aead::AES_256_GCM, *key, base::as_byte_span(json), iv,
                         base::as_byte_span(EntryData(listed.id)));
  std::vector<uint8_t> entry = {kEntryVersion};
  entry.insert(entry.end(), iv.begin(), iv.end());
  entry.insert(entry.end(), sealed.begin(), sealed.end());
  return base::Base64Encode(entry);
}

std::optional<Listed> OpenEntry(const std::string& id,
                                const std::string& entry,
                                const std::array<uint8_t, 32>& key) {
  std::string bytes;
  if (!base::Base64Decode(entry, &bytes) || bytes.size() < 1 + kIvSize ||
      static_cast<uint8_t>(bytes[0]) != kEntryVersion) {
    return std::nullopt;
  }
  const base::span<const uint8_t> all = base::as_byte_span(bytes);
  const std::optional<std::vector<uint8_t>> json =
      crypto::aead::Open(crypto::aead::AES_256_GCM, key, all.subspan(1 + kIvSize),
                         all.subspan(size_t{1}, kIvSize), base::as_byte_span(EntryData(id)));
  if (!json) {
    return std::nullopt;
  }
  const std::optional<base::DictValue> dict = base::JSONReader::ReadDict(
      std::string_view(reinterpret_cast<const char*>(json->data()), json->size()),
      base::JSON_PARSE_RFC);
  const std::string* link_key = dict ? dict->FindString("key") : nullptr;
  std::string key_bytes;
  if (!link_key ||
      !base::Base64UrlDecode(*link_key, base::Base64UrlDecodePolicy::DISALLOW_PADDING,
                             &key_bytes) ||
      key_bytes.size() != 32) {
    return std::nullopt;
  }
  Listed listed;
  listed.id = id;
  std::ranges::copy(key_bytes, listed.key.begin());
  if (const std::string* node = dict->FindString("node")) {
    listed.node = *node;
  }
  listed.kind = dict->FindInt("kind").value_or(BC_SHARE_FOLDER);
  if (const std::string* title = dict->FindString("title")) {
    listed.title = *title;
  }
  listed.tabs = static_cast<size_t>(dict->FindDouble("tabs").value_or(0));
  listed.folders = static_cast<size_t>(dict->FindDouble("folders").value_or(0));
  if (std::optional<double> color = dict->FindDouble("color")) {
    listed.color = static_cast<uint32_t>(*color) & 0xffffff;
  }
  if (const std::string* icon = dict->FindString("icon")) {
    listed.icon = *icon;
  }
  listed.shared_at = dict->FindDouble("shared").value_or(0);
  return listed;
}

ShareLink LinkOf(const Listed& listed) {
  ShareLink link;
  link.id = listed.id;
  link.key = listed.key;
  return link;
}

// |listed| as the app hears it, its strings |listed|'s and |link|'s.
bc_share ShareOf(const Listed& listed, const std::string& link) {
  bc_share share = {};
  share.id = listed.id.c_str();
  share.link = link.c_str();
  share.kind = listed.kind;
  share.title = listed.title.c_str();
  share.tabs = listed.tabs;
  share.folders = listed.folders;
  share.has_color = listed.color.has_value();
  share.color = listed.color.value_or(0);
  share.icon = listed.icon.c_str();
  share.shared_at = listed.shared_at;
  return share;
}

std::string LinkOfListed(const Listed& listed) {
  return ShareLinkUrl(sync::ShareServer(), LinkOf(listed)).spec();
}

int ProblemOf(int status) {
  switch (status) {
    case 401:
      return BC_SHARE_PROBLEM_SIGNED_OUT;
    case 413:
      return BC_SHARE_PROBLEM_TOO_BIG;
    case 429:
      return BC_SHARE_PROBLEM_TOO_MANY;
    default:
      return BC_SHARE_PROBLEM_SERVER;
  }
}

void ListAnswered(int status, std::string body) {
  if (!Host().shares_listed) {
    return;
  }
  const std::optional<std::array<uint8_t, 32>> key = sync::ShareKey(kListInfo);
  if (!key) {
    Cache().clear();
    Host().shares_listed(Host().context, nullptr, 0, BC_SHARE_PROBLEM_SIGNED_OUT);
    return;
  }
  std::optional<base::Value> json =
      status == 200 ? base::JSONReader::Read(body, base::JSON_PARSE_RFC) : std::nullopt;
  if (!json || !json->is_list()) {
    LOG(ERROR) << "browser: shares not listed, " << status;
    Host().shares_listed(Host().context, nullptr, 0, ProblemOf(status));
    return;
  }
  std::vector<Listed> listed;
  for (const base::Value& each : json->GetList()) {
    const base::DictValue* dict = each.GetIfDict();
    const std::string* id = dict ? dict->FindString("id") : nullptr;
    const std::string* entry = dict ? dict->FindString("entry") : nullptr;
    if (!id || !entry) {
      continue;
    }
    // An entry sealed under another secret, as before the user started over, opens to nothing.
    if (std::optional<Listed> opened = OpenEntry(*id, *entry, *key)) {
      listed.push_back(std::move(*opened));
    }
  }
  std::ranges::sort(listed, std::ranges::greater(), &Listed::shared_at);
  Cache() = listed;
  LOG(ERROR) << "browser: shares listed, " << listed.size();
  std::vector<std::string> links;
  for (const Listed& each : listed) {
    links.push_back(LinkOfListed(each));
  }
  std::vector<bc_share> shares;
  for (size_t i = 0; i < listed.size(); ++i) {
    shares.push_back(ShareOf(listed[i], links[i]));
  }
  Host().shares_listed(Host().context, shares.data(), shares.size(), BC_SHARE_PROBLEM_NONE);
}

void Made(uint64_t request, const Listed* listed, size_t left_out, bool replaced, int problem) {
  LOG(ERROR) << "browser: share made, " << problem << (replaced ? " replaced" : "");
  if (!Host().share_made) {
    return;
  }
  if (!listed) {
    Host().share_made(Host().context, request, nullptr, left_out, false, problem);
    return;
  }
  const std::string link = LinkOfListed(*listed);
  const bc_share share = ShareOf(*listed, link);
  Host().share_made(Host().context, request, &share, left_out, replaced, problem);
}

void PostMade(uint64_t request, size_t left_out, int problem) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE,
      base::BindOnce([](uint64_t request, size_t left_out,
                        int problem) { Made(request, nullptr, left_out, false, problem); },
                     request, left_out, problem));
}

// What bc_unshare |id| came to, which the app shows when it isn't NONE, as the link still opens.
void Unshared(const std::string& id, int problem) {
  LOG(ERROR) << "browser: unshared, " << problem;
  if (Host().unshared) {
    Host().unshared(Host().context, id.c_str(), problem);
  }
}

// What bc_share_make would share.
struct Collected {
  ShareSnapshot snapshot;
  size_t left_out = 0;
  // A folder's or a Space's sync node, which makes it the same one on every device.
  std::string node;
  // Each kept tab's favicon, in the order a walk of the snapshot meets its tabs, or an empty
  // bitmap, which Pack encodes off the UI thread. Empty once encoded, and when only counting.
  std::vector<SkBitmap> favicons;
};

// The title of |tab|'s page at |url|: its page's now if it's there, else the latest at |url| in
// its history, else none, as a page a pinned tab went on to may say what it holds, as a mail's
// subject does, and its address alone goes.
std::string PageTitle(const Tab& tab, const GURL& url) {
  const SavedTab saved = tab.Save();
  const size_t count = saved.history.size();
  if (saved.current >= 0 && static_cast<size_t>(saved.current) < count &&
      (tab.Url() == url || saved.history[saved.current].virtual_url() == url)) {
    return base::UTF16ToUTF8(saved.history[saved.current].title());
  }
  for (size_t i = count; i-- > 0;) {
    if (saved.history[i].virtual_url() == url) {
      return base::UTF16ToUTF8(saved.history[i].title());
    }
  }
  return std::string();
}

// A copy of |tab|'s favicon's pixels for Pack, as its own may change on the UI thread meanwhile,
// or an empty bitmap.
SkBitmap FaviconBitmap(const Tab& tab) {
  const gfx::Image& favicon = tab.favicon();
  if (favicon.IsEmpty()) {
    return SkBitmap();
  }
  const SkBitmap& shown = favicon.AsImageSkia().GetRepresentation(2).GetBitmap();
  SkBitmap copy;
  if (shown.drawsNothing() || !copy.tryAllocPixels(shown.info()) ||
      !shown.readPixels(copy.pixmap())) {
    return SkBitmap();
  }
  return copy;
}

// |row| of |space| as a share's tab, or none, counted in |left_out|, as its address isn't one a
// link takes. A pinned tab goes at the address it's pinned at, a Today tab at the one it shows.
std::optional<ShareItem> TabItem(const Space& space,
                                 bc_id row,
                                 Collected& collected,
                                 bool favicons) {
  const Tab* tab = Tab::Get(row);
  if (!tab) {
    return std::nullopt;
  }
  const GURL pinned = space.PinnedUrl(row);
  const GURL shown = tab->Url();
  ShareItem item;
  item.url = ShareAddress(pinned.is_valid() ? pinned : shown);
  if (!ShareTakes(item.url)) {
    ++collected.left_out;
    return std::nullopt;
  }
  item.title = tab->name();
  if (item.title.empty()) {
    item.title = PageTitle(*tab, pinned.is_valid() ? pinned : shown);
  }
  if (favicons) {
    // The favicon is the page's it shows, so only for the site the link names.
    collected.favicons.push_back(url::IsSameOriginWith(ShareAddress(shown), item.url)
                                     ? FaviconBitmap(*tab)
                                     : SkBitmap());
  }
  return item;
}

// The rows directly in |folder|, else outside every folder, top to bottom, with what's in them.
std::vector<ShareItem> Rows(const Space& space, bc_id folder, Collected& collected, bool favicons) {
  std::vector<ShareItem> items;
  for (bc_id row : space.pinned_rows()) {
    if (space.FolderOf(row) != folder) {
      continue;
    }
    if (space.IsFolder(row)) {
      ShareItem item;
      item.folder = true;
      item.title = space.FolderName(row);
      item.items = Rows(space, row, collected, favicons);
      items.push_back(std::move(item));
      continue;
    }
    if (std::optional<ShareItem> item = TabItem(space, row, collected, favicons)) {
      items.push_back(std::move(*item));
    }
  }
  return items;
}

// The user's own Space, listed: never an agent's or a window's own.
bool Shareable(const Space* space) {
  return space && !space->agent() && !space->own();
}

// What bc_share_make would share. Only to count, |favicons| false, it takes none.
std::optional<Collected> Collect(int kind, const bc_id* items, size_t count, bool favicons) {
  if (!items || !count) {
    return std::nullopt;
  }
  Collected collected;
  ShareSnapshot& snapshot = collected.snapshot;
  snapshot.kind = kind;
  snapshot.made = base::Time::Now().InSecondsFSinceUnixEpoch();
  if (kind == BC_SHARE_SPACE) {
    const Space* space = count == 1 ? Space::FromId(items[0]) : nullptr;
    if (!Shareable(space)) {
      return std::nullopt;
    }
    snapshot.title = space->name();
    snapshot.color = space->color();
    snapshot.icon = space->icon();
    snapshot.items = Rows(*space, 0, collected, favicons);
    collected.node = space->node().id.AsLowercaseString();
    return collected;
  }
  if (kind == BC_SHARE_FOLDER) {
    const Space* space = count == 1 ? Space::HoldingFolder(items[0]) : nullptr;
    if (!Shareable(space) || !space->IsFolder(items[0])) {
      return std::nullopt;
    }
    snapshot.title = space->FolderName(items[0]);
    snapshot.icon = space->FolderIcon(items[0]);
    snapshot.items = Rows(*space, items[0], collected, favicons);
    if (const Space::Node* node = space->NodeOf(items[0])) {
      collected.node = node->id.AsLowercaseString();
    }
    return collected;
  }
  if (kind != BC_SHARE_TABS) {
    return std::nullopt;
  }
  const Space* space = Space::Holding(items[0]);
  if (!Shareable(space)) {
    return std::nullopt;
  }
  // Each chosen tab with its split's, in the Space's order.
  std::vector<bc_id> chosen;
  for (size_t i = 0; i < count; ++i) {
    if (Space::Holding(items[i]) != space || !space->Has(items[i])) {
      return std::nullopt;
    }
    for (bc_id pane : space->Panes(items[i])) {
      if (!std::ranges::contains(chosen, pane)) {
        chosen.push_back(pane);
      }
    }
  }
  std::vector<bc_id> order = space->pinned_rows();
  order.insert(order.end(), space->today().begin(), space->today().end());
  snapshot.title = space->name();
  for (bc_id row : order) {
    if (!std::ranges::contains(chosen, row) || space->IsFolder(row)) {
      continue;
    }
    if (std::optional<ShareItem> item = TabItem(*space, row, collected, favicons)) {
      snapshot.items.push_back(std::move(*item));
    }
  }
  return collected;
}

// Gives |items|' tabs their favicons from |bitmaps|, in the order Collect took them, as PNGs a
// snapshot keeps.
void EncodeFavicons(std::vector<ShareItem>& items,
                    const std::vector<SkBitmap>& bitmaps,
                    size_t& next) {
  for (ShareItem& item : items) {
    if (item.folder) {
      EncodeFavicons(item.items, bitmaps, next);
      continue;
    }
    if (next >= bitmaps.size()) {
      return;
    }
    const SkBitmap& bitmap = bitmaps[next++];
    if (bitmap.drawsNothing()) {
      continue;
    }
    std::optional<std::vector<uint8_t>> png =
        gfx::PNGCodec::EncodeBGRASkBitmap(bitmap, /*discard_transparency=*/false);
    if (png && png->size() <= kShareFaviconCap) {
      item.favicon.assign(png->begin(), png->end());
    }
  }
}

// A snapshot, its favicons encoded, and it sealed, off the UI thread.
struct Packed {
  ShareSnapshot snapshot;
  std::optional<SealedShare> sealed;
};

Packed Pack(ShareSnapshot snapshot,
            std::vector<SkBitmap> favicons,
            ShareLink link,
            bool with_favicons) {
  size_t next = 0;
  EncodeFavicons(snapshot.items, favicons, next);
  Packed packed;
  packed.sealed = SealShare(snapshot, link, kShareBlobCap, with_favicons);
  packed.snapshot = std::move(snapshot);
  return packed;
}

void Send(uint64_t request, Collected collected, Listed listed, bool replacing, int tries,
          bool favicons);

// What the server said to a snapshot sent under |listed|'s link.
void Sent(uint64_t request,
          Collected collected,
          Listed listed,
          bool replacing,
          int tries,
          bool favicons,
          int status,
          std::string answer) {
  if (status == 200 || status == 201 || status == 204) {
    Made(request, &listed, collected.left_out, replacing, BC_SHARE_PROBLEM_NONE);
    ShareList();
    return;
  }
  // Unshared meanwhile, on another device: a new link. A new id the server has, which a random
  // 128 bits makes all but impossible: another one. Each once.
  if (((replacing && (status == 404 || status == 410)) || (!replacing && status == 409)) &&
      tries == 0) {
    const ShareLink link = NewShareLink();
    listed.id = link.id;
    listed.key = link.key;
    Send(request, std::move(collected), std::move(listed), /*replacing=*/false, tries + 1,
         favicons);
    return;
  }
  // The server keeps less than the core sends: without its favicons, once.
  if (status == 413 && favicons) {
    Send(request, std::move(collected), std::move(listed), replacing, tries, /*favicons=*/false);
    return;
  }
  Made(request, nullptr, collected.left_out, false, ProblemOf(status));
}

// |packed| sent to the server under |listed|'s link.
void Sealed(uint64_t request,
            Collected collected,
            Listed listed,
            bool replacing,
            int tries,
            Packed packed) {
  collected.snapshot = std::move(packed.snapshot);
  collected.favicons.clear();
  if (!packed.sealed) {
    Made(request, nullptr, collected.left_out, false, BC_SHARE_PROBLEM_TOO_BIG);
    return;
  }
  const std::optional<std::string> entry = SealEntry(listed);
  if (!entry) {
    Made(request, nullptr, collected.left_out, false, BC_SHARE_PROBLEM_SIGNED_OUT);
    return;
  }
  base::DictValue body;
  if (!replacing) {
    body.Set("id", listed.id);
  }
  body.Set("blob", base::Base64Encode(packed.sealed->blob));
  body.Set("entry", *entry);
  const std::string path =
      replacing ? base::StrCat({"/api/shares/", listed.id}) : std::string("/api/shares");
  const bool favicons = packed.sealed->favicons;
  sync::AskForShares(replacing ? "PUT" : "POST", path,
                     base::WriteJson(body).value_or(std::string()),
                     base::BindOnce(&Sent, request, std::move(collected), std::move(listed),
                                    replacing, tries, favicons));
}

// Seals |collected| under |listed|'s link, off the UI thread, with its favicons if |favicons| and
// they fit, then sends it as a new link or, |replacing|, over the one the account has for the
// same folder or Space.
void Send(uint64_t request,
          Collected collected,
          Listed listed,
          bool replacing,
          int tries,
          bool favicons) {
  ShareSnapshot snapshot = std::move(collected.snapshot);
  std::vector<SkBitmap> bitmaps = std::move(collected.favicons);
  const ShareLink link = LinkOf(listed);
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::TaskPriority::USER_BLOCKING, base::TaskShutdownBehavior::SKIP_ON_SHUTDOWN},
      base::BindOnce(&Pack, std::move(snapshot), std::move(bitmaps), link, favicons),
      base::BindOnce(&Sealed, request, std::move(collected), std::move(listed), replacing, tries));
}

// The link the account has for |listed|'s node, from its server's list now, then Send.
void SendForNode(uint64_t request, Collected collected, Listed listed) {
  sync::AskForShares(
      "GET", "/api/shares", std::string(),
      base::BindOnce(
          [](uint64_t request, Collected collected, Listed listed, int status, std::string body) {
            if (status != 200) {
              Made(request, nullptr, collected.left_out, false, ProblemOf(status));
              return;
            }
            const std::optional<std::array<uint8_t, 32>> key = sync::ShareKey(kListInfo);
            if (!key) {
              Made(request, nullptr, collected.left_out, false, BC_SHARE_PROBLEM_SIGNED_OUT);
              return;
            }
            // A list it can't read can't say there's no link for the node, and a second link for
            // it would leave the first one out of date and unlisted.
            const std::optional<base::Value> json =
                base::JSONReader::Read(body, base::JSON_PARSE_RFC);
            if (!json || !json->is_list()) {
              Made(request, nullptr, collected.left_out, false, BC_SHARE_PROBLEM_SERVER);
              return;
            }
            bool replacing = false;
            for (const base::Value& each : json->GetList()) {
              const base::DictValue* dict = each.GetIfDict();
              const std::string* id = dict ? dict->FindString("id") : nullptr;
              const std::string* entry = dict ? dict->FindString("entry") : nullptr;
              std::optional<Listed> opened =
                  id && entry ? OpenEntry(*id, *entry, *key) : std::nullopt;
              if (opened && opened->node == listed.node) {
                listed.id = opened->id;
                listed.key = opened->key;
                replacing = true;
                break;
              }
            }
            Send(request, std::move(collected), std::move(listed), replacing, 0,
                 /*favicons=*/true);
          },
          request, std::move(collected), std::move(listed)));
}

// The offer a share the user opened makes for its tab (tab_share_found), until it's answered or
// the tab goes on to another page.
class ShareOffer : public content::WebContentsUserData<ShareOffer>,
                   public content::WebContentsObserver {
 public:
  ShareOffer(const ShareOffer&) = delete;
  ShareOffer& operator=(const ShareOffer&) = delete;
  ~ShareOffer() override = default;

  // The share |snapshot| from |host| commits in |navigation|.
  static void Expect(content::WebContents* contents,
                     int64_t navigation,
                     ShareSnapshot snapshot,
                     std::string host) {
    if (ShareOffer* before = FromWebContents(contents)) {
      before->Gone();
    }
    CreateForWebContents(contents);
    ShareOffer* offer = FromWebContents(contents);
    offer->navigation_ = navigation;
    offer->snapshot_ = std::move(snapshot);
    offer->host_ = std::move(host);
  }

  bool offered() const { return offered_; }

  // The share, the offer gone. Deletes this.
  ShareSnapshot Take() {
    ShareSnapshot snapshot = std::move(snapshot_);
    Gone();
    return snapshot;
  }

 private:
  friend class content::WebContentsUserData<ShareOffer>;

  explicit ShareOffer(content::WebContents* contents)
      : content::WebContentsUserData<ShareOffer>(*contents),
        content::WebContentsObserver(contents) {}

  // content::WebContentsObserver:
  void DidFinishNavigation(content::NavigationHandle* navigation) override {
    if (!navigation->IsInPrimaryMainFrame() || navigation->IsSameDocument()) {
      return;
    }
    if (navigation->GetNavigationId() == navigation_ && navigation->HasCommitted() && !offered_) {
      offered_ = true;
      Offer();
      return;
    }
    if (navigation->HasCommitted()) {
      Gone();
    }
  }
  void WebContentsDestroyed() override {
    if (offered_) {
      Report(Tab::IdOf(web_contents()), std::nullopt);
    }
  }

  void Offer() {
    LOG(ERROR) << "browser: share found";
    const ShareCounts counts = CountShare(snapshot_.items);
    Found found;
    found.kind = snapshot_.kind;
    found.title = snapshot_.title;
    found.host = host_;
    found.tabs = counts.tabs;
    found.folders = counts.folders;
    found.color = snapshot_.color;
    found.icon = snapshot_.icon;
    Report(Tab::IdOf(web_contents()), std::move(found));
  }

  // Deletes this.
  void Gone() {
    const bc_id tab = Tab::IdOf(web_contents());
    const bool offered = offered_;
    content::WebContents* contents = web_contents();
    contents->RemoveUserData(UserDataKey());
    if (offered) {
      Report(tab, std::nullopt);
    }
  }

  struct Found {
    int kind = BC_SHARE_FOLDER;
    std::string title;
    std::string host;
    size_t tabs = 0;
    size_t folders = 0;
    std::optional<uint32_t> color;
    std::string icon;
  };

  // From a task of its own, as the app may be in a call that made it.
  static void Report(bc_id tab, std::optional<Found> found) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](bc_id tab, std::optional<Found> found) {
                         if (!Host().tab_share_found) {
                           return;
                         }
                         if (!found) {
                           Host().tab_share_found(Host().context, tab, nullptr);
                           return;
                         }
                         bc_share_found told = {};
                         told.kind = found->kind;
                         told.title = found->title.c_str();
                         told.host = found->host.c_str();
                         told.tabs = found->tabs;
                         told.folders = found->folders;
                         told.has_color = found->color.has_value();
                         told.color = found->color.value_or(0);
                         told.icon = found->icon.c_str();
                         Host().tab_share_found(Host().context, tab, &told);
                       },
                       tab, std::move(found)));
  }

  int64_t navigation_ = 0;
  ShareSnapshot snapshot_;
  std::string host_;
  bool offered_ = false;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(ShareOffer);

// A share's answer opened, and its page, off the UI thread.
struct Opening {
  ShareOpened opened;
  std::string page;
};

Opening OpenAndShow(std::string body, ShareLink link, std::string host) {
  Opening opening;
  opening.opened = OpenShare(base::as_byte_span(body), link);
  if (opening.opened.snapshot) {
    opening.page = SharePage(*opening.opened.snapshot, host);
  }
  return opening;
}

// Whether |navigation| is in a tab of the user's own, as bc_tab_answer_share says, where a link
// opens as a share: not an agent's or a private window's.
bool UsersTab(content::NavigationHandle& navigation) {
  content::WebContents* contents = navigation.GetWebContents();
  if (contents->GetBrowserContext()->IsOffTheRecord()) {
    return false;
  }
  const Tab* tab = Tab::FromWebContents(contents);
  const Space* space = tab ? Space::Holding(tab->id()) : nullptr;
  return space && !space->agent() && !space->off_the_record();
}

// Looks at a navigation's answer for Ilo-Share: v1 at a link's address. Then the server's page
// doesn't commit: the core asks for the share's data and commits a page of its own in its place,
// as the ad block commits its warning. It holds nothing, and asks nothing of the tab, until it
// sees that header.
class ShareThrottle : public content::NavigationThrottle {
 public:
  explicit ShareThrottle(content::NavigationThrottleRegistry& registry)
      : content::NavigationThrottle(registry) {}

  // content::NavigationThrottle:
  ThrottleCheckResult WillProcessResponse() override {
    const net::HttpResponseHeaders* headers = navigation_handle()->GetResponseHeaders();
    if (!headers || !headers->HasHeaderValue("Ilo-Share", "v1")) {
      return PROCEED;
    }
    const GURL& url = navigation_handle()->GetURL();
    std::optional<ShareLink> link = ShareLinkOf(url);
    if (!link || !UsersTab(*navigation_handle())) {
      return PROCEED;
    }
    fetch_ = std::make_unique<Fetch>(*this, std::move(*link), url.host());
    if (!fetch_->Ask(url.Resolve(base::StrCat({"/s/", fetch_->link().id, "/data"})))) {
      fetch_.reset();
      return PROCEED;
    }
    return DEFER;
  }
  const char* GetNameForLogging() override { return "browser_core::ShareThrottle"; }

 private:
  // Asking for a share's data, from its link's server alone.
  class Fetch {
   public:
    Fetch(ShareThrottle& throttle, ShareLink link, std::string host)
        : throttle_(throttle), link_(std::move(link)), host_(std::move(host)) {}

    const ShareLink& link() const { return link_; }

    bool Ask(const GURL& data) {
      network::mojom::NetworkContext* network = throttle_->navigation_handle()
                                                    ->GetWebContents()
                                                    ->GetBrowserContext()
                                                    ->GetDefaultStoragePartition()
                                                    ->GetNetworkContext();
      if (!network) {
        return false;
      }
      auto params = network::mojom::URLLoaderFactoryParams::New();
      params->process_id = network::OriginatingProcessId::browser();
      params->is_trusted = true;
      params->is_orb_enabled = false;
      // Its own, never the profile's, which could pick a client certificate or answer a sign-in
      // for the user.
      params->url_loader_network_observer = refuser_.Bind();
      params->network_restrictions_id = network::GetNoOpNetworkRestrictionsId();
      network->CreateURLLoaderFactory(factory_.BindNewPipeAndPassReceiver(), std::move(params));

      auto request = std::make_unique<network::ResourceRequest>();
      request->url = data;
      request->method = net::HttpRequestHeaders::kGetMethod;
      request->credentials_mode = network::mojom::CredentialsMode::kOmit;
      request->referrer_policy = net::ReferrerPolicy::NO_REFERRER;
      request->do_not_prompt_for_login = true;
      // Nothing kept in the cache, and nothing shared with the user's browsing.
      request->load_flags = net::LOAD_DISABLE_CACHE;
      request->trusted_params = network::ResourceRequest::TrustedParams();
      request->trusted_params->isolation_info = net::IsolationInfo::CreateTransient(std::nullopt);
      loader_ = network::SimpleURLLoader::Create(std::move(request), kShareData);
      // No cookie read or written. Unlike a link card's, a server on the machine or the network is
      // asked: the link names it, and the user opened it.
      loader_->SetURLLoaderFactoryOptions(network::mojom::kURLLoadOptionBlockAllCookies);
      loader_->SetAllowHttpErrorResults(true);
      // The link's own server answers, or nothing does.
      loader_->SetOnRedirectCallback(
          base::BindRepeating(&Fetch::Redirected, base::Unretained(this)));
      loader_->DownloadToString(factory_.get(),
                                base::BindOnce(&Fetch::Answered, base::Unretained(this)),
                                kShareBlobCap);
      timeout_.Start(FROM_HERE, kTimeout, base::BindOnce(&Fetch::TimedOut, base::Unretained(this)));
      return true;
    }

   private:
    // Each ends in the throttle's Commit, which deletes this, so nothing of this is touched after.
    void TimedOut() { throttle_->Commit(ShareRefusedPage(false, std::nullopt), "unanswered"); }

    void Redirected(const GURL& from,
                    const net::RedirectInfo& redirect,
                    const network::mojom::URLResponseHead& head,
                    std::vector<std::string>* removed_headers) {
      throttle_->Commit(ShareRefusedPage(false, std::nullopt), "redirected");
    }

    void Answered(std::optional<std::string> body) {
      const network::mojom::URLResponseHead* head = loader_->ResponseInfo();
      const int status = head && head->headers ? head->headers->response_code() : 0;
      const int error = loader_->NetError();
      loader_.reset();
      if (status == 410) {
        throttle_->Commit(ShareRefusedPage(true, std::nullopt), "unshared");
        return;
      }
      if (error == net::ERR_INSUFFICIENT_RESOURCES) {
        throttle_->Commit(ShareRefusedPage(false, ShareRefusal::kTooBig), "too big");
        return;
      }
      // A link to a share the server never had was copied wrong.
      if (status == 404) {
        throttle_->Commit(ShareRefusedPage(false, ShareRefusal::kKey), "unknown");
        return;
      }
      if (status != 200 || !body) {
        throttle_->Commit(ShareRefusedPage(false, std::nullopt), "unanswered");
        return;
      }
      // Opening a megabyte takes longer than a frame, so not on the UI thread. The timeout still
      // runs meanwhile.
      base::ThreadPool::PostTaskAndReplyWithResult(
          FROM_HERE,
          {base::TaskPriority::USER_BLOCKING, base::TaskShutdownBehavior::SKIP_ON_SHUTDOWN},
          base::BindOnce(&OpenAndShow, std::move(*body), link_, host_),
          base::BindOnce(&Fetch::Opened, weak_factory_.GetWeakPtr()));
    }

    void Opened(Opening opening) {
      if (!opening.opened.snapshot) {
        const ShareRefusal refusal = opening.opened.refusal;
        throttle_->Commit(ShareRefusedPage(false, refusal),
                         refusal == ShareRefusal::kTooBig  ? "too big"
                         : refusal == ShareRefusal::kNewer ? "newer"
                                                           : "key");
        return;
      }
      content::NavigationHandle* navigation = throttle_->navigation_handle();
      ShareOffer::Expect(navigation->GetWebContents(), navigation->GetNavigationId(),
                         std::move(*opening.opened.snapshot), host_);
      throttle_->Commit(std::move(opening.page), "shown");
    }

    const raw_ref<ShareThrottle> throttle_;
    const ShareLink link_;
    const std::string host_;
    NetworkRefuser refuser_;
    mojo::Remote<network::mojom::URLLoaderFactory> factory_;
    std::unique_ptr<network::SimpleURLLoader> loader_;
    base::OneShotTimer timeout_;
    base::WeakPtrFactory<Fetch> weak_factory_{this};
  };

  // |said| is what the core's log says of it, which holds nothing of the share.
  void Commit(std::string page, const char* said) {
    LOG(ERROR) << "browser: share page " << said;
    fetch_.reset();
    // Last: the navigation, and this with it, can go here.
    CancelDeferredNavigation({CANCEL, net::ERR_BLOCKED_BY_CLIENT, std::move(page)});
  }

  std::unique_ptr<Fetch> fetch_;
};

// Rows for |items| in |space|, in |folder| or outside every folder, from |index| on, each tab
// unloaded until the user opens it.
void AddRows(Space& space, bc_id folder, const std::vector<ShareItem>& items) {
  size_t index = 0;
  for (const ShareItem& item : items) {
    if (item.folder) {
      const bc_id made =
          space.AddFolder(item.title.empty() ? "Folder" : item.title, folder, index++);
      AddRows(space, made, item.items);
      continue;
    }
    // Without its page until it's first shown, as a pinned tab sync brings.
    SavedTab saved;
    sessions::SerializedNavigationEntry& page = saved.history.emplace_back();
    page.set_index(0);
    page.set_virtual_url(item.url);
    page.set_original_request_url(item.url);
    page.set_title(base::UTF8ToUTF16(item.title));
    page.set_transition_type(ui::PAGE_TRANSITION_AUTO_BOOKMARK);
    page.set_timestamp(base::Time::Now());
    const bc_id row = Tab::Restore(space.profile(), saved);
    if (row) {
      space.AddPinned(row, folder, index++, item.url);
    }
  }
}

}  // namespace

bool ShareCount(int kind,
                const bc_id* items,
                size_t count,
                size_t* tabs,
                size_t* folders,
                size_t* left_out) {
  const std::optional<Collected> collected = Collect(kind, items, count, /*favicons=*/false);
  if (!collected) {
    return false;
  }
  const ShareCounts counts = CountShare(collected->snapshot.items);
  if (tabs) {
    *tabs = counts.tabs;
  }
  if (folders) {
    *folders = counts.folders;
  }
  if (left_out) {
    *left_out = collected->left_out;
  }
  return true;
}

uint64_t ShareMake(int kind, const bc_id* items, size_t count) {
  std::optional<Collected> collected = Collect(kind, items, count, /*favicons=*/true);
  if (!collected) {
    return 0;
  }
  const uint64_t request = ++g_last_request;
  if (!sync::ShareServer().is_valid() || !sync::ShareKey(kListInfo)) {
    PostMade(request, collected->left_out, BC_SHARE_PROBLEM_SIGNED_OUT);
    return request;
  }
  const ShareCounts counts = CountShare(collected->snapshot.items);
  if (!counts.tabs) {
    PostMade(request, collected->left_out, BC_SHARE_PROBLEM_EMPTY);
    return request;
  }
  const ShareLink link = NewShareLink();
  Listed listed;
  listed.id = link.id;
  listed.key = link.key;
  listed.node = collected->node;
  listed.kind = kind;
  listed.title = collected->snapshot.title;
  listed.tabs = counts.tabs;
  listed.folders = counts.folders;
  listed.color = collected->snapshot.color;
  listed.icon = collected->snapshot.icon;
  listed.shared_at = base::Time::Now().InMillisecondsFSinceUnixEpoch();
  if (listed.node.empty()) {
    Send(request, std::move(*collected), std::move(listed), /*replacing=*/false, 0,
         /*favicons=*/true);
  } else {
    SendForNode(request, std::move(*collected), std::move(listed));
  }
  return request;
}

void ShareList() {
  sync::AskForShares("GET", "/api/shares", std::string(), base::BindOnce(&ListAnswered));
}

void Unshare(const std::string& id) {
  // No server has a link by another name.
  if (id.size() != kIdLength || !std::ranges::all_of(id, [](char c) {
        return base::IsAsciiAlphaNumeric(c) || c == '-' || c == '_';
      })) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Unshared, id, BC_SHARE_PROBLEM_NONE));
    return;
  }
  if (!sync::ShareServer().is_valid()) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Unshared, id, BC_SHARE_PROBLEM_SIGNED_OUT));
    return;
  }
  sync::AskForShares("DELETE", base::StrCat({"/api/shares/", id}), std::string(),
                     base::BindOnce(
                         [](std::string id, int status, std::string body) {
                           // Gone already, as unshared on another device, is as asked.
                           const bool gone = status == 200 || status == 204 || status == 404 ||
                                             status == 410;
                           Unshared(id, gone ? BC_SHARE_PROBLEM_NONE : ProblemOf(status));
                           if (gone) {
                             ShareList();
                           }
                         },
                         id));
}

bc_id TabAnswerShare(bc_id tab, bc_id space_id, bool add) {
  content::WebContents* contents = Tab::FromId(tab);
  ShareOffer* offer = contents ? ShareOffer::FromWebContents(contents) : nullptr;
  if (!offer || !offer->offered()) {
    return 0;
  }
  const std::string profile = Tab::Get(tab)->profile_dir();
  ShareSnapshot snapshot = offer->Take();
  if (!add) {
    return 0;
  }
  if (snapshot.kind == BC_SHARE_SPACE) {
    Space* made = Space::Create(snapshot.title.empty() ? "Shared" : snapshot.title);
    made->SetProfile(profile);
    made->SetColor(snapshot.color);
    made->SetIcon(snapshot.icon);
    AddRows(*made, 0, snapshot.items);
    LOG(ERROR) << "browser: share added, space " << made->id();
    return made->id();
  }
  Space* space = Space::FromId(space_id);
  if (!Shareable(space)) {
    return 0;
  }
  size_t end = 0;
  for (bc_id row : space->pinned_rows()) {
    end += space->FolderOf(row) == 0;
  }
  const bc_id folder = space->AddFolder(snapshot.title.empty() ? "Shared" : snapshot.title, 0, end);
  if (!snapshot.icon.empty()) {
    space->SetFolderIcon(folder, snapshot.icon);
  }
  AddRows(*space, folder, snapshot.items);
  LOG(ERROR) << "browser: share added, folder " << folder;
  return folder;
}

void AddShareThrottle(content::NavigationThrottleRegistry& registry) {
  content::NavigationHandle& navigation = registry.GetNavigationHandle();
  // The user's: typed, pasted, a link from another app, or one a page started on their gesture.
  // Whose tab it is waits for a share's answer (UsersTab), as most navigations aren't one.
  if (!navigation.IsInPrimaryMainFrame() ||
      (navigation.IsRendererInitiated() && !navigation.HasUserGesture())) {
    return;
  }
  registry.AddThrottle(std::make_unique<ShareThrottle>(registry));
}

}  // namespace browser_core
