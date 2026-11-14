#include "browser_core/site_controls.h"

#include <algorithm>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/ad_block.h"
#include "browser_core/developer_mode.h"
#include "browser_core/drm_request.h"
#include "browser_core/host.h"
#include "browser_core/native_ad_block.h"
#include "browser_core/passwords.h"
#include "browser_core/permission_question.h"
#include "browser_core/system_engine.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/certificate_viewer.h"
#include "chrome/browser/content_settings/host_content_settings_map_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/ui/page_info/chrome_page_info_delegate.h"
#include "chrome/browser/ui/page_info/chrome_page_info_ui_delegate.h"
#include "components/content_settings/browser/page_specific_content_settings.h"
#include "components/content_settings/core/browser/permission_settings_registry.h"
#include "components/content_settings/core/browser/website_settings_info.h"
#include "components/content_settings/core/browser/website_settings_registry.h"
#include "components/content_settings/core/common/content_settings.h"
#include "components/permissions/object_permission_context_base.h"
#include "components/strings/grit/components_branded_strings.h"
#include "components/strings/grit/components_strings.h"
#include "components/url_formatter/elide_url.h"
#include "content/public/browser/media_session.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_entry.h"
#include "content/public/browser/ssl_status.h"
#include "content/public/browser/web_contents.h"
#include "extensions/browser/extension_action.h"
#include "net/cert/x509_certificate.h"
#include "services/media_session/public/mojom/media_session.mojom.h"
#include "ui/base/l10n/l10n_util.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

using content_settings::SettingSource;
using media_session::mojom::MediaSessionAction;

// A BC_DEVICE_ value for a chosen device's kind, which picks its glyph.
int DeviceTypeOf(ContentSettingsType type) {
  switch (type) {
    case ContentSettingsType::SERIAL_CHOOSER_DATA:
      return BC_DEVICE_SERIAL;
    case ContentSettingsType::HID_CHOOSER_DATA:
      return BC_DEVICE_HID;
    case ContentSettingsType::BLUETOOTH_CHOOSER_DATA:
      return BC_DEVICE_BLUETOOTH;
    default:
      return BC_DEVICE_USB;
  }
}

// The panel's tab's, while it shows.
std::unique_ptr<SiteControls>& Shown() {
  static base::NoDestructor<std::unique_ptr<SiteControls>> shown;
  return *shown;
}

SiteControls* ShownFor(bc_id tab) {
  return Shown() && Tab::IdOf(Shown()->web_contents()) == tab ? Shown().get() : nullptr;
}

const content_settings::PermissionSettingsInfo::Delegate* DelegateFor(ContentSettingsType type) {
  const content_settings::PermissionSettingsInfo* info =
      content_settings::PermissionSettingsRegistry::GetInstance()->Get(type);
  return info ? &info->delegate() : nullptr;
}

// A setting as one of the three the panel shows, as Chrome's page info words it.
int ValueOf(const content_settings::PermissionSettingsInfo::Delegate& delegate,
            const PermissionSetting& setting) {
  if (delegate.IsAnyPermissionAllowed(setting)) {
    return BC_SITE_ALLOW;
  }
  return delegate.IsUndecided(setting) ? BC_SITE_ASK : BC_SITE_BLOCK;
}

// Chrome lets the user change only what the user set: not a policy's, an extension's or a
// supervisor's, and its page info leaves out the choices a type doesn't take.
uint32_t ChoicesFor(const content_settings::PermissionSettingsInfo::Delegate& delegate,
                    ChromePageInfoUiDelegate& ui_delegate,
                    const PageInfo::PermissionInfo& permission) {
  if (permission.source != SettingSource::kUser) {
    return 0;
  }
  uint32_t choices = 1u << BC_SITE_BLOCK;
  if (ui_delegate.ShouldShowAllow(permission.type)) {
    choices |= 1u << BC_SITE_ALLOW;
  }
  if (delegate.IsUndecided(permission.default_setting) ||
      ui_delegate.ShouldShowAsk(permission.type)) {
    choices |= 1u << BC_SITE_ASK;
  }
  return choices;
}

