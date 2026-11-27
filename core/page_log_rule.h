#ifndef BROWSER_CORE_PAGE_LOG_RULE_H_
#define BROWSER_CORE_PAGE_LOG_RULE_H_

#include <stddef.h>
#include <stdint.h>

#include <deque>
#include <map>
#include <string>
#include <string_view>

#include "url/gurl.h"

namespace browser_core {

// The page log's Space hook (page_log.h), called after every Space change as BoostSpacesChanged
// is, so space.cc needs nothing of the page log's and space_unittest.cc gives it a stub.
void PageLogsSpacesChanged();

namespace page_log {

// What an agent's page_log answers for a tab it owns (#4377): the page's failed requests, and only
// in Developer Mode, which the user turns on for a site, its console and those requests whole.
// What it keeps is made to be answered: nothing here is ever more than the answer may hold.

// Whether a header named |name| may be in an answer. Never Cookie, Authorization,
// Proxy-Authorization or Set-Cookie, in any case, even in Developer Mode: an agent gets a page's
// sessions, never its secrets.
bool Kept(std::string_view name);

// |url| as an answer outside Developer Mode gives it: scheme, host, port and path, without the
// query, the fragment, or a user name or password.
std::string Trimmed(const GURL& url);

// Whether a request failed: it ended in a network error, its blocking ones included, or with a
// status of 400 or more. A redirect isn't a failure.
bool Failed(int net_error, int status);

// How much a page's log keeps, oldest dropped first.
inline constexpr size_t kMostRequests = 200;
inline constexpr size_t kMostMessages = 500;
inline constexpr size_t kMostMessageBytes = 8 * 1024;
inline constexpr size_t kMostBodyBytes = 64 * 1024;
inline constexpr size_t kMostBytes = 1024 * 1024;

using Headers = std::map<std::string, std::string>;

struct Request {
  std::string method;
  GURL url;
  // The HTTP status, 0 if none came.
  int status = 0;
  // Chromium's network error, 0 for none, and its name, such as net::ERR_BLOCKED_BY_CLIENT.
  int net_error = 0;
  std::string error;
};

struct Message {
  // verbose, info, warning or error.
  std::string level;
  std::string text;
  std::string source;
  int line = 0;
};

// A failed request whole, as Developer Mode's DevTools session heard it.
struct Detail {
  GURL url;
  int status = 0;
  Headers request_headers;
  Headers response_headers;
  std::string body;
  bool body_base64 = false;
};

// One tab's log, from its page's last cross-document navigation.
class Log {
 public:
  Log();
  ~Log();
  Log(const Log&) = delete;
  Log& operator=(const Log&) = delete;

  // A new page: what was kept goes, and the page is in Developer Mode or not.
  void NewPage(bool developer_mode);
  // The user turned Developer Mode on or off for the page's site. Off, the console and the
  // requests' details go, as an answer from then on may not hold them.
  void SetDeveloperMode(bool on);
  bool developer_mode() const { return developer_mode_; }
  // Whether the page wants the DevTools session that hears requests whole: only in Developer Mode.
  bool WantsSession() const { return developer_mode_; }

  // Keeps |request| if it Failed.
  void Add(Request request);
  // Keeps |message| in Developer Mode only.
  void Add(Message message);
  // Keeps a failed request's details under DevTools' |id| in Developer Mode only, without the
  // headers Kept refuses.
  void Add(const std::string& id, Detail detail);
  // The body DevTools gave for the request |id| kept, if it's still kept.
  void Body(const std::string& id, std::string body, bool base64);
  bool HasDetail(const std::string& id) const;

  // The answer, as UTF-8 JSON: {"developer_mode", "requests", and in Developer Mode "console",
  // "dropped"}. A request in Developer Mode carries the details kept for its address and status.
  std::string Json() const;

 private:
  void Fit();
  static size_t SizeOf(const Request& request);
  static size_t SizeOf(const Message& message);
  static size_t SizeOf(const Detail& detail);

  bool developer_mode_ = false;
  std::deque<Request> requests_;
  std::deque<Message> console_;
  // In the order heard, by DevTools' request id.
  std::deque<std::pair<std::string, Detail>> details_;
  size_t bytes_ = 0;
  size_t dropped_requests_ = 0;
  size_t dropped_messages_ = 0;
};

}  // namespace page_log

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_LOG_RULE_H_
