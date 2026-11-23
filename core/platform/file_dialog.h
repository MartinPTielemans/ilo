// Ending a system file dialog whose answer nobody may hear any more, as an agent's file field's is
// once its call ended with the dialog up (#3759).

#ifndef BROWSER_CORE_PLATFORM_FILE_DIALOG_H_
#define BROWSER_CORE_PLATFORM_FILE_DIALOG_H_

#include <memory>

#include "browser_core/public/browser_core.h"
#include "ui/gfx/native_ui_types.h"
#include "ui/shell_dialogs/select_file_dialog.h"

namespace browser_core::platform {

// What closes one dialog while its listener waits for the dialog's answer. Its owner keeps it until
// that answer, and no longer.
class FileDialogCloser {
 public:
  virtual ~FileDialogCloser() = default;
};

// Ends |dialog|, the one the field |field| opened over |owner| (null if that window has gone), which
// is still up. On the Mac and Linux the dialog closes as it's told its listener is gone, which then
// hears nothing more, and this returns null. On Windows it runs in a process of its own, which
// can't be told: the dialog keeps its listener, and what this returns closes it as its Cancel would.
// The listener then hears its answer once, and has to drop it.
std::unique_ptr<FileDialogCloser> EndFileDialog(ui::SelectFileDialog& dialog,
                                                gfx::NativeWindow owner,
                                                bc_id field);

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_FILE_DIALOG_H_
