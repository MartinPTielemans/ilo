#include "browser_core/main_delegate.h"

#include <cstdio>
#include <stdlib.h>

#include <algorithm>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/base_switches.h"
#include "base/command_line.h"
#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/scoped_multi_source_observation.h"
#include "base/scoped_observation.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/ad_block.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/auto_archive.h"
#include "browser_core/cast_dialog.h"
#include "browser_core/crash_reports.h"
#include "browser_core/downloads.h"
#include "browser_core/developer_mode.h"
#include "browser_core/drm_request.h"
#include "browser_core/hooks.h"
#include "browser_core/host.h"
#include "browser_core/install_question.h"
#include "browser_core/leave_dialog.h"
#include "browser_core/link_preview.h"
#include "browser_core/mini_player.h"
#include "browser_core/notes.h"
#include "browser_core/open_app_dialog.h"
#include "browser_core/page_menu.h"
#include "browser_core/password_offer.h"
#include "browser_core/permission_question.h"
#include "browser_core/platform/keychain.h"
#include "browser_core/platform/native_hosts.h"
#include "browser_core/profiles.h"
#include "browser_core/search.h"
#include "browser_core/session.h"
#include "browser_core/sign_in_dialog.h"
#include "browser_core/sync.h"
#include "browser_core/sync_spaces.h"
#include "browser_core/system_engine.h"
#include "browser_core/tab.h"
#include "browser_core/widevine.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/chrome_browser_main.h"
#include "chrome/browser/chrome_browser_main_extra_parts.h"
#include "chrome/browser/chrome_content_browser_client.h"
#include "chrome/browser/devtools/chrome_devtools_manager_delegate.h"
#include "chrome/browser/lifetime/application_lifetime.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/lifetime/termination_notification.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "chrome/browser/profiles/profile_manager_observer.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "chrome/browser/ui/startup/startup_tab.h"
#include "chrome/browser/ui/startup/startup_tab_provider.h"
#include "chrome/common/chrome_content_client.h"
#include "chrome/common/chrome_switches.h"
#include "chrome/common/profiler/main_thread_stack_sampling_profiler.h"
#include "components/keep_alive_registry/keep_alive_types.h"
#include "components/keep_alive_registry/scoped_keep_alive.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_view_delegate.h"
#include "content/public/common/content_features.h"
#include "content/public/common/content_switches.h"
#include "url/gurl.h"

#if BUILDFLAG(IS_WIN) || BUILDFLAG(IS_LINUX)
#include "browser_core/platform/app_library.h"
#endif
#if BUILDFLAG(IS_WIN)
#include "base/strings/utf_string_conversions.h"
#include "base/values.h"
#include "base/win/win_util.h"
#include "chrome/common/pref_names.h"
#include "components/prefs/pref_service.h"
#endif
#if BUILDFLAG(IS_LINUX)
#include "browser_core/platform/linux/wayland.h"
#include "ui/ozone/public/ozone_switches.h"
#endif
#if !BUILDFLAG(IS_WIN) && !BUILDFLAG(IS_LINUX)
// chrome_main.cc's entry point, in the same framework.
extern "C" int ChromeMain(int argc, const char** argv);
#endif

