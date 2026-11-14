#include "browser_core/crash_reports.h"

#include <algorithm>
#include <array>
#include <cstdio>
#include <ctime>
#include <iterator>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/base_paths.h"
#include "base/command_line.h"
#include "base/debug/dump_without_crashing.h"
#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/functional/bind.h"
#include "base/immediate_crash.h"
#include "base/location.h"
#include "base/memory/scoped_refptr.h"
#include "base/no_destructor.h"
#include "base/path_service.h"
#include "base/strings/string_util.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "browser_core/session.h"
#include "build/build_config.h"
#include "chrome/browser/after_startup_task_utils.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "chrome/browser/sessions/exit_type_service.h"
#include "chrome/common/chrome_paths.h"
#include "chrome/common/chrome_switches.h"
#include "components/crash/core/app/crashpad.h"
#include "components/crash/core/common/crash_keys.h"
#include "components/prefs/pref_service.h"
#include "net/base/mime_util.h"
#include "net/http/http_response_headers.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/mojom/fetch_api.mojom-shared.h"
#include "third_party/crashpad/crashpad/client/crash_report_database.h"
#include "third_party/crashpad/crashpad/client/crashpad_info.h"
#include "url/gurl.h"

#if BUILDFLAG(IS_MAC)
#include <mach/mach.h>

#include "base/apple/scoped_mach_port.h"
#include "third_party/crashpad/crashpad/client/crashpad_client.h"
#endif

