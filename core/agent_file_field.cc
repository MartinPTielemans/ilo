#include "browser_core/agent_file_field.h"

#include <map>
#include <utility>

#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "base/task/thread_pool.h"
#include "browser_core/agent_page.h"
#include "browser_core/agent_permissions.h"
#include "browser_core/agent_session.h"
#include "browser_core/host.h"
#include "browser_core/platform/file_dialog.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "net/base/mime_util.h"
#include "ui/shell_dialogs/select_file_policy.h"
#include "ui/shell_dialogs/selected_file_info.h"
#include "ui/views/widget/widget.h"

namespace browser_core {

namespace {

std::map<bc_id, std::unique_ptr<AgentFileField>>& Fields() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<AgentFileField>>> fields;
  return *fields;
}

bc_id g_next_field = 1;

// The agent whose Space has |tab|, or null.
const Space::Agent* AgentOf(bc_id tab) {
  const Space* space = Space::Holding(tab);
  return space && space->agent() ? &*space->agent() : nullptr;
}

// Whether |tab| is still in the Space of the agent |agent|, not only of any agent.
bool IsAgents(bc_id tab, const std::string& agent) {
  const Space::Agent* now = AgentOf(tab);
  return now && now->id == agent;
}

// Whether any of |files| is a folder, as a Mac package (.app, .rtfd) the open panel lets the user
// pick is: Chrome would hand the page that folder's path, and read access under it.
bool AnyFolder(const std::vector<std::string>& files) {
  for (const std::string& file : files) {
    if (base::DirectoryExists(base::FilePath::FromUTF8Unsafe(file))) {
      return true;
    }
  }
  return false;
}

// What the dialog filters by, from the field's accept, or none to show every file.
std::unique_ptr<ui::SelectFileDialog::FileTypeInfo> TypesOf(
    const std::vector<std::string>& accepts) {
  std::vector<base::FilePath::StringType> extensions;
  for (const std::string& type : accepts) {
    if (type.starts_with('.')) {
      extensions.push_back(base::FilePath::FromUTF8Unsafe(type.substr(1)).value());
    } else {
      net::GetExtensionsForMimeType(type, &extensions);
    }
  }
  if (extensions.empty()) {
    return nullptr;
  }
  auto types = std::make_unique<ui::SelectFileDialog::FileTypeInfo>();
  types->extensions.push_back(std::move(extensions));
  // As a page's own dialog has it, the user may still show every file.
  types->include_all_files = true;
  types->allowed_paths = ui::SelectFileDialog::FileTypeInfo::NATIVE_PATH;
  return types;
}

std::string PathOf(const ui::SelectedFileInfo& file) {
  return file.path().AsUTF8Unsafe();
}

}  // namespace

// The listener of a field's dialog, apart from the field so that it can outlive it (#3759). Until
// the field ends it hands the dialog's answer to the field. Once the field has ended with the
// dialog up, it holds what's closing the dialog, and drops the answer unread. The dialog answers its
// listener once, a cancelled or broken run too, and that answer ends the run and all it holds: so
// nothing closing a dialog outlives it, and none can close a dialog the user opens later.
class AgentFileField::DialogRun : public ui::SelectFileDialog::Listener {
 public:
  DialogRun(bc_id field, std::unique_ptr<ui::SelectFileDialog::FileTypeInfo> types)
      : field_(field), types_(std::move(types)) {}
  DialogRun(const DialogRun&) = delete;
  DialogRun& operator=(const DialogRun&) = delete;
  ~DialogRun() override = default;

  // Whether the system has a dialog to open: Linux has none when neither a toolkit nor the portal
  // answers, which Chrome's pickers allow for.
  bool Create() {
    dialog_ = ui::SelectFileDialog::Create(this, /*policy=*/nullptr);
    return !!dialog_;
  }

  // Over |owner|, where the system starts it: no title, folder or name of the page's or the agent's.
  void Open(gfx::NativeWindow owner, bool many) {
    // Kept, as the dialog may answer before SelectFile returns.
    scoped_refptr<ui::SelectFileDialog> dialog = dialog_;
    dialog->SelectFile(
        many ? ui::SelectFileDialog::SELECT_OPEN_MULTI_FILE : ui::SelectFileDialog::SELECT_OPEN_FILE,
        std::u16string(), base::FilePath(), types_.get(), types_ ? 1 : 0,
        base::FilePath::StringType(), owner);
  }

  // The field ended with the dialog still up, over |owner|.
  void FieldEnded(gfx::NativeWindow owner) {
    ended_ = true;
    closer_ = platform::EndFileDialog(*dialog_, owner, field_);
    // Without one, the dialog has let go of its listener and won't answer.
    if (!closer_) {
      delete this;
    }
  }