// Chrome's page info shows its reset button for a setting the user made that it lets them change.
bool Resettable(ChromePageInfoUiDelegate& ui_delegate, const PageInfo::PermissionInfo& permission) {
  return permission.source == SettingSource::kUser && permission.setting &&
         (ui_delegate.ShouldShowAllow(permission.type) ||
          ui_delegate.ShouldShowAsk(permission.type));
}

// What Site Controls list of Chrome's page info rows: what differs for the site (#2334). Chrome also
// lists every type whose profile default stopped asking, which says nothing about this site; that
// row shows here only once the site has its own value, a policy or an extension sets it, the
// user changed it here since the page loaded (so a line doesn't vanish under their hand), or the
// tab used it: played sound, registered for automatic Picture in Picture, or holds files open.
bool Differs(content::WebContents* contents, const PageInfo::PermissionInfo& permission) {
  if (permission.setting || permission.source != SettingSource::kUser ||
      permission.type == ContentSettingsType::SOUND ||
      permission.type == ContentSettingsType::AUTO_PICTURE_IN_PICTURE ||
      permission.type == ContentSettingsType::FILE_SYSTEM_WRITE_GUARD) {
    return true;
  }
  const auto* settings =
      content_settings::PageSpecificContentSettings::GetForFrame(contents->GetPrimaryMainFrame());
  return settings && settings->HasContentSettingChangedViaPageInfo(permission.type);
}

// The system page a tab shows in place of Chromium's, else null.
platform::SystemPage* SystemPageOf(content::WebContents* contents) {
  const Tab* tab = Tab::FromWebContents(contents);
  return tab ? tab->system_page() : nullptr;
}

// Whether the page plays a video that Picture in Picture can take, or has, as the media session
// that Chrome's media controls use says.
int PictureInPictureOf(content::WebContents* contents) {
  content::MediaSession* session =
      SystemPageOf(contents) ? nullptr : content::MediaSession::GetIfExists(contents);
  if (!session) {
    return BC_PICTURE_IN_PICTURE_NONE;
  }
  const std::vector<MediaSessionAction> actions = session->GetMediaSessionActionsSync();
  if (std::ranges::contains(actions, MediaSessionAction::kExitPictureInPicture)) {
    return BC_PICTURE_IN_PICTURE_EXIT;
  }
  const bool playing = session->GetMediaSessionInfoSync()->playback_state ==
                       media_session::mojom::MediaPlaybackState::kPlaying;
  return playing && std::ranges::contains(actions, MediaSessionAction::kEnterPictureInPicture)
             ? BC_PICTURE_IN_PICTURE_ENTER
             : BC_PICTURE_IN_PICTURE_NONE;
}

// The certificate the page shown came with, as Chrome's page info reads it, else null. WebKit
// shows its own page's nowhere ilo can.
net::X509Certificate* CertificateOf(content::WebContents* contents) {
  if (SystemPageOf(contents)) {
    return nullptr;
  }
  content::NavigationEntry* entry = contents->GetController().GetVisibleEntry();
  return entry ? entry->GetSSL().certificate.get() : nullptr;
}

// The registry's name for the type, which the app picks a glyph by.
const char* KeyOf(ContentSettingsType type) {
  const content_settings::WebsiteSettingsInfo* info =
      content_settings::WebsiteSettingsRegistry::GetInstance()->Get(type);
  return info ? info->name().c_str() : "";
}

// A BC_AD_BLOCKING_ value for the site of |contents|'s page.
int AdBlockingOf(content::WebContents* contents) {
  const std::optional<bool> on = AdBlockingOnSite(contents);
  return !on ? BC_AD_BLOCKING_NONE : *on ? BC_AD_BLOCKING_ON : BC_AD_BLOCKING_OFF;
}

// A BC_DEVELOPER_MODE_ value for the site of |contents|'s page.
int DeveloperModeOf(content::WebContents* contents) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  const GURL& url = contents->GetLastCommittedURL();
  if (!developer_mode::Switchable(profile, url)) {
    return BC_DEVELOPER_MODE_NONE;
  }
  return developer_mode::On(profile, url) ? BC_DEVELOPER_MODE_ON : BC_DEVELOPER_MODE_OFF;
}

}  // namespace

