#include "browser_core/agent_file_field_rule.h"

#include <algorithm>
#include <cctype>

namespace browser_core {

namespace {

std::string Lower(std::string text) {
  std::ranges::transform(text, text.begin(),
                         [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
  return text;
}

std::string Trimmed(const std::string& text) {
  const size_t start = text.find_first_not_of(" \t\n\r\f");
  if (start == std::string::npos) {
    return std::string();
  }
  return text.substr(start, text.find_last_not_of(" \t\n\r\f") - start + 1);
}

// The value of |field|'s attribute |name|, which HTML names in any case.
std::optional<std::string> Attribute(const FileFieldFacts& field, const std::string& name) {
  for (size_t i = 0; i + 1 < field.attributes.size(); i += 2) {
    if (Lower(field.attributes[i]) == name) {
      return field.attributes[i + 1];
    }
  }
  return std::nullopt;
}

bool IsFileInput(const FileFieldFacts& field) {
  const std::optional<std::string> type = Attribute(field, "type");
  return Lower(field.name) == "input" && type && Lower(Trimmed(*type)) == "file";
}

}  // namespace

bool PageMayOpenFileDialog(bool agents, bool held) {
  return !agents || held;
}

std::optional<std::string> FileFieldRefusal(const FileFieldFacts& field) {
  if (!IsFileInput(field)) {
    return "That element isn't a file field. Call upload on an input type=file (listed in "
           "snapshot as a button, usually “Choose File” or “Browse”).";
  }
  if (Attribute(field, "webkitdirectory")) {
    return "That field takes a folder. Hand the tab to the user with hand_off.";
  }
  return std::nullopt;
}

std::string DocumentFrame(const std::vector<DocumentChild>& children) {
  // DOM's Node.ELEMENT_NODE, of which a document has one at most.
  constexpr int kElement = 1;
  const auto element = std::ranges::find(children, kElement, &DocumentChild::type);
  if (element == children.end() || element->holds_document) {
    return std::string();
  }
  // HTMLFrameOwnerElement's kinds, named in any case, as an XML document keeps its own, and by
  // local name too, as one there can be named with a prefix (h:iframe).
  const std::string name = Lower(element->name);
  const std::string local_name = Lower(element->local_name);
  for (const char* owner : {"iframe", "frame", "object", "embed", "fencedframe"}) {
    if (name == owner || local_name == owner) {
      return std::string();
    }
  }
  return element->frame;
}

std::optional<std::string> FileFieldPlaceRefusal(const FileFieldPlace& place) {
  // An opaque origin serializes as "null", which would equal another opaque one's.
  if (place.frame.empty() || place.origin.empty() || place.origin == "null") {
    return "That field is in a frame with no site of its own, so the browser can't say which "
           "site is asking. Hand the tab to the user with hand_off.";
  }
  return std::nullopt;
}

bool FileFieldTakesMany(const FileFieldFacts& field) {
  return Attribute(field, "multiple").has_value();
}

std::vector<std::string> FileFieldAccepts(const FileFieldFacts& field) {
  std::vector<std::string> types;
  const std::string accept = Attribute(field, "accept").value_or("");
  size_t start = 0;
  while (start <= accept.size()) {
    const size_t comma = std::min(accept.find(',', start), accept.size());
    const std::string type = Lower(Trimmed(accept.substr(start, comma - start)));
    start = comma + 1;
    const size_t slash = type.find('/');
    const bool extension = type.size() > 1 && type[0] == '.';
    const bool mime = slash != std::string::npos && slash > 0 && slash + 1 < type.size();
    if ((extension || mime) && type.find_first_of(" \t") == std::string::npos) {
      types.push_back(type);
    }
  }
  return types;
}

bool FilesLand(const FileFieldPlace& shown, const FileFieldNow& now, size_t count) {
  return count > 0 && now.same_document && now.agents && !now.held && now.in_page &&
         now.place.frame == shown.frame && now.place.origin == shown.origin &&
         !FileFieldPlaceRefusal(now.place) && !FileFieldRefusal(now.facts) &&
         (count == 1 || FileFieldTakesMany(now.facts));
}

bool MayPutFiles(const FileFieldPlace& shown, const FileFieldPutNow& now, size_t count) {
  return count > 0 && (count == 1 || now.many) && now.same_document &&
         now.origin == shown.origin && !FileFieldPlaceRefusal({shown.frame, now.origin}) &&
         now.agents && !now.held;
}

}  // namespace browser_core