namespace browser_core {

namespace {

bc_host g_host;

// The links another app sent, to the app. Only pages and files: Chrome strips its own
// direct-launch scheme first, and any other scheme an app sends is no page to show.
void OpenLinks(const std::vector<GURL>& urls) {
  std::vector<std::string> specs;
  for (const GURL& url : urls) {
    if (url.is_valid() && (url.SchemeIsHTTPOrHTTPS() || url.SchemeIsFile())) {
      specs.push_back(url.spec());
    }
  }
  if (specs.empty() || !g_host.open_links) {
    return;
  }
  std::vector<const char*> links;
  for (const std::string& spec : specs) {
    links.push_back(spec.c_str());
  }
  g_host.open_links(g_host.context, links.data(), links.size());
}

#if !BUILDFLAG(IS_MAC)
// The links in a launch's command line, read as Chrome reads them: paths relative to |cur_dir|,
// and "? words" searched with the profile's engine, which needs a profile.
void OpenCommandLine(const base::CommandLine& command_line,
                     const base::FilePath& cur_dir,
                     Profile* profile) {
  if (!profile) {
    profile = ProfileManager::GetLastUsedProfileIfLoaded();
  }
  if (!profile) {
    return;
  }
  std::vector<GURL> urls;
  for (const StartupTab& tab :
       StartupTabProviderImpl().GetCommandLineTabs(command_line, cur_dir, profile)) {
    urls.push_back(tab.url);
  }
  OpenLinks(urls);
}
#endif

// A host's manifest from another browser's folders, said in the log, as the user can't see where
// a host came from.
base::FilePath FindNativeHost(const std::string& name) {
  fprintf(stderr, "browser: native host lookup %s\n", name.c_str());
  base::FilePath manifest = platform::FindOtherBrowsersNativeHost(name);
  if (!manifest.empty()) {
    fprintf(stderr, "browser: native host %s from %s\n", name.c_str(),
            manifest.DirName().AsUTF8Unsafe().c_str());
  }
  return manifest;
}

// Keeps the browser process up while it has no windows, which Chrome would otherwise quit at, and
// tells the app when it can call in and when it has to stop. After that it logs the stages of
// Chrome's teardown as each starts, with the time since the stop, so a slow quit says which it
// was in (#959): the profiles, Chrome's known hang on the Mac, then the threads and their
// blocking tasks.
class ExtraParts : public ChromeBrowserMainExtraParts,
                   public ProfileManagerObserver,
                   public ProfileObserver {
 public:
  // Chrome has just set its crash keys again, with the switches about:flags added.
  void PreCreateThreads() override {
    crash_reports::LeaveOutFolders();
#if BUILDFLAG(IS_WIN)
    // A work account whose organisation requires a compliant device signs in only with Windows'
    // own Microsoft sign-in, which Chromium leaves off unless a policy turns it on (#1345). It's
    // on by default on a device joined or registered to a work or school directory, and stays off
    // anywhere else, a home PC signed in with a personal Microsoft account too. The
    // CloudAPAuthEnabled policy wins either way. Windows' answer is kept, and Chrome asks for it
    // on this thread anyway as it loads the first profile, unless the device is in a local domain.
    if (base::win::IsJoinedToAzureAD()) {
      g_browser_process->local_state()->SetDefaultPrefValue(prefs::kCloudApAuthEnabled,
                                                            base::Value(1));
    }
#endif
  }

  // Before Chrome stops taking factories, as the first profile is made.
  void PreProfileInit() override {
    system_engine::RegisterProfilePrefs();
    RegisterSearchPrefs();
    RegisterArchivePrefs();
    RegisterDownloadPrefs();
    developer_mode::RegisterProfilePrefs();
    notes::RegisterProfilePrefs();
    widevine::Start();
  }

  void PostProfileInit(Profile* profile, bool is_initial_profile) override {
    crash_reports::ProfileStarted(profile);
  }

  void PreBrowserStart() override {
    // A tab's "Leave site?" asks with the app's dialog, in place of the NSAlert Chrome installed.
    LeaveDialog::Install();
    keep_alive_ = std::make_unique<ScopedKeepAlive>(KeepAliveOrigin::APP_CONTROLLER,
                                                    KeepAliveRestartOption::DISABLED);
    // A quit can start in Chrome too, such as from the Dock or the system logging out, and Chrome
    // only says it's terminating once every Browser has closed.
    app_terminating_ = browser_shutdown::AddAppTerminatingCallback(
        base::BindOnce(&ExtraParts::OnAppTerminating, base::Unretained(this)));
  }

  void PostBrowserStart() override {
#if BUILDFLAG(IS_LINUX)
    platform::FloatPictureInPictureBubbles();
#endif
    if (Profile* profile = StartProfile()) {
      UseProfile(profile);
    }
    sync::Start();
    // Once the Spaces the last launch saved are read, and so once the run loop is running.
    LoadSession(base::BindOnce([] {
      if (g_host.started) {
        g_host.started(g_host.context);
      }
      crash_reports::Started();
    }));
  }

  void PostMainMessageLoopRun() override {
    // Their Browsers are gone: quitting closed them, and the run loop waited for that.
    widevine::Stop();
    FlushSession();
    Window::CloseAll();
    Tab::StopHearingCaptures();
    ReleaseProfiles();
    if (g_host.stopping) {
      g_host.stopping(g_host.context);
    }
    stopped_ = base::TimeTicks::Now();
    ProfileManager* manager = g_browser_process->profile_manager();
    if (!manager) {
      return;
    }
    manager_observation_.Observe(manager);
    for (Profile* profile : manager->GetLoadedProfiles()) {
      profile_observations_.AddObservation(profile);
      for (Profile* off_the_record : profile->GetAllOffTheRecordProfiles()) {
        profile_observations_.AddObservation(off_the_record);
      }
    }
  }

  // Every thread has stopped, the thread pool's blocking tasks done; then Chrome lets go of the
  // browser process and returns to the app. Not after a start that never ran the loop.
  void PostDestroyThreads() override {
    if (!stopped_.is_null()) {
      LogStage("threads stopped");
    }
  }

  void OnProfileManagerDestroying() override {
    LogStage("profiles going");
    manager_observation_.Reset();
  }

  void OnProfileWillBeDestroyed(Profile* profile) override {
    std::string name = profile->GetBaseName().AsUTF8Unsafe();
    LogStage("profile " + name + (profile->IsOffTheRecord() ? " private" : "") + " going");
    profile_observations_.RemoveObservation(profile);
  }

 private:
  void LogStage(const std::string& stage) {
    fprintf(stderr, "browser: quit: %s at %lld ms\n", stage.c_str(),
            static_cast<long long>((base::TimeTicks::Now() - stopped_).InMilliseconds()));
  }

  void OnAppTerminating() {
    // check.sh reads this line. It names the way a quit came, whatever started it: the app's
    // own (bc_quit) logs its reason before this, and the system's Apple Event on the Mac too.
    fprintf(stderr, "browser: terminating, %s\n",
            ShutdownName(browser_shutdown::GetShutdownType()));
    keep_alive_.reset();
    // A remote-debugging switch, with the window Chrome doesn't open at startup, has Chrome hold
    // the browser up until a client closes it over the protocol, so the user's quit never ended.
    ChromeDevToolsManagerDelegate::AllowBrowserToClose();
  }

  static const char* ShutdownName(browser_shutdown::ShutdownType type) {
    switch (type) {
      case browser_shutdown::ShutdownType::kNotValid:
        return "no shutdown type";
      case browser_shutdown::ShutdownType::kWindowClose:
        return "the last window closed";
      case browser_shutdown::ShutdownType::kBrowserExit:
        return "an exit asked for";
      case browser_shutdown::ShutdownType::kEndSession:
        return "the session ending";
      case browser_shutdown::ShutdownType::kSilentExit:
        return "a silent exit";
      case browser_shutdown::ShutdownType::kOtherExit:
        return "another exit";
    }
    return "an unknown shutdown type";
  }

  std::unique_ptr<ScopedKeepAlive> keep_alive_;
  base::CallbackListSubscription app_terminating_;
  base::TimeTicks stopped_;
  base::ScopedObservation<ProfileManager, ProfileManagerObserver> manager_observation_{this};
  base::ScopedMultiSourceObservation<Profile, ProfileObserver> profile_observations_{this};
};

class ContentBrowserClient : public ChromeContentBrowserClient {
 public:
  std::unique_ptr<content::BrowserMainParts> CreateBrowserMainParts(
      bool is_integration_test) override {
    std::unique_ptr<content::BrowserMainParts> parts =
        ChromeContentBrowserClient::CreateBrowserMainParts(is_integration_test);
    static_cast<ChromeBrowserMainParts*>(parts.get())->AddParts(std::make_unique<ExtraParts>());
    return parts;
  }