void SiteControls::Open(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents) {
    return;
  }
  // The last first, as its window may be this one's, whose actions it puts back to the pinned.
  Shown().reset();
  Shown() = base::WrapUnique(new SiteControls(contents));
}

void SiteControls::Close() {
  Shown().reset();
}

void SiteControls::SetPermission(bc_id tab, int type, int value) {
  SiteControls* controls = ShownFor(tab);
  if (!controls || !controls->page_info_) {
    return;
  }
  auto permission =
      std::ranges::find(controls->permissions_, static_cast<ContentSettingsType>(type),
                        &PageInfo::PermissionInfo::type);
  const auto* delegate =
      permission != controls->permissions_.end() ? DelegateFor(permission->type) : nullptr;
  if (!delegate || value < 0 || value > BC_SITE_ASK ||
      !(ChoicesFor(*delegate, *controls->ui_delegate_, *permission) & (1u << value))) {
    return;
  }
  // Ask where asking is what the site gets by default is no setting of its own, which Chrome's
  // page info passes as none.
  std::optional<PermissionSetting> setting;
  if (value != BC_SITE_ASK || !delegate->IsUndecided(permission->default_setting)) {
    setting = delegate->ToPermissionSetting(value == BC_SITE_ALLOW   ? CONTENT_SETTING_ALLOW
                                            : value == BC_SITE_BLOCK ? CONTENT_SETTING_BLOCK
                                                                     : CONTENT_SETTING_ASK);
  }
  // Allow or Block for a request held quietly answers it, as the card would have, so the page
  // hears. The answer goes first, as listing the permissions again drops the held row.
  const ContentSettingsType kind = permission->type;
  const std::optional<url::Origin> origin = permission->requesting_origin;
  if (kind == ContentSettingsType::NOTIFICATIONS && value != BC_SITE_ASK &&
      PermissionQuestion::Quiet(tab)) {
    PermissionQuestion::Answer(
        tab, value == BC_SITE_ALLOW ? BC_PERMISSION_ALLOW : BC_PERMISSION_BLOCK);
  }
  // PageInfo lists the permissions again, which reports them.
  controls->page_info_->OnSitePermissionChanged(kind, setting, origin, /*is_one_time=*/false);
}

void SiteControls::RemoveDevice(bc_id tab, size_t index) {
  SiteControls* controls = ShownFor(tab);
  if (!controls || !controls->page_info_ || index >= controls->devices_.size()) {
    return;
  }
  const PageInfoUI::ChosenObjectInfo& device = *controls->devices_[index];
  if (device.chooser_object->source == content_settings::SettingSource::kPolicy) {
    return;
  }
  // As Chrome's page info's delete button. PageInfo lists them again, which replaces |device|.
  const base::Value value(device.chooser_object->value.Clone());
  controls->page_info_->OnSiteChosenObjectDeleted(*device.ui_info, value);
}

void SiteControls::Reset(bc_id tab) {
  SiteControls* controls = ShownFor(tab);
  if (!controls || !controls->page_info_) {
    return;
  }
  // As Chrome's page info's button: each setting goes back to the default. A copy, as each
  // change lists the permissions again.
  const PermissionInfoList permissions = controls->permissions_;
  for (const PageInfo::PermissionInfo& permission : permissions) {
    if (Resettable(*controls->ui_delegate_, permission)) {
      controls->page_info_->OnSitePermissionChanged(permission.type, std::nullopt,
                                                    permission.requesting_origin,
                                                    /*is_one_time=*/false);
    }
  }
}

void SiteControls::PageChanged(bc_id tab) {
  if (SiteControls* controls = ShownFor(tab)) {
    controls->Read();
  }
}

void SiteControls::LoginsChanged(Profile* profile) {
  SiteControls* controls = Shown().get();
  if (controls && Profile::FromBrowserContext(controls->web_contents()->GetBrowserContext())
                          ->GetOriginalProfile() == profile) {
    controls->Changed();
  }
}

