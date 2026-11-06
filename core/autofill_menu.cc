#include "browser_core/autofill_menu.h"

#include <optional>

#include "browser_core/agent_autofill.h"
#include "browser_core/tab.h"
#include "components/autofill/content/browser/content_autofill_client.h"
#include "components/autofill/content/browser/content_autofill_driver.h"
#include "components/autofill/core/browser/foundations/autofill_driver.h"
#include "components/autofill/core/browser/foundations/autofill_manager.h"
#include "components/autofill/core/browser/foundations/scoped_autofill_managers_observation.h"
#include "components/autofill/core/common/aliases.h"
#include "components/autofill/core/common/unique_ids.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_user_data.h"

namespace browser_core {

namespace {

// The field of a page its focus last went to, as autofill hears from the renderer, until the focus
// goes to something that isn't a form's field.
class FocusedField : public content::WebContentsUserData<FocusedField>,
                     public autofill::AutofillManager::Observer {
 public:
  FocusedField(const FocusedField&) = delete;
  FocusedField& operator=(const FocusedField&) = delete;
  ~FocusedField() override = default;

  const std::optional<autofill::FieldGlobalId>& field() const { return field_; }

 private:
  friend class content::WebContentsUserData<FocusedField>;

  FocusedField(content::WebContents* contents, autofill::ContentAutofillClient* client)
      : content::WebContentsUserData<FocusedField>(*contents) {
    // A tab often comes with its page, whose frames have their managers already.
    managers_.Observe(client, autofill::ScopedAutofillManagersObservation::InitializationPolicy::
                                  kObservePreexistingManagers);
  }

  // autofill::AutofillManager::Observer:
  void OnAfterFocusOnFormField(autofill::AutofillManager& manager,
                               autofill::FormGlobalId form,
                               autofill::FieldGlobalId field) override {
    field_ = field;
  }
  void OnAfterFocusOnNonFormField(autofill::AutofillManager& manager) override { field_.reset(); }

  std::optional<autofill::FieldGlobalId> field_;
  autofill::ScopedAutofillManagersObservation managers_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(FocusedField);

}  // namespace

// static
void AutofillMenu::Watch(content::WebContents* contents) {
  if (auto* client = autofill::ContentAutofillClient::FromWebContents(contents)) {
    FocusedField::CreateForWebContents(contents, client);
  }
}

// static
bool AutofillMenu::Show(bc_id tab, bool logins) {
  content::WebContents* contents = Tab::FromId(tab);
  FocusedField* focused = contents ? FocusedField::FromWebContents(contents) : nullptr;
  if (!focused || !focused->field() || IsAgentTab(contents)) {
    return false;
  }
  // Chrome shows its dropdown only while the page has the keyboard, in the frame with the focus.
  const autofill::FieldGlobalId field = *focused->field();
  content::RenderWidgetHostView* view = contents->GetRenderWidgetHostView();
  content::RenderFrameHost* frame = contents->GetFocusedFrame();
  if (!view || !view->HasFocus() || !frame ||
      autofill::LocalFrameToken(frame->GetFrameToken().value()) != field.frame_token) {
    return false;
  }
  // As the base class, where the call is public; ContentAutofillDriver hides it.
  autofill::AutofillDriver* driver = autofill::ContentAutofillDriver::GetForRenderFrameHost(frame);
  if (!driver) {
    return false;
  }
  // As a click in the field opens it, or as Chrome's Select Password does, with every login.
  driver->RendererShouldTriggerSuggestions(
      field, logins ? autofill::AutofillSuggestionTriggerSource::kManualFallbackPasswords
                    : autofill::AutofillSuggestionTriggerSource::kFormControlElementClicked);
  return true;
}

}  // namespace browser_core
