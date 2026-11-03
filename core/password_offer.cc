#include "browser_core/password_offer.h"

#include <algorithm>
#include <map>
#include <memory>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "chrome/browser/ui/actions/chrome_action_id.h"
#include "chrome/browser/ui/page_action/page_action_controller.h"
#include "chrome/browser/ui/page_action/page_action_model.h"
#include "chrome/browser/ui/page_action/page_action_model_observer.h"
#include "chrome/browser/ui/passwords/bubble_controllers/save_update_bubble_controller.h"
#include "chrome/browser/ui/passwords/manage_passwords_ui_controller.h"
#include "chrome/browser/ui/passwords/passwords_model_delegate.h"
#include "chrome/browser/ui/tabs/public/tab_features.h"
#include "chrome/grit/generated_resources.h"
#include "components/password_manager/core/browser/password_form.h"
#include "components/password_manager/core/common/password_manager_ui.h"
#include "components/tabs/public/tab_interface.h"
#include "components/url_formatter/elide_url.h"
#include "content/public/browser/web_contents_user_data.h"
#include "ui/base/l10n/l10n_util.h"

namespace browser_core {

namespace {

// The tabs' offers, one a tab at most, as Chrome's BubbleManager shows one bubble at a time.
std::map<bc_id, std::unique_ptr<SaveUpdateBubbleController>>& Offers() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<SaveUpdateBubbleController>>> offers;
  return *offers;
}

// The password's characters, counted by code point so that an emoji draws one dot.
size_t CodePoints(std::u16string_view password) {
  return std::ranges::count_if(password,
                               [](char16_t unit) { return unit < 0xDC00 || unit > 0xDFFF; });
}

// The tab Moving keeps the offer of, or 0.
bc_id& MovingTab() {
  static bc_id tab = 0;
  return tab;
}

bool IsOffer(password_manager::ui::State state) {
  return state == password_manager::ui::PENDING_PASSWORD_STATE ||
         state == password_manager::ui::PENDING_PASSWORD_UPDATE_STATE;
}

// The offer's login is still the one the password manager holds. It moves on without hiding the
// offer, such as when the page logs in again, and answering a login it no longer holds would save
// nothing or fail its checks.
bool Current(const SaveUpdateBubbleController& offer) {
  content::WebContents* contents = offer.GetWebContents();
  base::WeakPtr<PasswordsModelDelegate> delegate =
      contents ? PasswordsModelDelegateFromWebContents(contents) : nullptr;
  return delegate && delegate->GetState() == offer.state();
}

// The BC_PASSWORDS_ value for the password manager's state on |contents|'s page: no key where
// Chrome's toolbar shows none, and where it shows one for an account chooser or an automatic
// sign-in, which have no UI here.
int KeyOf(content::WebContents* contents) {
  base::WeakPtr<PasswordsModelDelegate> delegate = PasswordsModelDelegateFromWebContents(contents);
  if (!delegate) {
    return BC_PASSWORDS_NONE;
  }
  const password_manager::ui::State state = delegate->GetState();
  if (IsOffer(state)) {
    return BC_PASSWORDS_OFFER;
  }
  switch (state) {
    case password_manager::ui::INACTIVE_STATE:
    case password_manager::ui::CREDENTIAL_REQUEST_STATE:
    case password_manager::ui::AUTO_SIGNIN_STATE:
      return BC_PASSWORDS_NONE;
    default:
      return BC_PASSWORDS_SAVED;
  }
}

}  // namespace

