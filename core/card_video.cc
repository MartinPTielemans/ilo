#include "browser_core/card_video.h"

#include <algorithm>
#include <cmath>
#include <optional>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "base/task/sequenced_task_runner.h"
#include "browser_core/platform/window_views.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "cc/layers/deadline_policy.h"
#include "chrome/browser/media/webrtc/media_capture_devices_dispatcher.h"
#include "chrome/browser/media/webrtc/media_stream_capture_indicator.h"
#include "chrome/browser/picture_in_picture/picture_in_picture_window_manager.h"
#include "components/viz/common/surfaces/frame_sink_id.h"
#include "components/viz/common/surfaces/surface_id.h"
#include "content/public/browser/media_session.h"
#include "content/public/browser/overlay_window.h"
#include "content/public/browser/picture_in_picture_window_controller.h"
#include "content/public/browser/video_picture_in_picture_window_controller.h"
#include "content/public/browser/web_contents.h"
#include "services/media_session/public/mojom/media_session.mojom.h"
#include "third_party/skia/include/core/SkColor.h"
#include "ui/compositor/compositor.h"
#include "ui/compositor/layer.h"
#include "ui/compositor/layer_solid_color.h"
#include "ui/compositor/layer_surface.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rounded_corners_f.h"
#include "ui/gfx/geometry/size.h"
#include "ui/views/view.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_observer.h"

namespace browser_core {

namespace {

// What the app last asked to show: one video at a time, across windows.
struct Ask {
  bc_id tab;
  bc_id window;
  gfx::Rect frame;
  double corner_radius;
};

std::optional<Ask>& Asked() {
  static std::optional<Ask> asked;
  return asked;
}

// |natural|, as large as fits in |box|, in its middle: the black about it is the card's.
gfx::Rect Fitted(const gfx::Size& box, const gfx::Size& natural) {
  if (natural.IsEmpty() || box.IsEmpty()) {
    return gfx::Rect(box);
  }
  const double scale = std::min(static_cast<double>(box.width()) / natural.width(),
                                static_cast<double>(box.height()) / natural.height());
  const gfx::Size size(std::min(box.width(), static_cast<int>(std::lround(natural.width() * scale))),
                       std::min(box.height(),
                                static_cast<int>(std::lround(natural.height() * scale))));
  return gfx::Rect((box.width() - size.width()) / 2, (box.height() - size.height()) / 2,
                   size.width(), size.height());
}

// The tab whose video a card asked to go into Picture in Picture, until it does.
bc_id& Entering() {
  static bc_id entering = 0;
  return entering;
}

class VideoWindow;

// The window showing its video at a card, if one is.
VideoWindow*& InCard() {
  static VideoWindow* in_card = nullptr;
  return in_card;
}

class VideoWindow : public content::VideoOverlayWindow, public views::WidgetObserver {
 public:
  VideoWindow(content::VideoPictureInPictureWindowController* controller,
              ChromeVideoWindow chrome)
      : controller_(controller), make_chrome_(std::move(chrome)) {}

  ~VideoWindow() override {
    // Its controller is going, so the card goes unlogged.
    DropCard(/*log=*/false);
    if (InCard() == this) {
      InCard() = nullptr;
    }
  }

  // The tab whose video this is.
  bc_id tab() const {
    content::WebContents* contents = controller_->GetWebContents();
    return contents ? Tab::IdOf(contents) : 0;
  }

  // Puts the card's video where the app last asked, in that window.
  void Place() {
    const std::optional<Ask>& ask = Asked();
    Window* window = ask && ask->tab == tab() ? Window::FromId(ask->window) : nullptr;
    views::Widget* browser = window ? window->widget() : nullptr;
    if (!browser || !browser->IsVisible()) {
      HideCard();
      // Still its card's, so it shows again once its window does.
      if (ask && ask->tab == tab()) {
        InCard() = this;
      }
      return;
    }
    if (!card_ || browser_ != browser) {
      DropCard();
      MakeCard(browser);
    }
    gfx::Rect bounds = ask->frame;
    bounds.Offset(browser->GetClientAreaBoundsInScreen().OffsetFromOrigin());
    card_->SetBounds(bounds);
    card_->GetContentsView()->layer()->SetRoundedCornerRadius(
        gfx::RoundedCornersF(static_cast<float>(ask->corner_radius)));
    video_view_->SetBoundsRect(Fitted(bounds.size(), natural_size_));
    ShowSurfaceInCard();
    if (!in_card_) {
      card_->ShowInactive();
      in_card_ = true;
    }
    InCard() = this;
    // check.sh waits for this line.
    LOG(ERROR) << "browser: card video " << tab() << " in its card at " << bounds.ToString();
    controller_->UpdateLayerBounds();
  }

