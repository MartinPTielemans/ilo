// What a page in an agent's Space is refused of the user's device and person, and the activity
// entry that says so. The answers are agent_permission_rule.h's for the tab the page is in
// (agent_space_page.h): nothing a page or an agent's call says changes one.

#ifndef BROWSER_CORE_AGENT_PERMISSIONS_H_
#define BROWSER_CORE_AGENT_PERMISSIONS_H_

#include "base/files/file_path.h"
#include "browser_core/agent_permission_rule.h"
#include "chrome/browser/download/download_confirmation_reason.h"
#include "chrome/browser/download/download_target_determiner_delegate.h"
#include "components/content_settings/core/common/content_settings_types.h"
#include "content/public/browser/global_routing_id.h"

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

// Whether a handle on a file or a folder made for |frame| keeps no grant: one on a file of the
// user's for a page in an agent's Space, with its entry. |site_storage| says it is in the site's
// own storage. A worker that is no frame's is in no tab, and keeps what it has.
bool NoFileHandle(content::GlobalRenderFrameHostId frame, bool site_storage);

// Whether the page in |frame| gets no colour picker, which is the system's and reads the screen
// where the user clicks: a page in an agent's Space, held or not, with its entry.
bool NoColorPicker(content::RenderFrameHost* frame);

// Whether the page in |frame| gets no print preview: a page in an agent's Space the user doesn't
// hold, with its entry. |by_users_command| says the browser's own Print command started it, not
// the page, and what the user does makes no entry.
bool NoPrint(content::RenderFrameHost* frame, bool by_users_command);

// Closes a print preview open over the page in |contents| if the page is refused one now, as the
// preview's own Cancel does: where the user's hold of a tab ends and where a tab of theirs
// becomes an agent's, so none stays over a page a thread works on.
void ClosePrintPreview(content::WebContents* contents);

// Whether a load for the page in |contents| that a server asks a certificate of is ended in place
// of the chooser for one of the user's: a page in an agent's Space the user doesn't hold, where
// the user has any to offer, with its entry at the tab.
bool NoClientCertificate(content::WebContents* contents, bool any_to_offer);

// Whether the page in |frame| is refused a list or picker of its own drawn outside it: the
// system's menu for a select (|system_menu|) or a popup window the page draws. A page in an
// agent's Space the user doesn't hold, whoever asked. It makes no entry: it is no question to the
// user, and an agent chooses in a select without opening it.
bool NoPagePopup(content::RenderFrameHost* frame, bool system_menu);

// Chrome's question of where |download|'s file goes, asked before any sheet is made for it:
// whether the core took it, which it does for a download of a page in an agent's Space the user
// doesn't hold. One taken is answered through |callback| at once: on to |suggested_path| where
// only the user's setting would ask, cancelled where that place can't take it.
bool TakesDownloadQuestion(download::DownloadItem* download,
                           const base::FilePath& suggested_path,
                           DownloadConfirmationReason reason,
                           DownloadTargetDeterminerDelegate::ConfirmationCallback* callback);

// Whether the page in |contents| is refused the list of the user's devices to cast to, asked
// before the list is made and before the tab is brought to the front for it: a page in an agent's
// Space the user doesn't hold, whoever asked, the page or a command. Notes the refusal.
bool NoCast(content::WebContents* contents);

// Whether the page in |frame| is refused the system's sheet to share with, which Chrome's share
// asks at each share it would otherwise show: a page in an agent's Space the user doesn't hold.
// Notes the refusal.
bool NoShare(content::RenderFrameHost* frame);

// Whether the page in |contents| is refused the dialog that asks whether to install it as an app,
// asked as each such dialog would be made, whoever started the install: a page in an agent's Space
// the user doesn't hold. Notes the refusal.
bool NoAppInstall(content::WebContents* contents);

// Whether the page in |contents| is refused the sheet that offers the user's accounts with another
// site to sign in with, asked as each of its requests to sign in so starts: a page in an agent's
// Space the user doesn't hold. Notes the refusal.
bool NoFederatedSignIn(content::WebContents* contents);

// Whether the page in |contents| is refused the question whether to add an extension, asked
// before the question is put on a window: a page in an agent's Space the user doesn't hold, so
// that no agent's click puts one on the user's window. Notes the refusal.
bool NoExtensionInstall(content::WebContents* contents);

// Closes what Chrome drew over the page in |contents| as a tab's dialog, a certificate chooser
// among them, and the list of devices to cast it to, if the page is in an agent's Space the user
// doesn't hold, as Chrome closes them when a page is left: where the user's hold of a tab ends and
// where a tab of theirs becomes an agent's. What a hold let the user be asked ends with it, as
// cancelled.
void CloseTabDialogs(content::WebContents* contents);

// Notes in the activity log that the browser refused |kind| to the page of |frame|, or of
// |contents|, if it's in an agent's Space: once per document and kind, however often the page
// asks, and after the task that refused it, since the app takes no entry inside a call of its own
// into the core. |cancelled_question| for a question taken before it was put to the user, whose
// entry has a reason of its own in one state (agent_permission_rule::EntryReason) and is made
// once more for it.
void NoteRefused(agent_permission_rule::Kind kind,
                 content::RenderFrameHost* frame,
                 bool cancelled_question = false);
void NoteRefused(agent_permission_rule::Kind kind, content::WebContents* contents);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_PERMISSIONS_H_
