#include "browser_core/sync.h"

#include <algorithm>
#include <array>
#include <list>
#include <map>
#include <memory>
#include <optional>
#include <sstream>
#include <string>
#include <utility>
#include <vector>

#include "base/auto_reset.h"
#include "base/base64.h"
#include "base/base64url.h"
#include "base/command_line.h"
#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/files/important_file_writer.h"
#include "base/functional/bind.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/observer_list.h"
#include "base/path_service.h"
#include "base/scoped_observation.h"
#include "base/strings/escape.h"
#include "base/strings/string_split.h"
#include "base/strings/utf_string_conversions.h"
#include "base/system/sys_info.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/uuid.h"
#include "base/values.h"
#include "browser_core/hooks.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "browser_core/sync_guard.h"
#include "browser_core/sync_pairing.h"
#include "browser_core/sync_relay.h"
#include "browser_core/sync_words.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_destroyer.h"
#include "chrome/browser/signin/identity_manager_factory.h"
#include "chrome/browser/sync/sync_invalidations_service_factory.h"
#include "chrome/browser/sync/sync_service_factory.h"
#include "chrome/common/chrome_paths.h"
#include "components/os_crypt/async/browser/os_crypt_async.h"
#include "components/os_crypt/async/common/encryptor.h"
#include "components/signin/public/identity_manager/access_token_info.h"
#include "components/signin/public/identity_manager/account_info.h"
#include "components/sync/base/command_line_switches.h"
#include "components/sync/base/data_type.h"
#include "components/sync/base/passphrase_enums.h"
#include "components/sync/base/user_selectable_type.h"
#include "components/sync/engine/events/protocol_event.h"
#include "components/sync/engine/events/protocol_event_observer.h"
#include "components/sync/invalidations/fcm_registration_token_observer.h"
#include "components/sync/invalidations/interested_data_types_handler.h"
#include "components/sync/invalidations/invalidations_listener.h"
#include "components/sync/invalidations/sync_invalidations_service.h"
#include "components/sync/protocol/sync.pb.h"
#include "components/sync/protocol/sync_invalidations_payload.pb.h"
#include "components/sync/service/sync_auth_manager.h"
#include "components/sync/service/sync_service.h"
#include "components/sync/service/sync_service_observer.h"
#include "components/sync/service/sync_user_settings.h"
#include "components/web_modal/modal_dialog_host.h"
#include "components/web_modal/web_contents_modal_dialog_host.h"
#include "components/web_modal/web_contents_modal_dialog_manager.h"
#include "components/web_modal/web_contents_modal_dialog_manager_delegate.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "crypto/hash.h"
#include "crypto/random.h"
#include "google_apis/gaia/core_account_id.h"
#include "google_apis/gaia/gaia_id.h"
#include "google_apis/gaia/google_service_auth_error.h"
#include "net/base/net_errors.h"
#include "net/base/url_util.h"
#include "net/http/http_response_headers.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/mojom/fetch_api.mojom-shared.h"
#include "services/network/public/mojom/url_response_head.mojom.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/views/controls/webview/webview.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_delegate.h"
#include "ui/views/widget/widget_observer.h"
#include "url/gurl.h"

namespace browser_core::sync {

namespace {

// ilo-server's OIDC client id for the browser (protocol.md, Sign-in).
constexpr char kClientId[] = "ilo";

// The prefix of the sign-in window's off-the-record profile's id, by which Chrome's passkey dialog
// in it reads as a normal window's: the user opened no Incognito window (#1302).
constexpr char kSignInProfile[] = "BrowserCore::SyncSignIn";

#if BUILDFLAG(IS_MAC)
constexpr char kOs[] = "macOS";
#elif BUILDFLAG(IS_WIN)
constexpr char kOs[] = "Windows";
#else
constexpr char kOs[] = "Linux";
#endif

// What a user types in the sign-in window, and the tokens and devices of its account.
constexpr net::NetworkTrafficAnnotationTag kAccount =
    net::DefineNetworkTrafficAnnotation("browser_core_sync_account", R"(
      semantics {
        sender: "Sync"
        description:
          "Gets and renews the tokens of the browser's account on its server, lists the account's "
          "devices, removes one, and signs this one out."
        trigger:
          "The user signs in, out, or opens or edits the device list in Settings › Account; and "
          "while signed in, as sync's access token expires, about once an hour."
        data:
          "The account's sign-in code and refresh token, this device's name, and the access token."
        destination: OTHER
        destination_other: "The account's server: the build's own, or one the user named."
      }
      policy {
        cookies_allowed: NO
        setting: "Settings › Account: Sign Out."
        policy_exception_justification: "Only while the user is signed in to an account."
      })");

std::string Base64Url(base::span<const uint8_t> bytes) {
  std::string encoded;
  base::Base64UrlEncode(bytes, base::Base64UrlEncodePolicy::OMIT_PADDING, &encoded);
  return encoded;
}

// A 32-byte value from the relay, as base64url without padding.
std::optional<std::array<uint8_t, 32>> Key32(const std::string* encoded) {
  if (!encoded) {
    return std::nullopt;
  }
  std::optional<std::vector<uint8_t>> bytes =
      base::Base64UrlDecode(*encoded, base::Base64UrlDecodePolicy::IGNORE_PADDING);
  if (!bytes || bytes->size() != 32) {
    return std::nullopt;
  }
  std::array<uint8_t, 32> key;
  std::ranges::copy(*bytes, key.begin());
  return key;
}

std::string Frame(base::DictValue frame) {
  return base::WriteJson(frame).value_or(std::string());
}

// Where the account is kept: the server in the clear, the tokens and the secret encrypted with the
// OS's key, as Chrome keeps passwords.
base::FilePath StorePath() {
  base::FilePath dir;
  base::PathService::Get(chrome::DIR_USER_DATA, &dir);
  return dir.AppendASCII("Account");
}

scoped_refptr<base::SequencedTaskRunner> FileRunner() {
  static base::NoDestructor<scoped_refptr<base::SequencedTaskRunner>> runner(
      base::ThreadPool::CreateSequencedTaskRunner(
          {base::MayBlock(), base::TaskShutdownBehavior::BLOCK_SHUTDOWN}));
  return *runner;
}

// The sign-in window: the server's sign-in page alone, in an off-the-record profile of its own,
// until the page sends it to |redirect|, which it reports instead of loading. Chrome's dialogs for
// the page, such as its passkey dialog, show over it as over a tab's page; without a host Chrome
// shows none, and the page's request waits with nothing on screen (#1295).
class SignInWindow : public views::WidgetObserver,
                     public content::WebContentsObserver,
                     public web_modal::WebContentsModalDialogManagerDelegate,
                     public web_modal::WebContentsModalDialogHost {
 public:
  SignInWindow(const GURL& url,
               const GURL& redirect,
               base::OnceCallback<void(std::optional<GURL>)> done)
      : redirect_(redirect), done_(std::move(done)) {
    Profile* start = StartProfile();
    profile_ = start ? start->GetOffTheRecordProfile(
                           Profile::OTRProfileID::CreateUnique(kSignInProfile),
                           /*create_if_needed=*/true)
                     : nullptr;
    if (!profile_) {
      return;
    }
    delegate_ = std::make_unique<views::WidgetDelegate>();
    delegate_->SetTitle(base::UTF8ToUTF16(url.host()));
    delegate_->SetCanResize(false);
    auto web_view = std::make_unique<views::WebView>(profile_);
    web_view_ = web_view.get();
    // The page has the keyboard as the window opens, for the address its field asks for.
    delegate_->SetInitiallyFocusedView(web_view_);
    delegate_->SetContentsView(std::move(web_view));

    // Over the front window, as the app's dialogs are.
    constexpr gfx::Size kSize(440, 640);
    gfx::Rect bounds(kSize);
    std::vector<Window*> windows = Window::All();
    if (!windows.empty() && windows.front()->widget()) {
      bounds = windows.front()->widget()->GetWindowBoundsInScreen();
      bounds.ClampToCenteredSize(kSize);
    }
    widget_ = std::make_unique<views::Widget>();
    views::Widget::InitParams params(views::Widget::InitParams::CLIENT_OWNS_WIDGET,
                                     views::Widget::InitParams::TYPE_WINDOW);
    params.delegate = delegate_.get();
    params.bounds = bounds;
    params.name = "BrowserCoreSyncSignIn";
    widget_->Init(std::move(params));
    widget_observation_.Observe(widget_.get());
    content::WebContents* contents = web_view_->GetWebContents();
    Observe(contents);
    web_modal::WebContentsModalDialogManager::CreateForWebContents(contents);
    web_modal::WebContentsModalDialogManager::FromWebContents(contents)->SetDelegate(this);
    web_view_->LoadInitialURL(url);
    widget_->Show();
  }

  ~SignInWindow() override {
    // Chrome's dialogs over the page close first: they keep the host (ModalDialogHost).
    for (web_modal::ModalDialogHostObserver& observer : dialog_observers_) {
      observer.OnHostDestroying();
    }
    Observe(nullptr);
    widget_observation_.Reset();
    web_view_ = nullptr;
    widget_.reset();
    delegate_.reset();
    if (profile_) {
      ProfileDestroyer::DestroyOTRProfileWhenAppropriate(profile_);
    }
  }

  bool open() const { return widget_ != nullptr; }

  void Activate() {
    if (widget_) {
      widget_->Activate();
    }
  }

 private:
  void Caught(content::NavigationHandle* navigation) {
    const GURL& url = navigation->GetURL();
    if (!done_ || url.DeprecatedGetOriginAsURL() != redirect_.DeprecatedGetOriginAsURL() ||
        url.path() != redirect_.path()) {
      return;
    }
    // The tokens come from this, so the page itself never loads.
    std::move(done_).Run(url);
  }

  void Closed() {
    if (done_) {
      std::move(done_).Run(std::nullopt);
    }
  }

  // content::WebContentsObserver:
  void DidStartNavigation(content::NavigationHandle* navigation) override { Caught(navigation); }
  void DidRedirectNavigation(content::NavigationHandle* navigation) override {
    Caught(navigation);
  }
  // For the checks, which type in the page once it's there.
  void DidFinishLoad(content::RenderFrameHost* frame, const GURL& url) override {
    if (frame->IsInPrimaryMainFrame()) {
      LOG(ERROR) << "browser: sync sign-in loaded " << url.path();
    }
  }

  // views::WidgetObserver:
  void OnWidgetDestroying(views::Widget* widget) override {
    Observe(nullptr);
    widget_observation_.Reset();
    Closed();
  }

  // web_modal::WebContentsModalDialogManagerDelegate:
  web_modal::WebContentsModalDialogHost* GetWebContentsModalDialogHost(
      content::WebContents* contents) override {
    return this;
  }
  void SetWebContentsBlocked(content::WebContents* contents, bool blocked) override {
    // For the checks, which wait for the dialog before its keys and for it to go before the page's.
    LOG(ERROR) << "browser: sync sign-in dialog " << (blocked ? "shown" : "closed");
    // As a tab's page does, the page takes the keyboard back once the dialog has gone.
    if (!blocked) {
      contents->Focus();
    }
  }

  // web_modal::WebContentsModalDialogHost:
  gfx::NativeView GetHostView() const override { return widget_->GetNativeView(); }
  gfx::Point GetDialogPosition(const gfx::Size& size) override {
    // As over a tab's page: centred across the page, at its top. The page fills the window.
    return gfx::Point((web_view_->width() - size.width()) / 2, 0);
  }
  gfx::Size GetMaximumDialogSize() override { return web_view_->size(); }
  void AddObserver(web_modal::ModalDialogHostObserver* observer) override {
    dialog_observers_.AddObserver(observer);
  }
  void RemoveObserver(web_modal::ModalDialogHostObserver* observer) override {
    dialog_observers_.RemoveObserver(observer);
  }

  const GURL redirect_;
  base::OnceCallback<void(std::optional<GURL>)> done_;
  raw_ptr<Profile> profile_ = nullptr;
  std::unique_ptr<views::WidgetDelegate> delegate_;
  raw_ptr<views::WebView> web_view_ = nullptr;
  std::unique_ptr<views::Widget> widget_;
  base::ScopedObservation<views::Widget, views::WidgetObserver> widget_observation_{this};
  base::ObserverList<web_modal::ModalDialogHostObserver> dialog_observers_;
};

// Change notifications from the browser's relay. Its token names this device to sync, which
// publishes it in the device's DeviceInfo as it would FCM's; ilo-server tells every other device
// of the account instead, so the token only has to be there.
class RelayInvalidations : public syncer::SyncInvalidationsService {
 public:
  RelayInvalidations()
      : device_("ilo-relay:" + base::Uuid::GenerateRandomV4().AsLowercaseString()) {}

