#ifndef BROWSER_CORE_PROFILES_H_
#define BROWSER_CORE_PROFILES_H_

#include <set>
#include <string>
#include <vector>

#include "base/functional/callback_forward.h"
#include "browser_core/public/browser_core.h"

class Profile;

namespace content {
class WebContents;
}

namespace browser_core {

// The profiles the Spaces' tabs open in (decision 5). Each is named by its directory under Chrome's
// user data directory, such as "Default", which is what Spaces and the session keep, and gets an id
// for the app the first time the core sees it. A profile stays loaded once a Space has used it.

// The profile the core started with, Chrome's last used, which Spaces saved without one are in.
Profile* StartProfile();
// |dir|'s profile if it's loaded, else the one the core started with. Null before Chrome made it.
Profile* ProfileFor(const std::string& dir);
// The directory of |profile|.
std::string DirOf(Profile* profile);
// The directory of the profile |contents| is in.
std::string DirOf(content::WebContents* contents);
// Loads each profile in |dirs| that exists and keeps it loaded, then runs |done|.
void LoadProfiles(const std::set<std::string>& dirs, base::OnceClosure done);
// Keeps |profile| loaded while the core runs, with its search engine and downloads seen to. Once.
void UseProfile(Profile* profile);
// |profile|'s Profile if it's loaded. Else null, and loads it, once, then spaces_changed, so
// Settings can show what a profile no Space is in keeps.
Profile* ProfileOrLoad(bc_id profile);
// Lets the profiles go, at quit.
void ReleaseProfiles();
// Blocks automatic Picture in Picture by default in every profile loaded while the user has it off
// in Settings, as Chrome's own default does, so a site the user allows in its Site Controls still
// may. Turned on again, the default goes back to Chrome's, but only when it |changed|, so a
// default chosen in Chrome's settings stays.
void ApplyAutoPictureInPicture(bool changed);
// |profile|'s off-the-record profile, made if there's none: the session its private windows
// share. Chrome destroys it once the last of their Browsers has gone, which ends the session, and
// then private_session_ended. A session that has ended isn't handed out again while Chrome waits
// to destroy it: a new one starts. Only opening a private window starts one.
Profile* StartPrivateSession(Profile* profile);
// |profile|'s off-the-record profile while its private session runs, else null. Never makes one.
Profile* PrivateProfileOf(Profile* profile);
// Ends |profile|'s private session once its last private window and Browser have gone, if Chrome
// hasn't, and tells the app once it has ended. Each way it doesn't end it is logged.
void EndPrivateSession(Profile* profile);

// The profiles: the one the core started with, then the others in the order they were made.
std::vector<bc_id> Profiles();
bc_id ProfileId(const std::string& dir);
// Empty for an id the core didn't give.
std::string ProfileDir(bc_id profile);
// "Default" for one Chrome named itself.
std::string ProfileName(bc_id profile);
// Names |profile| |name|, unless |name| is empty.
bool RenameProfile(bc_id profile, const std::string& name);
// Makes a profile named |name| and loads it, then runs |made| with it, or null if Chrome couldn't.
// Returns its id, or 0.
bc_id CreateProfile(const std::string& name, base::OnceCallback<void(Profile*)> made);
// Deletes |profile| and everything in it, unless a Space or a favorite is in it, it's the one the
// core started with, or it's the last.
bool DeleteProfile(bc_id profile);

}  // namespace browser_core

#endif  // BROWSER_CORE_PROFILES_H_
