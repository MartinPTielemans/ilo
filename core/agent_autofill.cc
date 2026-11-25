#include "browser_core/agent_autofill.h"

#include <string>
#include <vector>

#include "base/containers/span.h"
#include "base/functional/callback_helpers.h"
#include "base/memory/raw_ptr.h"
#include "browser_core/agent_session.h"
#include "browser_core/agent_space_page.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "components/autofill/content/browser/content_autofill_driver.h"
#include "components/autofill/core/browser/form_structure.h"
#include "components/autofill/core/browser/foundations/autofill_driver.h"
#include "components/autofill/core/browser/foundations/autofill_manager.h"
#include "components/autofill/core/common/form_data.h"
#include "components/password_manager/content/browser/content_password_manager_driver.h"
#include "components/password_manager/core/browser/password_manager_interface.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"

namespace browser_core {

namespace {

bool IsAgentTab(bc_id tab) {
  return AgentSpaceOf(tab) && !IsHeld(tab);
}

// Gives the password manager the main frame's forms again, which it dropped while the tab was the
// agent's. The renderer sends them only as the page loads or they change, so this asks autofill's
// agent to send them all, and hands the main frame's to the password manager as its own agent
// would have. A sign-in form in a frame of another site fills once the page loads again.
class Refill : public content::WebContentsUserData<Refill>,
               public content::WebContentsObserver,
               public autofill::AutofillManager::Observer {
 public:
  Refill(const Refill&) = delete;
  Refill& operator=(const Refill&) = delete;
  ~Refill() override { Stop(); }

  void Start() {
    Stop();
    content::RenderFrameHost* frame = web_contents()->GetPrimaryMainFrame();
    // As the base class, where the calls below are public; ContentAutofillDriver hides them.
    autofill::AutofillDriver* driver =
        autofill::ContentAutofillDriver::GetForRenderFrameHost(frame);
    if (!driver) {
      return;
    }
    manager_ = &driver->GetAutofillManager();
    manager_->AddObserver(this);
    // Cleared first, or the renderer sends only the forms that changed.
    driver->ClearFormCacheInAllFrames();
    driver->TriggerFormExtractionInAllFrames(base::DoNothing());
  }

 private:
  friend class content::WebContentsUserData<Refill>;

  explicit Refill(content::WebContents* contents)
      : content::WebContentsUserData<Refill>(*contents),
        content::WebContentsObserver(contents) {}

  void Stop() {
    if (manager_) {
      manager_->RemoveObserver(this);
      manager_ = nullptr;
    }
  }

  // autofill::AutofillManager::Observer:
  void OnAfterFormsSeen(autofill::AutofillManager& manager,
                        base::span<const autofill::FormGlobalId> updated_forms,
                        base::span<const autofill::FormGlobalId> removed_forms) override {
    Stop();
    content::RenderFrameHost* frame = web_contents()->GetPrimaryMainFrame();
    const autofill::LocalFrameToken main(frame->GetFrameToken().value());
    std::vector<autofill::FormData> forms;
    for (const autofill::FormGlobalId& id : updated_forms) {
      const autofill::FormStructure* form = manager.FindCachedFormById(id);
      if (!form || id.frame_token != main) {
        continue;
      }
      // Autofill's form takes in its frames' fields; the password manager's is one frame's.
      autofill::FormData data = form->ToFormData();
      std::vector<autofill::FormFieldData> fields = data.fields();
      std::erase_if(fields, [&](const auto& field) { return field.host_frame() != main; });
      data.set_fields(std::move(fields));
      forms.push_back(std::move(data));
    }
    auto* driver = password_manager::ContentPasswordManagerDriver::GetForRenderFrameHost(frame);
    if (driver && driver->GetPasswordManager() && !forms.empty()) {
      driver->GetPasswordManager()->OnPasswordFormsParsed(driver, forms);
    }
  }

  void OnAutofillManagerStateChanged(autofill::AutofillManager& manager,
                                     autofill::AutofillManager::LifecycleState previous,
                                     autofill::AutofillManager::LifecycleState new_state) override {
    if (new_state == autofill::AutofillManager::LifecycleState::kPendingDeletion) {
      Stop();
    }
  }

  // content::WebContentsObserver: the new page's own load fills it.
  void PrimaryPageChanged(content::Page& page) override { Stop(); }

  raw_ptr<autofill::AutofillManager> manager_ = nullptr;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(Refill);

}  // namespace

bool IsAgentTab(content::WebContents* contents) {
  const Tab* tab = contents ? Tab::FromWebContents(contents) : nullptr;
  return tab && IsAgentTab(tab->id());
}

bool RefuseAgentPasskey(content::WebContents* contents, bool asks) {
  const Tab* tab = contents ? Tab::FromWebContents(contents) : nullptr;
  if (!tab || !IsAgentTab(tab->id())) {
    return false;
  }
  // One that asks nobody, such as a page making a passkey for the login it just filled, is
  // cancelled: no passkey of the user's comes or goes from an agent's work.
  if (!asks) {
    return true;
  }
  const bc_id id = tab->id();
  const std::string agent = AgentSpaceOf(id)->agent()->id;
  Hold(id);
  // With no session there, the app hears from here that the tab is the user's.
  if (!EndTabSession(id, BC_AGENT_SESSION_PASSKEY) && Host().agent_session_ended) {
    Host().agent_session_ended(Host().context, agent.c_str(), id, BC_AGENT_SESSION_PASSKEY);
  }
  return false;
}

void FillForUser(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  // One with no page fills as its page loads.
  if (!contents || IsAgentTab(tab)) {
    return;
  }
  Refill::CreateForWebContents(contents);
  Refill::FromWebContents(contents)->Start();
}

}  // namespace browser_core
