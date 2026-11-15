#include "browser_core/live_folder.h"

#include <array>
#include <map>
#include <memory>
#include <optional>
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
        sender: "Live Folders"
        description:
          "A Live Folder's lists: the GitHub pull requests waiting on the review of the user "
          "signed in to GitHub in the Space's profile, and their own open ones, from GitHub's "
          "search, as the user's own visit to it would ask."
        trigger:
          "Only what the user does with a Live Folder in a Space: the window coming back, its "
          "Space shown, the folder made or opened, a GitHub page loaded in its Space, or Refresh. "
          "The app asks at most once in two minutes but for Refresh. Never on a timer, never "
          "again after a failure."
        data: "Two fixed searches, with the profile's GitHub cookies."
        destination: WEBSITE
      }
      policy {
        cookies_allowed: YES
        cookies_store: "user"
        setting: "Delete the Live Folder, or Convert to Folder, from its menu or the Space's."
        policy_exception_justification: "Only for a Live Folder the user made."
      })");

// How long a list is waited for. Not a timer that asks again: a request that hangs ends, failed,
// so the app may ask again on what the user does next.
constexpr base::TimeDelta kTimeout = base::Seconds(30);

constexpr std::array<live_folder::List, 2> kLists = {live_folder::List::kReviewRequests,
                                                     live_folder::List::kCreated};

// A folder's fetch, until both its lists have come.
struct Fetch {
  // The directory of the profile whose session it asked with: its rows are that profile's alone.
  std::string profile;
  std::array<std::unique_ptr<network::SimpleURLLoader>, kLists.size()> loaders;
  std::array<std::optional<live_folder::Answer>, kLists.size()> answers;
  size_t waiting = kLists.size();
};

std::map<bc_id, Fetch>& Fetches() {
  static base::NoDestructor<std::map<bc_id, Fetch>> fetches;
  return *fetches;
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

void Report(bc_id folder, int answer, const std::vector<live_folder::Item>& items) {
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
  Host().folder_live_answered(Host().context, folder, answer, rows.data(), rows.size());
}

void Answered(bc_id folder, size_t list, std::optional<std::string> body) {
  auto it = Fetches().find(folder);
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
  // Made plain, deleted or moved to an agent's Space while it waited: nobody asked for this.
  const Space* space = Space::HoldingFolder(folder);
  if (!space || !space->FolderLive(folder)) {
    return;
  }
  // Moved while it waited to a Space on another profile, or one not loaded: it fails rather than
  // give that Space rows from the session it was asked with.
  const std::string now = ProfileOf(*space) ? space->profile() : std::string();
  const live_folder::Outcome outcome =
      live_folder::OutcomeOf({done.answers.begin(), done.answers.end()}, done.profile, now);
  Report(folder, static_cast<int>(outcome.kind), outcome.items);
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

}  // namespace

void FetchLiveFolder(bc_id folder) {
  const Space* space = Space::HoldingFolder(folder);
  if (!space || !space->FolderLive(folder) || space->agent() || space->own() ||
      Fetches().contains(folder)) {
    return;
  }
  Profile* profile = ProfileOf(*space);
  if (!profile) {
    // Its profile isn't loaded: the fetch fails, a task later, as one that went would.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](bc_id folder) {
                         const Space* space = Space::HoldingFolder(folder);
                         if (space && space->FolderLive(folder)) {
                           Report(folder, BC_LIVE_FAILED, {});
                         }
                       },
                       folder));
    return;
  }
  scoped_refptr<network::SharedURLLoaderFactory> factory =
      profile->GetDefaultStoragePartition()->GetURLLoaderFactoryForBrowserProcess();
  Fetch& fetch = Fetches()[folder];
  fetch.profile = space->profile();
  for (size_t list = 0; list < kLists.size(); ++list) {
    fetch.loaders[list] = Ask(kLists[list]);
  }
  // Each loader answers a task later at the soonest, after both have started.
  for (size_t list = 0; list < kLists.size(); ++list) {
    fetch.loaders[list]->DownloadToString(factory.get(), base::BindOnce(&Answered, folder, list),
                                          live_folder::kMaxBytes);
  }
}

void DropLiveFolderFetch(bc_id folder) {
  Fetches().erase(folder);
}

}  // namespace browser_core