void SiteControls::SetAdBlocking(bc_id tab, bool on) {
  SiteControls* controls = ShownFor(tab);
  if (!controls || SystemPageOf(controls->web_contents())) {
    return;
  }
  SetAdBlockingOnSite(controls->web_contents(), on);
  // At once, as the page it reloads only reports once it commits.
  controls->Changed();
}

void SiteControls::PictureInPicture(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  content::MediaSession* session =
      contents ? content::MediaSession::GetIfExists(contents) : nullptr;
  switch (contents ? PictureInPictureOf(contents) : BC_PICTURE_IN_PICTURE_NONE) {
    case BC_PICTURE_IN_PICTURE_ENTER:
      session->EnterPictureInPicture();
      break;
    case BC_PICTURE_IN_PICTURE_EXIT:
      session->ExitPictureInPicture();
      break;
  }
}

void SiteControls::ShowCertificate(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  net::X509Certificate* certificate = contents ? CertificateOf(contents) : nullptr;
  if (certificate) {
    ShowCertificateViewer(contents, contents->GetTopLevelNativeWindow(), certificate);
  }
}

SiteControls::SiteControls(content::WebContents* contents)
    : content::WebContentsObserver(contents), tab_(Tab::IdOf(contents)) {
  // Before the site, so the panel has its extensions as it first shows.
  if (Window* window = Window::Showing(tab_)) {
    window_ = window->id();
    window->ShowEveryExtensionAction(true);
  }
  // Its private windows' too, as the dispatcher is the profile's.
  actions_.Observe(extensions::ExtensionActionDispatcher::Get(contents->GetBrowserContext()));
  Read();
}

SiteControls::~SiteControls() {
  if (NativeAdBlock()) {
    StopWatchingNativeAdBlocked();
  }
  if (Window* window = Window::FromId(window_)) {
    window->ShowEveryExtensionAction(false);
  }
}

void SiteControls::Read() {
  platform::SystemPage* page = SystemPageOf(web_contents());
  const GURL url = page ? page->url() : web_contents()->GetLastCommittedURL();
  url_ = url;
  page_info_.reset();
  permissions_.clear();
  ui_delegate_ = std::make_unique<ChromePageInfoUiDelegate>(web_contents(), url);
  site_ = base::UTF16ToUTF8(url_formatter::FormatUrlForSecurityDisplay(
      url, url_formatter::SchemeDisplay::OMIT_CRYPTOGRAPHIC));
  Profile* profile = Profile::FromBrowserContext(web_contents()->GetBrowserContext());
  // Not for an agent's tab, which stays in Chromium at a listed service too.
  system_engine_ = system_engine::Remembered(profile, url) && system_engine::Allowed(tab_);
  // Where the protected-media note's button would show the page there: a page of the tab asked,
  // or the user closed the site's note or turned the site off, which this is the way back from.
  system_engine_offered_ =
      !system_engine_ && !page && system_engine::Allowed(tab_) &&
      (MediaUnsupported(web_contents()) || system_engine::NoteClosed(profile, url));
  if (page) {
    // Chromium's permissions aren't the system page's, which asks for none, and WebKit says only
    // whether the connection is secure.
    connection_ = page->secure() ? BC_CONNECTION_SECURE : BC_CONNECTION_NOT_SECURE;
    connection_summary_ = l10n_util::GetStringUTF16(
        page->secure() ? IDS_PAGE_INFO_SECURE_SUMMARY : IDS_PAGE_INFO_NOT_SECURE_SUMMARY);
  } else if (PageInfo::IsFileOrInternalPage(url)) {
    // As Chrome's bubble for these, which says what the page is and lists nothing.
    connection_ = BC_CONNECTION_INTERNAL;
    connection_summary_ = l10n_util::GetStringUTF16(
        url.SchemeIsFile() ? IDS_PAGE_INFO_FILE_PAGE : IDS_PAGE_INFO_INTERNAL_PAGE);
  } else {
    page_info_ = std::make_unique<PageInfo>(
        std::make_unique<ChromePageInfoDelegate>(web_contents()), web_contents(), url);
    // It lists the permissions and the connection now, and the cookies later, which the panel
    // doesn't show.
    page_info_->InitializeUiState(this, base::DoNothing());
  }
  // The browser's own ad blocker counts what it blocked on the page, which comes a moment later.
  if (NativeAdBlock()) {
    native_blocked_ = -1;
    WatchNativeAdBlocked(web_contents(), base::BindRepeating(&SiteControls::NativeAdBlocked,
                                                             weak_factory_.GetWeakPtr()));
  }
  Changed();
}