  void CreateThrottlesForNavigation(content::NavigationThrottleRegistry& registry) override {
    ChromeContentBrowserClient::CreateThrottlesForNavigation(registry);
    system_engine::CreateThrottlesForNavigation(registry);
    AddLinkPreviewThrottle(registry);
  }

  bool HandleExternalProtocol(
      const GURL& url,
      content::WebContents::Getter web_contents_getter,
      content::FrameTreeNodeId frame_tree_node_id,
      content::NavigationUIData* navigation_data,
      bool is_primary_main_frame,
      bool is_in_fenced_frame_tree,
      network::mojom::WebSandboxFlags sandbox_flags,
      ui::PageTransition page_transition,
      bool has_user_gesture,
      const std::optional<url::Origin>& initiating_origin,
      content::RenderFrameHost* initiator_document,
      const net::IsolationInfo& isolation_info,
      mojo::PendingRemote<network::mojom::URLLoaderFactory>* out_factory) override {
    // A page in an agent's Space opens no other app, as one Chrome's own agent acts on doesn't,
    // and a saved "Always allow" doesn't count there.
    content::WebContents* contents = web_contents_getter.Run();
    if (contents && !OpenAppDialog::MayOpen(contents)) {
      return false;
    }
    return ChromeContentBrowserClient::HandleExternalProtocol(
        url, std::move(web_contents_getter), frame_tree_node_id, navigation_data,
        is_primary_main_frame, is_in_fenced_frame_tree, sandbox_flags, page_transition,
        has_user_gesture, initiating_origin, initiator_document, isolation_info, out_factory);
  }
};

// Chrome's, but for the page's URL and top origin, which Chrome records for its crash reports.
class ContentClient : public ChromeContentClient {
 public:
  void SetActiveURL(const GURL& url, std::string top_origin) override {}
};

}  // namespace

const bc_host& Host() {
  return g_host;
}

bc_id NewId() {
  static bc_id last = 0;
  return ++last;
}

// Keeps the app's callbacks and puts the core's hooks in Chrome.
void Install(const bc_host& host) {
  g_host = host;
  g_browser_core_supplies_windows = true;
  g_browser_core_adjust_navigate = &Window::AdjustNavigate;
  // A tab's Cast dialog is the app's card, in place of the Views dialog.
  g_browser_core_cast_dialog = &CastDialog::For;
  g_browser_core_can_dock_devtools = &Window::CanDockDevTools;
  g_browser_core_update_devtools = &Window::UpdateDevTools;
  g_browser_core_show_devtools = &Window::ShowDevTools;
  g_browser_core_trim_page_menu = &Window::TrimPageMenu;
  g_browser_core_show_page_menu = &PageMenu::Show;
  g_browser_core_update_target_url = &Tab::LinkTargetChanged;
  content::SetPageLinkDragHandler(&Tab::LinkDragged);
  g_browser_core_permission_prompt = &PermissionQuestion::Create;
  g_browser_core_extension_install_dialog = &InstallQuestion::Ask;
  g_browser_core_populate_frame_binders = &PopulateFrameBinders;
  g_browser_core_password_prompt = &PasswordOffer::Prompt;
#if BUILDFLAG(IS_MAC)
  g_browser_core_open_links = &OpenLinks;
#else
  g_browser_core_open_command_line = &OpenCommandLine;
#endif
  g_browser_core_page_hung = &Tab::PageHung;
  g_browser_core_offline_game = &Tab::GameRunning;
  g_browser_core_split_together = &Window::SplitTogether;
  g_browser_core_activate_contents = &Window::ActivateContents;
  g_browser_core_find_native_host = &FindNativeHost;
  g_browser_core_login_handler = &SignInDialog::Create;
  g_browser_core_external_protocol_dialog = &OpenAppDialog::Ask;
  sync::Install();
  sync_spaces::Install();
  SetUpAdBlock();
  SetUpMiniPlayer();
  g_browser_core_component_allowed = &widevine::MayUpdate;
  g_browser_core_no_passwords = &IsAgentTab;
  g_browser_core_no_autofill = &IsAgentTab;
  g_browser_core_refuse_passkey = &RefuseAgentPasskey;
}

int Run(int argc, const char** argv, const bc_host& host) {
#if BUILDFLAG(IS_WIN) || BUILDFLAG(IS_LINUX)
  // Chromium's own executable starts Chromium here, and loads the app (platform/app_library.h).
  return 1;
#else
  Install(host);
  // Only the browser's process logs its end: a helper's has a type.
  const bool browser = std::none_of(argv, argv + argc, [](const char* arg) {
    return std::string_view(arg).starts_with("--type=");
  });
  const int status = ChromeMain(argc, argv);
  if (browser) {
    // check.sh reads this line, the last the browser logs as it ends cleanly.
    fprintf(stderr, "browser: exited with status %d\n", status);
  }
  return status;
#endif
}

void Quit() {
  // Posted, so the quit starts once the event that asked for it is done: started from inside the
  // app's key event for ⌘Q, the tab closed and its Browser never did.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE,
                                                              base::BindOnce(&chrome::AttemptExit));
}

