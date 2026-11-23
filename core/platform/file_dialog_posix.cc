#include "browser_core/platform/file_dialog.h"

namespace browser_core::platform {

std::unique_ptr<FileDialogCloser> EndFileDialog(ui::SelectFileDialog& dialog,
                                                gfx::NativeWindow owner,
                                                bc_id field) {
  // The Mac's panel and Linux's toolkit or portal dialog close as their listener goes.
  dialog.ListenerDestroyed();
  return nullptr;
}

}  // namespace browser_core::platform
