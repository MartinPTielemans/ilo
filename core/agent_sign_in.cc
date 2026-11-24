#include "browser_core/agent_sign_in.h"

#include <algorithm>
#include <optional>
#include <set>
#include <string_view>
#include <utility>
#include <vector>

#include "base/barrier_closure.h"
#include "base/containers/flat_set.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/functional/callback_helpers.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/strings/escape.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/types/expected.h"
#include "base/values.h"
#include "browser_core/agent_page.h"
#include "browser_core/agent_session.h"
#include "browser_core/agent_sign_in_rule.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/password_manager/chrome_password_manager_client.h"
#include "components/affiliations/core/browser/match_type.h"
#include "components/autofill/content/browser/content_autofill_driver.h"
#include "components/autofill/core/browser/form_structure.h"
#include "components/autofill/core/browser/foundations/autofill_driver.h"
#include "components/autofill/core/browser/foundations/autofill_manager.h"
#include "components/autofill/core/common/form_data.h"
#include "components/autofill/core/common/form_field_data.h"
#include "components/autofill/core/common/unique_ids.h"
#include "components/device_reauth/device_auth_availability_checker.h"
#include "components/password_manager/core/browser/password_manager_client.h"
#include "components/password_manager/content/browser/content_password_manager_driver.h"
#include "components/password_manager/core/browser/form_parsing/form_data_parser.h"
#include "components/password_manager/core/browser/password_form.h"
#include "components/password_manager/core/browser/password_form_digest.h"
#include "components/password_manager/core/browser/password_store/password_store_consumer.h"
#include "components/password_manager/core/browser/password_store/password_store_interface.h"
#include "components/password_manager/core/browser/password_store/stored_credential.h"
#include "content/public/browser/back_forward_cache.h"
#include "content/public/browser/global_routing_id.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "mojo/public/cpp/bindings/callback_helpers.h"
#include "url/origin.h"
#include "url/url_constants.h"

