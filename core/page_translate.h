#ifndef BROWSER_CORE_PAGE_TRANSLATE_H_
#define BROWSER_CORE_PAGE_TRANSLATE_H_

#include <cstdint>
#include <set>
#include <string>
#include <string_view>

#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "base/values.h"
#include "browser_core/public/browser_core.h"
#include "components/translate/core/browser/translate_driver.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"

namespace browser_core {

// A tab's page in another language, for the app to translate on the device (#671): the language
// Chrome's detection finds for each page (tab_language), and the browser's own script that
// gathers the page's text, shows the app's translation in its place and puts it back. Chrome's
// translate itself needs Google's servers and script, which ungoogled-chromium leaves out, so
// none of it runs; its world for scripts is free for ours, which the page can't reach.
class PageTranslate : public content::WebContentsObserver,
                      public content::WebContentsUserData<PageTranslate>,
                      public translate::TranslateDriver::LanguageDetectionObserver {
 public:
  PageTranslate(const PageTranslate&) = delete;
  PageTranslate& operator=(const PageTranslate&) = delete;
  ~PageTranslate() override;

  // See bc_tab_translate_read, bc_tab_translate_show and bc_tab_translate_revert.
  void Read(uint64_t request);
  void Show(uint64_t request, std::string_view texts);
  void Revert();

  // A tab with nothing to read is told so, from a task of its own.
  static void ReportNoTexts(bc_id tab, uint64_t request);

  // Hears the languages Chrome's translate client finds, once the tab's helpers have made it.
  void ObserveDriver();

 private:
  friend class content::WebContentsUserData<PageTranslate>;

  PageTranslate(content::WebContents* contents, bc_id tab);

  // Runs the script's |op| for |request| in the page's main frame, with |texts| as JSON.
  void Run(std::string_view op,
           uint64_t request,
           std::string_view texts,
           content::RenderFrameHost::JavaScriptResultCallback done);
  void Gathered(uint64_t request, base::Value texts);
  void ReportLanguage(const std::string& language);

  // content::WebContentsObserver:
  void PrimaryPageChanged(content::Page& page) override;
  void WebContentsDestroyed() override;

  // translate::TranslateDriver::LanguageDetectionObserver:
  void OnTranslateDriverDestroyed(translate::TranslateDriver* driver) override;
  void OnLanguageDetermined(const translate::LanguageDetectionDetails& details) override;

  const bc_id tab_;
  // What the app last heard for the page, so a page Chrome registers again isn't told twice.
  std::string language_;
  // The reads the page hasn't answered, each told null if the page goes first.
  std::set<uint64_t> reading_;
  base::ScopedObservation<translate::TranslateDriver,
                          translate::TranslateDriver::LanguageDetectionObserver>
      driver_observation_{this};
  base::WeakPtrFactory<PageTranslate> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_TRANSLATE_H_
