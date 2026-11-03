#ifndef BROWSER_CORE_CAST_DIALOG_H_
#define BROWSER_CORE_CAST_DIALOG_H_

#include <memory>
#include <string>
#include <vector>

#include "base/memory/weak_ptr.h"
#include "browser_core/public/browser_core.h"
#include "chrome/browser/ui/media_router/cast_dialog_controller.h"
#include "chrome/browser/ui/media_router/ui_media_sink.h"
#include "components/media_router/browser/media_router_dialog_controller.h"
#include "content/public/browser/web_contents_user_data.h"

namespace media_router {
class MediaRouterUI;
}

namespace browser_core {

// A tab's Cast dialog as the app's card shows it, in place of Chrome's (tab_cast): Chrome's media
// router UI, which finds the devices and casts to them, reported to the app. Chrome asks for it
// from its page menu's Cast… and a page's request to cast, as well as bc_tab_cast. One shows at a
// time.
class CastDialog : public media_router::MediaRouterDialogController,
                   public media_router::CastDialogController::Observer,
                   public content::WebContentsUserData<CastDialog> {
 public:
  // |contents|'s, made the first time, which Chrome's media router asks for through
  // g_browser_core_cast_dialog in place of the Views dialog's controller.
  static media_router::MediaRouterDialogController* For(content::WebContents* contents);
  // See bc_tab_cast and the calls after it.
  static void Open(bc_id tab);
  static void Close();
  static void To(bc_id tab, const char* id);

  CastDialog(const CastDialog&) = delete;
  CastDialog& operator=(const CastDialog&) = delete;
  ~CastDialog() override;

  // media_router::MediaRouterDialogController:
  bool IsShowingMediaRouterDialog() const override;

 private:
  friend class content::WebContentsUserData<CastDialog>;
  explicit CastDialog(content::WebContents* contents);

  // media_router::MediaRouterDialogController:
  void CreateMediaRouterDialog(
      media_router::MediaRouterDialogActivationLocation activation_location) override;
  void CloseMediaRouterDialog() override;

  // media_router::CastDialogController::Observer:
  void OnModelUpdated(const media_router::CastDialogModel& model) override;
  void OnCastingStarted() override;

  // Tells the app from a task of its own: the model changes inside the app's calls.
  void ReportSoon();
  void Report();

  std::unique_ptr<media_router::MediaRouterUI> ui_;
  std::u16string header_;
  std::vector<media_router::UIMediaSink> sinks_;
  bool reporting_ = false;
  base::WeakPtrFactory<CastDialog> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_CAST_DIALOG_H_
