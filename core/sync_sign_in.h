// Where the sign-in window goes, what a server's discovery says it offers, and what the app's own
// sign-in road (ilo-server's /native/sign-in) sends and answers, apart from the account (sync.cc),
// so browser_core_unittests tests them alone.

#ifndef BROWSER_CORE_SYNC_SIGN_IN_H_
#define BROWSER_CORE_SYNC_SIGN_IN_H_

#include <optional>
#include <string>
#include <string_view>

#include "base/values.h"
#include "url/gurl.h"

namespace browser_core::sync_sign_in {

// ilo-server's OIDC client id for the browser (protocol.md, Sign-in).
inline constexpr char kClientId[] = "ilo";

// Where the sign-in window starts on the server's page.
enum class Road {
  // The page's first step: the address, the code, a passkey or Google.
  kPage,
  // Straight on to Google's page (ilo-server's provider=google), or the page where the server has
  // no Google.
  kGoogle,
  // The page's passkey step alone (ilo-server's ilo_road=passkey), for an app that takes the
  // address and the code itself.
  kPasskey,
};

// |server|'s /authorize for the browser, back to |redirect| with |state|, committing to the PKCE
// verifier |challenge| is the hash of, on |road|. |login_hint|, if not empty, is the address the
// user typed in the app, which a server that can't take the code from the app fills in on its page
// (OpenID Connect's login_hint).
GURL AuthorizeUrl(const GURL& server,
                  const GURL& redirect,
                  std::string_view state,
                  std::string_view challenge,
                  Road road,
                  std::string_view login_hint = {});

// Whether a server's discovery (/.well-known/openid-configuration) says it signs in with Google:
// its ilo_google, true and nothing else.
bool OffersGoogle(const base::DictValue& discovery);

// Whether a server's discovery says the app may take the address and the code itself: its
// ilo_native_sign_in, true and nothing else. An older ilo-server or another OpenID provider says
// nothing, and its page takes them.
bool SignsInNatively(const base::DictValue& discovery);

// The code as typed, as the server takes it: six ASCII digits once spaces are left out, or none.
std::optional<std::string> CleanCode(std::string_view typed);

// The JSON bodies of /native/sign-in, /native/sign-in/resend and /native/sign-in/code.
std::string StartBody(std::string_view challenge, std::string_view email);
std::string ResendBody(std::string_view sign_in);
std::string CodeBody(std::string_view sign_in,
                     std::string_view code,
                     std::string_view verifier,
                     std::string_view device_name);

// The BC_SYNC_PROBLEM_ for the road's refusal |error|, an "error" of its answer: unknown ones, or
// none, are BC_SYNC_PROBLEM_SERVER.
int Problem(std::string_view error);

// Whether the refusal |error| to a code or a new code ends the sign-in, back to the address,
// rather than keeping the code step for another try.
bool EndsSignIn(std::string_view error);

// Overwrites |secret| before it's freed, so the code or a verifier doesn't stay in memory after
// its use, then empties it.
void Wipe(std::string& secret);

// Whether the server's |status| for DELETE /api/identities/google means Google no longer signs in
// to the account: removed, or removed already, by another device. Any other answer, or none (0),
// leaves it.
bool GoogleRemoved(int status);

}  // namespace browser_core::sync_sign_in

#endif  // BROWSER_CORE_SYNC_SIGN_IN_H_
