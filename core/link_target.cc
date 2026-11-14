#include "browser_core/link_target.h"

#include <algorithm>

#include "base/strings/escape.h"
#include "base/strings/utf_string_conversions.h"
#include "components/url_formatter/url_formatter.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "url/gurl.h"
#include "url/third_party/mozilla/url_parse.h"

namespace browser_core {

namespace {

// Where the last |labels| dot-separated labels of |host| begin, or 0 if it has fewer.
size_t LabelsFrom(const std::u16string& host, size_t labels) {
  size_t from = host.size();
  for (size_t i = 0; i < labels; ++i) {
    const size_t dot = from == 0 ? std::u16string::npos : host.rfind(u'.', from - 1);
    if (dot == std::u16string::npos) {
      return 0;
    }
    from = dot;
  }
  return from + 1;
}

}  // namespace

LinkTarget FormatLinkTarget(const GURL& url) {
  url::Parsed parsed;
  const std::u16string text = url_formatter::FormatUrl(
      url,
      url_formatter::kFormatUrlOmitHTTPS |
          url_formatter::kFormatUrlOmitTrailingSlashOnBareHostname,
      base::UnescapeRule::SPACES, &parsed, nullptr, nullptr);
  LinkTarget target;
  target.text = base::UTF16ToUTF8(text);
  if (!parsed.host.is_nonempty()) {
    return target;
  }
  // A trailing dot makes a host fully qualified; the registry's answer leaves it out.
  size_t host_end = parsed.host.begin + parsed.host.len;
  if (text[host_end - 1] == u'.') {
    --host_end;
  }
  const std::u16string host = text.substr(parsed.host.begin, host_end - parsed.host.begin);
  std::string domain = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  if (!domain.empty() && domain.back() == '.') {
    domain.pop_back();
  }
  // The domain is in the host's ASCII form and the text may show it in Unicode, but the labels
  // are the same in number.
  size_t begin = 0;
  if (!domain.empty()) {
    begin = LabelsFrom(host, std::ranges::count(domain, '.') + 1);
  }
  target.domain_begin =
      base::UTF16ToUTF8(text.substr(0, parsed.host.begin + begin)).size();
  target.domain_end = base::UTF16ToUTF8(text.substr(0, host_end)).size();
  return target;
}

std::string SiteOf(const GURL& url) {
  if (!url.SchemeIsHTTPOrHTTPS() || !url.has_host()) {
    return std::string();
  }
  std::string site = net::registry_controlled_domains::GetDomainAndRegistry(
      url, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  if (site.empty()) {
    site = url.host();
  }
  if (site.size() > 1 && site.back() == '.') {
    site.pop_back();
  }
  return site;
}

}  // namespace browser_core
