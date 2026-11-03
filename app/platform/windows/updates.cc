// Updates on Windows (docs/ux-spec.md › Updates), for a copy installed with winget or from the
// website: every 8 hours, and when asked, it reads the Mac's appcast for a newer Windows installer,
// downloads it in the background and checks its EdDSA signature against the key built in, then
// says it's ready, which Restart to Update in the sidebar and Settings › About finish. The
// installer runs silently once ilo has gone: after Restart to Update it relaunches ilo, after any
// other quit it doesn't, so the next launch is the new version, as Sparkle does on the Mac. Nothing
// shows a window of its own. WinSparkle, which this replaces, showed one before every download and
// ran the installer in front of the user (#1418), and has no way not to. A copy from the Microsoft
// Store is an MSIX, which the Store updates, so it never starts.

#include <windows.h>

#include <appmodel.h>
#include <bcrypt.h>
#include <shlobj.h>
#include <winhttp.h>

#include <algorithm>
#include <atomic>
#include <cctype>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

#include "../host.h"

// host.cc's: runs |task| on the main thread.
void HostLater(void (*task)(void));
// ed25519.cc's.
bool Ed25519Verify(const uint8_t signature[64], const uint8_t public_key[32],
                   const uint8_t hash[64]);

namespace {

// A release build's, from build.rs: the feed, the key its updates are signed with, and the version
// and build number the appcast's are, as chrome.exe's own version resource is Chromium's. A nightly
// feed beside it, if the build carries one. A check build's (UPDATE_CHECK_BUILD) are on the check's
// own server instead (check/nightly.sh).
#if defined(UPDATE_FEED) && defined(UPDATE_PUBLIC_KEY) && defined(APP_VERSION) && defined(APP_BUILD)
constexpr bool kReleaseBuild = true;
#else
constexpr bool kReleaseBuild = false;
#define UPDATE_FEED ""
#define UPDATE_PUBLIC_KEY ""
#define APP_VERSION ""
#define APP_BUILD ""
#endif
#ifndef UPDATE_FEED_NIGHTLY
#define UPDATE_FEED_NIGHTLY ""
#endif
#ifdef UPDATE_CHECK_BUILD
constexpr bool kCheckBuild = true;
#else
constexpr bool kCheckBuild = false;
#endif
#define WIDE2(s) L##s
#define WIDE(s) WIDE2(s)

// How long between checks, as Sparkle's on the Mac, in 100 ns, as a FILETIME counts.
constexpr ULONGLONG kInterval = 8ULL * 60 * 60 * 10'000'000;
// Where the last check's time is kept, under HKEY_CURRENT_USER, so a launch doesn't check again
// before it's due, and the build a quit started installing, so the next launch can tell whether it
// went in. A check build's are its own, so it never moves a copy of ilo's next check.
constexpr const wchar_t* kSettings =
    kCheckBuild ? L"Software\\ilo\\Updates (check)" : L"Software\\ilo\\Updates";
constexpr wchar_t kInstalling[] = L"Installing";
// No appcast is this big, and no installer: past it, the download stops.
constexpr size_t kMostFeed = 4 << 20;
constexpr ULONGLONG kMostInstaller = 1ULL << 30;

void (*g_quit)(void) = nullptr;
void (*g_changed)(int stage, const char* version) = nullptr;
// Wakes the thread to check now.
HANDLE g_wake = nullptr;
// The stage the thread last said and its version, for the main thread to tell.
std::mutex g_said_lock;
int g_stage = HOST_UPDATE_IDLE;
std::string g_version;
// The installer, once verified, held open so nothing can change it until it runs, the build it
// installs, and whether the quit it runs at is Restart to Update's, which relaunches ilo.
std::atomic<bool> g_ready{false};
std::wstring g_installer;
// Whether the checks read the nightly feed (host_follow_nightly).
std::atomic<bool> g_nightly{false};
ULONGLONG g_build = 0;
HANDLE g_held = INVALID_HANDLE_VALUE;
bool g_relaunch = false;

// An MSIX has a package identity; winget's and the website's installs don't.
bool FromTheStore() {
  UINT32 length = 0;
  return GetCurrentPackageFullName(&length, nullptr) != APPMODEL_ERROR_NO_PACKAGE;
}

std::vector<uint8_t> Base64(const std::string& text) {
  static const std::string kAlphabet =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  std::vector<uint8_t> bytes;
  int value = 0, bits = -8;
  for (char c : text) {
    if (c == '=' || c == ' ' || c == '\n' || c == '\r' || c == '\t') {
      continue;
    }
    const size_t at = kAlphabet.find(c);
    if (at == std::string::npos) {
      return {};
    }
    value = (value << 6) + static_cast<int>(at);
    bits += 6;
    if (bits >= 0) {
      bytes.push_back(static_cast<uint8_t>((value >> bits) & 0xff));
      bits -= 8;
    }
  }
  return bytes;
}

// The key built in, 32 bytes, or empty if it isn't one.
const std::vector<uint8_t>& PublicKey() {
  static const std::vector<uint8_t> key = [] {
    std::vector<uint8_t> bytes = Base64(UPDATE_PUBLIC_KEY);
    return bytes.size() == 32 ? bytes : std::vector<uint8_t>();
  }();
  return key;
}

std::wstring Wide(const std::string& text) {
  std::wstring wide(text.size(), L'\0');
  const int length = MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()),
                                         wide.data(), static_cast<int>(wide.size()));
  wide.resize(length > 0 ? length : 0);
  return wide;
}

