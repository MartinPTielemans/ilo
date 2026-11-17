#include "browser_core/live_folder.h"

#include <array>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "browser_core/live_folder_rule.h"
#include "browser_core/profiles.h"
#include "browser_core/space.h"
#include "chrome/browser/profiles/profile.h"
#include "content/public/browser/storage_partition.h"
#include "net/base/isolation_info.h"
#include "net/base/load_flags.h"
#include "net/base/net_errors.h"
#include "net/cookies/site_for_cookies.h"
#include "net/http/http_request_headers.h"
#include "net/http/http_response_headers.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/mojom/url_response_head.mojom.h"
#include "url/gurl.h"
#include "url/origin.h"

namespace browser_core {

namespace {

constexpr net::NetworkTrafficAnnotationTag kLiveFolder =
    net::DefineNetworkTrafficAnnotation("browser_core_live_folder", R"(
      semantics {
        sender: "Live Folders and hover cards"
        description:
          "A Live Folder's lists, and a pinned tab's or favorite's hover card's: the GitHub pull "
          "requests waiting on the review of the user signed in to GitHub in the profile, and "
          "their own open ones, from GitHub's search, as the user's own visit to it would ask."
        trigger:
          "Only what the user does with a Live Folder in a Space: the window coming back, its "
          "Space shown, the folder made or opened, a GitHub page loaded in its Space, or Refresh. "
          "Or the user resting the pointer on a pinned tab or favorite whose page is on "
          "https://github.com, for its hover card, in a Space of their own, never an agent's or a "
          "private window's. At most once in two minutes for each profile, however many ask, but "
          "for a Live Folder's Refresh. Never on a timer, never again after a failure."
        data: "Two fixed searches, with the profile's GitHub cookies."
        destination: WEBSITE
      }
      policy {
        cookies_allowed: YES
        cookies_store: "user"
        setting:
          "Delete the Live Folder, or Convert to Folder, from its menu or the Space's. For hover "
          "cards, What's recent in hover cards in Settings, or the site's switch in Site Controls."
        policy_exception_justification:
          "Only for a Live Folder the user made, or a hover card the user rests on."
      })");

// How long a list is waited for. Not a timer that asks again: a request that hangs ends, failed,
// so the app may ask again on what the user does next.
constexpr base::TimeDelta kTimeout = base::Seconds(30);

constexpr std::array<live_folder::List, 2> kLists = {live_folder::List::kReviewRequests,
                                                     live_folder::List::kCreated};

// A profile's fetch of a source's lists, until both have come.
struct Fetch {
  std::array<std::unique_ptr<network::SimpleURLLoader>, kLists.size()> loaders;
  std::array<std::optional<live_folder::Answer>, kLists.size()> answers;
  size_t waiting = kLists.size();
};

std::map<live_folder::SharedFetches::Key, Fetch>& Fetches() {
  static base::NoDestructor<std::map<live_folder::SharedFetches::Key, Fetch>> fetches;
  return *fetches;
}

live_folder::SharedFetches& Shared() {
  static base::NoDestructor<live_folder::SharedFetches> shared;
  return *shared;
}

// The folders waiting on an answer, each once: one made plain while it waits leaves.
std::set<bc_id>& Waiting() {
  static base::NoDestructor<std::set<bc_id>> waiting;
  return *waiting;
}

// The folder's Space's profile, which is the user's own, loaded, and not off the record; else
// null, never another profile the Space's falls back to.
Profile* ProfileOf(const Space& space) {
  if (space.agent() || space.own()) {
    return nullptr;
  }
  Profile* profile = ProfileFor(space.profile());
  return profile && DirOf(profile) == space.profile() && !profile->IsOffTheRecord() ? profile
                                                                                    : nullptr;
}

static_assert(static_cast<int>(live_folder::Kind::kAnswered) == BC_LIVE_ANSWERED);
static_assert(static_cast<int>(live_folder::Kind::kSignedOut) == BC_LIVE_SIGNED_OUT);
static_assert(static_cast<int>(live_folder::Kind::kFailed) == BC_LIVE_FAILED);

void Report(bc_id folder,
            int answer,
            base::Time answered,
            const std::vector<live_folder::Item>& items) {
  // The rows are the user's account's, so nothing of them is logged, here or by the app.
  if (!Host().folder_live_answered) {
    return;
  }
  std::vector<bc_live_item> rows;
  rows.reserve(items.size());
  for (const live_folder::Item& item : items) {
    rows.push_back({.list = static_cast<int>(item.list),
                    .repo = item.repo.c_str(),
                    .number = item.number,
                    .title = item.title.c_str(),
                    .state = static_cast<int>(item.state)});
  }
  Host().folder_live_answered(Host().context, folder, answer,
                              answered.InMillisecondsSinceUnixEpoch(), rows.data(), rows.size());
}

// Answers the folder that asked with |asked|'s session, unless it was made plain since.
void Deliver(bc_id folder, const std::string& asked, const live_folder::Kept& kept) {
  if (!Waiting().erase(folder)) {
    return;
  }
  // Made plain, deleted or moved to an agent's Space while it waited: nobody asked for this.
  const Space* space = Space::HoldingFolder(folder);
  if (!space || !space->FolderLive(folder)) {
    return;
  }
  // Moved while it waited to a Space on another profile, or one not loaded: it fails rather than
  // give that Space rows from the session it was asked with.
  const std::string now = ProfileOf(*space) ? space->profile() : std::string();
  const live_folder::Outcome outcome = live_folder::OutcomeOf(kept.answers, asked, now);
  Report(folder, static_cast<int>(outcome.kind), kept.answered, outcome.items);
}

void Answered(const live_folder::SharedFetches::Key& key,
              size_t list,
              std::optional<std::string> body) {
  auto it = Fetches().find(key);
  if (it == Fetches().end()) {
    return;
  }
  Fetch& fetch = it->second;
  const std::unique_ptr<network::SimpleURLLoader> loader = std::move(fetch.loaders[list]);
  const network::mojom::URLResponseHead* head = loader->ResponseInfo();
  // A redirect fails the request (kError), so an answer is the site's own, at the address asked.
  const bool came =
      loader->NetError() == net::OK && body && head && head->headers &&
      head->headers->response_code() == 200 &&
      url::Origin::Create(loader->GetFinalURL()).IsSameOriginWith(GURL(live_folder::kSite));
  if (came) {
    fetch.answers[list] = live_folder::Parse(*body, kLists[list]);
  }
  if (--fetch.waiting) {
    return;
  }
  const Fetch done = std::move(it->second);
  Fetches().erase(it);
  Shared().Ended(key, {done.answers.begin(), done.answers.end()});
}

std::unique_ptr<network::SimpleURLLoader> Ask(live_folder::List list) {
  const url::Origin site = url::Origin::Create(GURL(live_folder::kSite));
  auto request = std::make_unique<network::ResourceRequest>();
  request->url = live_folder::ListUrl(list);
  request->method = net::HttpRequestHeaders::kGetMethod;
  request->headers.SetHeader(net::HttpRequestHeaders::kAccept, "application/json");
  // As the user's own visit to the site: its cookies, the site as the first party, and never a
  // redirect. The HTTP cache is neither read nor written, so no answer, and none of its rows,
  // stays on disk.
  request->credentials_mode = network::mojom::CredentialsMode::kInclude;
  request->mode = network::mojom::RequestMode::kSameOrigin;
  request->redirect_mode = network::mojom::RedirectMode::kError;
  request->destination = network::mojom::RequestDestination::kEmpty;
  request->load_flags = net::LOAD_DISABLE_CACHE;
  request->request_initiator = site;
  request->site_for_cookies = net::SiteForCookies::FromOrigin(site);
  request->trusted_params = network::ResourceRequest::TrustedParams();
  request->trusted_params->isolation_info = net::IsolationInfo::Create(
      net::IsolationInfo::RequestType::kOther, site, site, request->site_for_cookies);
  std::unique_ptr<network::SimpleURLLoader> loader =
      network::SimpleURLLoader::Create(std::move(request), kLiveFolder);
  loader->SetTimeoutDuration(kTimeout);
  return loader;
}

// Asks the site for |key|'s lists on |profile|'s session.
void Start(const live_folder::SharedFetches::Key& key, Profile* profile) {
  scoped_refptr<network::SharedURLLoaderFactory> factory =
      profile->GetDefaultStoragePartition()->GetURLLoaderFactoryForBrowserProcess();
  Fetch& fetch = Fetches()[key];
  for (size_t list = 0; list < kLists.size(); ++list) {
    fetch.loaders[list] = Ask(kLists[list]);
  }
  // Each loader answers a task later at the soonest, after both have started.
  for (size_t list = 0; list < kLists.size(); ++list) {
    fetch.loaders[list]->DownloadToString(factory.get(), base::BindOnce(&Answered, key, list),
                                          live_folder::kMaxBytes);
  }
}

}  // namespace

void FetchSourceLists(Profile* profile,
                      int source,
                      bool refresh,
                      base::OnceCallback<void(const live_folder::Kept&)> done) {
  const live_folder::SharedFetches::Key key(DirOf(profile), source);
  Shared().Ask(key, refresh, base::BindOnce(&Start, key, base::Unretained(profile)),
               std::move(done));
}

void FetchLiveFolder(bc_id folder, bool refresh) {
  const Space* space = Space::HoldingFolder(folder);
  const int source = space ? space->FolderLive(folder) : 0;
  if (!source || space->agent() || space->own() || !Waiting().insert(folder).second) {
    return;
  }
  Profile* profile = ProfileOf(*space);
  if (!profile) {
    // Its profile isn't loaded: the fetch fails, a task later, as one that went would. No lists
    // fails in OutcomeOf even if the profile has loaded by then.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Deliver, folder, space->profile(),
                                  live_folder::Kept{.answered = base::Time::Now()}));
    return;
  }
  FetchSourceLists(profile, source, refresh, base::BindOnce(&Deliver, folder, space->profile()));
}

void DropLiveFolderFetch(bc_id folder) {
  Waiting().erase(folder);
}

}  // namespace browser_core
