#include "browser_core/cast_dialog.h"

#include <algorithm>
#include <optional>
#include <string>
#include <utility>

#include "base/functional/bind.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "chrome/browser/media/router/media_router_feature.h"
#include "chrome/browser/ui/media_router/cast_dialog_model.h"
#include "chrome/browser/ui/media_router/media_cast_mode.h"
#include "chrome/browser/ui/media_router/media_router_ui.h"
#include "components/media_router/browser/media_router_metrics.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

using media_router::MediaCastMode;
using media_router::UIMediaSinkState;

CastDialog*& Shown() {
  static CastDialog* shown = nullptr;
  return shown;
}

int StateOf(UIMediaSinkState state) {
  switch (state) {
    case UIMediaSinkState::AVAILABLE:
      return BC_CAST_AVAILABLE;
    case UIMediaSinkState::CONNECTING:
      return BC_CAST_CONNECTING;
    case UIMediaSinkState::CONNECTED:
      return BC_CAST_CONNECTED;
    case UIMediaSinkState::DISCONNECTING:
      return BC_CAST_DISCONNECTING;
    case UIMediaSinkState::UNAVAILABLE:
      return BC_CAST_UNAVAILABLE;
  }
  return BC_CAST_UNAVAILABLE;
}

// Chrome's dialog's choice for casting a tab: the page's presentation where it has one, which a
// site such as YouTube plays on the device itself, else the tab mirrored.
std::optional<MediaCastMode> ModeFor(const media_router::UIMediaSink& sink) {
  for (MediaCastMode mode : {MediaCastMode::PRESENTATION, MediaCastMode::TAB_MIRROR}) {
    if (sink.cast_modes.contains(mode)) {
      return mode;
    }
  }
  return std::nullopt;
}

}  // namespace

// static
media_router::MediaRouterDialogController* CastDialog::For(content::WebContents* contents) {
  CreateForWebContents(contents);
  return FromWebContents(contents);
}

// static
void CastDialog::Open(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents || !media_router::MediaRouterEnabled(contents->GetBrowserContext())) {
    return;
  }
  // Chrome's, through the factory For is, as WebContentsUserData has one of its own.
  MediaRouterDialogController::GetOrCreateForWebContents(contents)->ShowMediaRouterDialog(
      media_router::MediaRouterDialogActivationLocation::APP_MENU);
}

// static
void CastDialog::Close() {
  if (Shown()) {
    Shown()->HideMediaRouterDialog();
  }
}

// static
void CastDialog::To(bc_id tab, const char* id) {
  CastDialog* dialog = Shown();
  if (!dialog || !dialog->ui_ || !id || Tab::IdOf(dialog->initiator()) != tab) {
    return;
  }
  auto found = std::ranges::find(dialog->sinks_, std::string(id), &media_router::UIMediaSink::id);
  if (found == dialog->sinks_.end()) {
    return;
  }
  // A copy, as the calls below can update the model, and the sinks with it.
  const media_router::UIMediaSink sink = *found;
  if (sink.issue) {
    dialog->ui_->ClearIssue(sink.issue->id());
  } else if (sink.route) {
    dialog->ui_->StopCasting(sink.route->media_route_id());
  } else if (std::optional<MediaCastMode> mode = ModeFor(sink);
             mode && sink.state == UIMediaSinkState::AVAILABLE) {
    dialog->ui_->StartCasting(sink.id, *mode);
  }
}

CastDialog::CastDialog(content::WebContents* contents)
    : MediaRouterDialogController(contents),
      content::WebContentsUserData<CastDialog>(*contents) {}

CastDialog::~CastDialog() {
  if (Shown() == this) {
    Shown() = nullptr;
  }
}

bool CastDialog::IsShowingMediaRouterDialog() const {
  return !!ui_;
}

void CastDialog::CreateMediaRouterDialog(media_router::MediaRouterDialogActivationLocation) {
  if (Shown() && Shown() != this) {
    Shown()->HideMediaRouterDialog();
  }
  Shown() = this;
  ui_ = start_presentation_context_
            ? media_router::MediaRouterUI::CreateWithStartPresentationContextAndMirroring(
                  initiator(), std::move(start_presentation_context_))
            : media_router::MediaRouterUI::CreateWithDefaultMediaSourceAndMirroring(initiator());
  // As Chrome's Views controller does: the UI asks for its own end, such as once the page's
  // presentation request is answered.
  ui_->RegisterDestructor(
      base::BindOnce(&CastDialog::HideMediaRouterDialog, base::Unretained(this)));
  // Reports the model at once.
  ui_->AddObserver(this);
  ReportSoon();
}

void CastDialog::CloseMediaRouterDialog() {
  if (!ui_) {
    return;
  }
  ui_->RemoveObserver(this);
  ui_.reset();
  header_.clear();
  sinks_.clear();
  if (Shown() == this) {
    Shown() = nullptr;
  }
  ReportSoon();
}

void CastDialog::OnModelUpdated(const media_router::CastDialogModel& model) {
  header_ = model.dialog_header();
  sinks_ = model.media_sinks();
  ReportSoon();
}

void CastDialog::OnCastingStarted() {
  // Chrome's dialog closes once casting starts. Posted: this comes inside StartCasting.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<CastDialog> dialog) {
                       if (dialog) {
                         dialog->HideMediaRouterDialog();
                       }
                     },
                     weak_factory_.GetWeakPtr()));
}

void CastDialog::ReportSoon() {
  if (reporting_) {
    return;
  }
  reporting_ = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&CastDialog::Report, weak_factory_.GetWeakPtr()));
}

void CastDialog::Report() {
  reporting_ = false;
  const bc_id tab = Tab::IdOf(initiator());
  if (!Host().tab_cast || !tab) {
    return;
  }
  if (!ui_) {
    Host().tab_cast(Host().context, tab, nullptr, nullptr, 0);
    return;
  }
  // The strings first, as a device's points into them.
  std::vector<std::string> names;
  std::vector<std::string> statuses;
  names.reserve(sinks_.size());
  statuses.reserve(sinks_.size());
  for (const media_router::UIMediaSink& sink : sinks_) {
    names.push_back(base::UTF16ToUTF8(sink.friendly_name));
    statuses.push_back(base::UTF16ToUTF8(sink.GetStatusTextForDisplay()));
  }
  std::vector<bc_cast_device> devices;
  devices.reserve(sinks_.size());
  for (size_t i = 0; i < sinks_.size(); ++i) {
    devices.push_back({
        .id = sinks_[i].id.c_str(),
        .name = names[i].c_str(),
        .state = StateOf(sinks_[i].state),
        .status = statuses[i].c_str(),
    });
  }
  const std::string header = base::UTF16ToUTF8(header_);
  Host().tab_cast(Host().context, tab, header.c_str(), devices.data(), devices.size());
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(CastDialog);

}  // namespace browser_core
