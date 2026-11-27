#include "browser_core/held_page.h"

#include <cstdint>
#include <utility>

#include "base/containers/span.h"
#include "base/functional/bind.h"
#include "base/location.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/held_page_rule.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "components/autofill/content/browser/content_autofill_driver.h"
#include "components/autofill/core/browser/autofill_field.h"
#include "components/autofill/core/browser/form_structure.h"
#include "components/autofill/core/browser/foundations/autofill_driver.h"
#include "components/autofill/core/browser/foundations/autofill_manager.h"
#include "components/autofill/core/common/form_data.h"
#include "components/autofill/core/common/form_field_data.h"
#include "components/autofill/core/common/html_field_types.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "ui/accessibility/ax_mode.h"
#include "ui/accessibility/ax_tree_update.h"
#include "url/gurl.h"

namespace browser_core::held_page {

namespace {

using held_page_rule::Facts;
using held_page_rule::Field;

// As page_text.cc's snapshot: a page's frames and their names are far fewer.
constexpr size_t kMaxNodes = 20000;
constexpr base::TimeDelta kSnapshotTimeout = base::Seconds(5);

Field FieldOf(const autofill::FormFieldData& field, bool one_time_code) {
  return {
      .password = field.form_control_type() == autofill::FormControlType::kInputPassword,
      .focusable = field.is_focusable(),
      .one_time_code = one_time_code,
  };
}

// As the password manager's renderer half parsed the field's autocomplete.
bool ParsedOneTimeCode(const autofill::FormFieldData& field) {
  return field.parsed_autocomplete() &&
         field.parsed_autocomplete()->field_type == autofill::HtmlFieldType::kOneTimeCode;
}

// Bound to the tab's id, which a closing tab's watch doesn't outlive.
void Tell(bc_id tab, bc_held_page page) {
  if (Host().held_page) {
    Host().held_page(Host().context, tab, &page);
  }
}

// A held tab's pages, from the hold until the core lets it go. Each page it commits after the hold
// is reported once its main frame loaded and the password manager saw its forms rendered, and
// again once a later form or frame turns a fact true.
class Watcher : public content::WebContentsUserData<Watcher>,
                public content::WebContentsObserver,
                public autofill::AutofillManager::Observer {
 public:
  Watcher(const Watcher&) = delete;
  Watcher& operator=(const Watcher&) = delete;
  ~Watcher() override { StopAutofill(); }

  void Rendered(content::RenderFrameHost* frame, const std::vector<autofill::FormData>& forms) {
    // The page the hold began on is the agent's step, never reported.
    if (!document_ || frame->GetOutermostMainFrame() != web_contents()->GetPrimaryMainFrame()) {
      return;
    }
    std::vector<Field> fields;
    for (const autofill::FormData& form : forms) {
      for (const autofill::FormFieldData& field : form.fields()) {
        fields.push_back(FieldOf(field, ParsedOneTimeCode(field)));
      }
    }
    Learn(held_page_rule::FactsOf(fields));
    if (frame->IsInPrimaryMainFrame()) {
      // Autofill parsed the forms by now or soon after, and says so in OnAfterFormsSeen.
      WatchAutofill(frame);
      Snapshot();
    }
  }

 private:
  friend class content::WebContentsUserData<Watcher>;

  explicit Watcher(content::WebContents* contents)
      : content::WebContentsUserData<Watcher>(*contents),
        content::WebContentsObserver(contents) {}

  // What the page turned true, and a report if it was told of before and this is new.
  void Learn(const Facts& more) {
    if (held_page_rule::Add(facts_, more) && reported_) {
      Report();
    }
  }

  void Report() {
    const Tab* tab = Tab::FromWebContents(web_contents());
    if (!tab) {
      return;
    }
    reported_ = true;
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Tell, tab->id(),
                                  bc_held_page{.document = document_,
                                               .password = facts_.password,
                                               .one_time_code = facts_.one_time_code,
                                               .captcha = facts_.captcha}));
  }

  // The names the page's frames have, for its CAPTCHA, in one snapshot at a time: one asked for
  // while another is out is taken once that answers.
  void Snapshot() {
    if (snapshotting_) {
      again_ = true;
      return;
    }
    snapshotting_ = true;
    web_contents()->RequestAXTreeSnapshot(
        base::BindOnce(&Watcher::Snapshotted, weak_factory_.GetWeakPtr(), document_),
        ui::kAXModeWebContentsOnly, kMaxNodes, kSnapshotTimeout,
        content::WebContents::AXTreeSnapshotPolicy::kAll);
  }

