#include "browser_core/sync_sign_in.h"

#include <string>

#include "base/json/string_escape.h"
#include "browser_core/public/browser_core.h"
#include "net/base/url_util.h"
#include "third_party/boringssl/src/include/openssl/mem.h"

namespace browser_core::sync_sign_in {

GURL AuthorizeUrl(const GURL& server,
                  const GURL& redirect,
                  std::string_view state,
                  std::string_view challenge,
                  Road road,
                  std::string_view login_hint) {
  GURL url = server.Resolve("/authorize");
  url = net::AppendQueryParameter(url, "response_type", "code");
  url = net::AppendQueryParameter(url, "client_id", kClientId);
  url = net::AppendQueryParameter(url, "redirect_uri", redirect.spec());
  url = net::AppendQueryParameter(url, "scope", "openid email offline_access");
  url = net::AppendQueryParameter(url, "state", std::string(state));
  url = net::AppendQueryParameter(url, "code_challenge", std::string(challenge));
  url = net::AppendQueryParameter(url, "code_challenge_method", "S256");
  if (road == Road::kGoogle) {
    url = net::AppendQueryParameter(url, "provider", "google");
  }
  if (road == Road::kPasskey) {
    url = net::AppendQueryParameter(url, "ilo_road", "passkey");
  }
  if (!login_hint.empty()) {
    url = net::AppendQueryParameter(url, "login_hint", std::string(login_hint));
  }
  return url;
}

bool OffersGoogle(const base::DictValue& discovery) {
  return discovery.FindBool("ilo_google").value_or(false);
}

bool SignsInNatively(const base::DictValue& discovery) {
  return discovery.FindBool("ilo_native_sign_in").value_or(false);
}

std::optional<std::string> CleanCode(std::string_view typed) {
  std::string code;
  // Never grown, so no copy is left behind in a freed buffer.
  code.reserve(6);
  for (char c : typed) {
    if (c == ' ') {
      continue;
    }
    if (c < '0' || c > '9' || code.size() == 6) {
      Wipe(code);
      return std::nullopt;
    }
    code.push_back(c);
  }
  if (code.size() != 6) {
    Wipe(code);
    return std::nullopt;
  }
  return code;
}

std::string StartBody(std::string_view challenge, std::string_view email) {
  return std::string("{\"client_id\":\"") + kClientId +
         "\",\"code_challenge_method\":\"S256\",\"code_challenge\":" +
         base::GetQuotedJSONString(challenge) + ",\"email\":" + base::GetQuotedJSONString(email) +
         "}";
}

std::string ResendBody(std::string_view sign_in) {
  return "{\"sign_in\":" + base::GetQuotedJSONString(sign_in) + "}";
}

std::string CodeBody(std::string_view sign_in,
                     std::string_view code,
                     std::string_view verifier,
                     std::string_view device_name) {
  // Built in one string, which the caller wipes: the code is digits, so it needs no escaping and
  // leaves no escaped copy behind.
  std::string body = "{\"sign_in\":" + base::GetQuotedJSONString(sign_in) + ",\"code\":\"";
  body.append(code);
  body += "\",\"code_verifier\":" + base::GetQuotedJSONString(verifier) +
          ",\"device_name\":" + base::GetQuotedJSONString(device_name) + "}";
  return body;
}

int Problem(std::string_view error) {
  if (error == "invalid_email") {
    return BC_SYNC_PROBLEM_NOT_AN_ADDRESS;
  }
  if (error == "undeliverable") {
    return BC_SYNC_PROBLEM_NO_MAIL;
  }
  if (error == "limit_address" || error == "limit_client" || error == "limit_server" ||
      error == "no_more_codes") {
    return BC_SYNC_PROBLEM_CODE_LIMIT;
  }
  if (error == "mail_failed") {
    return BC_SYNC_PROBLEM_MAIL_FAILED;
  }
  if (error == "wrong_code") {
    return BC_SYNC_PROBLEM_WRONG_CODE;
  }
  if (error == "too_many_tries") {
    return BC_SYNC_PROBLEM_TOO_MANY_TRIES;
  }
  if (error == "expired" || error == "used" || error == "sign_in_expired") {
    return BC_SYNC_PROBLEM_CODE_EXPIRED;
  }
  return BC_SYNC_PROBLEM_SERVER;
}

bool EndsSignIn(std::string_view error) {
  return error != "wrong_code" && error != "too_many_tries" && error != "expired" &&
         error != "used" && error != "limit_address" && error != "limit_client" &&
         error != "limit_server" && error != "no_more_codes" && error != "mail_failed";
}

void Wipe(std::string& secret) {
  OPENSSL_cleanse(secret.data(), secret.size());
  secret.clear();
}

bool GoogleRemoved(int status) {
  return status == 204 || status == 404;
}

}  // namespace browser_core::sync_sign_in