  // The types another device changed, as field numbers, or every type this one wants if none.
  void Deliver(const std::vector<int>& field_numbers) {
    if (!token_) {
      return;
    }
    sync_pb::SyncInvalidationsPayload payload;
    std::vector<int> numbers = field_numbers;
    if (numbers.empty() && interested_data_types_) {
      for (syncer::DataType type : *interested_data_types_) {
        numbers.push_back(syncer::GetSpecificsFieldNumberFromDataType(type));
      }
    }
    for (int number : numbers) {
      payload.add_data_type_invalidations()->set_data_type_id(number);
    }
    payload.set_version(base::Time::Now().InMillisecondsSinceUnixEpoch());
    const std::string serialized = payload.SerializeAsString();
    for (syncer::InvalidationsListener& listener : listeners_) {
      listener.OnInvalidationReceived(serialized);
    }
  }

  // syncer::SyncInvalidationsService:
  // The token comes after the call returns, as FCM's does: the engine watches it from before its
  // backend starts, and a change it saw first would reach the backend before it exists.
  void StartListening() override {
    if (token_) {
      return;
    }
    base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&RelayInvalidations::Listen, weak_factory_.GetWeakPtr()));
  }
  void StopListening() override {
    weak_factory_.InvalidateWeakPtrs();
    token_.reset();
  }
  void StopListeningPermanently() override {
    const bool had_token = token_.has_value();
    StopListening();
    if (had_token) {
      NotifyToken();
    }
  }
  void AddListener(syncer::InvalidationsListener* listener) override {
    if (!listeners_.HasObserver(listener)) {
      listeners_.AddObserver(listener);
    }
  }
  bool HasListener(syncer::InvalidationsListener* listener) override {
    return listeners_.HasObserver(listener);
  }
  void RemoveListener(syncer::InvalidationsListener* listener) override {
    listeners_.RemoveObserver(listener);
  }
  void AddTokenObserver(syncer::FCMRegistrationTokenObserver* observer) override {
    token_observers_.AddObserver(observer);
  }
  void RemoveTokenObserver(syncer::FCMRegistrationTokenObserver* observer) override {
    token_observers_.RemoveObserver(observer);
  }
  std::optional<std::string> GetFCMRegistrationToken() const override { return token_; }
  void SetInterestedDataTypesHandler(syncer::InterestedDataTypesHandler* handler) override {
    interested_data_types_handler_ = handler;
  }
  std::optional<syncer::DataTypeSet> GetInterestedDataTypes() const override {
    return interested_data_types_;
  }
  void SetInterestedDataTypes(const syncer::DataTypeSet& data_types) override {
    interested_data_types_ = data_types;
    if (interested_data_types_handler_) {
      interested_data_types_handler_->OnInterestedDataTypesChanged();
    }
  }
  void SetCommittedAdditionalInterestedDataTypesCallback(
      InterestedDataTypesAppliedCallback callback) override {
    if (interested_data_types_handler_) {
      interested_data_types_handler_->SetCommittedAdditionalInterestedDataTypesCallback(
          std::move(callback));
    }
  }

 private:
  void Listen() {
    token_ = device_;
    NotifyToken();
  }

  void NotifyToken() {
    for (syncer::FCMRegistrationTokenObserver& observer : token_observers_) {
      observer.OnFCMRegistrationTokenChanged();
    }
  }

