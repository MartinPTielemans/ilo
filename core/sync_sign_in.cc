#include "browser_core/sync_sign_in.h"

#include <string>

#include "net/base/url_util.h"

namespace browser_core::sync_sign_in {

GURL AuthorizeUrl(const GURL& server,
                  const GURL& redirect,
                  std::string_view state,
                  std::string_view challenge,
                  bool google) {
  GURL url = server.Resolve("/authorize");
  url = net::AppendQueryParameter(url, "response_type", "code");
  url = net::AppendQueryParameter(url, "client_id", kClientId);
  url = net::AppendQueryParameter(url, "redirect_uri", redirect.spec());
  url = net::AppendQueryParameter(url, "scope", "openid email offline_access");
  url = net::AppendQueryParameter(url, "state", std::string(state));
  url = net::AppendQueryParameter(url, "code_challenge", std::string(challenge));
  url = net::AppendQueryParameter(url, "code_challenge_method", "S256");
  if (google) {
    url = net::AppendQueryParameter(url, "provider", "google");
  }
  return url;
}

bool OffersGoogle(const base::DictValue& discovery) {
  return discovery.FindBool("ilo_google").value_or(false);
}

bool GoogleRemoved(int status) {
  return status == 204 || status == 404;
}

}  // namespace browser_core::sync_sign_in
