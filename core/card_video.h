// A media card's live video (bc_tab_show_video). The page's video goes into Picture in Picture, as
// for Chrome's floating window, and the renderer sends its frames to a surface. A window of the
// core's shows that surface at the card, over the app's view, so the video draws as the page plays
// it, with no copy and nothing while no card shows it. Every video's Picture in Picture window is
// the core's: at the card while one asked for it, and otherwise Chrome's, which it holds, so a video
// floated from its card moves between the two without leaving Picture in Picture.
#ifndef BROWSER_CORE_CARD_VIDEO_H_
#define BROWSER_CORE_CARD_VIDEO_H_

#include <memory>

#include "base/functional/callback_forward.h"
#include "browser_core/public/browser_core.h"

namespace content {
class VideoOverlayWindow;
class VideoPictureInPictureWindowController;
class WebContents;
}  // namespace content

namespace gfx {
class Rect;
}  // namespace gfx

namespace browser_core {

// Makes Chrome's own window for a video in Picture in Picture.
using ChromeVideoWindow = base::OnceCallback<std::unique_ptr<content::VideoOverlayWindow>()>;

// The window for |controller|'s video, which shows it at its card while one asks for it, and in
// the window |chrome| makes otherwise.
std::unique_ptr<content::VideoOverlayWindow> NewVideoWindow(
    content::VideoPictureInPictureWindowController* controller,
    ChromeVideoWindow chrome);

// Shows |tab|'s video at |frame| in |window|'s content, with corners of |corner_radius|, or with an
// empty frame stops (bc_tab_show_video).
void ShowCardVideo(bc_id tab, bc_id window, const gfx::Rect& frame, double corner_radius);

// Whether |contents| has a video and isn't in a call, so its card can show it (media_video).
bool CardCanShowVideo(content::WebContents* contents);

// Whether |contents|'s card shows its video.
bool CardShowsVideo(content::WebContents* contents);

// Floats |contents|'s video from its card into Chrome's window, as bc_tab_media's Picture in
// Picture does. Returns whether its card had it.
bool FloatCardVideo(content::WebContents* contents);

}  // namespace browser_core

#endif  // BROWSER_CORE_CARD_VIDEO_H_