void SiteControls::SetPermissionInfo(const PermissionInfoList& permission_info_list,
                                     ChosenObjectInfoList chosen_object_info_list) {
  permissions_ = permission_info_list;
  // A page holding a request to show notifications quietly lists them, at the default, as
  // Chrome's page info does on Android alone: the bell in the URL field opens the panel there.
  if (PermissionQuestion::Quiet(tab_) &&
      std::ranges::find(permissions_, ContentSettingsType::NOTIFICATIONS,
                        &PageInfo::PermissionInfo::type) == permissions_.end()) {
    PageInfo::PermissionInfo notifications;
    notifications.type = ContentSettingsType::NOTIFICATIONS;
    notifications.source = content_settings::SettingSource::kUser;
    notifications.default_setting =
        HostContentSettingsMapFactory::GetForProfile(web_contents()->GetBrowserContext())
            ->GetDefaultPermissionSetting(ContentSettingsType::NOTIFICATIONS, nullptr);
    permissions_.push_back(std::move(notifications));
  }
  devices_ = std::move(chosen_object_info_list);
  Changed();
}

void SiteControls::SetIdentityInfo(const IdentityInfo& identity_info) {
  const std::unique_ptr<SecurityDescription> description = GetSecurityDescription(identity_info);
  connection_summary_ = description ? description->summary : std::u16string();
  connection_ = identity_info.identity_status == PageInfo::SITE_IDENTITY_STATUS_INTERNAL_PAGE
                    ? BC_CONNECTION_INTERNAL
                : description && description->summary_style == SecuritySummaryColor::GREEN
                    ? BC_CONNECTION_SECURE
                    : BC_CONNECTION_NOT_SECURE;
  Changed();
}

void SiteControls::PrimaryPageChanged(content::Page& page) {
  // Chromium's page waits under the system page, whose commits come from PageChanged.
  if (!SystemPageOf(web_contents())) {
    Read();
  }
}

void SiteControls::MediaStartedPlaying(const MediaPlayerInfo& info,
                                       const content::MediaPlayerId& id) {
  Changed();
}

void SiteControls::MediaStoppedPlaying(const MediaPlayerInfo& info,
                                       const content::MediaPlayerId& id,
                                       MediaStoppedReason reason) {
  Changed();
}

void SiteControls::MediaPictureInPictureChanged(bool is_picture_in_picture) {
  Changed();
}

void SiteControls::WebContentsDestroyed() {
  // PageInfo can't outlive its tab. This destroys this.
  Close();
}

void SiteControls::OnExtensionActionUpdated(extensions::ExtensionAction* action,
                                            content::WebContents* contents,
                                            content::BrowserContext* context) {
  if (contents == web_contents() && IsAdBlocker(action->extension_id())) {
    Changed();
  }
}

void SiteControls::NativeAdBlocked(int blocked) {
  native_blocked_ = blocked;
  Changed();
}

void SiteControls::OnShuttingDown() {
  actions_.Reset();
}

void SiteControls::Changed() {
  if (reporting_) {
    return;
  }
  reporting_ = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&SiteControls::Report, weak_factory_.GetWeakPtr()));
}

