#ifndef BROWSER_CORE_AGENT_FILE_FIELD_H_
#define BROWSER_CORE_AGENT_FILE_FIELD_H_

#include <cstdint>
#include <memory>
#include <string>
#include <vector>

#include "base/memory/raw_ptr.h"
#include "browser_core/agent_file_field_rule.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/weak_document_ptr.h"
#include "ui/shell_dialogs/select_file_dialog.h"

namespace content {
class RenderFrameHost;
class WebContents;
}  // namespace content

namespace browser_core {

// A file field in an agent's tab that the agent asked the user to fill (#3656): its card's
// Choose File… opens the system's file dialog, the browser's own over the user's window, and the
// files the user picks go into that field, or nowhere. The page's own file dialogs are refused in
// an agent's tab nobody holds (PageMayOpen), so this is the only way a file reaches one. Nothing
// from the page or the agent picks where the dialog starts or what it suggests: the field's accept
// only filters it, and its multiple only lets the user pick more than one. No file's name or path
// is told to anyone, or logged.
class AgentFileField {
 public:
  AgentFileField(const AgentFileField&) = delete;
  AgentFileField& operator=(const AgentFileField&) = delete;
  ~AgentFileField();

  // The field |element| of |tab|, in |frame| at |place|, which the agent's card names, for
  // bc_file_field_choose: its id, or 0 if the tab is in no agent's Space. It ends the tab's
  // earlier field, if any.
  static bc_id Shown(bc_id tab,
                     int64_t element,
                     content::RenderFrameHost& frame,
                     FileFieldPlace place,
                     bool many,
                     std::vector<std::string> accepts);
  // See bc_file_field_choose and bc_file_field_drop.
  static bool Choose(bc_id field, bc_id window);
  static void Drop(bc_id field);
  // See bc_file_field_put.
  static bool Put(bc_id field, std::vector<std::string> files);
  // The tab closed, or the agent's session in it ended: its fields are dropped.
  static void TabEnded(bc_id tab);

  // g_browser_core_page_file_dialog: whether the page in |contents| may open its own file dialog,
  // a file input's (kind 1) or a File System Access picker (kind 2). Each refusal is in the
  // activity log.
  static bool PageMayOpen(content::WebContents* contents, int kind);

 private:
  AgentFileField(bc_id tab,
                 std::string agent,
                 int64_t element,
                 content::WeakDocumentPtr document,
                 FileFieldPlace place,
                 bool many,
                 std::vector<std::string> accepts);

  // The listener of the field's dialog, which may outlive the field.
  class DialogRun;

  // The user picked |files| for the field |id|, which go into it if it's still the one the card
  // named, or none.
  static void Picked(bc_id id, std::vector<std::string> files);
  // The same, once a blocking task said whether any of |files| is a folder, which never goes in.
  static void Checked(bc_id id, std::vector<std::string> files, bool folder);
  // The field as it is, |read|, whose frame's document and tab this fills in: whether |count|
  // files go into the field |id| names.
  static bool Lands(bc_id id, size_t count, const FileFieldNow& read);
  // Forgets the field |id|, closing its dialog, then answers file_field_done, posted, with |result|
  // and |count|, if a Choose is still unanswered.
  static void End(bc_id id, int result, int count);

  const bc_id tab_;
  // The agent whose card it is, whose Space the tab must still be in at the press and the pick.
  const std::string agent_;
  const int64_t element_;
  // The document the field was in as the card showed: invalid once its frame has another.
  const content::WeakDocumentPtr document_;
  const FileFieldPlace place_;
  const bool many_;
  const std::vector<std::string> accepts_;
  // The window the user pressed Choose File… in, whose dialog's owner it is.
  bc_id window_ = 0;
  // The run of the field's dialog while the dialog is up: it lets the run go as it ends.
  raw_ptr<DialogRun> run_ = nullptr;
  // Whether a Choose is unanswered: from the user's press until file_field_done.
  bool asked_ = false;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_FILE_FIELD_H_