  const std::string device_;
  // Unset until listening, as FCM's is until it has one; an empty token would mean none ever.
  std::optional<std::string> token_;
  base::ObserverList<syncer::InvalidationsListener> listeners_;
  base::ObserverList<syncer::FCMRegistrationTokenObserver> token_observers_;
  raw_ptr<syncer::InterestedDataTypesHandler> interested_data_types_handler_ = nullptr;
  std::optional<syncer::DataTypeSet> interested_data_types_;
  base::WeakPtrFactory<RelayInvalidations> weak_factory_{this};
};

using TokenDone = base::OnceCallback<void(GoogleServiceAuthError, signin::AccessTokenInfo)>;
using Fetched = base::OnceCallback<void(int status, std::optional<base::DictValue> json)>;

// This device asking the others for the secret (protocol.md, Pairing: the requester).
struct Request {
  PairingRequest pairing;
  std::string id;
  base::OneShotTimer expiry;
};

// Another device asking this one, which holds the secret (the approver).
struct Asked {
  explicit Asked(const std::array<uint8_t, 32>& commitment) : pairing(commitment) {}

  PairingAnswer pairing;
  std::string id;
  std::string name;
  std::string os;
  // The server's times, in milliseconds since the Unix epoch, 0 unsaid.
  double asked_at = 0;
  double signed_in_at = 0;
  bool accepted = false;
  base::OneShotTimer expiry;
};