  // Out of Picture in Picture, playing on in its page.
  void End() { controller_->Close(/*should_pause_video=*/false); }

  // From the card into Chrome's window, as the card's float button asks.
  void Float() {
    DropCard();
    ShowInChrome();
  }

  // content::VideoOverlayWindow:
  bool IsActive() const override { return !InCardNow() && chrome_ && chrome_->IsActive(); }

  void Close() override {
    DropCard();
    if (chrome_) {
      chrome_->Close();
    }
  }

  void ShowInactive() override {
    const std::optional<Ask>& ask = Asked();
    const bool entering = Entering() == tab();
    if (entering) {
      Entering() = 0;
    }
    // Asked for, it shows at its card or nowhere: never floating unasked.
    if (ask && ask->tab == tab()) {
      if (chrome_) {
        chrome_->Hide();
      }
      // The card's even while its tab shows, so the app's empty frame ends it.
      InCard() = this;
      if (!Window::Showing(ask->tab)) {
        Place();
      }
      return;
    }
    // Its card went before it came, so it leaves at once.
    if (entering) {
      base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&VideoWindow::Leave, weak_factory_.GetWeakPtr()));
      return;
    }
    ShowInChrome();
  }

  void Hide() override {
    HideCard();
    if (chrome_) {
      chrome_->Hide();
    }
  }

  bool IsVisible() const override {
    return InCardNow() || (chrome_ && chrome_->IsVisible());
  }

  gfx::Rect GetBounds() override {
    if (InCardNow()) {
      return card_->GetWindowBoundsInScreen();
    }
    return chrome_ ? chrome_->GetBounds() : gfx::Rect();
  }

  void UpdateNaturalSize(const gfx::Size& natural_size) override {
    natural_size_ = natural_size;
    if (InCardNow()) {
      video_view_->SetBoundsRect(Fitted(card_->GetWindowBoundsInScreen().size(), natural_size));
    }
    Forward(&VideoOverlayWindow::UpdateNaturalSize, natural_size);
  }

  void SetSurfaceId(const viz::SurfaceId& surface_id) override {
    surface_id_ = surface_id;
    if (InCardNow()) {
      ShowSurfaceInCard();
    }
    Forward(&VideoOverlayWindow::SetSurfaceId, surface_id);
  }