// Chrome's passwords page action for a tab, which its toolbar would show as the key, watched for
// the app. Chrome changes it each time the password manager's state for the page does, and this
// then tells the app what the key is for, from a task of its own, as Chrome changes it inside the
// app's calls.
class KeyIcon : public content::WebContentsUserData<KeyIcon>,
                public page_actions::PageActionModelObserver {
 public:
  ~KeyIcon() override = default;

  void OnPageActionModelChanged(const page_actions::PageActionModelInterface& model) override {
    ReportSoon();
  }

  void OnPageActionModelWillBeDeleted(
      const page_actions::PageActionModelInterface& model) override {
    observation_.Reset();
  }

  bool Watching() const { return observation_.IsObserving(); }

  void Watch(page_actions::PageActionController& controller) {
    controller.AddObserver(kActionShowPasswordsBubbleOrPage, observation_);
    ReportSoon();
  }

 private:
  friend WebContentsUserData;
  explicit KeyIcon(content::WebContents* contents) : WebContentsUserData(*contents) {}

  void ReportSoon() {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&KeyIcon::Report, weak_factory_.GetWeakPtr()));
  }

  void Report() {
    const int key = KeyOf(&GetWebContents());
    if (key == reported_) {
      return;
    }
    reported_ = key;
    if (Host().tab_passwords_changed) {
      Host().tab_passwords_changed(Host().context, Tab::IdOf(&GetWebContents()), key);
    }
  }

  base::ScopedObservation<page_actions::PageActionModelInterface,
                          page_actions::PageActionModelObserver>
      observation_{this};
  int reported_ = BC_PASSWORDS_NONE;
  base::WeakPtrFactory<KeyIcon> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(KeyIcon);

void PasswordOffer::WatchKey(content::WebContents* contents) {
  tabs::TabInterface* tab = tabs::TabInterface::MaybeGetFromContents(contents);
  tabs::TabFeatures* features = tab ? tab->GetTabFeatures() : nullptr;
  page_actions::PageActionController* actions =
      features ? features->page_action_controller() : nullptr;
  if (!actions || !actions->ActionExists(kActionShowPasswordsBubbleOrPage)) {
    return;
  }
  KeyIcon::CreateForWebContents(contents);
  KeyIcon* key = KeyIcon::FromWebContents(contents);
  if (!key->Watching()) {
    key->Watch(*actions);
  }
}

void PasswordOffer::ShowAgain(bc_id tab) {
  content::WebContents* contents = Tab::FromId(tab);
  if (!contents || KeyOf(contents) != BC_PASSWORDS_OFFER || Offers().contains(tab)) {
    return;
  }
  // As a click on Chrome's key does: the password manager asks for its bubble, which Prompt shows
  // as the app's offer.
  if (auto* passwords = ManagePasswordsUIController::FromWebContents(contents)) {
    passwords->QueueOrShowBubble(/*user_action=*/true);
  }
}

void PasswordOffer::Prompt(content::WebContents* contents, bool show) {
  const bc_id tab = Tab::IdOf(contents);
  if (!show) {
    if (tab != MovingTab()) {
      Drop(tab);
    }
    return;
  }
  base::WeakPtr<PasswordsModelDelegate> delegate = PasswordsModelDelegateFromWebContents(contents);
  if (!delegate) {
    return;
  }
  if (!IsOffer(delegate->GetState())) {
    // Chrome's other password bubbles, such as the one confirming a save, show nothing: the Site
    // Controls list a site's logins in place of its manage bubble, and Settings every one. Each
    // closes as soon as it shows, as if the user had closed it, so the BubbleManager moves on.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](base::WeakPtr<PasswordsModelDelegate> delegate) {
                         if (delegate) {
                           delegate->OnBubbleHidden();
                         }
                       },
                       delegate));
    return;
  }
  Drop(tab);
  // Made as Chrome's bubble makes it, which tells the password manager it's shown.
  Offers()[tab] = std::make_unique<SaveUpdateBubbleController>(
      delegate, PasswordBubbleControllerBase::DisplayReason::kAutomatic);
  Changed(tab);
}

void PasswordOffer::Answer(bc_id tab, int answer, const char* username) {
  auto found = Offers().find(tab);
  if (found == Offers().end()) {
    return;
  }
  SaveUpdateBubbleController& offer = *found->second;
  if (!Current(offer)) {
    Drop(tab);
    return;
  }
  const bool update = offer.state() == password_manager::ui::PENDING_PASSWORD_UPDATE_STATE;
  // As Chrome's bubble, which hands over the username field's text before each of its buttons,
  // with the password it was given.
  if (username) {
    std::u16string edited;
    base::TrimString(base::UTF8ToUTF16(username), u" ", &edited);
    offer.OnCredentialEdited(std::move(edited), offer.pending_password().password_value);
  }
  switch (answer) {
    case BC_OFFER_ACCEPT:
      offer.OnSaveClicked();
      break;
    case BC_OFFER_NEVER:
      // An update has no Never: the site's passwords are already saved.
      if (!update) {
        offer.OnNeverForThisSiteClicked();
      }
      break;
    case BC_OFFER_NOT_NOW:
      if (update) {
        offer.OnNoThanksClicked();
      } else {
        offer.OnNotNowClicked();
      }
      break;
  }
  // Unless answering it showed another.
  if (found = Offers().find(tab); found != Offers().end() && found->second.get() == &offer) {
    Drop(tab);
  }
}