class Account : public syncer::SyncServiceObserver,
                public syncer::ProtocolEventObserver,
                public SyncRelay::Delegate {
 public:
  static Account& Get() {
    static base::NoDestructor<Account> account;
    return *account;
  }

  // Hooks.

  syncer::SyncAccountInfo SyncAccount(const signin::IdentityManager* identity_manager) {
    if (!SignedIn() || !IsStart(identity_manager)) {
      return syncer::SyncAccountInfo();
    }
    CoreAccountInfo account;
    account.account_id = CoreAccountId::FromString(account_);
    account.gaia = GaiaId(account_);
    account.email = email_;
    // Consented, so the profile's own stores sync, as Sync-the-feature's do, rather than the
    // separate account stores of a signed-in user.
    return {.account_info = account,
            .is_sync_consented = true,
            .managed_status = signin::AccountManagedStatusFinderOutcome::kConsumerNotWellKnown};
  }

  // A token sync hasn't had yet, or else a new one: sync asks again only when the server refused
  // the last, or it expired.
  void SyncToken(const signin::IdentityManager* identity_manager, TokenDone done) {
    if (!SignedIn() || !IsStart(identity_manager)) {
      Answer(std::move(done), false);
      return;
    }
    if (Fresh() && !access_given_) {
      Answer(std::move(done), true);
      return;
    }
    Refresh(base::BindOnce(&Account::Answer, weak_factory_.GetWeakPtr(), std::move(done)));
  }

  void Watch(const signin::IdentityManager* identity_manager, base::RepeatingClosure changed) {
    watches_[identity_manager] = std::move(changed);
  }

  std::unique_ptr<syncer::SyncInvalidationsService> CreateInvalidations(Profile* profile) {
    return std::make_unique<RelayInvalidations>();
  }

  GURL SyncUrl() { return Server(); }

  // Calls.

  void Start() {
    // Never the device's own name, which is often its owner's: the server keeps this one.
    device_name_ = SyncDeviceLabel(std::string());
    base::SysInfo::GetHardwareInfo(base::BindOnce([](base::SysInfo::HardwareInfo info) {
      Get().device_name_ = SyncDeviceLabel(info.model);
    }));
    g_browser_process->os_crypt_async()->GetInstance(
        base::BindOnce(&Account::GotEncryptor, weak_factory_.GetWeakPtr()));
  }

  bool SetServer(const std::string& server) {
    GURL url(server);
    if (!server.empty() &&
        !(url.SchemeIs("https") || (url.SchemeIs("http") && net::IsLocalhost(url)))) {
      return false;
    }
    if (SignedIn() || signing_in_) {
      SignOut();
    }
    server_ = server.empty() ? GURL() : url.DeprecatedGetOriginAsURL();
    problem_ = BC_SYNC_PROBLEM_NONE;
    Save();
    Notify();
    return true;
  }

  void SignIn() {
    if (!loaded_ || SignedIn() || signing_in_) {
      if (sign_in_window_) {
        sign_in_window_->Activate();
      }
      return;
    }
    problem_ = BC_SYNC_PROBLEM_NONE;
    const GURL server = Server();
    if (!server.is_valid()) {
      problem_ = BC_SYNC_PROBLEM_SERVER;
      Notify();
      return;
    }
    std::array<uint8_t, 32> verifier_bytes;
    crypto::RandBytes(verifier_bytes);
    verifier_ = Base64Url(verifier_bytes);
    std::array<uint8_t, 16> state_bytes;
    crypto::RandBytes(state_bytes);
    state_ = Base64Url(state_bytes);
    redirect_ = server.Resolve("/signed-in");
    GURL authorize = server.Resolve("/authorize");
    authorize = net::AppendQueryParameter(authorize, "response_type", "code");
    authorize = net::AppendQueryParameter(authorize, "client_id", kClientId);
    authorize = net::AppendQueryParameter(authorize, "redirect_uri", redirect_.spec());
    authorize = net::AppendQueryParameter(authorize, "scope", "openid email offline_access");
    authorize = net::AppendQueryParameter(authorize, "state", state_);
    authorize = net::AppendQueryParameter(
        authorize, "code_challenge",
        Base64Url(crypto::hash::Sha256(base::as_byte_span(verifier_))));
    authorize = net::AppendQueryParameter(authorize, "code_challenge_method", "S256");
    signing_in_ = true;
    sign_in_window_ = std::make_unique<SignInWindow>(
        authorize, redirect_,
        base::BindOnce(&Account::SignInReturned, weak_factory_.GetWeakPtr(), generation_));
    if (!sign_in_window_->open()) {
      sign_in_window_.reset();
      signing_in_ = false;
      problem_ = BC_SYNC_PROBLEM_SERVER;
    }
    Notify();
  }

  void Cancel() {
    problem_ = BC_SYNC_PROBLEM_NONE;
    if (signing_in_ && !SignedIn()) {
      CloseSignInWindow();
      signing_in_ = false;
      ++generation_;
    }
    if (request_) {
      if (!request_->id.empty()) {
        SendPairing("pair_cancel", request_->id);
      }
      request_.reset();
    }
    Notify();
  }

  void SignOut() {
    if (SignedIn()) {
      Fetch("POST", Server().Resolve("/revoke"), std::string(),
            "token=" + base::EscapeUrlEncodedData(refresh_, true), base::DoNothing());
    }
    CloseSignInWindow();
    signing_in_ = false;
    Forget(BC_SYNC_PROBLEM_NONE);
  }

  std::string RecoveryWords() {
    const int status = Status();
    if (status == BC_SYNC_FIRST_DEVICE) {
      SyncSecret secret;
      crypto::RandBytes(secret);
      SetSecret(secret);
      problem_ = BC_SYNC_PROBLEM_NONE;
      Save();
      UseSecret();
      Notify();
    } else if (status != BC_SYNC_ON) {
      return std::string();
    }
    return SyncWordsOf(*secret_);
  }

  bool EnterRecoveryWords(const std::string& words) {
    const int status = Status();
    if (status != BC_SYNC_NEEDS_SECRET && status != BC_SYNC_PAIRING) {
      return false;
    }
    std::optional<SyncSecret> secret = SyncSecretOf(words);
    if (!secret) {
      return false;
    }
    if (request_) {
      if (!request_->id.empty()) {
        SendPairing("pair_cancel", request_->id);
      }
      request_.reset();
    }
    TakeSecret(*secret);
    return true;
  }

  // Deletes the account's data on the server with sync's own ClearServerData, which Chromium's
  // engine no longer sends. The server's new store birthday then has the engine here start over,
  // as it would after a reset from Google's dashboard, and find no passphrase.
  void StartOver() {
    const int status = Status();
    if (status != BC_SYNC_NEEDS_SECRET && status != BC_SYNC_PAIRING) {
      return;
    }
    problem_ = BC_SYNC_PROBLEM_NONE;
    if (request_) {
      if (!request_->id.empty()) {
        SendPairing("pair_cancel", request_->id);
      }
      request_.reset();
    }
    Notify();
    sync_pb::ClientToServerMessage message;
    message.set_share(email_);
    message.set_message_contents(sync_pb::ClientToServerMessage::CLEAR_SERVER_DATA);
    message.mutable_clear_server_data();
    WithToken(base::BindOnce(
        [](base::WeakPtr<Account> account, int generation, std::string body,
           const std::string& token) {
          if (!account || generation != account->generation_) {
            return;
          }
          if (token.empty()) {
            account->problem_ = BC_SYNC_PROBLEM_SERVER;
            account->Notify();
            return;
          }
          account->Fetch("POST", account->Server().Resolve("/command/"), token, body,
                         base::BindOnce(&Account::StartedOver, account, generation),
                         "application/octet-stream");
        },
        weak_factory_.GetWeakPtr(), generation_, message.SerializeAsString()));
  }

  void RequestPairing() {
    if (Status() != BC_SYNC_NEEDS_SECRET) {
      return;
    }
    problem_ = BC_SYNC_PROBLEM_NONE;
    if (!relay_open_) {
      problem_ = BC_SYNC_PROBLEM_SERVER;
      Notify();
      return;
    }
    request_ = std::make_unique<Request>();
    base::DictValue frame;
    frame.Set("type", "pair_request");
    frame.Set("commitment", Base64Url(request_->pairing.commitment()));
    frame.Set("name", device_name_);
    frame.Set("os", kOs);
    relay_->Send(Frame(std::move(frame)));
    Notify();
  }

  void AnswerPairing(const std::string& id, bool add) {
    if (!asked_ || asked_->id != id || asked_->pairing.code().empty() || !secret_) {
      return;
    }
    std::optional<std::vector<uint8_t>> sealed =
        add ? asked_->pairing.Seal(asked_->id, *secret_) : std::nullopt;
    if (sealed && relay_open_) {
      base::DictValue frame;
      frame.Set("type", "pair_seal");
      frame.Set("pairing", asked_->id);
      frame.Set("sealed", Base64Url(*sealed));
      relay_->Send(Frame(std::move(frame)));
    } else {
      SendPairing("pair_cancel", asked_->id);
    }
    asked_.reset();
  }

  void ReadDevices() {
    if (!SignedIn()) {
      return;
    }
    WithToken(base::BindOnce(
        [](base::WeakPtr<Account> account, const std::string& token) {
          if (!account || token.empty()) {
            return;
          }
          account->Fetch("GET", account->Server().Resolve("/api/devices"), token, std::string(),
                         base::BindOnce(&Account::DevicesRead, account));
        },
        weak_factory_.GetWeakPtr()));
  }

  void RemoveDevice(const std::string& id) {
    if (!SignedIn() || id.empty() || id == device_) {
      return;
    }
    const GURL url = Server().Resolve("/api/devices/" + base::EscapeAllExceptUnreserved(id));
    WithToken(base::BindOnce(
        [](base::WeakPtr<Account> account, GURL url, const std::string& token) {
          if (!account || token.empty()) {
            return;
          }
          account->Fetch("DELETE", url, token, std::string(),
                         base::BindOnce(
                             [](base::WeakPtr<Account> account, int status,
                                std::optional<base::DictValue> json) {
                               if (account) {
                                 account->ReadDevices();
                               }
                             },
                             account));
        },
        weak_factory_.GetWeakPtr(), url));
  }

  void WindowActivated() {
    if (asked_ && !asked_->accepted) {
      Accept();
    }
  }

 private:
  friend class base::NoDestructor<Account>;
  Account() = default;

  bool SignedIn() const { return !refresh_.empty(); }

  bool IsStart(const signin::IdentityManager* identity_manager) const {
    Profile* start = StartProfile();
    return start && identity_manager &&
           IdentityManagerFactory::GetForProfileIfExists(start) == identity_manager;
  }

  // The access token, with a minute to spare.
  bool Fresh() const {
    return !access_.empty() && access_expiry_ - base::Time::Now() > base::Minutes(1);
  }

  GURL Server() const {
    if (server_.is_valid()) {
      return server_;
    }
    GURL url(base::CommandLine::ForCurrentProcess()->GetSwitchValueASCII(
        syncer::kSyncServiceURL));
    return url.is_valid() ? url.DeprecatedGetOriginAsURL() : GURL();
  }

  syncer::SyncService* Service() const {
    Profile* start = StartProfile();
    return start ? SyncServiceFactory::GetForProfile(start) : nullptr;
  }

  // Whether the account's data has a passphrase, once sync's engine is up. A new account's server
  // leaves Nigori unset, and the commit that sets it up waits for the setup the core holds, so
  // until then Chromium knows no passphrase type: that's an account without one.
  std::optional<bool> HasPassphrase() const {
    syncer::SyncService* service = Service();
    if (!service || !service->IsEngineInitialized()) {
      return std::nullopt;
    }
    const std::optional<syncer::PassphraseType> type =
        service->GetUserSettings()->GetPassphraseType();
    return type && syncer::IsExplicitPassphrase(*type);
  }

  int Status() const {
    if (!SignedIn()) {
      return signing_in_ ? BC_SYNC_SIGNING_IN : BC_SYNC_SIGNED_OUT;
    }
    if (secret_) {
      return BC_SYNC_ON;
    }
    if (request_) {
      return BC_SYNC_PAIRING;
    }
    const std::optional<bool> passphrase = HasPassphrase();
    // Starting over, the engine stops and starts again before it finds the passphrase gone.
    if (starting_over_ && passphrase.value_or(true)) {
      return BC_SYNC_NEEDS_SECRET;
    }
    if (!passphrase) {
      return BC_SYNC_SIGNING_IN;
    }
    return *passphrase ? BC_SYNC_NEEDS_SECRET : BC_SYNC_FIRST_DEVICE;
  }

  void Notify() {
    if (!loaded_) {
      return;
    }
    const std::string server = Server().is_valid() ? Server().spec() : std::string();
    const std::string trimmed =
        !server.empty() && server.back() == '/' ? server.substr(0, server.size() - 1) : server;
    const std::string code = request_ ? request_->pairing.code() : std::string();
    const std::string email = SignedIn() ? email_ : std::string();
    const std::array<std::string, 3> now = {trimmed, email, code};
    const int status = Status();
    if (notified_ && status == notified_status_ && problem_ == notified_problem_ &&
        now == notified_strings_) {
      return;
    }
    notified_ = true;
    notified_status_ = status;
    notified_problem_ = problem_;
    notified_strings_ = now;
    if (!Host().sync_changed) {
      return;
    }
    bc_sync_state state = {.status = status,
                           .problem = problem_,
                           .server = trimmed.c_str(),
                           .default_server = !server_.is_valid() && !server.empty(),
                           .email = email.c_str(),
                           .code = code.c_str()};
    Host().sync_changed(Host().context, &state);
  }

  // The secret, which also keys passwords' client tags (sync_guard.h).
  void SetSecret(const std::optional<SyncSecret>& secret) {
    secret_ = secret;
    SetSyncPasswordTagKey(secret);
  }

  // Storage.

  void GotEncryptor(scoped_refptr<os_crypt_async::Encryptor> encryptor) {
    encryptor_ = std::move(encryptor);
    FileRunner()->PostTaskAndReplyWithResult(
        FROM_HERE, base::BindOnce([] {
          std::string json;
          return base::ReadFileToString(StorePath(), &json) ? json : std::string();
        }),
        base::BindOnce(&Account::Loaded, weak_factory_.GetWeakPtr()));
  }

  std::string Decrypt(const std::string* encoded) const {
    std::optional<std::vector<uint8_t>> data =
        encoded ? base::Base64Decode(*encoded) : std::nullopt;
    if (!data || !encryptor_) {
      return std::string();
    }
    return encryptor_->DecryptData(*data).value_or(std::string());
  }

  std::string Encrypt(const std::string& plain) const {
    std::optional<std::vector<uint8_t>> data =
        encryptor_ ? encryptor_->EncryptString(plain) : std::nullopt;
    return data ? base::Base64Encode(*data) : std::string();
  }

  void Loaded(std::string json) {
    std::optional<base::DictValue> root = base::JSONReader::ReadDict(json, base::JSON_PARSE_RFC);
    if (root) {
      if (const std::string* server = root->FindString("server")) {
        server_ = GURL(*server);
      }
      const std::string* email = root->FindString("email");
      const std::string* account = root->FindString("account");
      const std::string* device = root->FindString("device");
      const std::string refresh = Decrypt(root->FindString("refresh"));
      if (email && account && device && !refresh.empty()) {
        email_ = *email;
        account_ = *account;
        device_ = *device;
        refresh_ = refresh;
      }
      const std::string secret = Decrypt(root->FindString("secret"));
      if (SignedIn() && secret.size() == SyncSecret().size()) {
        SyncSecret kept;
        std::ranges::copy(secret, kept.begin());
        SetSecret(kept);
      }
    }
    loaded_ = true;
    if (syncer::SyncService* service = Service()) {
      ObserveService(service);
    }
    if (SignedIn()) {
      StartSyncing();
      ConnectRelay();
    }
    Notify();
  }

  void Save() {
    if (!loaded_) {
      return;
    }
    base::DictValue root;
    if (server_.is_valid()) {
      root.Set("server", server_.spec());
    }
    if (SignedIn()) {
      root.Set("email", email_);
      root.Set("account", account_);
      root.Set("device", device_);
      root.Set("refresh", Encrypt(refresh_));
      if (secret_) {
        root.Set("secret", Encrypt(std::string(secret_->begin(), secret_->end())));
      }
    }
    std::string json = base::WriteJson(root).value_or(std::string());
    FileRunner()->PostTask(FROM_HERE, base::BindOnce(
                                          [](std::string json) {
                                            base::ImportantFileWriter::WriteFileAtomically(
                                                StorePath(), json);
                                          },
                                          std::move(json)));
  }

  // Requests.

  void Fetch(const std::string& method,
             const GURL& url,
             const std::string& bearer,
             const std::string& form,
             Fetched done,
             const std::string& content_type = "application/x-www-form-urlencoded") {
    GURL::Replacements without_query;
    without_query.ClearQuery();
    without_query.ClearRef();
    LOG(ERROR) << "browser: sync request " << method << " "
               << url.ReplaceComponents(without_query).spec();
    auto request = std::make_unique<network::ResourceRequest>();
    request->url = url;
    request->method = method;
    request->credentials_mode = network::mojom::CredentialsMode::kOmit;
    if (!bearer.empty()) {
      request->headers.SetHeader("Authorization", "Bearer " + bearer);
    }
    auto it = loaders_.insert(loaders_.end(),
                              network::SimpleURLLoader::Create(std::move(request), kAccount));
    if (!form.empty() || method == "POST") {
      (*it)->AttachStringForUpload(form, content_type);
    }
    (*it)->SetAllowHttpErrorResults(true);
    (*it)->DownloadToString(
        g_browser_process->shared_url_loader_factory().get(),
        base::BindOnce(&Account::FetchDone, weak_factory_.GetWeakPtr(), it, std::move(done)),
        1 << 20);
  }

  void FetchDone(std::list<std::unique_ptr<network::SimpleURLLoader>>::iterator it,
                 Fetched done,
                 std::optional<std::string> body) {
    const network::mojom::URLResponseHead* head = (*it)->ResponseInfo();
    const int status = head && head->headers ? head->headers->response_code() : 0;
    loaders_.erase(it);
    std::optional<base::DictValue> json;
    if (body) {
      json = base::JSONReader::ReadDict(*body, base::JSON_PARSE_RFC);
    }
    std::move(done).Run(status, std::move(json));
  }

  // Takes tokens from /token's answer, and the account and device from its ID token, which came
  // straight from the server, so it needs no checking here.
  bool TakeTokens(const base::DictValue& json) {
    const std::string* access = json.FindString("access_token");
    const std::string* refresh = json.FindString("refresh_token");
    const std::optional<int> lifetime = json.FindInt("expires_in");
    const std::string* id_token = json.FindString("id_token");
    if (!access || !refresh || !lifetime || !id_token) {
      return false;
    }
    const std::vector<std::string> parts =
        base::SplitString(*id_token, ".", base::KEEP_WHITESPACE, base::SPLIT_WANT_ALL);
    std::string claims_json;
    if (parts.size() != 3 || !base::Base64UrlDecode(parts[1],
                                                    base::Base64UrlDecodePolicy::IGNORE_PADDING,
                                                    &claims_json)) {
      return false;
    }
    std::optional<base::DictValue> claims =
        base::JSONReader::ReadDict(claims_json, base::JSON_PARSE_RFC);
    const std::string* sub = claims ? claims->FindString("sub") : nullptr;
    const std::string* sid = claims ? claims->FindString("sid") : nullptr;
    const std::string* email = claims ? claims->FindString("email") : nullptr;
    if (!sub || !sid || (SignedIn() && *sub != account_)) {
      return false;
    }
    account_ = *sub;
    device_ = *sid;
    if (email) {
      email_ = *email;
    }
    access_ = *access;
    access_expiry_ = base::Time::Now() + base::Seconds(*lifetime);
    access_given_ = false;
    refresh_ = *refresh;
    return true;
  }

  void StartedOver(int generation, int status, std::optional<base::DictValue> json) {
    if (generation != generation_) {
      return;
    }
    if (status != 200) {
      problem_ = BC_SYNC_PROBLEM_SERVER;
      Notify();
      return;
    }
    starting_over_ = true;
    // The engine learns of the new birthday at its next request.
    if (syncer::SyncService* service = Service()) {
      service->TriggerRefresh(syncer::SyncService::TriggerRefreshSource::kUnknown,
                              {syncer::NIGORI});
    }
  }

  void SignInReturned(int generation, std::optional<GURL> url) {
    if (generation != generation_) {
      return;
    }
    // Closed here, after the window's call has returned.
    base::SequencedTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE,
                                                              std::move(sign_in_window_));
    std::string code;
    std::string state;
    if (!url || !net::GetValueForKeyInQuery(*url, "code", &code) ||
        !net::GetValueForKeyInQuery(*url, "state", &state) || state != state_) {
      signing_in_ = false;
      problem_ = url ? BC_SYNC_PROBLEM_SERVER : BC_SYNC_PROBLEM_NONE;
      Notify();
      return;
    }
    std::string form = "grant_type=authorization_code&client_id=" + std::string(kClientId) +
                       "&code=" + base::EscapeUrlEncodedData(code, true) +
                       "&redirect_uri=" + base::EscapeUrlEncodedData(redirect_.spec(), true) +
                       "&code_verifier=" + base::EscapeUrlEncodedData(verifier_, true) +
                       "&device_name=" + base::EscapeUrlEncodedData(device_name_, true);
    Fetch("POST", Server().Resolve("/token"), std::string(), form,
          base::BindOnce(&Account::SignedInTokens, weak_factory_.GetWeakPtr(), generation_));
  }

  void SignedInTokens(int generation, int status, std::optional<base::DictValue> json) {
    if (generation != generation_) {
      return;
    }
    signing_in_ = false;
    if (status != 200 || !json || !TakeTokens(*json)) {
      refresh_.clear();
      access_.clear();
      problem_ = BC_SYNC_PROBLEM_SERVER;
      Notify();
      return;
    }
    Save();
    StartSyncing();
    ConnectRelay();
    Notify();
  }

  void CloseSignInWindow() {
    if (sign_in_window_) {
      base::SequencedTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE,
                                                                std::move(sign_in_window_));
    }
  }

  // Runs |done| with a fresh access token, refreshing it first if it's about to expire, or with ""
  // if it can't.
  void WithToken(base::OnceCallback<void(const std::string&)> done) {
    if (Fresh()) {
      std::move(done).Run(access_);
      return;
    }
    Refresh(base::BindOnce(
        [](base::WeakPtr<Account> account, base::OnceCallback<void(const std::string&)> done,
           bool ok) { std::move(done).Run(account && ok ? account->access_ : std::string()); },
        weak_factory_.GetWeakPtr(), std::move(done)));
  }

  void Refresh(base::OnceCallback<void(bool)> done) {
    refresh_waiters_.push_back(std::move(done));
    if (refreshing_) {
      return;
    }
    refreshing_ = true;
    std::string form = "grant_type=refresh_token&client_id=" + std::string(kClientId) +
                       "&refresh_token=" + base::EscapeUrlEncodedData(refresh_, true);
    Fetch("POST", Server().Resolve("/token"), std::string(), form,
          base::BindOnce(&Account::Refreshed, weak_factory_.GetWeakPtr(), generation_));
  }

  void Refreshed(int generation, int status, std::optional<base::DictValue> json) {
    refreshing_ = false;
    if (generation != generation_) {
      RunRefreshWaiters(false);
      return;
    }
    if (status == 200 && json && TakeTokens(*json)) {
      Save();
      RunRefreshWaiters(true);
      return;
    }
    // A refresh token the server doesn't know: the device was removed or signed out.
    const std::string* error = json ? json->FindString("error") : nullptr;
    if ((status == 400 && error && *error == "invalid_grant") || status == 401) {
      Forget(BC_SYNC_PROBLEM_REMOVED);
      return;
    }
    RunRefreshWaiters(false);
  }

  void RunRefreshWaiters(bool ok) {
    std::vector<base::OnceCallback<void(bool)>> waiters;
    waiters.swap(refresh_waiters_);
    for (auto& waiter : waiters) {
      std::move(waiter).Run(ok);
    }
  }

  void Answer(TokenDone done, bool ok) {
    GoogleServiceAuthError error = GoogleServiceAuthError::AuthErrorNone();
    if (!SignedIn()) {
      error = GoogleServiceAuthError::FromInvalidGaiaCredentialsReason(
          GoogleServiceAuthError::InvalidGaiaCredentialsReason::CREDENTIALS_REJECTED_BY_SERVER);
    } else if (!ok) {
      error = GoogleServiceAuthError::FromConnectionError(net::ERR_FAILED);
    }
    signin::AccessTokenInfo info;
    if (error.state() == GoogleServiceAuthError::NONE) {
      info = signin::AccessTokenInfo(access_, access_expiry_, std::string());
      access_given_ = true;
    }
    // After the call returns, as Google's token fetcher answers.
    base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(std::move(done), std::move(error), std::move(info)));
  }

  // Forgets the account, as signed out, with |problem|.
  void Forget(int problem) {
    ++generation_;
    relay_retry_.Stop();
    relay_open_ = false;
    if (relay_) {
      base::SequencedTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE, std::move(relay_));
    }
    request_.reset();
    if (asked_) {
      asked_.reset();
      if (Host().sync_pairing_asked) {
        Host().sync_pairing_asked(Host().context, nullptr);
      }
    }
    setup_.reset();
    encrypting_ = false;
    starting_over_ = false;
    refresh_.clear();
    access_.clear();
    SetSecret(std::nullopt);
    email_.clear();
    account_.clear();
    device_.clear();
    problem_ = problem;
    Save();
    RunRefreshWaiters(false);
    AccountChanged();
    Notify();
  }

  // Sync.

  void AccountChanged() {
    for (auto& [identity_manager, changed] : watches_) {
      changed.Run();
    }
  }

  // Holds sync's data types back until the passphrase is set, so nothing commits under the
  // server's keystore key, which the server could read.
  void StartSyncing() {
    syncer::SyncService* service = Service();
    if (service && !sync_observation_.IsObserving()) {
      ObserveService(service);
    }
    if (service && !service->GetUserSettings()->IsInitialSyncFeatureSetupComplete() && !setup_) {
      setup_ = service->GetSetupInProgressHandle();
    }
    AccountChanged();
  }

  // Gives sync the secret's words, as the passphrase the account's data is encrypted with: a new
  // one on the account's first device, else the one it has.
  void UseSecret() {
    const std::optional<bool> passphrase = HasPassphrase();
    // Sync's settings tell its observers as each one changes, which calls this again, before
    // first-time setup is marked complete.
    if (!secret_ || !passphrase || using_secret_) {
      return;
    }
    base::AutoReset<bool> using_secret(&using_secret_, true);
    syncer::SyncUserSettings* settings = Service()->GetUserSettings();
    const std::string words = SyncWordsOf(*secret_);
    if (*passphrase) {
      // The account's data was started over with another secret since this device kept it.
      if (settings->IsPassphraseRequired() && !settings->SetDecryptionPassphrase(words)) {
        SetSecret(std::nullopt);
        Save();
        return;
      }
    } else if (!encrypting_) {
      encrypting_ = true;
      settings->SetEncryptionPassphrase(words);
    }
    if (!settings->IsInitialSyncFeatureSetupComplete()) {
      settings->SetSelectedTypes(/*sync_everything=*/false,
                                 {syncer::UserSelectableType::kBookmarks,
                                  syncer::UserSelectableType::kPasswords,
                                  syncer::UserSelectableType::kTabs});
      settings->SetInitialSyncFeatureSetupComplete();
    }
    setup_.reset();
  }

  // The secret from the words or a pairing, if it opens the account's data.
  void TakeSecret(const SyncSecret& secret) {
    problem_ = BC_SYNC_PROBLEM_NONE;
    syncer::SyncService* service = Service();
    syncer::SyncUserSettings* settings = service ? service->GetUserSettings() : nullptr;
    if (!settings || !settings->IsPassphraseRequired() ||
        !settings->SetDecryptionPassphrase(SyncWordsOf(secret))) {
      problem_ = BC_SYNC_PROBLEM_WRONG_WORDS;
      Notify();
      return;
    }
    SetSecret(secret);
    Save();
    UseSecret();
    Notify();
  }

  // Hears sync's service, and each request its engine makes to the server, which the log says.
  void ObserveService(syncer::SyncService* service) {
    sync_observation_.Observe(service);
    service->AddProtocolEventObserver(this);
    LogState(service);
  }

  // What sync's service is doing, each time that changes, so a check sees that a profile signed
  // out of the account starts nothing.
  void LogState(syncer::SyncService* sync) {
    using State = syncer::SyncService::TransportState;
    std::string state;
    switch (sync->GetTransportState()) {
      case State::DISABLED:
        state = "disabled";
        break;
      case State::PAUSED:
        state = "paused";
        break;
      case State::START_DEFERRED:
        state = "start deferred";
        break;
      case State::INITIALIZING:
        state = "initializing";
        break;
      case State::PENDING_DESIRED_CONFIGURATION:
        state = "pending configuration";
        break;
      case State::CONFIGURING:
        state = "configuring";
        break;
      case State::ACTIVE:
        state = "active";
        break;
    }
    if (sync->HasDisableReason(syncer::SyncService::DISABLE_REASON_NOT_SIGNED_IN)) {
      state += ", not signed in";
    }
    if (sync->HasDisableReason(syncer::SyncService::DISABLE_REASON_ENTERPRISE_POLICY)) {
      state += ", by policy";
    }
    if (sync->HasDisableReason(syncer::SyncService::DISABLE_REASON_UNRECOVERABLE_ERROR)) {
      state += ", after an error";
    }
    if (state != logged_state_) {
      logged_state_ = state;
      LOG(ERROR) << "browser: sync service " << state;
    }
  }

  // syncer::ProtocolEventObserver:
  void OnProtocolEvent(const syncer::ProtocolEvent& event) override {
    // Its type alone, such as "Commit Request": ProtocolEvent keeps GetType private.
    // The dict outlives the line, as the string FindString points to is the dict's.
    const base::DictValue value = event.ToValue(/*include_specifics=*/false);
    const std::string* type = value.FindString("type");
    LOG(ERROR) << "browser: sync engine " << (type ? *type : "event");
  }

  // syncer::SyncServiceObserver:
  void OnStateChanged(syncer::SyncService* sync) override {
    LogState(sync);
    if (starting_over_ && sync->IsEngineInitialized()) {
      starting_over_ = HasPassphrase().value_or(true);
      // The engine's restart dropped the hold on sync's data types with first-time setup.
      if (!starting_over_ && !setup_ &&
          !sync->GetUserSettings()->IsInitialSyncFeatureSetupComplete()) {
        setup_ = sync->GetSetupInProgressHandle();
      }
    }
    UseSecret();
    Notify();
  }
  void OnSyncShutdown(syncer::SyncService* sync) override {
    setup_.reset();
    sync->RemoveProtocolEventObserver(this);
    sync_observation_.Reset();
  }

  // Relay.

  GURL RelayUrl() const {
    GURL::Replacements replacements;
    replacements.SetSchemeStr(Server().SchemeIs("https") ? "wss" : "ws");
    replacements.SetPathStr("/relay");
    return Server().ReplaceComponents(replacements);
  }

  void ConnectRelay() {
    if (!SignedIn() || relay_ || connecting_) {
      return;
    }
    connecting_ = true;
    WithToken(base::BindOnce(
        [](base::WeakPtr<Account> account, int generation, const std::string& token) {
          if (!account) {
            return;
          }
          account->connecting_ = false;
          if (generation != account->generation_ || !account->SignedIn()) {
            return;
          }
          if (token.empty()) {
            account->RetryRelay();
            return;
          }
          LOG(ERROR) << "browser: sync relay " << account->RelayUrl().spec();
          account->relay_ = std::make_unique<SyncRelay>(account->RelayUrl(), token, account.get());
        },
        weak_factory_.GetWeakPtr(), generation_));
  }

  // Tries again later, waiting twice as long each time, up to 5 minutes; there's no timer while the
  // relay is connected.
  void RetryRelay() {
    relay_backoff_ = std::min(relay_backoff_.is_zero() ? base::Seconds(2) : relay_backoff_ * 2,
                              base::Minutes(5));
    relay_retry_.Start(FROM_HERE, relay_backoff_,
                       base::BindOnce(&Account::ConnectRelay, base::Unretained(this)));
  }

  // SyncRelay::Delegate:
  void OnRelayOpen() override {
    relay_open_ = true;
    relay_backoff_ = base::TimeDelta();
    // Anything another device committed while this one wasn't listening.
    if (Profile* start = StartProfile(); start && secret_) {
      if (auto* invalidations = static_cast<RelayInvalidations*>(
              SyncInvalidationsServiceFactory::GetForProfile(start))) {
        invalidations->Deliver({});
      }
    }
  }

  void OnRelayClosed(uint16_t code, int response_code) override {
    relay_open_ = false;
    base::SequencedTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE, std::move(relay_));
    if (request_) {
      request_.reset();
      problem_ = BC_SYNC_PROBLEM_EXPIRED;
    }
    if (asked_) {
      asked_.reset();
      if (Host().sync_pairing_asked) {
        Host().sync_pairing_asked(Host().context, nullptr);
      }
    }
    Notify();
    // Signed out on the server, or a token it refused: a refresh says which.
    if (code == 4001 || response_code == 401) {
      access_.clear();
      Refresh(base::BindOnce(
          [](base::WeakPtr<Account> account, bool ok) {
            if (account && account->SignedIn()) {
              ok ? account->ConnectRelay() : account->RetryRelay();
            }
          },
          weak_factory_.GetWeakPtr()));
      return;
    }
    RetryRelay();
  }

  void OnRelayMessage(const std::string& message) override {
    // A relay already let go of, whose last frames came in before it went.
    if (!relay_) {
      return;
    }
    std::optional<base::DictValue> frame =
        base::JSONReader::ReadDict(message, base::JSON_PARSE_RFC);
    const std::string* type = frame ? frame->FindString("type") : nullptr;
    if (!type) {
      return;
    }
    const std::string* pairing = frame->FindString("pairing");
    if (*type == "changed") {
      std::vector<int> types;
      if (const base::ListValue* list = frame->FindList("types")) {
        for (const base::Value& each : *list) {
          if (each.is_int()) {
            types.push_back(each.GetInt());
          }
        }
      }
      if (Profile* start = StartProfile()) {
        if (auto* invalidations = static_cast<RelayInvalidations*>(
                SyncInvalidationsServiceFactory::GetForProfile(start))) {
          invalidations->Deliver(types);
        }
      }
    } else if (*type == "pair_request" && pairing) {
      Ask(*frame);
    } else if (*type == "pair_requested" && pairing && request_ && request_->id.empty()) {
      request_->id = *pairing;
      StartExpiry(request_->expiry, *frame, *pairing);
    } else if (*type == "pair_accept" && pairing && request_ && request_->id == *pairing) {
      std::optional<PairingKey> approver = Key32(frame->FindString("public_key"));
      if (!approver) {
        return;
      }
      if (!request_->pairing.Accept(*approver)) {
        // A second key, from someone between the devices trying to make the codes agree.
        SendPairing("pair_cancel", *pairing);
        request_.reset();
        problem_ = BC_SYNC_PROBLEM_SERVER;
        Notify();
        return;
      }
      base::DictValue reveal;
      reveal.Set("type", "pair_reveal");
      reveal.Set("pairing", *pairing);
      reveal.Set("public_key", Base64Url(request_->pairing.public_key()));
      relay_->Send(Frame(std::move(reveal)));
      Notify();
    } else if (*type == "pair_reveal" && pairing && asked_ && asked_->id == *pairing &&
               asked_->accepted) {
      std::optional<PairingKey> requester = Key32(frame->FindString("public_key"));
      const bool shown = !asked_->pairing.code().empty();
      if (!requester || !asked_->pairing.Reveal(*requester)) {
        // Not the key the request committed to, or a second: someone between the devices.
        SendPairing("pair_cancel", asked_->id);
        asked_.reset();
        if (shown && Host().sync_pairing_asked) {
          Host().sync_pairing_asked(Host().context, nullptr);
        }
        return;
      }
      if (Host().sync_pairing_asked) {
        bc_sync_pairing asked = {.id = asked_->id.c_str(),
                                 .name = asked_->name.c_str(),
                                 .os = asked_->os.c_str(),
                                 .code = asked_->pairing.code().c_str(),
                                 .asked_at = asked_->asked_at,
                                 .signed_in_at = asked_->signed_in_at};
        Host().sync_pairing_asked(Host().context, &asked);
      }
    } else if (*type == "pair_sealed" && pairing && request_ && request_->id == *pairing) {
      std::optional<std::vector<uint8_t>> sealed;
      if (const std::string* encoded = frame->FindString("sealed")) {
        sealed = base::Base64UrlDecode(*encoded, base::Base64UrlDecodePolicy::IGNORE_PADDING);
      }
      std::optional<SyncSecret> secret =
          sealed ? request_->pairing.Open(*pairing, *sealed) : std::nullopt;
      request_.reset();
      if (!secret) {
        problem_ = BC_SYNC_PROBLEM_SERVER;
        Notify();
        return;
      }
      TakeSecret(*secret);
    } else if (*type == "pair_closed" && pairing) {
      const std::string* reason = frame->FindString("reason");
      Closed(*pairing, reason ? *reason : std::string());
    } else if (*type == "error") {
      if (request_ && (request_->id.empty() || (pairing && *pairing == request_->id))) {
        request_.reset();
        problem_ = BC_SYNC_PROBLEM_SERVER;
        Notify();
      } else if (asked_ && pairing && *pairing == asked_->id) {
        Closed(*pairing, "cancelled");
      }
    }
  }

  // Pairing.

  void SendPairing(const std::string& type, const std::string& pairing) {
    if (!relay_open_) {
      return;
    }
    base::DictValue frame;
    frame.Set("type", type);
    frame.Set("pairing", pairing);
    relay_->Send(Frame(std::move(frame)));
  }

  // Gives up at the pairing's expires_at, as the server ends it only at its next step.
  void StartExpiry(base::OneShotTimer& timer, const base::DictValue& frame,
                   const std::string& pairing) {
    const std::optional<double> expires_at = frame.FindDouble("expires_at");
    base::TimeDelta left = base::Minutes(5);
    if (expires_at) {
      left = std::clamp(base::Time::FromSecondsSinceUnixEpoch(*expires_at) - base::Time::Now(),
                        base::TimeDelta(), base::Minutes(5));
    }
    timer.Start(FROM_HERE, left,
                base::BindOnce(&Account::Closed, base::Unretained(this), pairing,
                               std::string("expired")));
  }

  void Ask(const base::DictValue& frame) {
    const std::string* pairing = frame.FindString("pairing");
    std::optional<std::array<uint8_t, 32>> commitment = Key32(frame.FindString("commitment"));
    // One at a time, and only with the secret to give.
    if (!secret_ || asked_ || !commitment) {
      return;
    }
    asked_ = std::make_unique<Asked>(*commitment);
    asked_->id = *pairing;
    const std::string* name = frame.FindString("name");
    const std::string* os = frame.FindString("os");
    asked_->name = name ? *name : std::string();
    asked_->os = os ? *os : std::string();
    // Seconds, from the server.
    asked_->asked_at = frame.FindDouble("asked_at").value_or(0) * 1000;
    asked_->signed_in_at = frame.FindDouble("signed_in_at").value_or(0) * 1000;
    StartExpiry(asked_->expiry, frame, *pairing);
    for (Window* window : Window::All()) {
      if (window->widget() && window->widget()->IsActive()) {
        Accept();
        break;
      }
    }
  }

  // Answers the request with this device's key, so the device the user is at takes it.
  void Accept() {
    asked_->accepted = true;
    base::DictValue frame;
    frame.Set("type", "pair_accept");
    frame.Set("pairing", asked_->id);
    frame.Set("public_key", Base64Url(asked_->pairing.public_key()));
    if (relay_open_) {
      relay_->Send(Frame(std::move(frame)));
    }
  }

  void Closed(const std::string& pairing, const std::string& reason) {
    if (request_ && request_->id == pairing) {
      if (reason == "expired") {
        SendPairing("pair_cancel", pairing);
      }
      request_.reset();
      problem_ = reason == "cancelled" ? BC_SYNC_PROBLEM_DECLINED : BC_SYNC_PROBLEM_EXPIRED;
      Notify();
    } else if (asked_ && asked_->id == pairing) {
      const bool shown = !asked_->pairing.code().empty();
      asked_.reset();
      if (shown && reason != "done" && Host().sync_pairing_asked) {
        Host().sync_pairing_asked(Host().context, nullptr);
      }
    }
  }

  void DevicesRead(int status, std::optional<base::DictValue> json) {
    const base::ListValue* list = json ? json->FindList("devices") : nullptr;
    if (status != 200 || !list || !Host().sync_devices_changed) {
      return;
    }
    struct Device {
      std::string id;
      std::string name;
      bool current;
    };
    std::vector<Device> devices;
    for (const base::Value& value : *list) {
      const base::DictValue* dict = value.GetIfDict();
      const std::string* id = dict ? dict->FindString("id") : nullptr;
      const std::string* name = dict ? dict->FindString("name") : nullptr;
      if (!id || !name) {
        continue;
      }
      const bool current = dict->FindBool("current").value_or(false);
      devices.insert(current ? devices.begin() : devices.end(), Device{*id, *name, current});
    }
    // The server keeps no system; the name, which the device gave, says what it is.
    std::vector<bc_sync_device> listed;
    for (const Device& device : devices) {
      listed.push_back({.id = device.id.c_str(),
                        .name = device.name.c_str(),
                        .os = "",
                        .current = device.current});
    }
    Host().sync_devices_changed(Host().context, listed.data(), listed.size());
  }

  bool loaded_ = false;
  scoped_refptr<os_crypt_async::Encryptor> encryptor_;
  // Unset for the build's own, from --sync-url.
  GURL server_;
  std::string email_;
  std::string account_;
  std::string device_;
  std::string device_name_;
  std::string refresh_;
  std::string access_;
  base::Time access_expiry_;
  // Sync has had access_, so asking again means it was refused.
  bool access_given_ = false;
  std::optional<SyncSecret> secret_;
  int problem_ = BC_SYNC_PROBLEM_NONE;
  // Bumped as the account is forgotten or a sign-in cancelled, so answers to the old are dropped.
  int generation_ = 0;

  bool signing_in_ = false;
  std::unique_ptr<SignInWindow> sign_in_window_;
  std::string verifier_;
  std::string state_;
  GURL redirect_;

  bool refreshing_ = false;
  std::vector<base::OnceCallback<void(bool)>> refresh_waiters_;
  std::list<std::unique_ptr<network::SimpleURLLoader>> loaders_;

  std::unique_ptr<SyncRelay> relay_;
  bool relay_open_ = false;
  bool connecting_ = false;
  base::TimeDelta relay_backoff_;
  base::OneShotTimer relay_retry_;

  std::unique_ptr<Request> request_;
  std::unique_ptr<Asked> asked_;

  std::map<const signin::IdentityManager*, base::RepeatingClosure> watches_;
  std::unique_ptr<syncer::SyncSetupInProgressHandle> setup_;
  // SetEncryptionPassphrase was called, and its answer is yet to come back as the type.
  bool encrypting_ = false;
  bool using_secret_ = false;
  // The server cleared the account's data for bc_sync_start_over, and the engine is yet to see.
  bool starting_over_ = false;
  base::ScopedObservation<syncer::SyncService, syncer::SyncServiceObserver> sync_observation_{
      this};

  bool notified_ = false;
  int notified_status_ = 0;
  int notified_problem_ = 0;
  std::array<std::string, 3> notified_strings_;
  std::string logged_state_;

  base::WeakPtrFactory<Account> weak_factory_{this};
};

}  // namespace