  // The rest are Chrome's window's alone, which gets them as it's made, and as they change.
  void SetPlaybackState(PlaybackState state) override {
    Forward(&VideoOverlayWindow::SetPlaybackState, state);
  }
  void SetPlayPauseButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetPlayPauseButtonVisibility, is_visible);
  }
  void SetSkipAdButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetSkipAdButtonVisibility, is_visible);
  }
  void SetNextTrackButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetNextTrackButtonVisibility, is_visible);
  }
  void SetPreviousTrackButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetPreviousTrackButtonVisibility, is_visible);
  }
  void SetHidePictureInPictureButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetHidePictureInPictureButtonVisibility, is_visible);
  }
  void SetMicrophoneMuted(bool muted) override {
    Forward(&VideoOverlayWindow::SetMicrophoneMuted, muted);
  }
  void SetCameraState(bool turned_on) override {
    Forward(&VideoOverlayWindow::SetCameraState, turned_on);
  }
  void SetMediaMuted(bool muted) override { Forward(&VideoOverlayWindow::SetMediaMuted, muted); }
  void SetToggleMicrophoneButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetToggleMicrophoneButtonVisibility, is_visible);
  }
  void SetToggleCameraButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetToggleCameraButtonVisibility, is_visible);
  }
  void SetHangUpButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetHangUpButtonVisibility, is_visible);
  }
  void SetNextSlideButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetNextSlideButtonVisibility, is_visible);
  }
  void SetPreviousSlideButtonVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetPreviousSlideButtonVisibility, is_visible);
  }
  void SetMediaPosition(const media_session::MediaPosition& position) override {
    Forward(&VideoOverlayWindow::SetMediaPosition, position);
  }
  void SetSourceTitle(const std::u16string& source_title) override {
    Forward(&VideoOverlayWindow::SetSourceTitle, source_title);
  }
  void SetFaviconImages(const std::vector<media_session::MediaImage>& images) override {
    Forward(&VideoOverlayWindow::SetFaviconImages, images);
  }
  void SetPlaybackControlsVisibility(bool is_visible) override {
    Forward(&VideoOverlayWindow::SetPlaybackControlsVisibility, is_visible);
  }
  void SetImmersiveVideoOptions(const content::ImmersiveOptions& options) override {
    Forward(&VideoOverlayWindow::SetImmersiveVideoOptions, options);
  }

  // views::WidgetObserver, for the browser window the card's in:
  void OnWidgetBoundsChanged(views::Widget*, const gfx::Rect&) override {
    // A window of its own off the Mac, which a browser window moving leaves behind.
    if (InCardNow()) {
      Place();
    }
  }

  void OnWidgetDestroying(views::Widget*) override {
    const bool was_in_card = InCardNow();
    DropCard();
    // Its card went with its window, so the video leaves Picture in Picture.
    if (was_in_card) {
      base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&VideoWindow::Leave, weak_factory_.GetWeakPtr()));
    }
  }

 private:
  bool InCardNow() const { return in_card_; }

  void Leave() {
    if (!InCardNow() && !(chrome_ && chrome_->IsVisible())) {
      End();
    }
  }

  // Calls |setter| on Chrome's window, now if it's made and as it's made otherwise.
  template <typename Param, typename Arg>
  void Forward(void (VideoOverlayWindow::*setter)(Param), Arg&& arg) {
    using Value = std::decay_t<Arg>;
    auto call = base::BindRepeating(
        [](void (VideoOverlayWindow::*setter)(Param), const Value& value,
           VideoOverlayWindow* window) { (window->*setter)(value); },
        setter, Value(std::forward<Arg>(arg)));
    if (chrome_) {
      call.Run(chrome_.get());
    }
    // Only each setter's last call matters to a window made later.
    const std::string key(reinterpret_cast<const char*>(&setter), sizeof(setter));
    std::erase_if(replay_, [&key](const Replay& replay) { return replay.first == key; });
    replay_.emplace_back(key, std::move(call));
  }

  using Replay = std::pair<std::string, base::RepeatingCallback<void(VideoOverlayWindow*)>>;

  void ShowInChrome() {
    if (!chrome_ && make_chrome_) {
      chrome_ = std::move(make_chrome_).Run();
      for (const Replay& replay : replay_) {
        replay.second.Run(chrome_.get());
      }
    }
    if (chrome_) {
      chrome_->ShowInactive();
    }
  }

  void MakeCard(views::Widget* browser) {
    card_ = std::make_unique<views::Widget>();
    views::Widget::InitParams params(views::Widget::InitParams::CLIENT_OWNS_WIDGET,
                                     views::Widget::InitParams::TYPE_POPUP);
    // Over the browser window, moving with it where the platform keeps a child window with its
    // parent, and never taking the keyboard or the pointer from the UI under it.
    params.parent = browser->GetNativeView();
    params.activatable = views::Widget::InitParams::Activatable::kNo;
    params.accept_events = false;
    params.opacity = views::Widget::InitParams::WindowOpacity::kTranslucent;
    params.shadow_type = views::Widget::InitParams::ShadowType::kNone;
    params.name = "BrowserCoreCardVideo";
#if BUILDFLAG(IS_OZONE)
    // A window of its own on Linux too, as on the Mac and Windows: X11's UI is a window over the
    // browser window's, which a view drawn in it would be under.
    params.use_accelerated_widget_override = true;
#endif
    card_->Init(std::move(params));
    platform::PassPointerThrough(card_.get());
    auto contents = std::make_unique<views::View>();
    contents->SetPaintToLayer(ui::LAYER_SOLID_COLOR);
    contents->layer()->AsSolidColor()->SetColor(SkColors::kBlack);
    contents->layer()->SetMasksToBounds(true);
    contents->layer()->SetIsFastRoundedCorner(true);
    auto video = std::make_unique<views::View>();
    video->SetPaintToLayer(ui::LAYER_SURFACE);
    video_view_ = contents->AddChildView(std::move(video));
    card_->SetContentsView(std::move(contents));
    browser_ = browser;
    browser_observation_.Observe(browser);
  }

  void ShowSurfaceInCard() {
    if (!card_ || !surface_id_.is_valid()) {
      return;
    }
    ui::Compositor* compositor = card_->GetCompositor();
    if (!compositor) {
      return;
    }
    if (registered_ != surface_id_.frame_sink_id()) {
      if (registered_) {
        compositor->RemoveChildFrameSink(*registered_);
      }
      compositor->AddChildFrameSink(surface_id_.frame_sink_id());
      registered_ = surface_id_.frame_sink_id();
    }
    ui::LayerSurface* surface = video_view_->layer()->AsSurface();
    surface->SetFallbackBackgroundColor(SkColors::kBlack);
    surface->SetShowSurface(surface_id_, video_view_->size(),
                            cc::DeadlinePolicy::UseDefaultDeadline(),
                            /*stretch_content_to_fill_bounds=*/true);
  }

  void HideCard(bool log = true) {
    if (in_card_) {
      in_card_ = false;
      card_->Hide();
      if (log) {
        // check.sh waits for this line.
        LOG(ERROR) << "browser: card video " << tab() << " left its card";
      }
    }
    if (InCard() == this) {
      InCard() = nullptr;
    }
  }

  void DropCard(bool log = true) {
    HideCard(log);
    browser_observation_.Reset();
    browser_ = nullptr;
    if (!card_) {
      return;
    }
    if (registered_ && card_->GetCompositor()) {
      card_->GetCompositor()->RemoveChildFrameSink(*registered_);
    }
    registered_.reset();
    video_view_ = nullptr;
    card_.reset();
  }

  const raw_ptr<content::VideoPictureInPictureWindowController> controller_;
  ChromeVideoWindow make_chrome_;
  std::unique_ptr<VideoOverlayWindow> chrome_;
  std::vector<Replay> replay_;
  gfx::Size natural_size_;
  viz::SurfaceId surface_id_;
  std::unique_ptr<views::Widget> card_;
  // Whether |card_| shows the video, which a Mac window's own visibility says only later.
  bool in_card_ = false;
  raw_ptr<views::View> video_view_ = nullptr;
  raw_ptr<views::Widget> browser_ = nullptr;
  std::optional<viz::FrameSinkId> registered_;
  base::ScopedObservation<views::Widget, views::WidgetObserver> browser_observation_{this};
  base::WeakPtrFactory<VideoWindow> weak_factory_{this};
};

}  // namespace