namespace browser_core {

namespace {

using autofill::FieldRendererId;
using password_manager::PasswordForm;
using password_manager::StoredCredential;

// What the agent hears. None says which account, how many there are or whether there were any,
// and "submitted" alone is never one of them (agent_sign_in_done).
constexpr char kGone[] =
    "The sign-in form was submitted and is gone. Read the page to see where you are.";
constexpr char kNotSignedIn[] =
    "ilo didn't sign this page in. Read the page to see where it is, or ask the user.";
constexpr char kUserStep[] =
    "Signing in on this page is the user's step. Ask them, or wait until they hand the tab back.";
constexpr char kTakenBack[] = "The user took the tab back, so ilo stopped signing it in.";
constexpr char kEnded[] = "The agent's session with the tab ended, so ilo stopped signing it in.";
constexpr char kSigningIn[] = "ilo is signing this tab in already. Wait for its answer.";
// The back-forward cache's reason for a document a sign-in filled. None of Chrome's embedder
// reasons (components/back_forward_cache/disabled_reason_id.h) covers it, and DevTools' report of
// why a page wasn't restored reads every id from that list, with no case for another, so it's the
// list's kUnknown, 0, with a description of ours.
constexpr content::BackForwardCache::DisabledReasonType kNoBackForwardCache = 0;
// How long the look waits for a navigation the submit started, as AgentPage and PageText wait for a
// page to load.
constexpr base::TimeDelta kSettleBound = base::Seconds(15);

// The main frame's forms as autofill last saw them, each with its fields in the main frame alone:
// autofill's form takes in its frames' fields, and nothing is filled in a frame.
std::vector<autofill::FormData> MainFrameForms(content::WebContents& contents) {
  std::vector<autofill::FormData> forms;
  content::RenderFrameHost* frame = contents.GetPrimaryMainFrame();
  autofill::AutofillDriver* driver = autofill::ContentAutofillDriver::GetForRenderFrameHost(frame);
  if (!driver) {
    return forms;
  }
  const autofill::LocalFrameToken main(frame->GetFrameToken().value());
  driver->GetAutofillManager().ForEachCachedForm([&](const autofill::FormStructure& form) {
    if (form.global_id().frame_token != main) {
      return;
    }
    autofill::FormData data = form.ToFormData();
    std::vector<autofill::FormFieldData> fields = data.fields();
    std::erase_if(fields, [&](const auto& field) { return field.host_frame() != main; });
    data.set_fields(std::move(fields));
    forms.push_back(std::move(data));
  });
  return forms;
}

// Has the renderer send the forms of every frame in |contents| again, then runs |then|, even if
// the page or its renderer went meanwhile.
void Extract(content::WebContents& contents, base::OnceClosure then) {
  autofill::AutofillDriver* driver =
      autofill::ContentAutofillDriver::GetForRenderFrameHost(contents.GetPrimaryMainFrame());
  if (!driver) {
    std::move(then).Run();
    return;
  }
  driver->TriggerFormExtractionInAllFrames(mojo::WrapCallbackWithDefaultInvokeIfNotRun(
      base::BindOnce([](base::OnceClosure then, bool) { std::move(then).Run(); }, std::move(then)),
      false));
}

// Empties each of |fields| in |frame|, then runs |then|, whether or not each could be.
void Empty(content::GlobalRenderFrameHostId frame,
           const std::vector<FieldRendererId>& fields,
           base::OnceClosure then) {
  content::RenderFrameHost* host = content::RenderFrameHost::FromID(frame);
  auto* driver =
      host ? password_manager::ContentPasswordManagerDriver::GetForRenderFrameHost(host) : nullptr;
  if (!driver || fields.empty()) {
    std::move(then).Run();
    return;
  }
  base::RepeatingClosure each = base::BarrierClosure(fields.size(), std::move(then));
  for (const FieldRendererId field : fields) {
    driver->FillField(
        field, std::u16string(), autofill::FieldPropertiesFlags::kNoFlags,
        mojo::WrapCallbackWithDefaultInvokeIfNotRun(
            base::BindOnce([](base::RepeatingClosure each, bool) { each.Run(); }, each), false));
  }
}

FormField::Kind KindOf(const autofill::FormFieldData& field) {
  if (field.IsPasswordInputElement()) {
    return FormField::Kind::kPassword;
  }
  if (field.form_control_type() == autofill::FormControlType::kInputEmail) {
    return FormField::Kind::kEmail;
  }
  return field.IsTextInputElement() ? FormField::Kind::kText : FormField::Kind::kOther;
}

const autofill::FormFieldData* FieldOf(const autofill::FormData& form, FieldRendererId id) {
  for (const autofill::FormFieldData& field : form.fields()) {
    if (field.renderer_id() == id) {
      return &field;
    }
  }
  return nullptr;
}

bool Shows(const base::Value& value, std::u16string_view password);

bool ShowsIn(const base::ListValue& list, std::u16string_view password) {
  return std::ranges::any_of(list,
                             [&](const base::Value& each) { return Shows(each, password); });
}

// Whether |value|, all an agent could read of a page or any part of it, shows |password|.
bool Shows(const base::Value& value, std::u16string_view password) {
  if (value.is_string()) {
    return ShowsPassword(base::UTF8ToUTF16(value.GetString()), password);
  }
  if (value.is_list()) {
    return ShowsIn(value.GetList(), password);
  }
  if (value.is_dict()) {
    for (const auto entry : value.GetDict()) {
      if (Shows(entry.second, password)) {
        return true;
      }
    }
  }
  return false;
}

// Whether |url|, which an agent reads of a tab or its frames, shows |password|, as it is or
// unescaped, as a form sent with GET puts its fields there.
bool UrlShows(const GURL& url, std::u16string_view password) {
  const std::string& spec = url.possibly_invalid_spec();
  return ShowsPassword(base::UTF8ToUTF16(spec), password) ||
         ShowsPassword(base::UTF8ToUTF16(base::UnescapeURLComponent(
                           spec, base::UnescapeRule::SPACES | base::UnescapeRule::PATH_SEPARATORS |
                                     base::UnescapeRule::URL_SPECIAL_CHARS_EXCEPT_PATH_SEPARATORS |
                                     base::UnescapeRule::REPLACE_PLUS_WITH_SPACE)),
                       password);
}

// A form step: the fields it fills and the selector of the one Enter goes into, the password's,
// or the username's on a username-first sign-in's first page. Or the user's, for a form that would
// send the login off the page's site.
struct FormStep {
  FieldRendererId username;
  FieldRendererId password;
  std::string selector;
  bool elsewhere = false;
};

// The step the main frame's forms allow: a login form's, with a current password field that's
// visible and editable and no new password, or else a username-only form's. None if neither.
std::optional<FormStep> StepOf(const std::vector<autofill::FormData>& forms,
                               const std::u16string& username,
                               const url::Origin& origin) {
  password_manager::FormDataParser parser;
  for (const autofill::FormData& form : forms) {
    std::unique_ptr<PasswordForm> parsed = parser.Parse(
        form, password_manager::FormDataParser::Mode::kFilling, {username}, std::nullopt);
    if (!parsed || !parsed->HasPasswordElement() || parsed->HasNewPasswordElement()) {
      continue;
    }
    const autofill::FormFieldData* password = FieldOf(form, parsed->password_element_renderer_id);
    if (!password || !password->is_visible() || password->is_readonly()) {
      continue;
    }
    std::optional<std::string> selector =
        FieldSelector(true, base::UTF16ToUTF8(password->id_attribute()),
                      base::UTF16ToUTF8(password->name_attribute()));
    if (!selector) {
      continue;
    }
    if (!PostsWithin(form.action(), form.is_action_empty(), origin)) {
      return FormStep{.elsewhere = true};
    }
    const autofill::FormFieldData* name = FieldOf(form, parsed->username_element_renderer_id);
    return FormStep{name && name->is_visible() && !name->is_readonly() ? name->renderer_id()
                                                                       : FieldRendererId(),
                    password->renderer_id(), *std::move(selector)};
  }
  for (const autofill::FormData& form : forms) {
    std::vector<FormField> fields;
    for (const autofill::FormFieldData& field : form.fields()) {
      fields.push_back({KindOf(field), field.autocomplete_attribute(), field.is_visible(),
                        field.is_readonly()});
    }
    const std::optional<size_t> only = UsernameOnlyField(fields);
    if (!only) {
      continue;
    }
    const autofill::FormFieldData& field = form.fields()[*only];
    std::optional<std::string> selector =
        FieldSelector(false, base::UTF16ToUTF8(field.id_attribute()),
                      base::UTF16ToUTF8(field.name_attribute()));
    if (selector) {
      if (!PostsWithin(form.action(), form.is_action_empty(), origin)) {
        return FormStep{.elsewhere = true};
      }
      return FormStep{field.renderer_id(), FieldRendererId(), *std::move(selector)};
    }
  }
  return std::nullopt;
}

// Where each agent's sign-in submitted a login and didn't sign in, for this run of the browser.
SignInTries& Tries() {
  static base::NoDestructor<SignInTries> tries;
  return *tries;
}

// A tab whose pages a sign-in withheld as they showed what was filled (WithheldHistory), for the
// tab's life: while one is shown, agents get neither its title nor its address, and may only take
// the tab to another page or close it.
class WithheldPage : public content::WebContentsUserData<WithheldPage>,
                     public content::WebContentsObserver {
 public:
  WithheldPage(const WithheldPage&) = delete;
  WithheldPage& operator=(const WithheldPage&) = delete;
  ~WithheldPage() override = default;

  // The page |contents| shows now is withheld, and what each of |pending| commits.
  static void Withhold(content::WebContents& contents, const base::flat_set<int64_t>& pending) {
    CreateForWebContents(&contents);
    FromWebContents(&contents)->history_.Withhold(
        contents.GetPrimaryMainFrame()->GetLastCommittedURL(),
        std::set<int64_t>(pending.begin(), pending.end()));
  }

  bool Shows(const GURL& now) const { return history_.Shows(now); }

  // content::WebContentsObserver:
  void DidFinishNavigation(content::NavigationHandle* navigation) override {
    if (!navigation->IsInPrimaryMainFrame()) {
      return;
    }
    if (!navigation->HasCommitted()) {
      history_.Ended(navigation->GetNavigationId());
      return;
    }
    history_.Committed(navigation->GetNavigationId(), navigation->GetURL(),
                       navigation->IsSameDocument());
  }

 private:
  friend class content::WebContentsUserData<WithheldPage>;

  explicit WithheldPage(content::WebContents* contents)
      : content::WebContentsUserData<WithheldPage>(*contents),
        content::WebContentsObserver(contents) {}

  WithheldHistory history_;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(WithheldPage);

class SignIn : public content::WebContentsUserData<SignIn>,
               public content::WebContentsObserver,
               public password_manager::PasswordStoreConsumer {
 public:
  SignIn(const SignIn&) = delete;
  SignIn& operator=(const SignIn&) = delete;
  ~SignIn() override = default;

  void Start() {
    content::WebContents& contents = GetWebContents();
    if (origin_.opaque() || origin_.scheme() != url::kHttpsScheme) {
      Finish(false, BC_AGENT_FAILURE_NONE, kNotSignedIn);
      return;
    }
    AgentPage::CreateForWebContents(&contents);
    AgentPage::FromWebContents(&contents)->WhenIdle(
        base::BindOnce(&SignIn::Idle, weak_factory_.GetWeakPtr()));
  }

  bool Choose(uint64_t request, size_t account) {
    if (!choosing_ || request != request_) {
      return false;
    }
    choosing_ = false;
    if (account >= usable_.size()) {
      Finish(false, BC_AGENT_FAILURE_USER_STEP, kUserStep);
      return true;
    }
    Chosen(usable_[account]);
    return true;
  }

  void Stop(bc_agent_failure failure) {
    // Nothing waits for the emptying: the page is the user's now, or nobody's.
    Empty(frame_, filled_, base::DoNothing());
    Finish(false, failure, failure == BC_AGENT_FAILURE_HELD ? kTakenBack : kEnded);
  }

 private:
  friend class content::WebContentsUserData<SignIn>;

  SignIn(content::WebContents* contents, std::string agent, uint64_t request)
      : content::WebContentsUserData<SignIn>(*contents),
        content::WebContentsObserver(contents),
        agent_(std::move(agent)),
        request_(request),
        tab_(Tab::IdOf(contents)),
        origin_(contents->GetPrimaryMainFrame()->GetLastCommittedOrigin()) {}

  bool SameOrigin() {
    return GetWebContents().GetPrimaryMainFrame()->GetLastCommittedOrigin() == origin_;
  }

  // An action under way has answered: the logins saved for the page's origin, from both stores.
  void Idle() {
    auto* client = ChromePasswordManagerClient::FromWebContents(&GetWebContents());
    if (!client || !SameOrigin()) {
      Finish(false, BC_AGENT_FAILURE_NONE, kNotSignedIn);
      return;
    }
    std::vector<password_manager::PasswordStoreInterface*> stores;
    for (password_manager::PasswordStoreInterface* store :
         {client->GetAccountPasswordStore(), client->GetProfilePasswordStore()}) {
      if (store) {
        stores.push_back(store);
      }
    }
    if (stores.empty()) {
      Finish(false, BC_AGENT_FAILURE_NONE, kNotSignedIn);
      return;
    }
    stores_ = stores.size();
    const password_manager::PasswordFormDigest digest(PasswordForm::Scheme::kHtml,
                                                      origin_.Serialize() + "/", origin_.GetURL());
    for (password_manager::PasswordStoreInterface* store : stores) {
      store->GetLogins(digest, weak_factory_.GetWeakPtr());
    }
  }

  // password_manager::PasswordStoreConsumer:
  void OnGetPasswordStoreResultsOrErrorFrom(
      password_manager::PasswordStoreInterface* store,
      base::expected<std::vector<StoredCredential>, password_manager::PasswordStoreBackendError>
          results) override {
    if (results.has_value()) {
      for (StoredCredential& login : results.value()) {
        logins_.push_back(std::move(login));
      }
    }
    if (--stores_ > 0) {
      return;
    }
    std::vector<SavedLogin> saved;
    for (const StoredCredential& login : logins_) {
      saved.push_back({
          .exact = login.match_type == affiliations::MatchType::kExact,
          .blocked = login.blocked_by_user,
          .federated = login.federation_origin.IsValid(),
          .html = login.scheme == PasswordForm::Scheme::kHtml,
          .has_password = !login.password_value.value().empty(),
          .account = login.in_store == PasswordForm::Store::kAccountStore,
          .realm = login.signon_realm,
          .username = login.username_value,
      });
    }
    usable_ = UsableLogins(saved, origin_);
    if (usable_.empty()) {
      Finish(false, BC_AGENT_FAILURE_NONE, kNotSignedIn);
      return;
    }
    if (usable_.size() == 1) {
      Chosen(usable_.front());
      return;
    }
    // With no host to ask the user, the pick is still theirs, never a wait with no end.
    if (!Host().sign_in_choose) {
      Finish(false, BC_AGENT_FAILURE_USER_STEP, kUserStep);
      return;
    }
    // More than one is the user's pick, in an ilo line, with no timer.
    choosing_ = true;
    std::vector<std::string> accounts;
    for (const size_t index : usable_) {
      accounts.push_back(base::UTF16ToUTF8(logins_[index].username_value));
    }
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(
            [](bc_id tab, uint64_t request, std::string site, std::vector<std::string> accounts) {
              if (!Host().sign_in_choose) {
                return;
              }
              std::vector<const char*> names;
              for (const std::string& account : accounts) {
                names.push_back(account.c_str());
              }
              Host().sign_in_choose(Host().context, tab, request, site.c_str(), names.data(),
                                    names.size());
            },
            tab_, request_, origin_.host(), std::move(accounts)));
  }

  void Chosen(size_t index) {
    // The others' passwords leave memory now.
    login_ = std::move(logins_[index]);
    logins_.clear();
    usable_.clear();
    // A login Chrome fills only after the user's own check is theirs, never asked for here.
    auto* client = ChromePasswordManagerClient::FromWebContents(&GetWebContents());
    if (!client) {
      Finish(false, BC_AGENT_FAILURE_NONE, kNotSignedIn);
      return;
    }
    std::unique_ptr<device_reauth::DeviceAuthAvailabilityChecker> checker =
        client->GetDeviceAuthAvailabilityChecker();
    if (checker && client->IsReauthBeforeFillingRequired(checker.get())) {
      Finish(false, BC_AGENT_FAILURE_USER_STEP, kUserStep);
      return;
    }
    Extract(GetWebContents(),
            base::BindOnce(&SignIn::NextStep, weak_factory_.GetWeakPtr(), /*first=*/true));
  }

  // The form step on the page now: the first, or the password after a username alone, on the same
  // origin and in the same call. Its field Enter goes into must be one the main frame's document
  // finds, outside any shadow root, before anything is filled.
  void NextStep(bool first) {
    std::optional<FormStep> step;
    if (SameOrigin()) {
      step = StepOf(MainFrameForms(GetWebContents()), login_->username_value, origin_);
    }
    if (step && step->elsewhere) {
      Elsewhere(first);
      return;
    }
    // After a username alone, only the password is filled, once.
    if (!first && step && step->password.is_null()) {
      step.reset();
    }
    if (!step) {
      if (first) {
        Finish(false, BC_AGENT_FAILURE_NONE, kNotSignedIn);
      } else {
        Settled();
      }
      return;
    }
    frame_ = GetWebContents().GetPrimaryMainFrame()->GetGlobalId();
    // Any control in the document that sends a form elsewhere, as a button with a formaction, makes
    // the step the user's before anything is filled: a page can tie any button to any form.
    AgentPage::FromWebContents(&GetWebContents())
        ->CheckSubmitters(base::BindOnce(&SignIn::Submitters, weak_factory_.GetWeakPtr(),
                                         std::move(*step), first));
  }

  void Submitters(FormStep step, bool first, bool clear) {
    if (!clear) {
      Elsewhere(first);
      return;
    }
    AgentPage* page = AgentPage::FromWebContents(&GetWebContents());
    std::string selector = step.selector;
    page->FindField(std::move(selector), /*submit=*/false,
                    base::BindOnce(&SignIn::Found, weak_factory_.GetWeakPtr(), std::move(step)));
  }

  // The step sends its form off the site: the user's, before anything is filled, or after a
  // username alone, once what was filled is emptied.
  void Elsewhere(bool first) {
    if (first && filled_.empty()) {
      Finish(false, BC_AGENT_FAILURE_USER_STEP, kUserStep);
      return;
    }
    user_step_ = true;
    Settled();
  }

  void Found(FormStep step, bool found) {
    if (!found) {
      // The first finds nothing filled yet; a later one has what the first filled to empty.
      if (filled_.empty()) {
        Finish(false, BC_AGENT_FAILURE_NONE, kNotSignedIn);
      } else {
        Settled();
      }
      return;
    }
    std::vector<std::pair<FieldRendererId, bool>> fields;
    if (!step.username.is_null() && !login_->username_value.empty()) {
      fields.emplace_back(step.username, false);
    }
    if (!step.password.is_null()) {
      fields.emplace_back(step.password, true);
    }
    Fill(std::move(fields), 0, std::move(step));
  }

  // Fills |fields| from |next| on, each as Chrome fills a saved login for the user, then submits.
  void Fill(std::vector<std::pair<FieldRendererId, bool>> fields, size_t next, FormStep step) {
    // The page may have gone on since the last look: each fill, and the Enter, only into the main
    // frame's document at the login's origin still.
    content::RenderFrameHost* host = content::RenderFrameHost::FromID(frame_);
    if (!host || host != GetWebContents().GetPrimaryMainFrame() ||
        host->GetLastCommittedOrigin() != origin_) {
      Settled();
      return;
    }
    if (next == fields.size()) {
      Submit(std::move(step));
      return;
    }
    auto* driver = password_manager::ContentPasswordManagerDriver::GetForRenderFrameHost(host);
    if (!driver) {
      Settled();
      return;
    }
    if (next == 0) {
      // The document never goes into the back-forward cache, where a page kept with the filled
      // fields would come back as it was, past the emptying. Its report string, which pages read
      // through the NotRestoredReasons API, says nothing.
      content::BackForwardCache::DisableForRenderFrameHost(
          host, content::BackForwardCache::DisabledReason(
                    content::BackForwardCache::DisabledSource::kEmbedder, kNoBackForwardCache,
                    "ilo filled a saved login in this document", /*context=*/"",
                    /*report_string=*/"masked"));
    }
    const auto [field, password] = fields[next];
    // Kept before it's filled, so a fill that lands but never answers is emptied too.
    filled_.push_back(field);
    driver->FillField(
        field, password ? login_->password_value.value() : login_->username_value,
        autofill::FieldPropertiesFlags::kAutofilledOnUserTrigger,
        mojo::WrapCallbackWithDefaultInvokeIfNotRun(
            base::BindOnce(
                [](base::WeakPtr<SignIn> sign_in,
                   std::vector<std::pair<FieldRendererId, bool>> fields, size_t next,
                   FormStep step, bool filled) {
                  if (!sign_in) {
                    return;
                  }
                  if (!filled) {
                    sign_in->Settled();
                    return;
                  }
                  sign_in->Fill(std::move(fields), next + 1, std::move(step));
                },
                weak_factory_.GetWeakPtr(), std::move(fields), next, std::move(step)),
            false));
  }

  // Enter in the field filled last, once a form and never again in the call, if the document still
  // has no control sending a form elsewhere: a script may have added one since the fill.
  void Submit(FormStep step) {
    AgentPage::FromWebContents(&GetWebContents())
        ->CheckSubmitters(base::BindOnce(&SignIn::SubmitIfClear, weak_factory_.GetWeakPtr(),
                                         std::move(step)));
  }

  void SubmitIfClear(FormStep step, bool clear) {
    if (!clear) {
      Elsewhere(/*first=*/false);
      return;
    }
    const bool password = !step.password.is_null();
    AgentPage::FromWebContents(&GetWebContents())
        ->FindField(std::move(step.selector), /*submit=*/true,
                    base::BindOnce(
                        [](base::WeakPtr<SignIn> sign_in, bool password, bool pressed) {
                          if (!sign_in) {
                            return;
                          }
                          sign_in->submitted_ |= pressed;
                          sign_in->password_submitted_ |= pressed && password;
                          if (!pressed) {
                            sign_in->Settled();
                            return;
                          }
                          // A username alone: the password's page comes next.
                          Extract(sign_in->GetWebContents(),
                                  base::BindOnce(password ? &SignIn::Settled
                                                          : &SignIn::PasswordStep,
                                                 sign_in));
                        },
                        weak_factory_.GetWeakPtr(), password));
  }

  void PasswordStep() { NextStep(/*first=*/false); }

  // The page has settled: whether the form has gone, then each field filled that's still in the
  // page is emptied, whether or not it has, before anything an agent could read is looked at. A
  // navigation the Enter started that the settling gave up on is waited for, to its commit or its
  // failure, so the look reads the address a form sent with GET put the password in.
  // Only those under way then: a frame that keeps navigating never keeps the tab held.
  void Settled() {
    if (waiting_) {
      return;
    }
    if (!navigations_.empty()) {
      waiting_ = true;
      awaited_ = navigations_;
      // Bounded as the core's other waits on a page: one that hasn't ended by then counts as
      // showing the password, and the tab is the user's.
      settle_bound_.Start(FROM_HERE, kSettleBound,
                          base::BindOnce(&SignIn::NeverSettled, base::Unretained(this)));
      return;
    }
    Look();
  }

  void NeverSettled() {
    // Nothing waits for the emptying: the tab is held from agents in this task.
    Empty(frame_, filled_, base::DoNothing());
    Withhold();
  }

  void Look() {
    Extract(GetWebContents(), base::BindOnce(&SignIn::Emptying, weak_factory_.GetWeakPtr()));
  }

  // content::WebContentsObserver: the navigations of the tab's frames under way, but in the page.
  void DidStartNavigation(content::NavigationHandle* navigation) override {
    if (!navigation->IsSameDocument() &&
        (navigation->IsInPrimaryMainFrame() || !navigation->IsInMainFrame())) {
      navigations_.insert(navigation->GetNavigationId());
    }
  }

  void DidFinishNavigation(content::NavigationHandle* navigation) override {
    navigations_.erase(navigation->GetNavigationId());
    awaited_.erase(navigation->GetNavigationId());
    if (waiting_ && awaited_.empty() && settle_bound_.IsRunning()) {
      settle_bound_.Stop();
      Look();
    }
  }

  void Emptying() {
    bool gone = true;
    if (content::RenderFrameHost::FromID(frame_) == GetWebContents().GetPrimaryMainFrame()) {
      for (const autofill::FormData& form : MainFrameForms(GetWebContents())) {
        for (const FieldRendererId field : filled_) {
          const autofill::FormFieldData* still = FieldOf(form, field);
          gone &= !still || !still->is_visible();
        }
      }
    }
    Empty(frame_, filled_,
          base::BindOnce(
              [](base::WeakPtr<SignIn> sign_in, bool gone) {
                if (!sign_in) {
                  return;
                }
                AgentPage::FromWebContents(&sign_in->GetWebContents())
                    ->ReadAll(base::BindOnce(&SignIn::Checked, sign_in, gone));
              },
              weak_factory_.GetWeakPtr(), gone));
  }

  // The tab is the user's if anything an agent could read shows the password: every frame's tree,
  // the fields' values in it too, which the outline and read_page come from, and every frame's
  // address and the tab's title. What couldn't be read counts as showing it.
  void Checked(bool gone, std::optional<std::vector<base::ListValue>> trees) {
    content::WebContents& contents = GetWebContents();
    const std::u16string password = login_->password_value.value();
    bool shows = !trees || ShowsPassword(contents.GetTitle(), password) ||
                 UrlShows(contents.GetVisibleURL(), password);
    if (trees) {
      for (const base::ListValue& tree : *trees) {
        shows = shows || ShowsIn(tree, password);
      }
    }
    contents.GetPrimaryMainFrame()->ForEachRenderFrameHost([&](content::RenderFrameHost* frame) {
      shows = shows || UrlShows(frame->GetLastCommittedURL(), password);
    });
    if (shows) {
      Withhold();
      return;
    }
    if (user_step_) {
      Finish(false, BC_AGENT_FAILURE_USER_STEP, kUserStep);
      return;
    }
    const bool signed_in = gone && password_submitted_;
    Finish(signed_in, BC_AGENT_FAILURE_NONE, signed_in ? kGone : kNotSignedIn);
  }

  // The tab is the user's, as a handoff holds it: the agent's session stays for when the user hands
  // it back, which empties what's filled before the agent's next action (EmptyForAgent). Held after
  // the sign-in has ended, as Finish deletes it, and in the same task, so no agent acts between.
  // Its title and address stay withheld from agents while it shows that page, or one at its
  // address again.
  void Withhold() {
    const bc_id tab = tab_;
    // A look that found the password counts as a try, forgiven as one is, so text placed in the
    // page before can't be tested sign-in after sign-in. Finish counts a form that was sent.
    if (!submitted_) {
      Tries().Failed(agent_, origin_);
    }
    WithheldPage::Withhold(GetWebContents(), navigations_);
    Finish(false, BC_AGENT_FAILURE_USER_STEP, kUserStep);
    Hold(tab);
    AgentPage::SessionEnded(tab);
  }

  // Answers the agent, tells the app of a sign-in, and ends: no agent's action waits any more.
  void Finish(bool signed_in, bc_agent_failure failure, std::string_view text) {
    weak_factory_.InvalidateWeakPtrs();
    if (submitted_ && !signed_in) {
      Tries().Failed(agent_, origin_);
    }
    const std::string account =
        login_ ? base::UTF16ToUTF8(login_->username_value) : std::string();
    login_.reset();
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE,
        base::BindOnce(
            [](std::string agent, bc_id tab, uint64_t request, std::string site,
               std::string account, bool signed_in, bool submitted, std::string text,
               bc_agent_failure failure) {
              if (signed_in && Host().agent_signed_in) {
                Host().agent_signed_in(Host().context, agent.c_str(), tab, site.c_str(),
                                       account.c_str());
              }
              if (Host().agent_sign_in_done) {
                Host().agent_sign_in_done(Host().context, tab, request, signed_in, submitted,
                                          text.c_str(), failure);
              }
            },
            agent_, tab_, request_, origin_.host(), account, signed_in, submitted_,
            std::string(text), failure));
    content::WebContents* contents = web_contents();
    if (!contents) {
      return;
    }
    // The first outline after the sign-in shows no field as holding what the agent typed.
    if (AgentPage* page = AgentPage::FromWebContents(contents)) {
      page->Forget();
    }
    contents->RemoveUserData(UserDataKey());
  }

