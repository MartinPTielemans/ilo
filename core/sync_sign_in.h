// Where the sign-in window goes and what a server's discovery says it offers, apart from the
// account (sync.cc), so browser_core_unittests tests them alone.

#ifndef BROWSER_CORE_SYNC_SIGN_IN_H_
#define BROWSER_CORE_SYNC_SIGN_IN_H_

#include <string_view>

#include "base/values.h"
#include "url/gurl.h"

namespace browser_core::sync_sign_in {

// ilo-server's OIDC client id for the browser (protocol.md, Sign-in).
inline constexpr char kClientId[] = "ilo";

// |server|'s /authorize for the browser, back to |redirect| with |state|, committing to the PKCE
// verifier |challenge| is the hash of. With |google|, the server's page goes straight on to
// Google's (ilo-server's provider=google), and falls back to itself if it has no Google.
GURL AuthorizeUrl(const GURL& server,
                  const GURL& redirect,
                  std::string_view state,
                  std::string_view challenge,
                  bool google);

// Whether a server's discovery (/.well-known/openid-configuration) says it signs in with Google:
// its ilo_google, true and nothing else.
bool OffersGoogle(const base::DictValue& discovery);

}  // namespace browser_core::sync_sign_in

#endif  // BROWSER_CORE_SYNC_SIGN_IN_H_