PasswordOffer::Moving::Moving(bc_id tab) : previous_(std::exchange(MovingTab(), tab)) {}

PasswordOffer::Moving::~Moving() {
  MovingTab() = previous_;
}

void PasswordOffer::Drop(bc_id tab) {
  auto found = Offers().find(tab);
  if (found == Offers().end()) {
    return;
  }
  // Chrome's widget destroys the controller, which tells the password manager the bubble is gone,
  // from a task: here, inside the BubbleManager's own call to hide it, it would show the next.
  base::SingleThreadTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE,
                                                                std::move(found->second));
  Offers().erase(found);
  Changed(tab);
}

void PasswordOffer::Changed(bc_id tab) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&PasswordOffer::Report, tab));
}

void PasswordOffer::Report(bc_id tab) {
  if (!Host().tab_offer_changed) {
    return;
  }
  auto found = Offers().find(tab);
  if (found != Offers().end() && !Current(*found->second)) {
    Drop(tab);
    return;
  }
  if (found == Offers().end()) {
    Host().tab_offer_changed(Host().context, tab, nullptr);
    return;
  }
  const SaveUpdateBubbleController& offer = *found->second;
  const bool update = offer.state() == password_manager::ui::PENDING_PASSWORD_UPDATE_STATE;
  const password_manager::PasswordForm& form = offer.pending_password();
  // As Chrome's username field offers them: the one the page submitted, then the other fields'
  // values that might be the username.
  std::vector<std::string> usernames;
  auto add = [&usernames](const std::u16string& username) {
    std::string utf8 = base::UTF16ToUTF8(username);
    if (!utf8.empty() && !std::ranges::contains(usernames, utf8)) {
      usernames.push_back(std::move(utf8));
    }
  };
  add(form.username_value);
  for (const password_manager::AlternativeElement& alternative : form.all_alternative_usernames) {
    add(alternative.value);
  }
  std::vector<const char*> names;
  for (const std::string& username : usernames) {
    names.push_back(username.c_str());
  }
  const std::string title = base::UTF16ToUTF8(offer.GetTitle());
  const std::string username = base::UTF16ToUTF8(form.username_value);
  // As Chrome's bubble names the site when it isn't the page's.
  const std::string site = base::UTF16ToUTF8(url_formatter::FormatOriginForSecurityDisplay(
      PasswordsModelDelegateFromWebContents(offer.GetWebContents())->GetOrigin(),
      url_formatter::SchemeDisplay::OMIT_HTTP_AND_HTTPS));
  // Chrome's three-button save bubble, and its two-button update bubble.
  const std::string accept = l10n_util::GetStringUTF8(
      update ? IDS_PASSWORD_MANAGER_UPDATE_BUTTON : IDS_PASSWORD_MANAGER_SAVE_BUTTON);
  const std::string decline = l10n_util::GetStringUTF8(IDS_PASSWORD_MANAGER_CANCEL_BUTTON);
  const std::string never =
      update ? std::string()
             : l10n_util::GetStringUTF8(IDS_PASSWORD_MANAGER_BUBBLE_BLOCKLIST_BUTTON);
  const bc_offer reported = {
      .kind = update ? BC_OFFER_KIND_PASSWORD_UPDATE : BC_OFFER_KIND_PASSWORD,
      .title = title.c_str(),
      .site = site.c_str(),
      .username = username.c_str(),
      .usernames = names.data(),
      .username_count = names.size(),
      .password_length = CodePoints(form.password_value.secure_value()),
      .accept = accept.c_str(),
      .decline = decline.c_str(),
      .never = never.c_str(),
      .details = "",
  };
  Host().tab_offer_changed(Host().context, tab, &reported);
}

}  // namespace browser_core