namespace browser_core::crash_reports {

namespace {

using crashpad::CrashReportDatabase;

// Not Chrome's --crash-test, which crashes the browser before its first window.
constexpr char kCrashTest[] = "crash-after-start";
constexpr char kCollector[] = "crash-upload-url";

constexpr net::NetworkTrafficAnnotationTag kCrashReport =
    net::DefineNetworkTrafficAnnotation("browser_core_crash_report", R"(
      semantics {
        sender: "Crash reports"
        description:
          "Sends Crashpad's minidump of a crash to the crash collector the build names, so its "
          "developers can fix it."
        trigger:
          "The user has Settings › Privacy › Send Crash Reports on, and a report hasn't been sent "
          "yet, checked once the browser has started and when the user turns it on. Or, with it "
          "off, the user pressed Send Report when the browser offered to send the report of its "
          "crash the time before."
        data:
          "The minidump: the crashed process's threads, their stacks and registers, its loaded "
          "modules and Crashpad's annotations: the browser's version, its switches but those "
          "naming the user's folders, its features and field trials, the GPU, the extensions' "
          "IDs and the event it was handling. For a Sentry collector, a field saying to keep no "
          "IP address with the report."
        destination: OTHER
        destination_other: "The collector the build names, which anyone can host."
      }
      policy {
        cookies_allowed: NO
        setting: "Settings › Privacy › Send Crash Reports, off by default."
        policy_exception_justification:
          "Off unless the user turns it on, or says to send one report."
      })");

// Where Crashpad writes the reports, if that's the profile's folder, as PlaceReports has it. On
// Windows chrome_elf starts Crashpad before the core runs, in Chromium's default folder, which any
// other Chromium on the machine shares, so the core offers, sends and deletes none of those.
std::optional<base::FilePath> ProfilesReports() {
  std::optional<base::FilePath> written = crash_reporter::GetCrashpadDatabasePath();
  base::FilePath profiles;
  if (!written || !base::PathService::Get(chrome::DIR_CRASH_DUMPS, &profiles) ||
      *written != profiles) {
    return std::nullopt;
  }
  return written;
}

// Without the profile's own reports there's no collector, as in a build that names none.
const GURL& Collector() {
  static const base::NoDestructor<GURL> collector([] {
    GURL url(base::CommandLine::ForCurrentProcess()->GetSwitchValueASCII(kCollector));
    return url.is_valid() && url.SchemeIsHTTPOrHTTPS() && ProfilesReports() ? url : GURL();
  }());
  return *collector;
}

// Where the reports are read and deleted: off the UI thread, one thing at a time, and never in the
// way of what the user is doing.
scoped_refptr<base::SequencedTaskRunner> Reports() {
  static base::NoDestructor<scoped_refptr<base::SequencedTaskRunner>> reports(
      base::ThreadPool::CreateSequencedTaskRunner(
          {base::MayBlock(), base::TaskPriority::BEST_EFFORT,
           base::TaskShutdownBehavior::SKIP_ON_SHUTDOWN}));
  return *reports;
}

// The id of the last report the collector took while Send Crash Reports was on, which Report a
// Problem lists (bc_last_crash_report). Its file is beside the reports, not in the Spaces' file,
// whose backups would keep it, and turning Send Crash Reports off deletes it.
std::string& Last() {
  static base::NoDestructor<std::string> last;
  return *last;
}

std::optional<base::FilePath> LastFile() {
  std::optional<base::FilePath> reports = ProfilesReports();
  return reports ? std::optional(reports->Append(FILE_PATH_LITERAL("Last Sent"))) : std::nullopt;
}

std::string ReadLast() {
  std::optional<base::FilePath> file = LastFile();
  std::string id;
  if (!file || !base::ReadFileToStringWithMaxSize(*file, &id, 32) || id.size() != 32 ||
      !std::ranges::all_of(id, base::IsHexDigit<char>)) {
    return std::string();
  }
  return id;
}

void WriteLast(const std::string& id) {
  if (std::optional<base::FilePath> file = LastFile()) {
    base::WriteFile(*file, id);
  }
}

void DeleteLast() {
  if (std::optional<base::FilePath> file = LastFile()) {
    base::DeleteFile(*file);
  }
}

// The Crashpad database the handler writes, if it's the profile's, as another reader of it, as
// Chrome's crashes page is.
std::unique_ptr<CrashReportDatabase> OpenReports() {
  std::optional<base::FilePath> path = ProfilesReports();
  return path ? CrashReportDatabase::InitializeWithoutCreating(*path) : nullptr;
}

// The user's home, read as each process starts, before its sandbox, and kept for the page
// processes the zygote forks on Linux, which can't read it.
base::FilePath& Home() {
  static base::NoDestructor<base::FilePath> home;
  return *home;
}

// The switches the crash keys leave out: Chrome's (chrome/common/crash_keys.cc), whose filter
// can't be called from here, and those naming the user's folders, as a path in the home names the
// user: the profile's, wherever it is, and any other.
bool LeftOutSwitch(const std::string& flag) {
  static constexpr auto kLeftOut = std::to_array<std::string_view>({
      "string-annotations", "enable-logging", "logging-level", "type", "v", "vmodule",
      "gpu-preferences", "metrics-client-id", "user-data-dir",
  });
  if (crash_keys::IsDefaultBoringSwitch(flag) || flag.starts_with("/prefetch:")) {
    return true;
  }
  const std::string home = Home().AsUTF8Unsafe();
  if (!home.empty() && flag.find(home) != std::string::npos) {
    return true;
  }
  return flag.starts_with("--") &&
         std::ranges::contains(kLeftOut, std::string_view(flag).substr(2, flag.find('=') - 2));
}

// The report's id as a Sentry event's: 32 hex digits, without the UUID's dashes.
std::string Id(const crashpad::UUID& uuid) {
  std::string id = uuid.ToString();
  std::erase(id, '-');
  return id;
}

// For a Sentry collector: the report's own id as the event's, so the id Report a Problem lists
// finds it (bc_last_crash_report), and no IP address, which it would take from its connection and
// keep with the report, with the place it gives. Other collectors leave the field.
std::string SentryFields(const crashpad::UUID& uuid) {
  return R"({"event_id":")" + Id(uuid) +
         R"(","sdk":{"name":"browser_core","version":"1","settings":{"infer_ip":"never"}}})";
}

struct Unsent {
  crashpad::UUID uuid;
  std::string minidump;
};

