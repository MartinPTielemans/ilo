#include "browser_core/agent_permissions.h"

#include <string>
#include <utility>

#include "base/functional/bind.h"
#include "base/location.h"
#include "base/logging.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/agent_space_page.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "chrome/browser/printing/print_preview_dialog_controller.h"
#include "chrome/browser/ui/webui/print_preview/print_preview_ui.h"
#include "components/permissions/permission_request.h"
#include "components/permissions/request_type.h"
#include "components/web_modal/web_contents_modal_dialog_manager.h"
#include "content/public/browser/document_user_data.h"
#include "content/public/browser/media_stream_request.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_ui.h"
#include "content/public/browser/web_ui_controller.h"
#include "third_party/blink/public/common/mediastream/media_stream_request.h"
#include "third_party/blink/public/mojom/mediastream/media_stream.mojom-shared.h"

namespace browser_core {

namespace {

using agent_permission_rule::Kind;

// The core's name for Chrome's |type|. A kind Chrome adds is refused until it's named here.
Kind KindOf(ContentSettingsType type) {
  switch (type) {
    case ContentSettingsType::MEDIASTREAM_CAMERA:
      return Kind::kCamera;
    case ContentSettingsType::CAMERA_PAN_TILT_ZOOM:
      return Kind::kCameraPanTiltZoom;
    case ContentSettingsType::MEDIASTREAM_MIC:
      return Kind::kMicrophone;
    case ContentSettingsType::GEOLOCATION:
    case ContentSettingsType::GEOLOCATION_WITH_OPTIONS:
      return Kind::kLocation;
    case ContentSettingsType::CLIPBOARD_READ_WRITE:
      return Kind::kClipboardRead;
    case ContentSettingsType::CLIPBOARD_SANITIZED_WRITE:
      return Kind::kClipboardWrite;
    case ContentSettingsType::DISPLAY_CAPTURE:
    case ContentSettingsType::DISPLAY_MEDIA_SYSTEM_AUDIO:
      return Kind::kScreenCapture;
    case ContentSettingsType::NOTIFICATIONS:
      return Kind::kNotifications;
    case ContentSettingsType::MIDI:
    case ContentSettingsType::MIDI_SYSEX:
      return Kind::kMidi;
    case ContentSettingsType::LOCAL_FONTS:
      return Kind::kLocalFonts;
    case ContentSettingsType::WINDOW_MANAGEMENT:
      return Kind::kWindowManagement;
    case ContentSettingsType::IDLE_DETECTION:
      return Kind::kIdleDetection;
    case ContentSettingsType::SENSORS:
      return Kind::kSensors;
    case ContentSettingsType::LOCAL_NETWORK_ACCESS:
    case ContentSettingsType::LOCAL_NETWORK:
    case ContentSettingsType::LOOPBACK_NETWORK:
      return Kind::kLocalNetwork;
    case ContentSettingsType::CAPTURED_SURFACE_CONTROL:
      return Kind::kCapturedSurfaceControl;
    case ContentSettingsType::SPEAKER_SELECTION:
      return Kind::kSpeakerSelection;
    case ContentSettingsType::KEYBOARD_LOCK:
      return Kind::kKeyboardLock;
    case ContentSettingsType::POINTER_LOCK:
      return Kind::kPointerLock;
    case ContentSettingsType::AUTOMATIC_FULLSCREEN:
      return Kind::kAutomaticFullscreen;
    case ContentSettingsType::VR:
      return Kind::kVr;
    case ContentSettingsType::AR:
      return Kind::kAr;
    case ContentSettingsType::HAND_TRACKING:
      return Kind::kHandTracking;
    case ContentSettingsType::WEB_APP_INSTALLATION:
      return Kind::kWebAppInstallation;
    case ContentSettingsType::NFC:
      return Kind::kNfc;
    case ContentSettingsType::SMART_CARD_GUARD:
    case ContentSettingsType::SMART_CARD_DATA:
      return Kind::kSmartCard;
    case ContentSettingsType::WEB_PRINTING:
      return Kind::kWebPrinting;
    case ContentSettingsType::WAKE_LOCK_SCREEN:
    case ContentSettingsType::WAKE_LOCK_SYSTEM:
    case ContentSettingsType::PROTECTED_MEDIA_IDENTIFIER:
    case ContentSettingsType::STORAGE_ACCESS:
    case ContentSettingsType::TOP_LEVEL_STORAGE_ACCESS:
    case ContentSettingsType::PERSISTENT_STORAGE:
    case ContentSettingsType::BACKGROUND_SYNC:
    case ContentSettingsType::PERIODIC_BACKGROUND_SYNC:
    case ContentSettingsType::BACKGROUND_FETCH:
    case ContentSettingsType::PAYMENT_HANDLER:
      return Kind::kLeftAsToday;
    default:
      return Kind::kOther;
  }
}

// The kinds this document's refusals have an entry for.
class Entries : public content::DocumentUserData<Entries> {
 public:
  ~Entries() override = default;