  // content::WebContentsObserver: the tab is closing, and nothing in it can be emptied.
  void WebContentsDestroyed() override {
    weak_factory_.InvalidateWeakPtrs();
    login_.reset();
    Observe(nullptr);
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](bc_id tab, uint64_t request, bool submitted) {
                         if (Host().agent_sign_in_done) {
                           Host().agent_sign_in_done(Host().context, tab, request, false,
                                                     submitted, kEnded, BC_AGENT_FAILURE_NONE);
                         }
                       },
                       tab_, request_, submitted_));
  }

  const std::string agent_;
  const uint64_t request_;
  const bc_id tab_;
  // The main frame's origin when the agent asked, the one the login is for.
  const url::Origin origin_;
  // How many stores have yet to answer, and the logins they gave, with their passwords, until one
  // is chosen.
  size_t stores_ = 0;
  std::vector<StoredCredential> logins_;
  std::vector<size_t> usable_;
  bool choosing_ = false;
  // The login the sign-in fills, the only password it keeps, until it answers.
  std::optional<StoredCredential> login_;
  // The frame filled in, and each field filled there.
  content::GlobalRenderFrameHostId frame_;
  std::vector<FieldRendererId> filled_;
  // Whether Enter went into a form, and into a password's.
  bool submitted_ = false;
  bool password_submitted_ = false;
  // The password's page was the user's step, its form sending off the site.
  bool user_step_ = false;
  // The tab's frames' navigations under way, and those the look waits for to end, once it does.
  base::flat_set<int64_t> navigations_;
  base::flat_set<int64_t> awaited_;
  bool waiting_ = false;
  base::OneShotTimer settle_bound_;
  base::WeakPtrFactory<SignIn> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(SignIn);