  void Snapshotted(uint64_t document, ui::AXTreeUpdate& update) {
    snapshotting_ = false;
    // One of the page before tells nothing of this one, which may have asked for its own since.
    if (document == document_) {
      Learn({.captcha = held_page_rule::CaptchaIn(update, BC_CAPTCHA_WORDS)});
      // The first report waits for the snapshot, so it says all three.
      if (!reported_) {
        Report();
      }
    }
    if (std::exchange(again_, false) && !(reported_ && facts_.captcha)) {
      Snapshot();
    }
  }

  void WatchAutofill(content::RenderFrameHost* frame) {
    // As the base class, where GetAutofillManager is public; ContentAutofillDriver hides it.
    autofill::AutofillDriver* driver =
        autofill::ContentAutofillDriver::GetForRenderFrameHost(frame);
    if (!driver || manager_ == &driver->GetAutofillManager()) {
      return;
    }
    StopAutofill();
    manager_ = &driver->GetAutofillManager();
    manager_->AddObserver(this);
    LearnAutofill(*manager_);
  }

  void StopAutofill() {
    if (manager_) {
      manager_->RemoveObserver(this);
      manager_ = nullptr;
    }
  }

  // Autofill's forms take in the fields of the page's frames.
  void LearnAutofill(const autofill::AutofillManager& manager) {
    std::vector<Field> fields;
    manager.ForEachCachedForm([&](const autofill::FormStructure& form) {
      for (const auto& field : form.fields()) {
        fields.push_back(
            FieldOf(*field, field->html_type() == autofill::HtmlFieldType::kOneTimeCode));
      }
    });
    Learn(held_page_rule::FactsOf(fields));
  }

  // autofill::AutofillManager::Observer:
  void OnAfterFormsSeen(autofill::AutofillManager& manager,
                        base::span<const autofill::FormGlobalId> updated_forms,
                        base::span<const autofill::FormGlobalId> removed_forms) override {
    LearnAutofill(manager);
  }

  void OnAutofillManagerStateChanged(autofill::AutofillManager& manager,
                                     autofill::AutofillManager::LifecycleState previous,
                                     autofill::AutofillManager::LifecycleState new_state) override {
    if (new_state == autofill::AutofillManager::LifecycleState::kPendingDeletion) {
      StopAutofill();
    }
  }

  // content::WebContentsObserver:
  void PrimaryPageChanged(content::Page& page) override {
    StopAutofill();
    ++document_;
    facts_ = Facts();
    reported_ = false;
    again_ = false;
  }

  // A frame that loads after the page was reported may be a CAPTCHA's.
  void DidFinishLoad(content::RenderFrameHost* frame, const GURL& url) override {
    if (reported_ && !facts_.captcha && !frame->IsInPrimaryMainFrame() &&
        frame->GetOutermostMainFrame() == web_contents()->GetPrimaryMainFrame()) {
      Snapshot();
    }
  }

  // The pages the tab committed since the hold: 0 is the one it began on.
  uint64_t document_ = 0;
  Facts facts_;
  bool reported_ = false;
  bool snapshotting_ = false;
  bool again_ = false;
  raw_ptr<autofill::AutofillManager> manager_ = nullptr;
  base::WeakPtrFactory<Watcher> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(Watcher);

}  // namespace

void Watch(bc_id tab) {
  if (content::WebContents* contents = Tab::FromId(tab)) {
    Watcher::CreateForWebContents(contents);
  }
}

void Unwatch(bc_id tab) {
  if (content::WebContents* contents = Tab::FromId(tab)) {
    contents->RemoveUserData(Watcher::UserDataKey());
  }
}

void FormsRendered(content::RenderFrameHost* frame, const std::vector<autofill::FormData>& forms) {
  auto* contents = content::WebContents::FromRenderFrameHost(frame);
  if (Watcher* watcher = contents ? Watcher::FromWebContents(contents) : nullptr) {
    watcher->Rendered(frame, forms);
  }
}

}  // namespace browser_core::held_page