// The oldest report not yet sent, read whole. One that can't be read is deleted.
std::optional<Unsent> ReadOldest() {
  std::unique_ptr<CrashReportDatabase> reports = OpenReports();
  std::vector<CrashReportDatabase::Report> pending;
  if (!reports || reports->GetPendingReports(&pending) != CrashReportDatabase::kNoError) {
    return std::nullopt;
  }
  std::ranges::sort(pending, {}, &CrashReportDatabase::Report::creation_time);
  for (const CrashReportDatabase::Report& report : pending) {
    std::string minidump;
    if (base::ReadFileToString(report.file_path, &minidump)) {
      return Unsent{report.uuid, std::move(minidump)};
    }
    reports->DeleteReport(report.uuid);
  }
  return std::nullopt;
}

// The report `uuid`, read whole, if it's still waiting to be sent.
std::optional<Unsent> Read(const crashpad::UUID& uuid) {
  std::unique_ptr<CrashReportDatabase> reports = OpenReports();
  CrashReportDatabase::Report report;
  std::string minidump;
  if (!reports || reports->LookUpCrashReport(uuid, &report) != CrashReportDatabase::kNoError ||
      report.uploaded || !base::ReadFileToString(report.file_path, &minidump)) {
    return std::nullopt;
  }
  return Unsent{uuid, std::move(minidump)};
}

struct Newest {
  crashpad::UUID uuid;
  time_t created;
};

// The newest report waiting to be sent, if it was written after `after`.
std::optional<Newest> NewestAfter(double after) {
  std::unique_ptr<CrashReportDatabase> reports = OpenReports();
  std::vector<CrashReportDatabase::Report> pending;
  if (!reports || reports->GetPendingReports(&pending) != CrashReportDatabase::kNoError) {
    return std::nullopt;
  }
  auto newest = std::ranges::max_element(pending, {}, &CrashReportDatabase::Report::creation_time);
  if (newest == pending.end() || newest->creation_time <= after) {
    return std::nullopt;
  }
  return Newest{newest->uuid, newest->creation_time};
}

void Delete(const crashpad::UUID& uuid) {
  if (std::unique_ptr<CrashReportDatabase> reports = OpenReports()) {
    reports->DeleteReport(uuid);
  }
}

// Every report the collector doesn't have: the handler's pending ones, and any Chromium recorded as
// not sent.
void DeleteUnsent() {
  std::unique_ptr<CrashReportDatabase> reports = OpenReports();
  if (!reports) {
    return;
  }
  std::vector<CrashReportDatabase::Report> unsent;
  std::vector<CrashReportDatabase::Report> completed;
  reports->GetPendingReports(&unsent);
  reports->GetCompletedReports(&completed);
  std::ranges::copy_if(completed, std::back_inserter(unsent),
                       [](const CrashReportDatabase::Report& report) { return !report.uploaded; });
  int deleted = 0;
  for (const CrashReportDatabase::Report& report : unsent) {
    deleted += reports->DeleteReport(report.uuid) == CrashReportDatabase::kNoError;
  }
  // check.sh waits for this line.
  fprintf(stderr, "browser: crash reports deleted %d\n", deleted);
}

// Whether a report is on its way. Reports go one at a time, so none goes twice.
bool g_busy = false;
// Counts the times the user turned sending off, so what was on its way then stops where it is.
int g_round = 0;

// The report the browser offered to send, until the user answers.
std::optional<crashpad::UUID>& Offered() {
  static std::optional<crashpad::UUID> offered;
  return offered;
}

std::unique_ptr<network::SimpleURLLoader>& Loader() {
  static base::NoDestructor<std::unique_ptr<network::SimpleURLLoader>> loader;
  return *loader;
}

void SendNext();

void Sent(int round,
          const crashpad::UUID& uuid,
          scoped_refptr<net::HttpResponseHeaders> headers) {
  if (round != g_round) {
    return;
  }
  Loader().reset();
  g_busy = false;
  if (!headers || headers->response_code() / 100 != 2) {
    // check.sh waits for this line.
    fprintf(stderr, "browser: crash report not sent\n");
    return;
  }
  // check.sh waits for this line.
  fprintf(stderr, "browser: crash report sent\n");
  // Kept only while Send Crash Reports is on, not for the one report the user said to send.
  if (Sending()) {
    Last() = Id(uuid);
    Reports()->PostTask(FROM_HERE, base::BindOnce(&WriteLast, Last()));
  }
  Reports()->PostTaskAndReply(FROM_HERE, base::BindOnce(&Delete, uuid), base::BindOnce(&SendNext));
}