SignIn* SignInOf(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  return contents ? SignIn::FromWebContents(contents) : nullptr;
}

void Refuse(bc_id tab, uint64_t request, const char* text, bc_agent_failure failure) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](bc_id tab, uint64_t request, std::string text, bc_agent_failure failure) {
                       if (Host().agent_sign_in_done) {
                         Host().agent_sign_in_done(Host().context, tab, request, false, false,
                                                   text.c_str(), failure);
                       }
                     },
                     tab, request, std::string(text), failure));
}

}  // namespace

void StartAgentSignIn(const std::string& agent, bc_id tab, uint64_t request) {
  if (agent.empty() || !Space::AgentMayAct(agent, tab)) {
    Refuse(tab, request, "The tab isn't in the agent's own Space.",
           BC_AGENT_FAILURE_NOT_ITS_SPACE);
    return;
  }
  if (IsHeld(tab)) {
    Refuse(tab, request,
           "The user has this tab until they hand it back. Ask for it with ask_for_tab.",
           BC_AGENT_FAILURE_HELD);
    return;
  }
  if (SigningIn(tab)) {
    Refuse(tab, request, kSigningIn, BC_AGENT_FAILURE_SIGNING_IN);
    return;
  }
  if (PageWithheld(tab)) {
    Refuse(tab, request, kWithheldPageRefusal, BC_AGENT_FAILURE_WITHHELD_PAGE);
    return;
  }
  // A tab the browser unloaded to save memory gets its page back, not shown.
  content::WebContents* contents = Window::PageOf(tab);
  if (!contents) {
    Refuse(tab, request, "The tab isn't in the agent's own Space.",
           BC_AGENT_FAILURE_NOT_ITS_SPACE);
    return;
  }
  // A login submitted here that didn't sign in may be a wrong one: another try is the user's, not
  // a step toward the site's lock-out.
  if (!Tries().MayTry(agent, contents->GetPrimaryMainFrame()->GetLastCommittedOrigin())) {
    Refuse(tab, request, kUserStep, BC_AGENT_FAILURE_USER_STEP);
    return;
  }
  SignIn::CreateForWebContents(contents, agent, request);
  SignIn::FromWebContents(contents)->Start();
}