void Install() {
  g_browser_core_sync_account = [](const signin::IdentityManager* identity_manager) {
    return Account::Get().SyncAccount(identity_manager);
  };
  g_browser_core_sync_token = [](const signin::IdentityManager* identity_manager, TokenDone done) {
    Account::Get().SyncToken(identity_manager, std::move(done));
  };
  g_browser_core_sync_account_watch = [](const signin::IdentityManager* identity_manager,
                                         base::RepeatingClosure changed) {
    Account::Get().Watch(identity_manager, std::move(changed));
  };
  g_browser_core_sync_invalidations = [](Profile* profile) {
    return Account::Get().CreateInvalidations(profile);
  };
  g_browser_core_sync_url = [] { return Account::Get().SyncUrl(); };
  g_browser_core_sync_hold_commit = &HoldSyncCommit;
  g_browser_core_sync_commit = [](syncer::DataType type, syncer::CommitRequestDataList& commit) {
    TrimSyncCommit(type, commit);
    // The hold keeps a type from committing until the passphrase encrypts it, so a commit that
    // isn't ciphertext is a bug that would give the server what it mustn't read. Stopping here
    // sends nothing; dropping the commit instead would leave its changes in flight for good.
    CHECK(SyncCommitIsCiphertext(type, commit))
        << "plaintext in a commit of " << syncer::DataTypeToDebugString(type);
  };
  g_browser_core_sync_password_tag = &SyncPasswordTag;
  g_browser_core_passkey_normal_wording = [](content::WebContents* contents) {
    const Profile* profile =
        contents ? Profile::FromBrowserContext(contents->GetBrowserContext()) : nullptr;
    if (!profile || !profile->IsOffTheRecord()) {
      return false;
    }
    // OTRProfileID's ToString is private; its stream operator is the public way to the id.
    std::ostringstream id;
    id << profile->GetOTRProfileID();
    return id.str().starts_with(kSignInProfile);
  };
}

void Start() {
  Account::Get().Start();
}

void WindowActivated() {
  Account::Get().WindowActivated();
}

bool SetServer(const std::string& server) {
  return Account::Get().SetServer(server);
}

void SignIn() {
  Account::Get().SignIn();
}

void Cancel() {
  Account::Get().Cancel();
}

void SignOut() {
  Account::Get().SignOut();
}

std::string RecoveryWords() {
  return Account::Get().RecoveryWords();
}

bool EnterRecoveryWords(const std::string& words) {
  return Account::Get().EnterRecoveryWords(words);
}

void StartOver() {
  Account::Get().StartOver();
}

void RequestPairing() {
  Account::Get().RequestPairing();
}

void AnswerPairing(const std::string& id, bool add) {
  Account::Get().AnswerPairing(id, add);
}

void ReadDevices() {
  Account::Get().ReadDevices();
}

void RemoveDevice(const std::string& id) {
  Account::Get().RemoveDevice(id);
}

}  // namespace browser_core::sync