void SiteControls::Report() {
  reporting_ = false;
  if (!Host().tab_site_controls) {
    return;
  }
  // The panel shows once, at its full height, so it waits for the profile's logins the first time
  // they're read, which reports again.
  Passwords* passwords =
      Passwords::Of(Profile::FromBrowserContext(web_contents()->GetBrowserContext()));
  if (passwords && !passwords->ready()) {
    return;
  }
  // uBlock Origin never sees the pages the system's engine shows.
  const int ad_blocking =
      SystemPageOf(web_contents()) ? BC_AD_BLOCKING_NONE : AdBlockingOf(web_contents());
  // It waits for the browser's own ad blocker's count too, which reports again.
  if (ad_blocking == BC_AD_BLOCKING_ON && NativeAdBlock() && native_blocked_ < 0) {
    return;
  }
  const std::vector<Passwords::Login> logins =
      passwords ? passwords->ForPage(url_) : std::vector<Passwords::Login>();
  const std::vector<bc_login> c_logins = Passwords::ToC(logins);
  std::vector<std::string> names;
  std::vector<std::string> states;
  std::vector<bc_site_permission> permissions;
  bool resettable = false;
  for (const PageInfo::PermissionInfo& permission : permissions_) {
    const auto* delegate = DelegateFor(permission.type);
    if (!delegate || !Differs(web_contents(), permission)) {
      continue;
    }
    names.push_back(base::UTF16ToUTF8(PageInfoUI::PermissionTypeToUIString(permission.type)));
    states.push_back(base::UTF16ToUTF8(
        PageInfoUI::PermissionStateToUIString(ui_delegate_.get(), permission)));
    permissions.push_back({
        .type = static_cast<int>(permission.type),
        .value = ValueOf(*delegate, permission.setting.value_or(permission.default_setting)),
        .choices = ChoicesFor(*delegate, *ui_delegate_, permission),
        .key = KeyOf(permission.type),
    });
    resettable |= Resettable(*ui_delegate_, permission);
  }
  for (size_t i = 0; i < permissions.size(); ++i) {
    permissions[i].name = names[i].c_str();
    permissions[i].state = states[i].c_str();
  }
  std::vector<std::string> device_names;
  std::vector<std::string> device_kinds;
  std::vector<bc_site_device> devices;
  for (const std::unique_ptr<PageInfoUI::ChosenObjectInfo>& device : devices_) {
    permissions::ObjectPermissionContextBase* context =
        page_info_ ? page_info_->GetChooserContextFromUIInfo(*device->ui_info) : nullptr;
    device_names.push_back(context ? base::UTF16ToUTF8(context->GetObjectDisplayNameForUI(
                                         device->chooser_object->value))
                                   : std::string());
    device_kinds.push_back(
        base::UTF16ToUTF8(l10n_util::GetStringUTF16(device->ui_info->description_string_id)));
    devices.push_back({
        .type = DeviceTypeOf(device->ui_info->content_settings_type),
        .removable = device->chooser_object->source != content_settings::SettingSource::kPolicy,
    });
  }
  for (size_t i = 0; i < devices.size(); ++i) {
    devices[i].name = device_names[i].c_str();
    devices[i].kind = device_kinds[i].c_str();
  }
  const std::string connection = base::UTF16ToUTF8(connection_summary_);
  std::string ad_blocked;
  if (ad_blocking == BC_AD_BLOCKING_ON) {
    ad_blocked = !NativeAdBlock()      ? AdBlockedOn(web_contents())
                 : native_blocked_ > 0 ? base::NumberToString(native_blocked_)
                                       : std::string();
  }
  // Chrome counts every row it lists in its button's words; these are the lines the panel shows.
  const std::string reset =
      resettable ? base::UTF16ToUTF8(l10n_util::GetPluralStringFUTF16(
                       IDS_PAGE_INFO_RESET_PERMISSIONS, static_cast<int>(permissions.size())))
                 : std::string();
  const bc_site_controls controls = {
      .site = site_.c_str(),
      .connection = connection_,
      .connection_summary = connection.c_str(),
      .permissions = permissions.data(),
      .count = permissions.size(),
      .reset = reset.c_str(),
      .picture_in_picture = PictureInPictureOf(web_contents()),
      .certificate = CertificateOf(web_contents()) != nullptr,
      .system_engine = system_engine_,
      .system_engine_offered = system_engine_offered_,
      .logins = c_logins.data(),
      .login_count = c_logins.size(),
      .ad_blocking = ad_blocking,
      .ad_blocked = ad_blocked.c_str(),
      .developer_mode = DeveloperModeOf(web_contents()),
      .devices = devices.data(),
      .device_count = devices.size(),
  };
  Host().tab_site_controls(Host().context, tab_, &controls);
}

}  // namespace browser_core