// Where updates are downloaded: %LOCALAPPDATA%\ilo\Updates, with a backslash.
std::wstring DownloadFolder() {
  PWSTR local = nullptr;
  std::wstring folder;
  if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
    folder = std::wstring(local) + L"\\ilo\\Updates\\";
  }
  CoTaskMemFree(local);
  return folder;
}

// Removes what an earlier launch downloaded: installed by now, or not, and checked for again.
void ClearFolder(const std::wstring& folder) {
  WIN32_FIND_DATAW found;
  HANDLE find = FindFirstFileW((folder + L"*").c_str(), &found);
  if (find == INVALID_HANDLE_VALUE) {
    return;
  }
  do {
    if (!(found.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)) {
      // An installer still running after the last quit can't go; the next launch takes it.
      DeleteFileW((folder + found.cFileName).c_str());
    }
  } while (FindNextFileW(find, &found));
  FindClose(find);
}

ULONGLONG Now() {
  FILETIME now;
  GetSystemTimeAsFileTime(&now);
  return (static_cast<ULONGLONG>(now.dwHighDateTime) << 32) | now.dwLowDateTime;
}

ULONGLONG LastCheck() {
  ULONGLONG last = 0;
  DWORD size = sizeof last;
  RegGetValueW(HKEY_CURRENT_USER, kSettings, L"LastCheck", RRF_RT_REG_QWORD, nullptr, &last,
               &size);
  return last;
}

void SetLastCheck(ULONGLONG time) {
  RegSetKeyValueW(HKEY_CURRENT_USER, kSettings, L"LastCheck", REG_QWORD, &time, sizeof time);
}

void TellStage() {
  int stage;
  std::string version;
  {
    std::lock_guard<std::mutex> lock(g_said_lock);
    stage = g_stage;
    version = g_version;
  }
  if (g_changed) {
    g_changed(stage, version.c_str());
  }
}

// From the thread: the main thread tells the app.
void Said(int stage, const std::string& version = "") {
  {
    std::lock_guard<std::mutex> lock(g_said_lock);
    g_stage = stage;
    g_version = version;
  }
  HostLater(TellStage);
}

