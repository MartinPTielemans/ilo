#include "browser_core/agent_permission_rule.h"

namespace browser_core::agent_permission_rule {

bool Refuses(Kind kind, const agent_space_rule::Tab& tab) {
  switch (kind) {
    case Kind::kLeftAsToday:
      return false;
    case Kind::kClipboardWrite:
      return agent_space_rule::Refuses(agent_space_rule::Ask::kCopy, tab);
    default:
      return tab.agents;
  }
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
    case Kind::kOther:
    case Kind::kLeftAsToday:
      return "other";
  }
  return "other";
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
