#include "browser_core/agent_permission_rule.h"

#include <initializer_list>

namespace browser_core::agent_permission_rule {

bool Refuses(Kind kind, const agent_space_rule::Tab& tab) {
  switch (kind) {
    case Kind::kLeftAsToday:
      return false;
    case Kind::kClipboardWrite:
      return agent_space_rule::Refuses(agent_space_rule::Ask::kCopy, tab);
    case Kind::kPrint:
    case Kind::kClientCertificate:
    case Kind::kCast:
    case Kind::kExtensionInstall:
    case Kind::kFederatedSignIn:
    case Kind::kShare:
    case Kind::kAppInstall:
      return tab.agents && !tab.held;
    default:
      return tab.agents;
  }
}

bool RefusesAsk(Kind kind, const agent_space_rule::Tab& tab, const Handed& handed) {
  const bool call = kind == Kind::kCamera || kind == Kind::kMicrophone;
  const bool users = handed.handed && handed.on_given_origin && tab.held && handed.users_press;
  return Refuses(kind, tab) && !(call && users);
}

const char* CodeWord(Kind kind) {
  switch (kind) {
    case Kind::kCamera:
      return "camera";
    case Kind::kCameraPanTiltZoom:
      return "camera_pan_tilt_zoom";
    case Kind::kMicrophone:
      return "microphone";
    case Kind::kLocation:
      return "location";
    case Kind::kClipboardRead:
      return "clipboard_read";
    case Kind::kClipboardWrite:
      return "clipboard_write";
    case Kind::kScreenCapture:
      return "screen_capture";
    case Kind::kNotifications:
      return "notifications";
    case Kind::kFilePicker:
      return "file_picker";
    case Kind::kMidi:
      return "midi";
    case Kind::kLocalFonts:
      return "local_fonts";
    case Kind::kWindowManagement:
      return "window_management";
    case Kind::kIdleDetection:
      return "idle_detection";
    case Kind::kSensors:
      return "sensors";
    case Kind::kLocalNetwork:
      return "local_network";
    case Kind::kCapturedSurfaceControl:
      return "captured_surface_control";
    case Kind::kSpeakerSelection:
      return "speaker_selection";
    case Kind::kKeyboardLock:
      return "keyboard_lock";
    case Kind::kPointerLock:
      return "pointer_lock";
    case Kind::kAutomaticFullscreen:
      return "automatic_fullscreen";
    case Kind::kVr:
      return "vr";
    case Kind::kAr:
      return "ar";
    case Kind::kHandTracking:
      return "hand_tracking";
    case Kind::kWebAppInstallation:
      return "web_app_installation";
    case Kind::kNfc:
      return "nfc";
    case Kind::kSmartCard:
      return "smart_card";
    case Kind::kWebPrinting:
      return "web_printing";
    case Kind::kFileHandle:
      return "file_handle";
    case Kind::kColorPicker:
      return "color_picker";
    case Kind::kPrint:
      return "print";
    case Kind::kClientCertificate:
      return "client_certificate";
    case Kind::kCast:
      return "cast";
    case Kind::kExtensionInstall:
      return "extension_install";
    case Kind::kFederatedSignIn:
      return "federated_sign_in";
    case Kind::kShare:
      return "share";
    case Kind::kAppInstall:
      return "app_install";
    case Kind::kOther:
    case Kind::kLeftAsToday:
      return "other";
  }
  return "other";
}

bool RefusesClientCertificate(bool any_to_offer, const agent_space_rule::Tab& tab) {
  return any_to_offer && Refuses(Kind::kClientCertificate, tab);
}

bool RefusesPagePopup(PagePopup popup, const agent_space_rule::Tab& tab) {
  switch (popup) {
    case PagePopup::kSystemMenu:
    case PagePopup::kWindow:
      return tab.agents && !tab.held;
  }
  return false;
}

namespace {

char Lower(char c) {
  return c >= 'A' && c <= 'Z' ? static_cast<char>(c - 'A' + 'a') : c;
}

// An attribute's keyword, which compares in any case.
bool Is(std::string_view value, std::string_view keyword) {
  if (value.size() != keyword.size()) {
    return false;
  }
  for (size_t i = 0; i < value.size(); ++i) {
    if (Lower(value[i]) != keyword[i]) {
      return false;
    }
  }
  return true;
}

// Whether a select's size attribute asks for more lines than one, read as HTML reads a number
// that can't be negative: spaces, a plus sign, then digits, and whatever follows them ignored.
bool ManyLines(std::string_view size) {
  size_t at = 0;
  while (at < size.size() && (size[at] == ' ' || size[at] == '\t' || size[at] == '\n' ||
                              size[at] == '\f' || size[at] == '\r')) {
    ++at;
  }
  if (at < size.size() && size[at] == '+') {
    ++at;
  }
  bool many = false;
  bool digits = false;
  for (; at < size.size() && size[at] >= '0' && size[at] <= '9'; ++at) {
    // Any digit after one that isn't a leading zero, or a first one over 1.
    many = many || digits || size[at] > '1';
    digits = digits || size[at] != '0';
  }
  return many;
}

ShutField FieldOf(const Markup& element) {
  if (element.tag == "SELECT") {
    return element.multiple || ManyLines(element.size) ? ShutField::kNone : ShutField::kSelect;
  }
  if (element.tag != "INPUT") {
    return ShutField::kNone;
  }
  if (Is(element.type, "color")) {
    return ShutField::kColor;
  }
  for (std::string_view type : {"date", "datetime-local", "month", "week", "time"}) {
    if (Is(element.type, type)) {
      return ShutField::kDateOrTime;
    }
  }
  return ShutField::kNone;
}

}  // namespace

ShutField ShutFieldClicked(const Markup& element, const agent_space_rule::Tab& tab) {
  const ShutField field = FieldOf(element);
  switch (field) {
    case ShutField::kNone:
      return field;
    case ShutField::kSelect:
      // The Mac draws it as the system's menu. Where it is a window of the page's, both are
      // refused alike.
      return RefusesPagePopup(PagePopup::kSystemMenu, tab) ? field : ShutField::kNone;
    case ShutField::kDateOrTime:
    case ShutField::kColor:
      return RefusesPagePopup(PagePopup::kWindow, tab) ? field : ShutField::kNone;
  }
  return ShutField::kNone;
}

std::string_view ShutWord(ShutField field) {
  switch (field) {
    case ShutField::kNone:
      return "";
    case ShutField::kSelect:
      return "select";
    case ShutField::kDateOrTime:
      return "date_time";
    case ShutField::kColor:
      return "color";
  }
  return "";
}

DownloadAnswer AnswersDownloadQuestion(DownloadQuestion question,
                                       const agent_space_rule::Tab& tab) {
  if (!tab.agents || tab.held) {
    return DownloadAnswer::kLeft;
  }
  switch (question) {
    case DownloadQuestion::kUsersSaveAs:
      return DownloadAnswer::kLeft;
    case DownloadQuestion::kUsersSetting:
      return DownloadAnswer::kGoesOn;
    case DownloadQuestion::kPlaceUnusable:
      return DownloadAnswer::kCancelled;
  }
  return DownloadAnswer::kLeft;
}

bool RefusesFileHandle(bool site_storage, const agent_space_rule::Tab& tab) {
  return !site_storage && Refuses(Kind::kFileHandle, tab);
}

bool EntriesMade::First(Kind kind) {
  const size_t at = static_cast<size_t>(kind);
  const bool first = !made_.test(at);
  made_.set(at);
  return first;
}

Taken TakesRequest(bool link_handler_offer, const agent_space_rule::Tab& tab) {
  if (!tab.agents) {
    return Taken::kNo;
  }
  return link_handler_offer ? Taken::kDropped : Taken::kCancelled;
}

}  // namespace browser_core::agent_permission_rule
