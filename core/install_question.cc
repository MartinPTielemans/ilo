#include "browser_core/install_question.h"

#include <algorithm>
#include <map>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/window.h"
#include "chrome/browser/extensions/browser_window_util.h"
#include "chrome/browser/extensions/extension_install_prompt_show_params.h"
#include "chrome/browser/profiles/profile.h"
#include "components/tabs/public/tab_interface.h"
#include "extensions/browser/install_prompt_data.h"
#include "extensions/browser/install_prompt_permissions.h"
#include "extensions/common/extension.h"
#include "ui/gfx/codec/png_codec.h"
#include "ui/gfx/image/image.h"

namespace browser_core {

namespace {

using DoneCallbackPayload = extensions::ExtensionInstallPromptClient::DoneCallbackPayload;

// The questions, in the order Chrome asked them.
std::vector<std::unique_ptr<InstallQuestion>>& Questions() {
  static base::NoDestructor<std::vector<std::unique_ptr<InstallQuestion>>> questions;
  return *questions;
}

// The question each window last told the app it asks.
std::map<bc_id, bc_id>& Told() {
  static base::NoDestructor<std::map<bc_id, bc_id>> told;
  return *told;
}

}  // namespace

// static
void InstallQuestion::Ask(std::unique_ptr<ExtensionInstallPromptShowParams> show_params,
                          extensions::ExtensionInstallPromptClient::DoneCallback done,
                          std::unique_ptr<extensions::InstallPromptData> prompt) {
  content::WebContents* contents = show_params->GetParentWebContents();
  tabs::TabInterface* tab = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  Window* window = tab ? Window::ForBrowser(tab->GetBrowserWindowInterface()) : nullptr;
  if (!window) {
    // Such as an extension's popup asking for more permissions: the profile's last window asks,
    // else the first.
    tab = nullptr;
    Profile* profile = show_params->profile();
    BrowserWindowInterface* browser =
        profile ? extensions::browser_window_util::GetLastActiveBrowserWithProfile(
                      *profile, /*include_incognito_or_parent=*/true)
                : nullptr;
    window = browser ? Window::ForBrowser(browser) : nullptr;
    if (!window) {
      // A window with a Space of its own is never the one to ask.
      const std::vector<Window*> all = Window::All();
      const auto regular = std::ranges::find_if(
          all, [](const Window* each) { return !each->own_space(); });
      window = regular == all.end() ? nullptr : *regular;
    }
  }
  // Chrome keeps extensions out of a private session, and the user is shown no way to add one to
  // it: a private window's tab asking is turned away.
  if (!window || window->off_the_record() || !prompt->extension() ||
      !Host().window_extension_question) {
    std::move(done).Run(DoneCallbackPayload(Result::ABORTED));
    return;
  }
  Questions().push_back(base::WrapUnique(new InstallQuestion(
      window->id(), tab ? tab->GetContents() : nullptr, std::move(done), std::move(prompt))));
  Changed(window->id());
}

// static
void InstallQuestion::Answer(bc_id question, bool accept) {
  // Posted: Chrome goes on with the install, the extension's enabling or its permissions inside
  // the call, which can tell the app of them.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&InstallQuestion::Finish, question,
                                accept ? Result::ACCEPTED : Result::USER_CANCELED));
}

// static
void InstallQuestion::WindowClosing(bc_id window) {
  // The app is told nothing more of a window that's gone.
  Told().erase(window);
  std::vector<bc_id> dropped;
  for (const std::unique_ptr<InstallQuestion>& question : Questions()) {
    if (question->window_ == window) {
      dropped.push_back(question->id_);
    }
  }
  for (bc_id question : dropped) {
    Finish(question, Result::ABORTED);
  }
}

InstallQuestion::InstallQuestion(bc_id window,
                                 content::WebContents* tab,
                                 extensions::ExtensionInstallPromptClient::DoneCallback done,
                                 std::unique_ptr<extensions::InstallPromptData> prompt)
    : content::WebContentsObserver(tab),
      id_(NewId()),
      window_(window),
      done_(std::move(done)),
      prompt_(std::move(prompt)) {}

