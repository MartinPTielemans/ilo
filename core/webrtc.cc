#include "browser_core/webrtc.h"

#include <memory>

#include "base/memory/raw_ptr.h"
#include "base/no_destructor.h"
#include "base/values.h"
#include "browser_core/session.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/net/proxy_service_factory.h"
#include "chrome/browser/policy/profile_policy_connector.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "chrome/common/pref_names.h"
#include "components/enterprise/buildflags/buildflags.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/prefs/pref_service.h"
#include "components/proxy_config/pref_proxy_config_tracker.h"
#include "net/proxy_resolution/proxy_config.h"
#include "net/proxy_resolution/proxy_config_service.h"
#include "net/proxy_resolution/proxy_config_with_annotation.h"
#include "third_party/blink/public/common/peerconnection/webrtc_ip_handling_policy.h"

#if BUILDFLAG(ENTERPRISE_PROXY)
#include "chrome/browser/enterprise/net/enterprise_proxy_service_factory.h"
#endif

namespace browser_core {

namespace {

bool g_settings_read = false;

// The proxy the profile's network uses, from its prefs, the command line, policies and the
// system's settings, as Chrome's own ProxyConfigMonitor follows it for the network service.
class ProxyFollower : public KeyedService, public net::ProxyConfigService::Observer {
 public:
  explicit ProxyFollower(Profile* profile) : prefs_(profile->GetPrefs()) {
    enterprise_net::EnterpriseProxyService* enterprise = nullptr;
#if BUILDFLAG(ENTERPRISE_PROXY)
    enterprise = EnterpriseProxyServiceFactory::GetForProfile(profile);
#endif
    tracker_ = ProxyServiceFactory::CreatePrefProxyConfigTrackerOfProfile(
        prefs_, g_browser_process->local_state(),
        profile->GetProfilePolicyConnector()->policy_service(), enterprise);
    service_ = ProxyServiceFactory::CreateProxyConfigService(tracker_.get(), profile);
    service_->AddObserver(this);
    net::ProxyConfigWithAnnotation config;
    const net::ProxyConfigService::ConfigAvailability availability =
        service_->GetLatestProxyConfig(&config);
    if (availability != net::ProxyConfigService::CONFIG_PENDING) {
      OnProxyConfigChanged(config, availability);
    }
  }

  void Apply() {
    const bool relay = !g_settings_read || !known_ || proxied_ || UserSettings().relay_calls;
    prefs_->SetDefaultPrefValue(prefs::kWebRTCIPHandlingPolicy,
                                base::Value(relay ? blink::kWebRTCIPHandlingDisableNonProxiedUdp
                                                  : blink::kWebRTCIPHandlingDefaultPublicInterfaceOnly));
  }

  void Shutdown() override {
    service_->RemoveObserver(this);
    tracker_->DetachFromPrefService();
  }

 private:
  void OnProxyConfigChanged(const net::ProxyConfigWithAnnotation& config,
                            net::ProxyConfigService::ConfigAvailability availability) override {
    const net::ProxyConfig& proxy = config.value();
    known_ = true;
    // Auto-detection alone doesn't count: it's on by default in Windows' settings, where it
    // mostly finds no proxy, and would relay nearly every call there.
    proxied_ = availability == net::ProxyConfigService::CONFIG_VALID &&
               (!proxy.proxy_rules().empty() || proxy.has_pac_url());
    Apply();
  }

  const raw_ptr<PrefService> prefs_;
  // In ProxyConfigMonitor's order, the tracker going first.
  std::unique_ptr<net::ProxyConfigService> service_;
  std::unique_ptr<PrefProxyConfigTracker> tracker_;
  bool known_ = false;
  bool proxied_ = false;
};

// A private window's profile shares its profile's defaults, so only the profile has one.
class FollowerFactory : public ProfileKeyedServiceFactory {
 public:
  static FollowerFactory* GetInstance() {
    static base::NoDestructor<FollowerFactory> instance;
    return instance.get();
  }

  static ProxyFollower* For(Profile* profile, bool create) {
    return static_cast<ProxyFollower*>(
        GetInstance()->GetServiceForBrowserContext(profile, create));
  }

  FollowerFactory() : ProfileKeyedServiceFactory("BrowserCoreProxyFollower") {
#if BUILDFLAG(ENTERPRISE_PROXY)
    DependsOn(EnterpriseProxyServiceFactory::GetInstance());
#endif
  }

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return std::make_unique<ProxyFollower>(Profile::FromBrowserContext(context));
  }
};

}  // namespace

void RegisterWebRtc() {
  FollowerFactory::GetInstance();
}

void FollowProxy(Profile* profile) {
  if (ProxyFollower* follower = FollowerFactory::For(profile, /*create=*/true)) {
    follower->Apply();
  }
}

void ApplyWebRtc() {
  g_settings_read = true;
  ProfileManager* manager = g_browser_process->profile_manager();
  if (!manager) {
    return;
  }
  for (Profile* profile : manager->GetLoadedProfiles()) {
    if (ProxyFollower* follower = FollowerFactory::For(profile, /*create=*/false)) {
      follower->Apply();
    }
  }
}

}  // namespace browser_core