bool SigningIn(bc_id tab) {
  return SignInOf(tab);
}

bool WithheldFromAgents(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  return contents && (SignIn::FromWebContents(contents) || PageWithheld(tab));
}

bool PageWithheld(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  const WithheldPage* page = contents ? WithheldPage::FromWebContents(contents) : nullptr;
  return page && page->Shows(contents->GetPrimaryMainFrame()->GetLastCommittedURL());
}

void ForgiveSignInTries(const url::Origin& origin) {
  Tries().Forgive(origin);
}

bool ChooseSignIn(bc_id tab, uint64_t request, size_t account) {
  SignIn* sign_in = SignInOf(tab);
  return sign_in && sign_in->Choose(request, account);
}

void StopSignIn(bc_id tab, bc_agent_failure failure) {
  if (SignIn* sign_in = SignInOf(tab)) {
    sign_in->Stop(failure);
  }
}

void EmptyForAgent(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents) {
    return;
  }
  AgentPage::CreateForWebContents(contents);
  AgentPage::FromWebContents(contents)->Step(base::BindOnce(
      [](base::WeakPtr<content::WebContents> contents, base::OnceClosure done) {
        if (!contents) {
          std::move(done).Run();
          return;
        }
        // As the page is now: what Chrome filled for the user and every password field.
        Extract(*contents,
                base::BindOnce(
                    [](base::WeakPtr<content::WebContents> contents, base::OnceClosure done) {
                      if (!contents) {
                        std::move(done).Run();
                        return;
                      }
                      std::vector<FieldRendererId> fields;
                      for (const autofill::FormData& form : MainFrameForms(*contents)) {
                        for (const autofill::FormFieldData& field : form.fields()) {
                          if (field.IsPasswordInputElement() ||
                              field.is_autofilled_according_to_renderer()) {
                            fields.push_back(field.renderer_id());
                          }
                        }
                      }
                      Empty(contents->GetPrimaryMainFrame()->GetGlobalId(), fields,
                            std::move(done));
                    },
                    contents, std::move(done)));
      },
      contents->GetWeakPtr()));
}

}  // namespace browser_core