 private:
  // ui::SelectFileDialog::Listener:
  void FileSelected(const ui::SelectedFileInfo& file, int index) override {
    Answered({PathOf(file)});
  }
  void MultiFilesSelected(const std::vector<ui::SelectedFileInfo>& files) override {
    std::vector<std::string> paths;
    for (const ui::SelectedFileInfo& file : files) {
      paths.push_back(PathOf(file));
    }
    Answered(std::move(paths));
  }
  void FileSelectionCanceled() override { Answered({}); }

  void Answered(std::vector<std::string> files) {
    if (ended_) {
      // Whatever the user picked, after the field's call ended, goes to no one. The dialog has
      // ended, so what was closing it goes now, before any other dialog can open.
      closer_.reset();
      LOG(ERROR) << "browser: file field " << field_ << " dialog ended";
    } else {
      auto found = Fields().find(field_);
      if (found != Fields().end() && found->second->run_ == this) {
        found->second->run_ = nullptr;
      }
      // Posted, as ending the field inside the dialog's own call could end the dialog there.
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&AgentFileField::Picked, field_, std::move(files)));
    }
    // After the dialog's call returns, as this may hold the last reference to it.
    base::SingleThreadTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE, this);
  }

  const bc_id field_;
  // What the dialog filters by, which lives as long as it.
  const std::unique_ptr<ui::SelectFileDialog::FileTypeInfo> types_;
  scoped_refptr<ui::SelectFileDialog> dialog_;
  bool ended_ = false;
  // Declared after the dialog, so it goes first.
  std::unique_ptr<platform::FileDialogCloser> closer_;
};

AgentFileField::AgentFileField(bc_id tab,
                               std::string agent,
                               int64_t element,
                               content::WeakDocumentPtr document,
                               FileFieldPlace place,
                               bool many,
                               std::vector<std::string> accepts)
    : tab_(tab),
      agent_(std::move(agent)),
      element_(element),
      document_(std::move(document)),
      place_(std::move(place)),
      many_(many),
      accepts_(std::move(accepts)) {}

AgentFileField::~AgentFileField() {
  // A dialog still up is ended, and its answer goes to no one.
  if (run_) {
    DialogRun* run = run_;
    run_ = nullptr;
    Window* owner = Window::FromId(window_);
    views::Widget* widget = owner ? owner->widget() : nullptr;
    run->FieldEnded(widget ? widget->GetNativeWindow() : gfx::NativeWindow());
  }
}

// static
bc_id AgentFileField::Shown(bc_id tab,
                            int64_t element,
                            content::RenderFrameHost& frame,
                            FileFieldPlace place,
                            bool many,
                            std::vector<std::string> accepts) {
  const Space::Agent* agent = AgentOf(tab);
  if (!agent) {
    return 0;
  }
  // One field a tab, so what's kept is bounded by tabs whatever an agent asks.
  TabEnded(tab);
  const bc_id id = g_next_field++;
  Fields()[id] = base::WrapUnique(new AgentFileField(tab, agent->id, element,
                                                     frame.GetWeakDocumentPtr(), std::move(place),
                                                     many, std::move(accepts)));
  LOG(ERROR) << "browser: file field " << id << " shown in " << tab;
  return id;
}

// static
bool AgentFileField::Choose(bc_id id, bc_id window) {
  auto found = Fields().find(id);
  Window* owner = Window::FromId(window);
  views::Widget* widget = owner ? owner->widget() : nullptr;
  if (found == Fields().end() || found->second->asked_ || !widget) {
    return false;
  }
  AgentFileField& field = *found->second;
  // A field that left before the press, as its tab did, gets no dialog.
  if (!field.document_.AsRenderFrameHostIfValid() || !IsAgents(field.tab_, field.agent_) ||
      IsHeld(field.tab_)) {
    field.asked_ = true;
    LOG(ERROR) << "browser: file field " << id << " left before the dialog";
    End(id, BC_FILE_FIELD_LEFT, 0);
    return true;
  }
  auto run = std::make_unique<DialogRun>(id, TypesOf(field.accepts_));
  if (!run->Create()) {
    LOG(ERROR) << "browser: file field " << id << " has no dialog";
    return false;
  }
  field.asked_ = true;
  field.window_ = window;
  // Held by the field before the dialog opens, which may answer before SelectFile returns.
  field.run_ = run.release();
  LOG(ERROR) << "browser: file field " << id << " dialog over window " << window;
  // Over the window whose card the user pressed.
  field.run_->Open(widget->GetNativeWindow(), field.many_);
  return true;
}

