#ifndef BROWSER_CORE_PERMISSION_QUESTION_H_
#define BROWSER_CORE_PERMISSION_QUESTION_H_

#include <memory>
#include <optional>
#include <vector>

#include "base/memory/weak_ptr.h"
#include "browser_core/public/browser_core.h"
#include "components/permissions/permission_prompt.h"

namespace browser_core {

// A page's permission request, which the app asks the user about in place of Chrome's bubble
// (tab_permission_asked). Chrome's PermissionRequestManager makes one while the tab is shown, and
// destroys it once the request is answered, the tab goes to another page or closes, or another tab
// is shown, keeping the request for when it's shown again.
class PermissionQuestion : public permissions::PermissionPrompt {
 public:
  // g_browser_core_permission_prompt.
  static std::unique_ptr<permissions::PermissionPrompt> Create(
      content::WebContents* contents,
      permissions::PermissionPrompt::Delegate* delegate);
  // See bc_tab_answer_permission.
  static void Answer(bc_id tab, int answer);
  // Whether the tab's question asks only to show notifications, with no click or key of the
  // user's behind it (bc_tab_permission_quiet).
  static bool Quiet(bc_id tab);

  PermissionQuestion(const PermissionQuestion&) = delete;
  PermissionQuestion& operator=(const PermissionQuestion&) = delete;
  ~PermissionQuestion() override;

  // permissions::PermissionPrompt:
  bool UpdateAnchor() override;
  TabSwitchingBehavior GetTabSwitchingBehavior() override;
  permissions::PermissionPromptDisposition GetPromptDisposition() const override;
  bool IsAskPrompt() const override;
  std::optional<gfx::Rect> GetViewBoundsInScreen() const override;
  std::vector<permissions::ElementAnchoredBubbleVariant> GetPromptVariants() const override;
  std::optional<permissions::feature_params::PermissionElementPromptPosition> GetPromptPosition()
      const override;

 private:
  PermissionQuestion(bc_id tab, permissions::PermissionPrompt::Delegate* delegate);

  // Tells the app the tab's question, or that it has none, from a task of its own: Chrome makes and
  // destroys questions inside the app's calls, such as the one that shows a tab.
  static void Changed(bc_id tab);
  static void Report(bc_id tab);

  const bc_id tab_;
  const base::WeakPtr<permissions::PermissionPrompt::Delegate> delegate_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_PERMISSION_QUESTION_H_
