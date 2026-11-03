#include "browser_core/autofill_offer.h"

#include <map>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/raw_ptr.h"
#include "base/no_destructor.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "chrome/browser/ui/autofill/autofill_bubble_base.h"
#include "chrome/browser/ui/autofill/payments/save_card_bubble_controller.h"
#include "chrome/browser/ui/autofill/payments/save_card_ui.h"
#include "chrome/browser/ui/autofill/save_address_bubble_controller.h"
#include "components/autofill/core/browser/data_model/payments/credit_card.h"
#include "components/autofill/core/browser/ui/payments/payments_ui_closed_reasons.h"
#include "components/url_formatter/elide_url.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

using autofill::PaymentsUiClosedReason;

// One tab's offer: the bubble Chrome's controller holds until it hides it, or until the user
// answers, as Chrome's views close themselves.
class Offer : public autofill::AutofillBubbleBase {
 public:
  Offer(bc_id tab, std::unique_ptr<autofill::SaveAddressBubbleController> address)
      : tab_(tab), address_(std::move(address)) {}
  Offer(bc_id tab, autofill::SaveCardBubbleController* card) : tab_(tab), card_(card) {}

  // Chrome ends it: another tab shows, the page goes, or another bubble takes its place.
  void Hide() override { Close(PaymentsUiClosedReason::kNotInteracted); }
  bool IsMouseHovered() const override { return false; }

  // The user's answer, as Chrome's views give it: the decision, then the bubble closing.
  void Answer(int answer) {
    const bool accept = answer == BC_OFFER_ACCEPT;
    if (address_ && !closed_) {
      if (accept) {
        address_->OnUserDecision(autofill::AutofillClient::AddressPromptUserDecision::kAccepted,
                                 address_->GetAutofillProfile());
      } else {
        address_->OnUserDecision(address_->GetCancelCallbackValue(), std::nullopt);
      }
    }
    if (card_ && !closed_ && accept) {
      card_->OnSaveButton({});
    }
    Close(accept ? PaymentsUiClosedReason::kAccepted : PaymentsUiClosedReason::kCancelled);
  }

  // What tab_offer_changed tells the app, in Chrome's words, or nothing once it has closed.
  void Report(const std::string& site) const;

 private:
  void Close(PaymentsUiClosedReason reason);

  const bc_id tab_;
  // The address's controller is the bubble's, as Chrome's view owns it; the card's, the tab's.
  std::unique_ptr<autofill::SaveAddressBubbleController> address_;
  raw_ptr<autofill::SaveCardBubbleController> card_ = nullptr;
  // OnBubbleClosed was called, which Chrome's controllers take once.
  bool closed_ = false;
};

// The tabs' offers, one a tab at most, as Chrome's BubbleManager shows one bubble at a time.
std::map<bc_id, std::unique_ptr<Offer>>& Offers() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<Offer>>> offers;
  return *offers;
}

// Tells the app |tab|'s offer, or that it has none, from a task of its own: Chrome shows and
// hides bubbles inside the app's calls, such as the one that shows a tab.
void Changed(bc_id tab) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](bc_id tab) {
                       if (!Host().tab_offer_changed) {
                         return;
                       }
                       auto found = Offers().find(tab);
                       content::WebContents* contents = Tab::FromId(tab);
                       if (found == Offers().end() || !contents) {
                         Host().tab_offer_changed(Host().context, tab, nullptr);
                         return;
                       }
                       // As the password's offer names the site.
                       const std::string site =
                           base::UTF16ToUTF8(url_formatter::FormatOriginForSecurityDisplay(
                               contents->GetPrimaryMainFrame()->GetLastCommittedOrigin(),
                               url_formatter::SchemeDisplay::OMIT_HTTP_AND_HTTPS));
                       found->second->Report(site);
                     },
                     tab));
}

// Ends |tab|'s offer |offer|, unless another has taken its place. It's deleted from a task, as
// Chrome's widget is: here, inside the controller's own call to hide it, it's still in use.
void Drop(bc_id tab, const Offer* offer) {
  auto found = Offers().find(tab);
  if (found == Offers().end() || found->second.get() != offer) {
    return;
  }
  base::SingleThreadTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE,
                                                                std::move(found->second));
  Offers().erase(found);
  Changed(tab);
}