void Restart() {
  // Posted as Quit is. Chrome starts its executable again with this one's switches once it has
  // quit, and --no-startup-window keeps Chrome from bringing back its own windows over the app's.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&chrome::AttemptRestart));
}

MainDelegate::MainDelegate(const StartupTimestamps& timestamps)
    : ChromeMainDelegate(timestamps) {}

MainDelegate::~MainDelegate() = default;

std::optional<int> MainDelegate::BasicStartupComplete() {
  base::CommandLine* command_line = base::CommandLine::ForCurrentProcess();
  if (!command_line->HasSwitch(switches::kProcessType)) {
#if BUILDFLAG(IS_WIN) || BUILDFLAG(IS_LINUX)
    // First, so the features below join the app's rather than being replaced by them. The app's
    // switches go after the ones Chromium was given, which still win.
    // The arguments the process was given, which the app reads its switches from, as Chromium read
    // them: on Linux it has since written its process title over them (#605).
    std::vector<std::string> given;
    for (const base::CommandLine::StringType& arg : command_line->argv()) {
#if BUILDFLAG(IS_WIN)
      given.push_back(base::WideToUTF8(arg));
#else
      given.push_back(arg);
#endif
    }
    std::vector<std::string> app_switches;
    const bc_host* host = platform::LoadApp(given, &app_switches);
    if (!host) {
      LOG(ERROR) << "no app: the engine has no " << BC_APP_LIBRARY << " beside it with "
                 << BC_APP_ENTRY;
      return 1;
    }
    Install(*host);
    base::CommandLine::StringVector app_argv(1);
    for (const std::string& app_switch : app_switches) {
#if BUILDFLAG(IS_WIN)
      app_argv.push_back(base::UTF8ToWide(app_switch));
#else
      app_argv.push_back(app_switch);
#endif
    }
    command_line->AppendArguments(base::CommandLine(app_argv), /*include_program=*/false);
#endif
#if BUILDFLAG(IS_LINUX)
    // The desktop's own protocol, not Chrome's auto pick: the app's UI joins the browser window on
    // the same connection, as a subsurface on Wayland and a child window on X11
    // (platform/linux/window_views.cc).
    if (!command_line->HasSwitch(switches::kOzonePlatform)) {
      const bool wayland = getenv("WAYLAND_DISPLAY") && *getenv("WAYLAND_DISPLAY");
      command_line->AppendSwitchASCII(switches::kOzonePlatform, wayland ? "wayland" : "x11");
    }
#endif
    // The app opens windows through the core, and Chrome's first run and default-browser prompts
    // are Chrome's UI, not ours.
    command_line->AppendSwitch(switches::kNoStartupWindow);
    command_line->AppendSwitch(switches::kNoFirstRun);
    command_line->AppendSwitch(switches::kNoDefaultBrowserCheck);
    // A pinned tab the user closes unloads in place, keeping its tab and the id the app knows it by.
    // Chrome's own discards, under memory pressure, then keep theirs too.
    std::string features = command_line->GetSwitchValueASCII(switches::kEnableFeatures);
    features += features.empty() ? "" : ",";
    command_line->AppendSwitchASCII(switches::kEnableFeatures,
                                    features + features::kWebContentsDiscard.name);
    // Blink's eye in a password field, Chrome's on Windows: it shows only once the user types into
    // the empty field, and goes, masking it again, as the field loses focus or is filled in or set
    // by the page, so a saved password is never shown. The core blurs a tab's page each time the
    // tab becomes an agent's to act in, so no agent sees one the user showed (agent_session.cc).
    std::string blink = command_line->GetSwitchValueASCII(switches::kEnableBlinkFeatures);
    blink += blink.empty() ? "" : ",";
    // And ⌥-click on a link opens it in a split view, as Arc's does, not Chrome's download
    // (chromium-core-alt-click-split.patch).
    command_line->AppendSwitchASCII(switches::kEnableBlinkFeatures,
                                    blink + "PasswordReveal,AltClickSplitView");
#if BUILDFLAG(IS_MAC)
    platform::NameKeychainItem();
#endif
  }
  return ChromeMainDelegate::BasicStartupComplete();
}