// GETs |url|, which must be https, as it redirects only to https, handing |take| each piece of
// the body until it says to stop. Whether the whole body came, with status 200. A check build's
// feed is http, on the check's own server at 127.0.0.1.
bool Get(const std::string& url, const std::function<bool(const uint8_t*, DWORD)>& take) {
  const std::wstring wide = Wide(url);
  URL_COMPONENTS parts = {sizeof parts};
  parts.dwHostNameLength = parts.dwUrlPathLength = parts.dwExtraInfoLength = ~DWORD{0};
  const bool cracked = WinHttpCrackUrl(wide.c_str(), 0, 0, &parts);
  const std::wstring host =
      cracked ? std::wstring(parts.lpszHostName, parts.dwHostNameLength) : std::wstring();
  const bool local = kCheckBuild && parts.nScheme == INTERNET_SCHEME_HTTP && host == L"127.0.0.1";
  if (!cracked || (parts.nScheme != INTERNET_SCHEME_HTTPS && !local)) {
    fprintf(stderr, "browser: update: not https: %s\n", url.c_str());
    return false;
  }
  const std::wstring path = std::wstring(parts.lpszUrlPath, parts.dwUrlPathLength) +
                            std::wstring(parts.lpszExtraInfo, parts.dwExtraInfoLength);
  bool done = false;
  HINTERNET session = WinHttpOpen(L"ilo/" WIDE(APP_VERSION), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                                  WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
  HINTERNET connection =
      session ? WinHttpConnect(session, host.c_str(), parts.nPort, 0) : nullptr;
  HINTERNET request =
      connection ? WinHttpOpenRequest(connection, L"GET", path.c_str(), nullptr,
                                      WINHTTP_NO_REFERER, WINHTTP_DEFAULT_ACCEPT_TYPES,
                                      local ? 0 : WINHTTP_FLAG_SECURE)
                 : nullptr;
  if (request) {
    // WinHTTP's default too: GitHub's release downloads redirect, but never to http.
    DWORD policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;
    WinHttpSetOption(request, WINHTTP_OPTION_REDIRECT_POLICY, &policy, sizeof policy);
    WinHttpSetTimeouts(request, 0, 30'000, 30'000, 60'000);
    DWORD status = 0, size = sizeof status;
    if (WinHttpSendRequest(request, WINHTTP_NO_ADDITIONAL_HEADERS, 0, WINHTTP_NO_REQUEST_DATA, 0,
                           0, 0) &&
        WinHttpReceiveResponse(request, nullptr) &&
        WinHttpQueryHeaders(request, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                            WINHTTP_HEADER_NAME_BY_INDEX, &status, &size,
                            WINHTTP_NO_HEADER_INDEX) &&
        status == 200) {
      std::vector<uint8_t> buffer(1 << 16);
      for (;;) {
        DWORD read = 0;
        if (!WinHttpReadData(request, buffer.data(), static_cast<DWORD>(buffer.size()), &read)) {
          break;
        }
        if (read == 0) {
          done = true;
          break;
        }
        if (!take(buffer.data(), read)) {
          break;
        }
      }
    } else {
      fprintf(stderr, "browser: update: %s answered %lu (%lu)\n", url.c_str(), status,
              GetLastError());
    }
  }
  for (HINTERNET handle : {request, connection, session}) {
    if (handle) {
      WinHttpCloseHandle(handle);
    }
  }
  return done;
}

std::string Unescaped(std::string text) {
  static const std::pair<const char*, char> kEntities[] = {
      {"&lt;", '<'}, {"&gt;", '>'}, {"&quot;", '"'}, {"&apos;", '\''}, {"&amp;", '&'}};
  for (const auto& [entity, c] : kEntities) {
    for (size_t at; (at = text.find(entity)) != std::string::npos;) {
      text.replace(at, strlen(entity), 1, c);
    }
  }
  return text;
}

// The value of attribute |name| in |tag|, unescaped, or empty.
std::string Attribute(const std::string& tag, const std::string& name) {
  const std::string key = name + "=\"";
  for (size_t at = tag.find(key); at != std::string::npos; at = tag.find(key, at + 1)) {
    if (at > 0 && isspace(static_cast<unsigned char>(tag[at - 1]))) {
      const size_t start = at + key.size(), end = tag.find('"', start);
      return end == std::string::npos ? "" : Unescaped(tag.substr(start, end - start));
    }
  }
  return "";
}

// The text of |item|'s element |name|, unescaped, or empty.
std::string Element(const std::string& item, const std::string& name) {
  const std::string open = "<" + name + ">", close = "</" + name + ">";
  const size_t start = item.find(open);
  if (start == std::string::npos) {
    return "";
  }
  const size_t end = item.find(close, start + open.size());
  return end == std::string::npos ? ""
                                  : Unescaped(item.substr(start + open.size(),
                                                          end - start - open.size()));
}

// A build number, or 0 if |text| isn't one.
ULONGLONG Build(const std::string& text) {
  if (text.empty() || text.find_first_not_of("0123456789") != std::string::npos) {
    return 0;
  }
  return strtoull(text.c_str(), nullptr, 10);
}

struct Update {
  ULONGLONG build = 0;
  std::string version, url, signature;
  ULONGLONG length = 0;
};

// The newest Windows installer in |feed| newer than this build, if there is one: marked for
// Windows, as a Mac's never is, at an https address, with a signature.
Update Newest(const std::string& feed) {
  const ULONGLONG own = Build(APP_BUILD);
  Update newest;
  for (size_t at = feed.find("<item"); at != std::string::npos; at = feed.find("<item", at + 1)) {
    const size_t end = feed.find("</item>", at);
    if (end == std::string::npos) {
      break;
    }
    const std::string item = feed.substr(at, end - at);
    const size_t open = item.find("<enclosure");
    if (open == std::string::npos) {
      continue;
    }
    const std::string enclosure = item.substr(open, item.find('>', open) - open);
    const std::string os = Attribute(enclosure, "sparkle:os");
    if (os != "windows-x64" && os != "windows") {
      continue;
    }
    Update update;
    std::string build = Element(item, "sparkle:version");
    update.build = Build(build.empty() ? Attribute(enclosure, "sparkle:version") : build);
    update.version = Element(item, "sparkle:shortVersionString");
    if (update.version.empty()) {
      update.version = Attribute(enclosure, "sparkle:shortVersionString");
    }
    if (update.version.empty()) {
      update.version = std::to_string(update.build);
    }
    update.url = Attribute(enclosure, "url");
    update.signature = Attribute(enclosure, "sparkle:edSignature");
    update.length = Build(Attribute(enclosure, "length"));
    if (update.build > own && update.build > newest.build && update.url.rfind("https://", 0) == 0 &&
        Base64(update.signature).size() == 64) {
      newest = update;
    }
  }
  return newest;
}

// SHA-512, with Windows' own CNG.
class Sha512 {
 public:
  Sha512() {
    if (BCryptOpenAlgorithmProvider(&algorithm_, BCRYPT_SHA512_ALGORITHM, nullptr, 0) != 0 ||
        BCryptCreateHash(algorithm_, &hash_, nullptr, 0, nullptr, 0, 0) != 0) {
      hash_ = nullptr;
    }
  }
  ~Sha512() {
    if (hash_) {
      BCryptDestroyHash(hash_);
    }
    if (algorithm_) {
      BCryptCloseAlgorithmProvider(algorithm_, 0);
    }
  }
  bool Add(const uint8_t* data, size_t size) {
    return hash_ &&
           BCryptHashData(hash_, const_cast<PUCHAR>(data), static_cast<ULONG>(size), 0) == 0;
  }
  bool Finish(uint8_t out[64]) { return hash_ && BCryptFinishHash(hash_, out, 64, 0) == 0; }

 private:
  BCRYPT_ALG_HANDLE algorithm_ = nullptr;
  BCRYPT_HASH_HANDLE hash_ = nullptr;
};

// Downloads |update| into the folder and checks its length and signature as it comes: whether it
// can be installed.
bool Download(const Update& update) {
  const std::wstring folder = DownloadFolder();
  const int made = folder.empty() ? ERROR_PATH_NOT_FOUND
                                  : SHCreateDirectoryExW(nullptr, folder.c_str(), nullptr);
  if (made != ERROR_SUCCESS && made != ERROR_ALREADY_EXISTS && made != ERROR_FILE_EXISTS) {
    fprintf(stderr, "browser: update: no folder to download into (%d)\n", made);
    return false;
  }
  // The version names the file, so only what a version has goes into its name.
  std::string name;
  for (char c : update.version) {
    name += isalnum(static_cast<unsigned char>(c)) || c == '.' || c == '-' ? c : '_';
  }
  const std::wstring installer = folder + L"ilo-" + Wide(name) + L"-x64-setup.exe";
  const std::wstring part = installer + L".part";
  HANDLE file = CreateFileW(part.c_str(), GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS,
                            FILE_ATTRIBUTE_NORMAL, nullptr);
  if (file == INVALID_HANDLE_VALUE) {
    fprintf(stderr, "browser: update: can't write the download (%lu)\n", GetLastError());
    return false;
  }
  // Ed25519 signs SHA-512(R || A || M): the signature's first half, the key, then the installer.
  const std::vector<uint8_t> signature = Base64(update.signature);
  Sha512 hash;
  hash.Add(signature.data(), 32);
  hash.Add(PublicKey().data(), 32);
  ULONGLONG got = 0;
  bool written = true;
  const bool came = Get(update.url, [&](const uint8_t* data, DWORD size) {
    got += size;
    DWORD wrote = 0;
    written = got <= kMostInstaller && WriteFile(file, data, size, &wrote, nullptr) &&
              wrote == size && hash.Add(data, size);
    return written;
  });
  CloseHandle(file);
  uint8_t digest[64];
  const char* wrong = !came || !written                         ? "it didn't download"
                      : update.length && got != update.length   ? "its length isn't the appcast's"
                      : !hash.Finish(digest)                    ? "it couldn't be hashed"
                      : !Ed25519Verify(signature.data(), PublicKey().data(), digest)
                          ? "its signature isn't the update key's"
                          : nullptr;
  if (wrong || !MoveFileExW(part.c_str(), installer.c_str(), MOVEFILE_REPLACE_EXISTING)) {
    fprintf(stderr, "browser: update %s: %s\n", update.version.c_str(),
            wrong ? wrong : "it couldn't be kept");
    DeleteFileW(part.c_str());
    return false;
  }
  // Read-only to everyone else from here until it runs, so what runs is what was verified.
  g_held = CreateFileW(installer.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING,
                       FILE_ATTRIBUTE_NORMAL, nullptr);
  if (g_held == INVALID_HANDLE_VALUE) {
    DeleteFileW(installer.c_str());
    return false;
  }
  g_installer = installer;
  return true;
}

void Check() {
  Said(HOST_UPDATE_CHECKING);
  SetLastCheck(Now());
  // The log names the feed, which the nightly run waits for.
  const char* url = g_nightly && *UPDATE_FEED_NIGHTLY ? UPDATE_FEED_NIGHTLY : UPDATE_FEED;
  fprintf(stderr, "browser: update feed %s\n", url);
  std::string feed;
  const bool read = Get(url, [&](const uint8_t* data, DWORD size) {
    feed.append(reinterpret_cast<const char*>(data), size);
    return feed.size() <= kMostFeed;
  });
  if (!read) {
    return Said(HOST_UPDATE_FAILED);
  }
  const Update update = Newest(feed);
  if (!update.build) {
    fprintf(stderr, "browser: update: none newer than build %s\n", APP_BUILD);
    return Said(HOST_UPDATE_CURRENT);
  }
  fprintf(stderr, "browser: update %s, build %llu, downloading\n", update.version.c_str(),
          update.build);
  Said(HOST_UPDATE_DOWNLOADING, update.version);
  if (!Download(update)) {
    return Said(HOST_UPDATE_FAILED);
  }
  fprintf(stderr, "browser: update %s ready\n", update.version.c_str());
  g_build = update.build;
  g_ready = true;
  Said(HOST_UPDATE_READY, update.version);
}

// Checks when one is due and whenever asked, until an update is ready, which waits for the quit.
DWORD WINAPI Watch(void*) {
  for (;;) {
    DWORD wait = INFINITE;
    if (!g_ready) {
      const ULONGLONG due = LastCheck() + kInterval, now = Now();
      // Never past one interval, should the clock have gone back since the last check.
      wait = due <= now ? 0 : static_cast<DWORD>((std::min)(due - now, kInterval) / 10'000);
    }
    WaitForSingleObject(g_wake, wait);
    if (!g_ready) {
      Check();
    }
  }
}

}  // namespace

bool host_updated_by_store(void) {
  static const bool store = FromTheStore();
  return store;
}

bool host_updates_available(void) {
  static const bool available = kReleaseBuild && !FromTheStore() && !PublicKey().empty();
  return available;
}

// build.rs defines APP_NIGHTLY for a build with APP_TRACK=nightly.
bool host_nightly_build(void) {
#ifdef APP_NIGHTLY
  return true;
#else
  return false;
#endif
}

bool host_nightly_feed(void) {
  return host_updates_available() && *UPDATE_FEED_NIGHTLY;
}

// The thread reads it at each check.
void host_follow_nightly(bool nightly) {
  g_nightly = nightly;
}

void host_start_updater(void (*quit)(void), void (*changed)(int stage, const char* version)) {
  if (g_wake || !host_updates_available()) {
    return;
  }
  g_quit = quit;
  g_changed = changed;
  ClearFolder(DownloadFolder());
  g_wake = CreateEventW(nullptr, FALSE, FALSE, nullptr);
  HANDLE thread = g_wake ? CreateThread(nullptr, 0, Watch, nullptr, 0, nullptr) : nullptr;
  if (!thread) {
    fprintf(stderr, "browser: no updates: the updater's thread didn't start\n");
    return;
  }
  CloseHandle(thread);
  fprintf(stderr, "browser: updater started\n");
}

void host_take_install(bool (*installed)(uint64_t build, uint64_t own)) {
  ULONGLONG build = 0;
  DWORD size = sizeof build;
  if (!host_updates_available() ||
      RegGetValueW(HKEY_CURRENT_USER, kSettings, kInstalling, RRF_RT_REG_QWORD, nullptr, &build,
                   &size) != ERROR_SUCCESS) {
    return;
  }
  RegDeleteKeyValueW(HKEY_CURRENT_USER, kSettings, kInstalling);
  const ULONGLONG own = Build(APP_BUILD);
  if (!installed(build, own)) {
    fprintf(stderr, "browser: update build %llu didn't install: this is build %llu\n", build, own);
    // About's "The update didn't install" stays for the user's Try Again, as no check due at
    // launch moves it on.
    SetLastCheck(Now());
  }
}

// Check for Updates…, whose stages the app shows in Settings › About.
void host_check_for_updates(void) {
  if (g_wake && !g_ready) {
    SetEvent(g_wake);
  }
}

void host_check_for_updates_quietly(void) {
  host_check_for_updates();
}

// Restart to Update: the quit runs the installer, which relaunches ilo once it's in.
void host_install_update(void) {
  if (!g_ready || !g_quit) {
    return;
  }
  fprintf(stderr, "browser: update installs now\n");
  g_relaunch = true;
  g_quit();
}

// Chromium is quitting: a ready update installs once this process has gone, silently, as ilo.iss
// waits for it to, and relaunches ilo if Restart to Update asked. Not as Windows shuts down, which
// would stop the installer halfway.
void host_finish_update(void) {
  if (!g_ready || GetSystemMetrics(SM_SHUTTINGDOWN)) {
    return;
  }
  // Before it starts, so one that never starts counts as one that failed.
  RegSetKeyValueW(HKEY_CURRENT_USER, kSettings, kInstalling, REG_QWORD, &g_build, sizeof g_build);
  std::wstring command = L"\"" + g_installer +
                         L"\" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /waitpid=" +
                         std::to_wstring(GetCurrentProcessId());
  if (g_relaunch) {
    command += L" /relaunch=1";
  }
  STARTUPINFOW startup = {sizeof startup};
  PROCESS_INFORMATION process = {};
  // Out of any job ilo was started in, which could end the installer with it.
  bool started = CreateProcessW(g_installer.c_str(), command.data(), nullptr, nullptr, FALSE,
                                CREATE_BREAKAWAY_FROM_JOB, nullptr, nullptr, &startup, &process) ||
                 CreateProcessW(g_installer.c_str(), command.data(), nullptr, nullptr, FALSE, 0,
                                nullptr, nullptr, &startup, &process);
  if (!started) {
    fprintf(stderr, "browser: update didn't start installing (%lu)\n", GetLastError());
    return;
  }
  fprintf(stderr, "browser: update installing as ilo quits%s\n",
          g_relaunch ? ", then relaunching" : "");
  CloseHandle(process.hThread);
  CloseHandle(process.hProcess);
}