bool CardCanShowVideo(content::WebContents* contents) {
  // Its media session's Picture in Picture goes to the page's own handler where it has one, which
  // floats a page of its own, as a call's does, or the music check/media.sh plays: never for a card
  // under the pointer.
  const scoped_refptr<MediaStreamCaptureIndicator> capture =
      MediaCaptureDevicesDispatcher::GetInstance()->GetMediaStreamCaptureIndicator();
  if (capture->IsCapturingVideo(contents) || capture->IsCapturingAudio(contents)) {
    return false;
  }
  const media_session::mojom::MediaSessionInfoPtr info =
      content::MediaSession::Get(contents)->GetMediaSessionInfoSync();
  return info && info->audio_video_states &&
         std::ranges::any_of(*info->audio_video_states, [](auto state) {
           return state == media_session::mojom::MediaAudioVideoState::kAudioVideo ||
                  state == media_session::mojom::MediaAudioVideoState::kVideoOnly;
         });
}

bool CardShowsVideo(content::WebContents* contents) {
  return InCard() && contents && InCard()->tab() == Tab::IdOf(contents);
}

std::unique_ptr<content::VideoOverlayWindow> NewVideoWindow(
    content::VideoPictureInPictureWindowController* controller,
    ChromeVideoWindow chrome) {
  return std::make_unique<VideoWindow>(controller, std::move(chrome));
}

void ShowCardVideo(bc_id tab, bc_id window, const gfx::Rect& frame, double corner_radius) {
  std::optional<Ask>& asked = Asked();
  VideoWindow* in_card = InCard();
  if (frame.IsEmpty()) {
    if (asked && asked->tab == tab && asked->window == window) {
      asked.reset();
    }
    // The video leaves Picture in Picture, and plays on in its page.
    if (in_card && in_card->tab() == tab) {
      in_card->End();
    }
    return;
  }
  const bool started = !asked || asked->tab != tab;
  asked = Ask{tab, window, frame, corner_radius};
  if (in_card && in_card->tab() == tab) {
    in_card->Place();
    return;
  }
  content::WebContents* contents = Tab::FromId(tab);
  if (!started || !contents || Window::Showing(tab) || !CardCanShowVideo(contents)) {
    return;
  }
  // A window that has a video already, the Mini Player or one the page floated, keeps it, as a
  // call's floating page does.
  if (!in_card && PictureInPictureWindowManager::GetInstance()->IsInPictureInPicture()) {
    return;
  }
  if (in_card) {
    in_card->End();
  }
  // The page's one video, or its own handler: its window comes from NewVideoWindow.
  Entering() = tab;
  content::MediaSession::Get(contents)->EnterPictureInPicture();
}

bool FloatCardVideo(content::WebContents* contents) {
  // The user's own Picture in Picture, which a card's that never came mustn't end.
  Entering() = 0;
  VideoWindow* in_card = InCard();
  if (!in_card || !contents || in_card->tab() != Tab::IdOf(contents)) {
    return false;
  }
  std::optional<Ask>& asked = Asked();
  if (asked && asked->tab == in_card->tab()) {
    asked.reset();
  }
  in_card->Float();
  return true;
}

}  // namespace browser_core