// The minidump, as a Breakpad collector takes it, which GlitchTip's and Sentry's minidump endpoints
// are, and Sentry's fields. Crashpad's handler would add the minidump's annotations as fields,
// which the minidump has anyway. `asked`, it's the one report the user said to send.
void Send(int round, bool asked, std::optional<Unsent> report) {
  if (round != g_round) {
    return;
  }
  if (!report || !(asked || Sending()) || !Collector().is_valid() || !g_browser_process) {
    g_busy = false;
    return;
  }
  const std::string boundary = net::GenerateMimeMultipartBoundary();
  std::string body;
  net::AddMultipartValueForUploadWithFileName("upload_file_minidump",
                                              report->uuid.ToString() + ".dmp", report->minidump,
                                              boundary, "application/octet-stream", &body);
  net::AddMultipartValueForUpload("sentry", SentryFields(report->uuid), boundary, "", &body);
  net::AddMultipartFinalDelimiterForUpload(boundary, &body);
  auto request = std::make_unique<network::ResourceRequest>();
  request->url = Collector();
  request->method = "POST";
  request->credentials_mode = network::mojom::CredentialsMode::kOmit;
  Loader() = network::SimpleURLLoader::Create(std::move(request), kCrashReport);
  Loader()->AttachStringForUpload(std::move(body), "multipart/form-data; boundary=" + boundary);
  Loader()->DownloadHeadersOnly(g_browser_process->shared_url_loader_factory().get(),
                                base::BindOnce(&Sent, round, report->uuid));
}

void SendNext() {
  if (g_busy || !Sending()) {
    return;
  }
  g_busy = true;
  Reports()->PostTaskAndReplyWithResult(FROM_HERE, base::BindOnce(&ReadOldest),
                                        base::BindOnce(&Send, g_round, false));
}

void Offer(std::optional<Newest> report) {
  if (!report || Sending()) {
    return;
  }
  UserSettings().crash_report_offered = report->created;
  SessionChanged();
  Offered() = report->uuid;
  // check.sh waits for this line.
  fprintf(stderr, "browser: crash report offered\n");
  if (Host().crash_report_offered) {
    Host().crash_report_offered(Host().context);
  }
}

void OfferNewest() {
  Reports()->PostTaskAndReplyWithResult(
      FROM_HERE, base::BindOnce(&NewestAfter, UserSettings().crash_report_offered),
      base::BindOnce(&Offer));
}

// Chrome marks a profile's session crashed as it starts, and clean as it quits.
bool LastSessionCrashed() {
  Profile* profile = ProfileManager::GetLastUsedProfileIfLoaded();
  return profile && ExitTypeService::GetLastSessionExitType(profile) == ExitType::kCrashed;
}

// Whether Crashpad's handler takes crashes yet. On the Mac the browser doesn't wait for it to start
// (chromium-crashpad-mac-async-start.patch): it takes them once it has the receive right of the
// port the browser's crashes go to.
bool HandlerReady() {
#if BUILDFLAG(IS_MAC)
  base::apple::ScopedMachSendRight port =
      crash_reporter::GetCrashpadClient().GetHandlerMachPort();
  mach_port_type_t type = 0;
  return port.is_valid() && mach_port_type(mach_task_self(), port.get(), &type) == KERN_SUCCESS &&
         !(type & MACH_PORT_TYPE_RECEIVE);
#else
  return true;
#endif
}

// Whether a profile's crashed mark is on disk (ProfileStarted).
bool g_marked = false;

void CrashOnceReady() {
  // Not before the mark: without it the next launch would offer nothing, and the check says so.
  if (!HandlerReady() || !g_marked) {
    // Only with the test's switch, so no other launch wakes for this.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostDelayedTask(
        FROM_HERE, base::BindOnce(&CrashOnceReady), base::Milliseconds(50));
    return;
  }
  // check.sh waits for this line.
  fprintf(stderr, "browser: crash test\n");
  // A crash asked for is no news for macOS's crash reporter to show the user.
  crashpad::CrashpadInfo::GetCrashpadInfo()->set_system_crash_reporter_forwarding(
      crashpad::TriState::kDisabled);
  base::ImmediateCrash();
}

}  // namespace

