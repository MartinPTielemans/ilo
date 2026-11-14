#include "browser_core/permission_question.h"

#include <algorithm>
#include <map>
#include <string>
#include <utility>
#include <variant>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/ptr_util.h"
#include "base/memory/raw_ptr.h"
#include "base/no_destructor.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "components/content_settings/core/common/content_settings_types.h"
#include "components/permissions/permission_request.h"
#include "components/permissions/permission_request_enums.h"
#include "components/permissions/permission_uma_constants.h"
#include "components/url_formatter/elide_url.h"

namespace browser_core {

namespace {

// The tabs' questions, one a tab at most, as Chrome asks one at a time.
std::map<bc_id, raw_ptr<PermissionQuestion>>& Questions() {
  static base::NoDestructor<std::map<bc_id, raw_ptr<PermissionQuestion>>> questions;
  return *questions;
}

}  // namespace

std::unique_ptr<permissions::PermissionPrompt> PermissionQuestion::Create(
    content::WebContents* contents,
    permissions::PermissionPrompt::Delegate* delegate) {
  // Chrome's quiet prompt is for sites that ask too much, which it would rather not ask about.
  if (delegate->ShouldCurrentRequestUseQuietUI()) {
    return nullptr;
  }
  return base::WrapUnique(new PermissionQuestion(Tab::IdOf(contents), delegate));
}

PermissionQuestion::PermissionQuestion(bc_id tab, permissions::PermissionPrompt::Delegate* delegate)
    : tab_(tab), delegate_(delegate->GetWeakPtr()) {
  Questions()[tab_] = this;
  Changed(tab_);
}

PermissionQuestion::~PermissionQuestion() {
  if (auto found = Questions().find(tab_); found != Questions().end() && found->second == this) {
    Questions().erase(found);
  }
  Changed(tab_);
}

void PermissionQuestion::Answer(bc_id tab, int answer) {
  auto found = Questions().find(tab);
  if (found == Questions().end() || !found->second->delegate_) {
    return;
  }
  permissions::PermissionPrompt::Delegate& delegate = *found->second->delegate_;
  // Options are for choosing how precise a location to give, which desktop Chrome doesn't offer.
  switch (answer) {
    case BC_PERMISSION_ALLOW:
      delegate.Accept(std::monostate());
      break;
    case BC_PERMISSION_BLOCK:
      delegate.Deny(std::monostate());
      break;
    case BC_PERMISSION_DISMISS:
      delegate.Dismiss(std::monostate());
      break;
  }
}

bool PermissionQuestion::Quiet(bc_id tab) {
  auto found = Questions().find(tab);
  if (found == Questions().end() || !found->second->delegate_) {
    return false;
  }
  const auto& requests = found->second->delegate_->Requests();
  // A request that doesn't say whether it had a gesture asks as a card, as all did before.
  return !requests.empty() &&
         std::ranges::all_of(
             requests, [](const std::unique_ptr<permissions::PermissionRequest>& request) {
               return request->GetContentSettingsType() == ContentSettingsType::NOTIFICATIONS &&
                      request->GetGestureType() ==
                          permissions::PermissionRequestGestureType::NO_GESTURE;
             });
}

void PermissionQuestion::Changed(bc_id tab) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&PermissionQuestion::Report, tab));
}

void PermissionQuestion::Report(bc_id tab) {
  if (!Host().tab_permission_asked) {
    return;
  }
  auto found = Questions().find(tab);
  if (found == Questions().end() || !found->second->delegate_) {
    Host().tab_permission_asked(Host().context, tab, "", nullptr, 0);
    return;
  }
  permissions::PermissionPrompt::Delegate& delegate = *found->second->delegate_;
  // As Chrome's prompt names the site.
  const std::string site = base::UTF16ToUTF8(url_formatter::FormatUrlForSecurityDisplay(
      delegate.GetRequestingOrigin(), url_formatter::SchemeDisplay::OMIT_CRYPTOGRAPHIC));
  std::vector<std::string> permissions;
  for (const std::unique_ptr<permissions::PermissionRequest>& request : delegate.Requests()) {
    permissions.push_back(base::UTF16ToUTF8(request->GetMessageTextFragment()));
  }
  std::vector<const char*> phrases;
  for (const std::string& permission : permissions) {
    phrases.push_back(permission.c_str());
  }
  Host().tab_permission_asked(Host().context, tab, site.c_str(), phrases.data(), phrases.size());
}

bool PermissionQuestion::UpdateAnchor() {
  // The app places it.
  return true;
}

permissions::PermissionPrompt::TabSwitchingBehavior PermissionQuestion::GetTabSwitchingBehavior() {
  // As Chrome's bubble: the question goes with its tab and comes back with it.
  return kDestroyPromptButKeepRequestPending;
}

permissions::PermissionPromptDisposition PermissionQuestion::GetPromptDisposition() const {
  return permissions::PermissionPromptDisposition::ANCHORED_BUBBLE;
}

bool PermissionQuestion::IsAskPrompt() const {
  return true;
}

std::optional<gfx::Rect> PermissionQuestion::GetViewBoundsInScreen() const {
  return std::nullopt;
}

std::vector<permissions::ElementAnchoredBubbleVariant> PermissionQuestion::GetPromptVariants()
    const {
  return {};
}

std::optional<permissions::feature_params::PermissionElementPromptPosition>
PermissionQuestion::GetPromptPosition() const {
  return std::nullopt;
}

}  // namespace browser_core
