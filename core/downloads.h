#ifndef BROWSER_CORE_DOWNLOADS_H_
#define BROWSER_CORE_DOWNLOADS_H_

#include "browser_core/public/browser_core.h"

class Profile;

namespace browser_core {

// Makes downloads go straight to the download folder unless the user asked otherwise. Before the
// first profile is made.
void RegisterDownloadPrefs();

// Tells the app of |profile|'s downloads from now on, those Chrome's history put back from the
// last launch too, in one list with the other profiles', as Arc's Library lists them. Then
// downloads_changed.
void ObserveDownloads(Profile* profile);

// What bc_download_act does to the download |id|.
void ActOnDownload(bc_id id, int action);

// Takes the downloads not running or paused off the list, keeping their files.
void ClearDownloads();

// Finds the icon the system shows for the download's file. Then download_icon.
void FindDownloadIcon(bc_id id);

// What bc_agent_download_answer does to the download |id| a page in an agent's Space started.
void AnswerAgentDownload(bc_id id, bool keep);

}  // namespace browser_core

#endif  // BROWSER_CORE_DOWNLOADS_H_
