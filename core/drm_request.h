#ifndef BROWSER_CORE_DRM_REQUEST_H_
#define BROWSER_CORE_DRM_REQUEST_H_

#include "content/public/browser/render_frame_host.h"
#include "mojo/public/cpp/bindings/binder_map.h"

namespace content {
class WebContents;
}

namespace browser_core {

// g_browser_core_populate_frame_binders: lets a frame's page tell the core which protected-media
// key system it asks for. The core tells the app when it's one ilo can't play
// (tab_media_unsupported), once for each key system a document asks for, and takes it back once the
// tab goes to another site. Never at a site whose note the user closed (CloseMediaNote). Asked on
// the user's press, the tab moves to the system's engine instead, or the note asks first
// (protected_move_rule.h).
void PopulateFrameBinders(mojo::BinderMapWithContext<content::RenderFrameHost*>* map);

// Takes back what the app was told for |contents|, as its tab goes to the system's engine, which
// plays what the page asked for.
void ForgetMediaUnsupported(content::WebContents* contents);

// The system's engine committed a document in |contents|'s tab, which Chromium doesn't see: the
// note saying the tab moved goes at the first after the moved page's own.
void MovedPageCommitted(content::WebContents* contents);

// Whether the app was told a page of |contents| asked for what ilo can't play.
bool MediaUnsupported(content::WebContents* contents);

// The user closed the note of |contents|: its profile keeps the site of its page from showing it
// again, and each tab at the site takes it back.
void CloseMediaNote(content::WebContents* contents);

}  // namespace browser_core

#endif  // BROWSER_CORE_DRM_REQUEST_H_