autofill::AutofillBubbleBase* Add(bc_id tab, std::unique_ptr<Offer> offer) {
  Offer* shown = offer.get();
  // A tab has one. Chrome's BubbleManager hides the one before first; if it hasn't, this does,
  // so its controller lets go of it.
  if (auto found = Offers().find(tab); found != Offers().end()) {
    found->second->Hide();
  }
  Offers()[tab] = std::move(offer);
  Changed(tab);
  return shown;
}

void Offer::Close(PaymentsUiClosedReason reason) {
  if (!std::exchange(closed_, true)) {
    if (address_) {
      address_->OnBubbleClosed();
    }
    if (card_) {
      card_->OnBubbleClosed(reason);
    }
  }
  Drop(tab_, this);
}

void Offer::Report(const std::string& site) const {
  // Chrome's bubble's lines: the address's summary, then its email and phone, or the card's name
  // and last digits, then its expiry.
  std::vector<std::u16string> lines;
  std::u16string title, accept, decline;
  int kind = BC_OFFER_KIND_ADDRESS;
  if (address_) {
    title = address_->GetWindowTitle();
    lines = {address_->GetAddressSummary(), address_->GetProfileEmail(),
             address_->GetProfilePhone()};
    accept = address_->GetOkButtonLabel();
    decline = address_->GetNegativeButtonLabel();
  } else {
    kind = BC_OFFER_KIND_CARD;
    const autofill::CreditCard& card = card_->GetCard();
    title = card_->GetWindowTitle();
    lines = {card.CardNameAndLastFourDigits(),
             card.AbbreviatedExpirationDateForDisplay(/*with_prefix=*/true)};
    accept = card_->GetAcceptButtonText();
    decline = card_->GetDeclineButtonText();
  }
  std::erase_if(lines, [](const std::u16string& line) { return line.empty(); });
  const std::string details = base::UTF16ToUTF8(base::JoinString(lines, u"\n"));
  const std::string title8 = base::UTF16ToUTF8(title);
  const std::string accept8 = base::UTF16ToUTF8(accept);
  const std::string decline8 = base::UTF16ToUTF8(decline);
  const bc_offer reported = {
      .kind = kind,
      .title = title8.c_str(),
      .site = site.c_str(),
      .username = "",
      .usernames = nullptr,
      .username_count = 0,
      .password_length = 0,
      .accept = accept8.c_str(),
      .decline = decline8.c_str(),
      // Chrome's address and card bubbles have two buttons.
      .never = "",
      .details = details.c_str(),
  };
  Host().tab_offer_changed(Host().context, tab_, &reported);
}

}  // namespace

autofill::AutofillBubbleBase* AutofillOffer::ShowAddress(
    content::WebContents* contents,
    std::unique_ptr<autofill::SaveAddressBubbleController> controller) {
  const bc_id tab = Tab::IdOf(contents);
  // Agents never see the user's addresses (agent_autofill.h).
  if (!tab || IsAgentTab(contents)) {
    return nullptr;
  }
  return Add(tab, std::make_unique<Offer>(tab, std::move(controller)));
}

autofill::AutofillBubbleBase* AutofillOffer::ShowCard(
    content::WebContents* contents,
    autofill::SaveCardBubbleController* controller) {
  const bc_id tab = Tab::IdOf(contents);
  // Only a save to this device: a save to Google Payments needs a Google account, which ilo
  // doesn't sign in to, and Manage Cards opens from the card's icon, which there isn't.
  const autofill::PaymentsBubbleType type = controller->GetPaymentsBubbleType();
  const bool local = type == autofill::PaymentsBubbleType::kLocalSave ||
                     type == autofill::PaymentsBubbleType::kLocalCvcSave;
  if (!tab || !local || IsAgentTab(contents)) {
    return nullptr;
  }
  return Add(tab, std::make_unique<Offer>(tab, controller));
}

bool AutofillOffer::Answer(bc_id tab, int answer) {
  auto found = Offers().find(tab);
  if (found == Offers().end()) {
    return false;
  }
  found->second->Answer(answer);
  return true;
}

}  // namespace browser_core
