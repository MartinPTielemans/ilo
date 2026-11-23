#include "browser_core/platform/file_dialog.h"

#include <windows.h>

#include <cwchar>
#include <iterator>
#include <map>
#include <vector>

#include "base/logging.h"
#include "base/no_destructor.h"
#include "chrome/services/util_win/public/mojom/util_win.mojom.h"
#include "content/public/browser/service_process_host.h"
#include "content/public/browser/service_process_info.h"
#include "ui/aura/window.h"
#include "ui/aura/window_tree_host.h"

namespace browser_core::platform {

namespace {

// Chrome runs each file dialog on Windows in a UtilWin service process of its own
// (ChromeSelectFileDialogFactory), whose thread sits in IFileDialog::Show until the user answers
// and takes no message from the browser until then. IFileDialog::Close works only from the
// dialog's own event handlers, which fire on the user's actions alone. So the dialog's window is
// sent WM_CLOSE, as Chromium's AutoCloseDialogEventHandler does when the owner window goes: the
// dialog ends as its Cancel ends it, the process answers no file and exits, and the browser window
// it kept disabled is enabled again.
class Closer : public FileDialogCloser, public content::ServiceProcessHost::Observer {
 public:
  Closer(bc_id field, HWND owner) : field_(field), owner_(owner) {}
  Closer(const Closer&) = delete;
  Closer& operator=(const Closer&) = delete;

  ~Closer() override {
    Stop();
  }

  // Closes the dialog if its window is up, and otherwise waits for its process's window to show.
  void Start() {
    content::ServiceProcessHost::AddObserver(this);
    observing_ = true;
    for (const content::ServiceProcessInfo& info :
         content::ServiceProcessHost::GetRunningProcessInfo()) {
      if (info.IsService<chrome::mojom::UtilWin>() && info.GetProcess().IsValid()) {
        Watch(info.GetProcess().Pid());
      }
    }
    Look();
    started_ = true;
  }

 private:
  static std::map<HWINEVENTHOOK, Closer*>& Hooks() {
    static base::NoDestructor<std::map<HWINEVENTHOOK, Closer*>> hooks;
    return *hooks;
  }

  // Whether |pid| is a UtilWin process that is running now.
  static bool IsUtilWin(DWORD pid) {
    for (const content::ServiceProcessInfo& info :
         content::ServiceProcessHost::GetRunningProcessInfo()) {
      if (info.IsService<chrome::mojom::UtilWin>() && info.GetProcess().IsValid() &&
          static_cast<DWORD>(info.GetProcess().Pid()) == pid) {
        return true;
      }
    }
    return false;
  }

  // Whether |window| is this run's dialog: a dialog box (#32770) that the field's browser window
  // owns, in a UtilWin process. Chromium runs one shell dialog for a window at a time, and keeps
  // that window disabled while it's up, so this is the one. Nothing else of the window is read.
  bool IsRuns(HWND window) const {
    wchar_t name[8] = {};
    if (::GetClassNameW(window, name, std::size(name)) != 6 || std::wcscmp(name, L"#32770") != 0 ||
        ::GetWindow(window, GW_OWNER) != owner_) {
      return false;
    }
    DWORD pid = 0;
    ::GetWindowThreadProcessId(window, &pid);
    return pid && pid != ::GetCurrentProcessId() && IsUtilWin(pid);
  }

  // Hooks the windows |pid| shows, then looks for one already up, so none shows unseen between.
  void Watch(DWORD pid) {
    if (closed_) {
      return;
    }
    HWINEVENTHOOK hook = ::SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_SHOW, nullptr, &Shown,
                                           pid, 0, WINEVENT_OUTOFCONTEXT);
    if (hook) {
      Hooks()[hook] = this;
      hooks_.push_back(hook);
    }
    Look();
  }

  // Closes the dialog if its window is up.
  void Look() {
    if (closed_) {
      return;
    }
    ::EnumWindows(&Each, reinterpret_cast<LPARAM>(this));
  }

  static BOOL CALLBACK Each(HWND window, LPARAM self) {
    Closer* closer = reinterpret_cast<Closer*>(self);
    if (!closer->IsRuns(window)) {
      return TRUE;
    }
    closer->Close(window);
    return FALSE;
  }

  void Close(HWND window) {
    if (closed_) {
      return;
    }
    closed_ = true;
    ::PostMessageW(window, WM_CLOSE, 0, 0);
    // Whether it was up when the field ended, or showed after, when the hook or a launch found it.
    LOG(ERROR) << "browser: file field " << field_ << " dialog told to close"
               << (started_ ? " as it showed" : "");
    Stop();
  }

  void Stop() {
    for (HWINEVENTHOOK hook : hooks_) {
      ::UnhookWinEvent(hook);
      Hooks().erase(hook);
    }
    hooks_.clear();
    if (observing_) {
      content::ServiceProcessHost::RemoveObserver(this);
      observing_ = false;
    }
  }

  // An out-of-context hook's calls come on the thread that set it, the UI thread.
  static void CALLBACK Shown(HWINEVENTHOOK hook,
                             DWORD event,
                             HWND window,
                             LONG object,
                             LONG child,
                             DWORD thread,
                             DWORD time) {
    if (object != OBJID_WINDOW || child != CHILDID_SELF) {
      return;
    }
    auto found = Hooks().find(hook);
    if (found != Hooks().end() && found->second->IsRuns(window)) {
      found->second->Close(window);
    }
  }

  // content::ServiceProcessHost::Observer:
  void OnServiceProcessLaunched(const content::ServiceProcessInfo& info) override {
    if (info.IsService<chrome::mojom::UtilWin>() && info.GetProcess().IsValid()) {
      Watch(info.GetProcess().Pid());
    }
  }

  const bc_id field_;
  const HWND owner_;
  bool closed_ = false;
  bool started_ = false;
  bool observing_ = false;
  std::vector<HWINEVENTHOOK> hooks_;
};

}  // namespace

std::unique_ptr<FileDialogCloser> EndFileDialog(ui::SelectFileDialog& dialog,
                                                gfx::NativeWindow owner,
                                                bc_id field) {
  // The owner as Chrome's dialog takes it, whose top-level window owns the dialog's. A window that
  // has gone closed the dialog already, through Chromium's own handler.
  aura::WindowTreeHost* host = owner ? owner->GetHost() : nullptr;
  const HWND root = host ? ::GetAncestor(host->GetAcceleratedWidget(), GA_ROOT) : nullptr;
  LOG(ERROR) << "browser: file field " << field << " closing its dialog";
  auto closer = std::make_unique<Closer>(field, root);
  if (root) {
    closer->Start();
  }
  return closer;
}

}  // namespace browser_core::platform
