#include "browser_core/page_log_rule.h"

#include <utility>

#include "base/json/json_writer.h"
#include "base/strings/string_util.h"
#include "base/values.h"

namespace browser_core::page_log {

bool Kept(std::string_view name) {
  for (std::string_view secret : {"cookie", "authorization", "proxy-authorization", "set-cookie"}) {
    if (base::EqualsCaseInsensitiveASCII(name, secret)) {
      return false;
    }
  }
  return true;
}

std::string Trimmed(const GURL& url) {
  if (!url.is_valid()) {
    return std::string();
  }
  GURL::Replacements without;
  without.ClearUsername();
  without.ClearPassword();
  without.ClearQuery();
  without.ClearRef();
  return url.ReplaceComponents(without).spec();
}

bool Failed(int net_error, int status) {
  return net_error != 0 || status >= 400;
}

Log::Log() = default;
Log::~Log() = default;

void Log::NewPage(bool developer_mode) {
  developer_mode_ = developer_mode;
  requests_.clear();
  console_.clear();
  details_.clear();
  bytes_ = 0;
  dropped_requests_ = 0;
  dropped_messages_ = 0;
}

void Log::SetDeveloperMode(bool on) {
  developer_mode_ = on;
  if (!on) {
    console_.clear();
    details_.clear();
    dropped_messages_ = 0;
    bytes_ = 0;
    for (const Request& request : requests_) {
      bytes_ += SizeOf(request);
    }
  }
}

void Log::Add(Request request) {
  if (!Failed(request.net_error, request.status)) {
    return;
  }
  bytes_ += SizeOf(request);
  requests_.push_back(std::move(request));
  if (requests_.size() > kMostRequests) {
    bytes_ -= SizeOf(requests_.front());
    requests_.pop_front();
    ++dropped_requests_;
  }
  Fit();
}

void Log::Add(Message message) {
  if (!developer_mode_) {
    return;
  }
  if (message.text.size() > kMostMessageBytes) {
    base::TruncateUTF8ToByteSize(message.text, kMostMessageBytes, &message.text);
  }
  bytes_ += SizeOf(message);
  console_.push_back(std::move(message));
  if (console_.size() > kMostMessages) {
    bytes_ -= SizeOf(console_.front());
    console_.pop_front();
    ++dropped_messages_;
  }
  Fit();
}

void Log::Add(const std::string& id, Detail detail) {
  if (!developer_mode_ || !Failed(0, detail.status)) {
    return;
  }
  for (Headers* headers : {&detail.request_headers, &detail.response_headers}) {
    std::erase_if(*headers, [](const auto& header) { return !Kept(header.first); });
  }
  bytes_ += SizeOf(detail);
  details_.emplace_back(id, std::move(detail));
  if (details_.size() > kMostRequests) {
    bytes_ -= SizeOf(details_.front().second);
    details_.pop_front();
  }
  Fit();
}

void Log::Body(const std::string& id, std::string body, bool base64) {
  for (auto& [kept_id, detail] : details_) {
    if (kept_id != id) {
      continue;
    }
    if (body.size() > kMostBodyBytes) {
      // A base64 body is ASCII, so a cut anywhere is a whole character.
      base::TruncateUTF8ToByteSize(body, kMostBodyBytes, &body);
    }
    bytes_ -= SizeOf(detail);
    detail.body = std::move(body);
    detail.body_base64 = base64;
    bytes_ += SizeOf(detail);
    Fit();
    return;
  }
}

bool Log::HasDetail(const std::string& id) const {
  for (const auto& [kept_id, detail] : details_) {
    if (kept_id == id) {
      return true;
    }
  }
  return false;
}

void Log::Fit() {
  // Bodies are the most of it, then the console; the failed requests themselves go last.
  while (bytes_ > kMostBytes && !details_.empty()) {
    bytes_ -= SizeOf(details_.front().second);
    details_.pop_front();
  }
  while (bytes_ > kMostBytes && !console_.empty()) {
    bytes_ -= SizeOf(console_.front());
    console_.pop_front();
    ++dropped_messages_;
  }
  while (bytes_ > kMostBytes && !requests_.empty()) {
    bytes_ -= SizeOf(requests_.front());
    requests_.pop_front();
    ++dropped_requests_;
  }
}

size_t Log::SizeOf(const Request& request) {
  return request.method.size() + request.url.spec().size() + request.error.size();
}

size_t Log::SizeOf(const Message& message) {
  return message.level.size() + message.text.size() + message.source.size();
}

size_t Log::SizeOf(const Detail& detail) {
  size_t size = detail.url.spec().size() + detail.body.size();
  for (const Headers* headers : {&detail.request_headers, &detail.response_headers}) {
    for (const auto& [name, value] : *headers) {
      size += name.size() + value.size();
    }
  }
  return size;
}

namespace {

base::DictValue HeadersValue(const Headers& headers) {
  base::DictValue value;
  for (const auto& [name, kept] : headers) {
    // Kept again where the answer is made, so no way in can put a secret in it.
    if (Kept(name)) {
      value.Set(name, kept);
    }
  }
  return value;
}

}  // namespace

std::string Log::Json() const {
  base::ListValue requests;
  for (const Request& request : requests_) {
    base::DictValue entry;
    entry.Set("method", request.method);
    entry.Set("url", developer_mode_ ? request.url.spec() : Trimmed(request.url));
    if (request.status) {
      entry.Set("status", request.status);
    }
    if (!request.error.empty()) {
      entry.Set("error", request.error);
    }
    if (developer_mode_) {
      // The newest details heard for the same address and status.
      for (auto it = details_.rbegin(); it != details_.rend(); ++it) {
        const Detail& detail = it->second;
        if (detail.url == request.url && detail.status == request.status) {
          entry.Set("request_headers", HeadersValue(detail.request_headers));
          entry.Set("response_headers", HeadersValue(detail.response_headers));
          if (!detail.body.empty()) {
            entry.Set(detail.body_base64 ? "body_base64" : "body", detail.body);
          }
          break;
        }
      }
    }
    requests.Append(std::move(entry));
  }
  base::DictValue answer;
  answer.Set("developer_mode", developer_mode_);
  answer.Set("requests", std::move(requests));
  if (developer_mode_) {
    base::ListValue console;
    for (const Message& message : console_) {
      console.Append(base::DictValue()
                         .Set("level", message.level)
                         .Set("text", message.text)
                         .Set("source", message.source)
                         .Set("line", message.line));
    }
    answer.Set("console", std::move(console));
  }
  if (dropped_requests_ || dropped_messages_) {
    base::DictValue dropped;
    dropped.Set("requests", static_cast<int>(dropped_requests_));
    if (developer_mode_) {
      dropped.Set("console", static_cast<int>(dropped_messages_));
    }
    answer.Set("dropped", std::move(dropped));
  }
  return base::WriteJson(answer).value_or(std::string());
}

}  // namespace browser_core::page_log
