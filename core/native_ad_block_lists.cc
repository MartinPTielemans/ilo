#include "browser_core/native_ad_block_lists.h"

#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/files/file.h"
#include "base/files/file_util.h"
#include "base/files/important_file_writer.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/location.h"
#include "base/path_service.h"
#include "base/strings/string_util.h"
#include "base/task/thread_pool.h"
#include "chrome/browser/browser_process.h"
#include "chrome/common/chrome_paths.h"
#include "net/http/http_response_headers.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/mojom/fetch_api.mojom-shared.h"
#include "services/network/public/mojom/url_response_head.mojom.h"

namespace browser_core {

namespace {

constexpr net::NetworkTrafficAnnotationTag kLists =
    net::DefineNetworkTrafficAnnotation("browser_core_ad_block_lists", R"(
      semantics {
        sender: "Ad blocking"
        description:
          "Downloads the filter lists the browser's own ad blocker blocks with, as uBlock Origin "
          "downloads its own, so it blocks the trackers and ad servers the lists' authors have "
          "added since the browser was built."
        trigger:
          "37 s after launch, or 2 s if a list is twice as old as it stays current, and then "
          "weekly, for each list turned on in Settings › Ad blocking "
          "whose copy is older than the list says it stays current, a few days, and at once for a "
          "list the user turns on that the browser has no copy of."
        data: "None: a request for a list, with no cookie."
        destination: OTHER
        destination_other: "uBlock Origin's mirrors of the lists, then the lists' authors' sites."
      }
      policy {
        cookies_allowed: NO
        setting: "Settings › Ad blocking. Turning it off stops the downloads."
        policy_exception_justification:
          "It carries nothing about the user, and Settings turns it off."
      })");

// More than the largest list uBlock Origin offers, a few MB.
constexpr size_t kLargest = 16 * 1024 * 1024;

base::FilePath Folder() {
  base::FilePath user_data;
  base::PathService::Get(chrome::DIR_USER_DATA, &user_data);
  return user_data.Append(FILE_PATH_LITERAL("Ad Blocking")).Append(FILE_PATH_LITERAL("Lists"));
}

// Those of |lists| due for a download. Off the UI thread.
std::vector<NativeAdBlockList> Due(std::vector<NativeAdBlockList> lists) {
  std::vector<NativeAdBlockList> due;
  const base::Time now = base::Time::Now();
  for (NativeAdBlockList& list : lists) {
    base::File::Info info;
    if (!base::GetFileInfo(DownloadedAdBlockList(list.key), &info) ||
        now - info.last_modified >= list.update_after) {
      due.push_back(std::move(list));
    }
  }
  return due;
}

// Whether |text| reads as a filter list, and not a server's page about an error.
bool LooksLikeList(const std::string& text) {
  const std::string_view start = base::TrimWhitespaceASCII(
      std::string_view(text).substr(0, 1024), base::TRIM_LEADING);
  return !start.empty() && start.front() != '<';
}

// Keeps |text| as the list's copy, whole or not at all. Off the UI thread.
bool Keep(const base::FilePath& file, const std::string& text) {
  return base::CreateDirectory(file.DirName()) &&
         base::ImportantFileWriter::WriteFileAtomically(file, text);
}

// One pass over the lists due, which owns itself until it's done.
class Update {
 public:
  Update(std::vector<NativeAdBlockList> due,
         base::OnceCallback<void(std::vector<std::string>)> done)
      : due_(std::move(due)), done_(std::move(done)) {}
  Update(const Update&) = delete;
  Update& operator=(const Update&) = delete;

  // Downloads the next list from its next address, or ends.
  void Next() {
    while (list_ < due_.size() && address_ >= due_[list_].addresses.size()) {
      ++list_;
      address_ = 0;
    }
    if (list_ >= due_.size()) {
      std::move(done_).Run(std::move(updated_));
      delete this;
      return;
    }
    auto request = std::make_unique<network::ResourceRequest>();
    request->url = due_[list_].addresses[address_];
    request->credentials_mode = network::mojom::CredentialsMode::kOmit;
    loader_ = network::SimpleURLLoader::Create(std::move(request), kLists);
    loader_->DownloadToString(g_browser_process->shared_url_loader_factory().get(),
                              base::BindOnce(&Update::Downloaded, base::Unretained(this)),
                              kLargest);
  }

 private:
  void Downloaded(std::optional<std::string> body) {
    const network::mojom::URLResponseHead* head = loader_->ResponseInfo();
    const int status = head && head->headers ? head->headers->response_code() : 0;
    loader_.reset();
    if (!body || status != 200 || !LooksLikeList(*body)) {
      ++address_;
      Next();
      return;
    }
    base::ThreadPool::PostTaskAndReplyWithResult(
        FROM_HERE, {base::MayBlock(), base::TaskPriority::BEST_EFFORT},
        base::BindOnce(&Keep, DownloadedAdBlockList(due_[list_].key), std::move(*body)),
        base::BindOnce(&Update::Kept, base::Unretained(this)));
  }

  void Kept(bool kept) {
    if (kept) {
      updated_.push_back(due_[list_].key);
    }
    ++list_;
    address_ = 0;
    Next();
  }

  std::vector<NativeAdBlockList> due_;
  base::OnceCallback<void(std::vector<std::string>)> done_;
  size_t list_ = 0;
  size_t address_ = 0;
  std::unique_ptr<network::SimpleURLLoader> loader_;
  std::vector<std::string> updated_;
};

}  // namespace

base::FilePath DownloadedAdBlockList(const std::string& key) {
  return Folder().AppendASCII(key + ".txt");
}

bool AnyNativeAdBlockListLongOutOfDate(const std::vector<NativeAdBlockList>& lists) {
  const base::Time now = base::Time::Now();
  for (const NativeAdBlockList& list : lists) {
    base::File::Info info;
    if (!base::GetFileInfo(DownloadedAdBlockList(list.key), &info) ||
        now - info.last_modified >= 2 * list.update_after) {
      return true;
    }
  }
  return false;
}

void UpdateNativeAdBlockLists(std::vector<NativeAdBlockList> lists,
                              base::OnceCallback<void(std::vector<std::string>)> done) {
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::BEST_EFFORT},
      base::BindOnce(&Due, std::move(lists)),
      base::BindOnce(
          [](base::OnceCallback<void(std::vector<std::string>)> done,
             std::vector<NativeAdBlockList> due) {
            (new Update(std::move(due), std::move(done)))->Next();
          },
          std::move(done)));
}

}  // namespace browser_core