InstallQuestion::~InstallQuestion() = default;

void InstallQuestion::WebContentsDestroyed() {
  Finish(id_, Result::ABORTED);
}

// static
void InstallQuestion::Finish(bc_id question, Result result) {
  std::vector<std::unique_ptr<InstallQuestion>>& questions = Questions();
  auto found = std::ranges::find(questions, question, [](const auto& each) { return each->id_; });
  if (found == questions.end()) {
    return;
  }
  // Out of the list first, as Chrome can ask again from the callback.
  std::unique_ptr<InstallQuestion> finished = std::move(*found);
  questions.erase(found);
  Changed(finished->window_);
  // Chrome's dialog tells the prompt's observers what the user chose.
  if (result == Result::ACCEPTED) {
    finished->prompt_->OnDialogAccepted();
  } else if (result == Result::USER_CANCELED) {
    finished->prompt_->OnDialogCanceled();
  }
  std::move(finished->done_).Run(DoneCallbackPayload(result));
}

// static
void InstallQuestion::Changed(bc_id window) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&InstallQuestion::Report, window));
}

// static
void InstallQuestion::Report(bc_id window) {
  const auto first = std::ranges::find(Questions(), window,
                                       [](const auto& each) { return each->window_; });
  InstallQuestion* asked = first == Questions().end() ? nullptr : first->get();
  const auto told = Told().find(window);
  if ((told == Told().end() ? 0 : told->second) == (asked ? asked->id_ : 0) ||
      !Window::FromId(window) || !Host().window_extension_question) {
    return;
  }
  if (!asked) {
    Told().erase(told);
    Host().window_extension_question(Host().context, window, nullptr);
    return;
  }
  Told()[window] = asked->id_;
  const extensions::InstallPromptData& prompt = *asked->prompt_;
  const extensions::Extension& extension = *prompt.extension();
  std::optional<std::vector<uint8_t>> icon;
  if (!prompt.icon().IsEmpty()) {
    icon = gfx::PNGCodec::EncodeBGRASkBitmap(*prompt.icon().ToSkBitmap(),
                                             /*discard_transparency=*/false);
  }
  const std::string title = base::UTF16ToUTF8(prompt.GetDialogTitle());
  const std::string heading = base::UTF16ToUTF8(prompt.GetPermissionsHeading());
  const std::string accept = base::UTF16ToUTF8(prompt.GetAcceptButtonLabel());
  const std::string cancel = base::UTF16ToUTF8(prompt.GetAbortButtonLabel());
  const extensions::InstallPromptPermissions permissions = prompt.GetPermissions();
  std::vector<std::string> warnings;
  std::vector<std::string> details;
  for (size_t i = 0; i < permissions.permissions.size(); ++i) {
    warnings.push_back(base::UTF16ToUTF8(permissions.permissions[i]));
    details.push_back(i < permissions.details.size() ? base::UTF16ToUTF8(permissions.details[i])
                                                     : std::string());
  }
  std::vector<const char*> warning_texts;
  std::vector<const char*> detail_texts;
  for (size_t i = 0; i < warnings.size(); ++i) {
    warning_texts.push_back(warnings[i].c_str());
    detail_texts.push_back(details[i].c_str());
  }
  const bc_extension_question question = {
      .id = asked->id_,
      .extension = extension.id().c_str(),
      .name = extension.name().c_str(),
      .icon = icon && !icon->empty() ? icon->data() : nullptr,
      .icon_size = icon ? icon->size() : 0,
      .title = title.c_str(),
      .heading = heading.c_str(),
      .accept = accept.c_str(),
      .cancel = cancel.c_str(),
      .warnings = warning_texts.data(),
      .details = detail_texts.data(),
      .warning_count = warning_texts.size(),
  };
  Host().window_extension_question(Host().context, window, &question);
}

}  // namespace browser_core