void MainDelegate::PreSandboxStartup() {
  // Chrome's starts Crashpad, which in the browser process reads where to write its reports.
  if (!base::CommandLine::ForCurrentProcess()->HasSwitch(switches::kProcessType)) {
    crash_reports::PlaceReports();
  }
  ChromeMainDelegate::PreSandboxStartup();
  crash_reports::LeaveOutPages();
  crash_reports::LeaveOutFolders();
}

#if BUILDFLAG(IS_LINUX)
void MainDelegate::ZygoteForked() {
  // Chrome's starts Crashpad in a page's process as the zygote forks it.
  ChromeMainDelegate::ZygoteForked();
  crash_reports::LeaveOutPages();
  crash_reports::LeaveOutFolders();
}
#endif

content::ContentClient* MainDelegate::CreateContentClient() {
  // In place of ChromeMainDelegate's, which is its member, in every process.
  static base::NoDestructor<ContentClient> content_client;
  return content_client.get();
}

std::optional<int> MainDelegate::PreBrowserMain() {
  std::optional<int> exit_code = ChromeMainDelegate::PreBrowserMain();
  // Chrome's PreBrowserMain made NSApp, which the app can't make before it: Chrome's class reads
  // the command line as it's made.
  if (!exit_code && g_host.launching) {
    g_host.launching(g_host.context);
  }
  return exit_code;
}

content::ContentBrowserClient* MainDelegate::CreateContentBrowserClient() {
  // As ChromeMainDelegate's, with our client in place of Chrome's.
  chrome_content_browser_client_ = std::make_unique<ContentBrowserClient>();
  chrome_content_browser_client_->SetSamplingProfiler(std::move(sampling_profiler_));
  return chrome_content_browser_client_.get();
}

}  // namespace browser_core