  agent_permission_rule::EntriesMade made;

 private:
  explicit Entries(content::RenderFrameHost* frame) : DocumentUserData(frame) {}

  friend DocumentUserData;
  DOCUMENT_USER_DATA_KEY_DECL();
};

DOCUMENT_USER_DATA_KEY_IMPL(Entries);

void Enter(std::string agent, bc_id tab, std::string action) {
  if (!Host().agent_activity) {
    return;
  }
  const bc_agent_activity entry = {
      .agent = agent.c_str(),
      .tab = tab,
      .action = action.c_str(),
      .refused = agent_permission_rule::kEntryReason,
  };
  Host().agent_activity(Host().context, &entry);
}

}  // namespace

bool NoPermission(content::RenderFrameHost* frame, ContentSettingsType type) {
  return agent_permission_rule::Refuses(
      KindOf(type), RuleTabOf(content::WebContents::FromRenderFrameHost(frame)));
}

void PermissionRefused(content::RenderFrameHost* frame, ContentSettingsType type) {
  // Chrome answers so for a page its own actor works on too, which is no refusal of the core's.
  if (frame && NoPermission(frame, type)) {
    NoteRefused(KindOf(type), frame);
  }
}

bool TakesPermissionRequest(content::RenderFrameHost* frame,
                            permissions::PermissionRequest* request) {
  const bool offer = request->request_type() == permissions::RequestType::kRegisterProtocolHandler;
  switch (agent_permission_rule::TakesRequest(
      offer, RuleTabOf(content::WebContents::FromRenderFrameHost(frame)))) {
    case agent_permission_rule::Taken::kNo:
      return false;
    case agent_permission_rule::Taken::kDropped:
      NoteRefused(Kind::kOther, frame);
      return true;
    case agent_permission_rule::Taken::kCancelled: {
      // A kind left as today is answered as in any tab until it would ask the user, which no
      // page here does: its question ends as every other's.
      const Kind kind = KindOf(request->GetContentSettingsType());
      NoteRefused(kind == Kind::kLeftAsToday ? Kind::kOther : kind, frame);
      request->Cancelled();
      return true;
    }
  }
  return false;
}

bool NoMediaAccess(content::WebContents* contents, const content::MediaStreamRequest& request) {
  const agent_space_rule::Tab tab = RuleTabOf(contents);
  bool refused = false;
  // A page may ask for several at once, and each refused has its entry: the camera and the
  // microphone are answered here because Chrome reads their status before it would request them,
  // so a refusal by the status alone would be told to nobody.
  const auto ask = [&](bool wanted, Kind kind) {
    if (wanted && agent_permission_rule::Refuses(kind, tab)) {
      refused = true;
      NoteRefused(kind, contents);
    }
  };
  ask(blink::IsScreenCaptureMediaType(request.video_type) ||
          blink::IsScreenCaptureMediaType(request.audio_type),
      Kind::kScreenCapture);
  ask(request.video_type == blink::mojom::MediaStreamType::DEVICE_VIDEO_CAPTURE, Kind::kCamera);
  ask(request.audio_type == blink::mojom::MediaStreamType::DEVICE_AUDIO_CAPTURE, Kind::kMicrophone);
  return refused;
}

bool NoClipboardWrite(content::RenderFrameHost* frame) {
  if (!agent_permission_rule::Refuses(
          Kind::kClipboardWrite, RuleTabOf(content::WebContents::FromRenderFrameHost(frame)))) {
    return false;
  }
  NoteRefused(Kind::kClipboardWrite, frame);
  return true;
}

bool NoFileHandle(content::GlobalRenderFrameHostId id, bool site_storage) {
  content::RenderFrameHost* frame = content::RenderFrameHost::FromID(id);
  if (!frame || !agent_permission_rule::RefusesFileHandle(
                    site_storage, RuleTabOf(content::WebContents::FromRenderFrameHost(frame)))) {
    return false;
  }
  NoteRefused(Kind::kFileHandle, frame);
  return true;
}

bool NoColorPicker(content::RenderFrameHost* frame) {
  if (!frame ||
      !agent_permission_rule::Refuses(
          Kind::kColorPicker, RuleTabOf(content::WebContents::FromRenderFrameHost(frame)))) {
    return false;
  }
  NoteRefused(Kind::kColorPicker, frame);
  return true;
}

bool NoPrint(content::RenderFrameHost* frame, bool by_users_command) {
  if (!frame || !agent_permission_rule::Refuses(
                    Kind::kPrint, RuleTabOf(content::WebContents::FromRenderFrameHost(frame)))) {
    return false;
  }
  if (!by_users_command) {
    NoteRefused(Kind::kPrint, frame);
  }
  return true;
}

void ClosePrintPreview(content::WebContents* contents) {
  if (!contents || !agent_permission_rule::Refuses(Kind::kPrint, RuleTabOf(contents))) {
    return;
  }
  auto* previews = printing::PrintPreviewDialogController::GetInstance();
  content::WebContents* preview =
      previews ? previews->GetPrintPreviewForContents(contents) : nullptr;
  content::WebUI* ui = preview ? preview->GetWebUI() : nullptr;
  if (!ui || !ui->GetController()) {
    return;
  }
  if (auto* print_preview = ui->GetController()->GetAs<printing::PrintPreviewUI>()) {
    print_preview->OnClosePrintPreviewDialog();
  }
}

bool NoClientCertificate(content::WebContents* contents, bool any_to_offer) {
  if (!contents ||
      !agent_permission_rule::RefusesClientCertificate(any_to_offer, RuleTabOf(contents))) {
    return false;
  }
  NoteRefused(Kind::kClientCertificate, contents->GetOutermostWebContents());
  return true;
}

void CloseTabDialogs(content::WebContents* contents) {
  if (!contents || !agent_permission_rule::Refuses(Kind::kClientCertificate, RuleTabOf(contents))) {
    return;
  }
  if (auto* dialogs = web_modal::WebContentsModalDialogManager::FromWebContents(contents)) {
    dialogs->CloseAllDialogs();
  }
}

void NoteRefused(agent_permission_rule::Kind kind, content::RenderFrameHost* frame) {
  content::WebContents* contents = content::WebContents::FromRenderFrameHost(frame);
  const Tab* tab = contents ? Tab::FromWebContents(contents->GetOutermostWebContents()) : nullptr;
  const Space* space = tab ? AgentSpaceOf(tab->id()) : nullptr;
  if (!space) {
    return;
  }
  // The page's document, whichever frame of it asks.
  Entries* entries = Entries::GetOrCreateForCurrentDocument(frame->GetOutermostMainFrame());
  if (!entries->made.First(kind)) {
    return;
  }
  LOG(ERROR) << "browser: permission " << tab->id() << " " << agent_permission_rule::CodeWord(kind)
             << " refused: an agent's tab";
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Enter, space->agent()->id, tab->id(),
                                std::string(agent_permission_rule::kEntryAction) +
                                    agent_permission_rule::CodeWord(kind)));
}

void NoteRefused(agent_permission_rule::Kind kind, content::WebContents* contents) {
  if (contents) {
    NoteRefused(kind, contents->GetPrimaryMainFrame());
  }
}

}  // namespace browser_core