void PlaceReports() {
  base::FilePath profiles =
      base::CommandLine::ForCurrentProcess()->GetSwitchValuePath(switches::kUserDataDir);
  if (!profiles.empty()) {
    base::PathService::Override(chrome::DIR_CRASH_DUMPS,
                                profiles.Append(FILE_PATH_LITERAL("Crashpad")));
  }
}

void ProfileStarted(Profile* profile) {
  if (!Collector().is_valid() || profile->IsOffTheRecord()) {
    return;
  }
  // Chrome copies the prefs here and writes them on its file sequence.
  profile->GetPrefs()->CommitPendingWrite(base::BindOnce([] { g_marked = true; }));
}

void LeaveOutPages() {
  crashpad::CrashpadInfo::GetCrashpadInfo()->set_gather_indirectly_referenced_memory(
      crashpad::TriState::kDisabled, 0);
  base::debug::SetDumpWithoutCrashingFunction(nullptr);
}

void LeaveOutFolders() {
  if (Home().empty()) {
    base::PathService::Get(base::DIR_HOME, &Home());
  }
  crash_keys::SetSwitchesFromCommandLine(*base::CommandLine::ForCurrentProcess(), &LeftOutSwitch);
}

std::string CollectorHost() {
  return std::string(Collector().host());
}

std::string LastReport() {
  return Last();
}

bool Sending() {
  return UserSettings().send_crash_reports && Collector().is_valid();
}

void SetSending(bool send) {
  if (UserSettings().send_crash_reports == send) {
    return;
  }
  UserSettings().send_crash_reports = send;
  SessionChanged();
  if (send) {
    SendNext();
    return;
  }
  ++g_round;
  g_busy = false;
  Loader().reset();
  Last().clear();
  Reports()->PostTask(FROM_HERE, base::BindOnce(&DeleteLast));
  Reports()->PostTask(FROM_HERE, base::BindOnce(&DeleteUnsent));
}

void Started() {
  if (base::CommandLine::ForCurrentProcess()->HasSwitch(kCrashTest)) {
    // Posted, so the app has done with being told it started.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE,
                                                                base::BindOnce(&CrashOnceReady));
    return;
  }
  if (Sending()) {
    Reports()->PostTaskAndReplyWithResult(FROM_HERE, base::BindOnce(&ReadLast),
                                          base::BindOnce([](std::string id) {
                                            // Unless one was sent or the switch turned off since.
                                            if (Sending() && Last().empty()) {
                                              Last() = std::move(id);
                                            }
                                          }));
    AfterStartupTaskUtils::PostTask(FROM_HERE, base::SequencedTaskRunner::GetCurrentDefault(),
                                    base::BindOnce(&SendNext));
  } else if (Collector().is_valid() && LastSessionCrashed()) {
    AfterStartupTaskUtils::PostTask(FROM_HERE, base::SequencedTaskRunner::GetCurrentDefault(),
                                    base::BindOnce(&OfferNewest));
  }
  if (!Sending()) {
    // Turned off as the browser quit, before its file was deleted.
    Reports()->PostTask(FROM_HERE, base::BindOnce(&DeleteLast));
  }
}

void Answer(bool send) {
  std::optional<crashpad::UUID> uuid = std::exchange(Offered(), std::nullopt);
  if (!uuid) {
    return;
  }
  // check.sh waits for this line.
  fprintf(stderr, "browser: crash report %s\n", send ? "to send" : "not to send");
  if (!send) {
    Reports()->PostTask(FROM_HERE, base::BindOnce(&Delete, *uuid));
    return;
  }
  // Already on its way with the rest, as the user turned sending on since.
  if (g_busy) {
    return;
  }
  g_busy = true;
  Reports()->PostTaskAndReplyWithResult(FROM_HERE, base::BindOnce(&Read, *uuid),
                                        base::BindOnce(&Send, g_round, true));
}

}  // namespace browser_core::crash_reports
