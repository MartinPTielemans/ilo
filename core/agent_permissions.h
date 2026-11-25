// What a page in an agent's Space is refused of the user's device and person, and the activity
// entry that says so. The answers are agent_permission_rule.h's for the tab the page is in
// (agent_space_page.h): nothing a page or an agent's call says changes one.

#ifndef BROWSER_CORE_AGENT_PERMISSIONS_H_
#define BROWSER_CORE_AGENT_PERMISSIONS_H_

#include "browser_core/agent_permission_rule.h"
#include "components/content_settings/core/common/content_settings_types.h"

namespace content {
class RenderFrameHost;
class WebContents;
struct MediaStreamRequest;
}  // namespace content

namespace permissions {
class PermissionRequest;
}  // namespace permissions

namespace browser_core {

// Whether the page in |frame| is refused the permission |type| whatever the profile has saved for
// its site. A read of the status as much as a request, so it makes no entry.
bool NoPermission(content::RenderFrameHost* frame, ContentSettingsType type);

// A request by the page in |frame| for |type| was refused so: its entry.
void PermissionRefused(content::RenderFrameHost* frame, ContentSettingsType type);

// Whether the core takes |request| before the browser makes a question of it for the user. One it
// takes is ended as cancelled, or dropped unanswered where it is a site's offer to handle a kind
// of link, and nothing keeps a pointer to it.
bool TakesPermissionRequest(content::RenderFrameHost* frame,
                            permissions::PermissionRequest* request);

// Whether the page in |contents| is refused |request| for the camera, the microphone or the
// picture of a screen, a window or a tab, before any handler has it and so before any picker.
bool NoMediaAccess(content::WebContents* contents, const content::MediaStreamRequest& request);

// Whether a write to the clipboard by the page in |frame| is dropped.
bool NoClipboardWrite(content::RenderFrameHost* frame);

// Notes in the activity log that the browser refused |kind| to the page of |frame|, or of
// |contents|, if it's in an agent's Space: once per document and kind, however often the page
// asks, and after the task that refused it, since the app takes no entry inside a call of its own
// into the core.
void NoteRefused(agent_permission_rule::Kind kind, content::RenderFrameHost* frame);
void NoteRefused(agent_permission_rule::Kind kind, content::WebContents* contents);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_PERMISSIONS_H_
