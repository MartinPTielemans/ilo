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
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/strings/strcat.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/uuid.h"
#include "base/values.h"
#include "browser_core/agent_autofill.h"
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

// The account's links' entries are sealed under a key from its secret, which the server never
// has, so any of its devices lists them and the server reads none.
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

// What bc_share_make would share.
struct Collected {
  ShareSnapshot snapshot;
  size_t left_out = 0;
  // A folder's or a Space's sync node, which makes it the same one on every device.
  std::string node;
};

ShareItem TabItem(bc_id id, size_t& left_out, bool& kept) {
  ShareItem item;
  kept = false;
  const Tab* tab = Tab::Get(id);
  if (!tab) {
    return item;
  }
  item.url = ShareAddress(tab->Url());
  if (!ShareTakes(item.url)) {
    ++left_out;
    return item;
  }
  kept = true;
  item.title = tab->name();
  if (item.title.empty()) {
    const SavedTab saved = tab->Save();
    if (saved.current >= 0 && static_cast<size_t>(saved.current) < saved.history.size()) {
      item.title = base::UTF16ToUTF8(saved.history[saved.current].title());
    }
  }
  const gfx::Image& favicon = tab->favicon();
  if (!favicon.IsEmpty()) {
    std::optional<std::vector<uint8_t>> png = gfx::PNGCodec::EncodeBGRASkBitmap(
        favicon.AsImageSkia().GetRepresentation(2).GetBitmap(), /*discard_transparency=*/false);
    if (png && png->size() <= kShareFaviconCap) {
      item.favicon.assign(png->begin(), png->end());
    }
  }
  return item;
}

// The rows directly in |folder|, else outside every folder, top to bottom, with what's in them.
std::vector<ShareItem> Rows(const Space& space, bc_id folder, size_t& left_out) {
  std::vector<ShareItem> items;
  for (bc_id row : space.pinned_rows()) {
    if (space.FolderOf(row) != folder) {
      continue;
    }
    if (space.IsFolder(row)) {
      ShareItem item;
      item.folder = true;
      item.title = space.FolderName(row);
      item.items = Rows(space, row, left_out);
      items.push_back(std::move(item));
      continue;
    }
    bool kept = false;
    ShareItem item = TabItem(row, left_out, kept);
    if (kept) {
      items.push_back(std::move(item));
    }
  }
  return items;
}

// The user's own Space, listed: never an agent's or a window's own.
bool Shareable(const Space* space) {
  return space && !space->agent() && !space->own();
}

std::optional<Collected> Collect(int kind, const bc_id* items, size_t count) {
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
    snapshot.items = Rows(*space, 0, collected.left_out);
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
    snapshot.items = Rows(*space, items[0], collected.left_out);
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
    bool kept = false;
    ShareItem item = TabItem(row, collected.left_out, kept);
    if (kept) {
      snapshot.items.push_back(std::move(item));
    }
  }
  return collected;
}

// Sends |collected| to the server under |listed|'s link, as a new link or, |replacing|, over the
// one the account has for the same folder or Space.
void Send(uint64_t request, Collected collected, Listed listed, bool replacing, int tries) {
  const std::optional<std::vector<uint8_t>> blob =
      SealShare(collected.snapshot, LinkOf(listed), kShareBlobCap);
  if (!blob) {
    PostMade(request, collected.left_out, BC_SHARE_PROBLEM_TOO_BIG);
    return;
  }
  const std::optional<std::string> entry = SealEntry(listed);
  if (!entry) {
    PostMade(request, collected.left_out, BC_SHARE_PROBLEM_SIGNED_OUT);
    return;
  }
  base::DictValue body;
  if (!replacing) {
    body.Set("id", listed.id);
  }
  body.Set("blob", base::Base64Encode(*blob));
  body.Set("entry", *entry);
  const std::string path =
      replacing ? base::StrCat({"/api/shares/", listed.id}) : std::string("/api/shares");
  sync::AskForShares(
      replacing ? "PUT" : "POST", path, base::WriteJson(body).value_or(std::string()),
      base::BindOnce(
          [](uint64_t request, Collected collected, Listed listed, bool replacing, int tries,
             int status, std::string answer) {
            if (status == 200 || status == 201 || status == 204) {
              Made(request, &listed, collected.left_out, replacing, BC_SHARE_PROBLEM_NONE);
              ShareList();
              return;
            }
            // Unshared meanwhile, on another device: a new link. A new id the server has, which a
            // random 128 bits makes all but impossible: another one, once.
            if (((replacing && (status == 404 || status == 410)) ||
                 (!replacing && status == 409)) &&
                tries < 2) {
              const ShareLink link = NewShareLink();
              listed.id = link.id;
              listed.key = link.key;
              Send(request, std::move(collected), std::move(listed), /*replacing=*/false,
                   tries + 1);
              return;
            }
            Made(request, nullptr, collected.left_out, false, ProblemOf(status));
          },
          request, std::move(collected), std::move(listed), replacing, tries));
}

// The link the account has for |node|, from its server's list now, then Send.
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
            const std::optional<base::Value> json =
                base::JSONReader::Read(body, base::JSON_PARSE_RFC);
            bool replacing = false;
            if (key && json && json->is_list()) {
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
            }
            Send(request, std::move(collected), std::move(listed), replacing, 0);
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

