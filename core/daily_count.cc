#include "browser_core/daily_count.h"

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <memory>
#include <optional>
#include <string>
#include <utility>

#include "base/command_line.h"
#include "base/functional/bind.h"
#include "base/json/json_writer.h"
#include "base/location.h"
#include "base/memory/scoped_refptr.h"
#include "base/no_destructor.h"
#include "base/system/sys_info.h"
#include "base/task/sequenced_task_runner.h"
#include "base/values.h"
#include "build/build_config.h"
#include "chrome/browser/browser_process.h"
#include "net/base/url_util.h"
#include "net/http/http_response_headers.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/mojom/fetch_api.mojom-shared.h"
#include "url/gurl.h"
#include "url/url_constants.h"

#if BUILDFLAG(IS_WIN)
#include "base/win/windows_version.h"
#endif

namespace browser_core::daily_count {

namespace {

constexpr char kEndpoint[] = "daily-count-url";

constexpr net::NetworkTrafficAnnotationTag kDailyCount =
    net::DefineNetworkTrafficAnnotation("browser_core_daily_count", R"(
      semantics {
        sender: "Daily usage count"
        description:
          "One anonymous count a day, which the server the build names adds to that day's totals, "
          "so its developers know how many copies are in use. It carries no ID, so no two counts "
          "can be linked to each other or to a copy."
        trigger:
          "About 30 s after launch, and when the browser becomes active again after the UTC day "
          "has changed, at most once per UTC day, while Settings › Privacy › Send a Daily Usage "
          "Count is on. One that fails waits for the next launch that day; a missed day is lost."
        data:
          "The browser's version and channel (release or nightly), the OS, its major version, the "
          "chip (arm64 or x64), and whether this is the copy's first run, its first count today "
          "(always), this ISO week and this month. No ID, no cookie, nothing about pages, tabs, "
          "Spaces, agents, accounts or settings."
        destination: OTHER
        destination_other: "The server the build names, ilo's own in official builds, which "
          "anyone can host."
      }
      policy {
        cookies_allowed: NO
        setting:
          "Settings › Privacy › Send a Daily Usage Count, on by default. Turning it off stops it "
          "at once."
        policy_exception_justification:
          "It carries nothing about the user or what they do, and Settings turns it off."
      })");

// https, or http to this machine for the checks' endpoint. A build that names no server passes
// none, and nothing is sent.
const GURL& Endpoint() {
  static const base::NoDestructor<GURL> endpoint([] {
    GURL url(base::CommandLine::ForCurrentProcess()->GetSwitchValueASCII(kEndpoint));
    const bool allowed =
        url.is_valid() && (url.SchemeIs(url::kHttpsScheme) ||
                           (url.SchemeIs(url::kHttpScheme) && net::IsLocalhost(url)));
    return allowed ? url : GURL();
  }());
  return *endpoint;
}

std::unique_ptr<network::SimpleURLLoader>& Loader() {
  static base::NoDestructor<std::unique_ptr<network::SimpleURLLoader>> loader;
  return *loader;
}

// A version as Cargo.toml has it, so the field can carry nothing else.
bool IsVersion(std::string_view version) {
  return !version.empty() && version.size() <= 32 &&
         std::ranges::all_of(version, [](char c) { return (c >= '0' && c <= '9') || c == '.'; });
}

std::string_view Os() {
#if BUILDFLAG(IS_MAC)
  return "macos";
#elif BUILDFLAG(IS_WIN)
  return "windows";
#elif BUILDFLAG(IS_LINUX)
  return "linux";
#else
  return "other";
#endif
}

// macOS's major version, Windows' 10 or 11, which Windows itself reports as 10 alike, or Linux's
// kernel's, as Linux has no version of its own.
int OsMajor() {
#if BUILDFLAG(IS_WIN)
  return base::win::GetVersion() >= base::win::Version::WIN11 ? 11 : 10;
#else
  int32_t major = 0;
  int32_t minor = 0;
  int32_t bugfix = 0;
  base::SysInfo::OperatingSystemVersionNumbers(&major, &minor, &bugfix);
  return major;
#endif
}

std::string_view Chip() {
#if defined(ARCH_CPU_ARM64)
  return "arm64";
#elif defined(ARCH_CPU_X86_64)
  return "x64";
#else
  return "other";
#endif
}

void Answer(base::OnceCallback<void(bool)> sent, bool ok) {
  base::SequencedTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE,
                                                           base::BindOnce(std::move(sent), ok));
}

void Sent(base::OnceCallback<void(bool)> sent, scoped_refptr<net::HttpResponseHeaders> headers) {
  Loader().reset();
  const bool ok = headers && headers->response_code() / 100 == 2;
  // check.sh waits for these lines.
  fputs(ok ? "browser: daily count sent\n" : "browser: daily count not sent\n", stderr);
  std::move(sent).Run(ok);
}

}  // namespace

void Send(std::string_view version,
          std::string_view channel,
          bool first,
          bool weekly,
          bool monthly,
          base::OnceCallback<void(bool)> sent) {
  if (!Endpoint().is_valid() || Loader() || !g_browser_process || !IsVersion(version) ||
      (channel != "release" && channel != "nightly")) {
    Answer(std::move(sent), false);
    return;
  }
  std::optional<std::string> body = base::WriteJson(base::DictValue()
                                                        .Set("version", version)
                                                        .Set("channel", channel)
                                                        .Set("os", Os())
                                                        .Set("os_major", OsMajor())
                                                        .Set("chip", Chip())
                                                        .Set("first", first)
                                                        .Set("daily", true)
                                                        .Set("weekly", weekly)
                                                        .Set("monthly", monthly));
  if (!body) {
    Answer(std::move(sent), false);
    return;
  }
  auto request = std::make_unique<network::ResourceRequest>();
  request->url = Endpoint();
  request->method = "POST";
  request->credentials_mode = network::mojom::CredentialsMode::kOmit;
  Loader() = network::SimpleURLLoader::Create(std::move(request), kDailyCount);
  Loader()->AttachStringForUpload(*body, "application/json");
  Loader()->DownloadHeadersOnly(g_browser_process->shared_url_loader_factory().get(),
                                base::BindOnce(&Sent, std::move(sent)));
}

}  // namespace browser_core::daily_count