// static
bool AgentFileField::Put(bc_id id, std::vector<std::string> files) {
  auto found = Fields().find(id);
  if (found == Fields().end() || found->second->asked_) {
    return false;
  }
  AgentFileField& field = *found->second;
  content::RenderFrameHost* frame = field.document_.AsRenderFrameHostIfValid();
  FileFieldPutNow now;
  now.same_document =
      frame && frame->GetDevToolsFrameToken().ToString() == field.place_.frame;
  now.origin = frame ? frame->GetLastCommittedOrigin().Serialize() : std::string();
  now.agents = IsAgents(field.tab_, field.agent_);
  now.held = IsHeld(field.tab_);
  now.many = field.many_;
  if (!MayPutFiles(field.place_, now, files.size())) {
    LOG(ERROR) << "browser: file field " << id << " put refused";
    return false;
  }
  field.asked_ = true;
  LOG(ERROR) << "browser: file field " << id << " put " << files.size();
  // As the user's are, after the same check for a folder, which never goes in.
  std::vector<std::string> checked = files;
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::USER_BLOCKING},
      base::BindOnce(&AnyFolder, std::move(checked)),
      base::BindOnce(&AgentFileField::Checked, id, std::move(files)));
  return true;
}

// static
void AgentFileField::Drop(bc_id id) {
  End(id, BC_FILE_FIELD_CLOSED, 0);
}

// static
void AgentFileField::TabEnded(bc_id tab) {
  std::vector<bc_id> ended;
  for (const auto& [id, field] : Fields()) {
    if (field->tab_ == tab) {
      ended.push_back(id);
    }
  }
  for (bc_id id : ended) {
    End(id, BC_FILE_FIELD_CLOSED, 0);
  }
}

// static
bool AgentFileField::PageMayOpen(content::WebContents* contents, int kind) {
  const bc_id tab = contents ? Tab::IdOf(contents) : 0;
  const Space::Agent* agent = tab ? AgentOf(tab) : nullptr;
  if (PageMayOpenFileDialog(agent != nullptr, tab && IsHeld(tab))) {
    return true;
  }
  LOG(ERROR) << "browser: page file dialog " << tab << " kind " << kind
             << " refused: an agent's tab";
  // The entry every refusal of the user's own has in an agent's Space, one for the document.
  NoteRefused(agent_permission_rule::Kind::kFilePicker, contents);
  return false;
}

// static
void AgentFileField::Picked(bc_id id, std::vector<std::string> files) {
  auto found = Fields().find(id);
  if (found == Fields().end()) {
    return;
  }
  LOG(ERROR) << "browser: file field " << id << " chose " << files.size();
  if (files.empty()) {
    End(id, BC_FILE_FIELD_NONE, 0);
    return;
  }
  std::vector<std::string> checked = files;
  base::ThreadPool::PostTaskAndReplyWithResult(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::USER_BLOCKING},
      base::BindOnce(&AnyFolder, std::move(checked)),
      base::BindOnce(&AgentFileField::Checked, id, std::move(files)));
}

// static
void AgentFileField::Checked(bc_id id, std::vector<std::string> files, bool folder) {
  auto found = Fields().find(id);
  if (found == Fields().end()) {
    return;
  }
  AgentFileField& field = *found->second;
  // A folder isn't a file, so the user chose none: the field and its page are as they were, which
  // the agent's next snapshot would find for nothing if it heard the field left.
  if (folder) {
    LOG(ERROR) << "browser: file field " << id << " chose a folder";
    End(id, BC_FILE_FIELD_NONE, 0);
    return;
  }
  content::WebContents* contents = Tab::FromId(field.tab_);
  AgentPage* page = contents ? AgentPage::FromWebContents(contents) : nullptr;
  const size_t count = files.size();
  if (!page) {
    End(id, BC_FILE_FIELD_LEFT, 0);
    return;
  }
  page->Fill(field.element_, std::move(files), base::BindOnce(&AgentFileField::Lands, id, count),
             base::BindOnce(
                 [](bc_id id, int count, bool put) {
                   LOG(ERROR) << "browser: file field " << id << (put ? " filled" : " left");
                   End(id, put ? BC_FILE_FIELD_PUT : BC_FILE_FIELD_LEFT, put ? count : 0);
                 },
                 id, static_cast<int>(count)));
}

// static
bool AgentFileField::Lands(bc_id id, size_t count, const FileFieldNow& read) {
  auto found = Fields().find(id);
  if (found == Fields().end()) {
    return false;
  }
  const AgentFileField& field = *found->second;
  FileFieldNow now = read;
  content::RenderFrameHost* frame = field.document_.AsRenderFrameHostIfValid();
  now.same_document =
      frame && frame->GetDevToolsFrameToken().ToString() == field.place_.frame;
  now.agents = IsAgents(field.tab_, field.agent_);
  now.held = IsHeld(field.tab_);
  return FilesLand(field.place_, now, count);
}

// static
void AgentFileField::End(bc_id id, int result, int count) {
  auto found = Fields().find(id);
  if (found == Fields().end()) {
    return;
  }
  const bool asked = found->second->asked_;
  // Its destructor closes a dialog still up.
  Fields().erase(found);
  if (!asked) {
    return;
  }
  // Posted, so the app never hears inside its own call.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](bc_id id, int result, int count) {
                       if (Host().file_field_done) {
                         Host().file_field_done(Host().context, id, result, count);
                       }
                     },
                     id, result, count));
}

}  // namespace browser_core