// Looks at a navigation's answer for Ilo-Share: v1 at a link's address. Then the server's page
// doesn't commit: the core asks for the share's data and commits a page of its own in its place,
// as the ad block commits its warning.
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
    link_ = ShareLinkOf(url);
    if (!link_) {
      return PROCEED;
    }
    host_ = url.host();
    if (!Ask(url.Resolve(base::StrCat({"/s/", link_->id, "/data"})))) {
      return PROCEED;
    }
    return DEFER;
  }
  const char* GetNameForLogging() override { return "browser_core::ShareThrottle"; }

 private:
  bool Ask(const GURL& data) {
    network::mojom::NetworkContext* network = navigation_handle()
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
    // Its own, never the profile's, which could pick a client certificate or answer a sign-in for
    // the user.
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
        base::BindRepeating(&ShareThrottle::Redirected, weak_factory_.GetWeakPtr()));
    loader_->DownloadToString(factory_.get(),
                              base::BindOnce(&ShareThrottle::Answered, weak_factory_.GetWeakPtr()),
                              kShareBlobCap);
    timeout_.Start(FROM_HERE, kTimeout,
                   base::BindOnce(&ShareThrottle::TimedOut, base::Unretained(this)));
    return true;
  }

  void TimedOut() { Commit(ShareRefusedPage(false, std::nullopt), "unanswered"); }

  void Redirected(const GURL& from,
                  const net::RedirectInfo& redirect,
                  const network::mojom::URLResponseHead& head,
                  std::vector<std::string>* removed_headers) {
    Commit(ShareRefusedPage(false, std::nullopt), "redirected");
  }

  void Answered(std::optional<std::string> body) {
    const network::mojom::URLResponseHead* head = loader_->ResponseInfo();
    const int status = head && head->headers ? head->headers->response_code() : 0;
    const int error = loader_->NetError();
    if (status == 410) {
      Commit(ShareRefusedPage(true, std::nullopt), "unshared");
      return;
    }
    if (error == net::ERR_INSUFFICIENT_RESOURCES) {
      Commit(ShareRefusedPage(false, ShareRefusal::kTooBig), "too big");
      return;
    }
    // A link to a share the server never had was copied wrong.
    if (status == 404) {
      Commit(ShareRefusedPage(false, ShareRefusal::kKey), "unknown");
      return;
    }
    if (status != 200 || !body) {
      Commit(ShareRefusedPage(false, std::nullopt), "unanswered");
      return;
    }
    ShareOpened opened = OpenShare(base::as_byte_span(*body), *link_);
    if (!opened.snapshot) {
      Commit(ShareRefusedPage(false, opened.refusal),
             opened.refusal == ShareRefusal::kTooBig  ? "too big"
             : opened.refusal == ShareRefusal::kNewer ? "newer"
                                                      : "key");
      return;
    }
    std::string page = SharePage(*opened.snapshot, host_);
    ShareOffer::Expect(navigation_handle()->GetWebContents(),
                       navigation_handle()->GetNavigationId(), std::move(*opened.snapshot), host_);
    Commit(std::move(page), "shown");
  }

  // |said| is what the core's log says of it, which holds nothing of the share.
  void Commit(std::string page, const char* said) {
    LOG(ERROR) << "browser: share page " << said;
    timeout_.Stop();
    loader_.reset();
    weak_factory_.InvalidateWeakPtrs();
    // Last: the navigation, and this with it, can go here.
    CancelDeferredNavigation({CANCEL, net::ERR_BLOCKED_BY_CLIENT, std::move(page)});
  }

  std::optional<ShareLink> link_;
  std::string host_;
  NetworkRefuser refuser_;
  mojo::Remote<network::mojom::URLLoaderFactory> factory_;
  std::unique_ptr<network::SimpleURLLoader> loader_;
  base::OneShotTimer timeout_;
  base::WeakPtrFactory<ShareThrottle> weak_factory_{this};
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
  const std::optional<Collected> collected = Collect(kind, items, count);
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
  std::optional<Collected> collected = Collect(kind, items, count);
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
    Send(request, std::move(*collected), std::move(listed), /*replacing=*/false, 0);
  } else {
    SendForNode(request, std::move(*collected), std::move(listed));
  }
  return request;
}

void ShareList() {
  sync::AskForShares("GET", "/api/shares", std::string(), base::BindOnce(&ListAnswered));
}

void Unshare(const std::string& id) {
  if (id.size() != kIdLength || !std::ranges::all_of(id, [](char c) {
        return base::IsAsciiAlphaNumeric(c) || c == '-' || c == '_';
      })) {
    return;
  }
  sync::AskForShares("DELETE", base::StrCat({"/api/shares/", id}), std::string(),
                     base::BindOnce([](int status, std::string body) {
                       LOG(ERROR) << "browser: unshared, " << status;
                       ShareList();
                     }));
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
  if (!navigation.IsInPrimaryMainFrame() ||
      (navigation.IsRendererInitiated() && !navigation.HasUserGesture())) {
    return;
  }
  content::WebContents* contents = navigation.GetWebContents();
  const Tab* tab = Tab::FromWebContents(contents);
  const Space* space = tab ? Space::Holding(tab->id()) : nullptr;
  if (!space || space->agent() || space->off_the_record() ||
      contents->GetBrowserContext()->IsOffTheRecord() || IsAgentTab(contents)) {
    return;
  }
  registry.AddThrottle(std::make_unique<ShareThrottle>(registry));
}

}  // namespace browser_core
