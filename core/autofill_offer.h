#ifndef BROWSER_CORE_AUTOFILL_OFFER_H_
#define BROWSER_CORE_AUTOFILL_OFFER_H_

#include <memory>

#include "browser_core/public/browser_core.h"

namespace autofill {
class AutofillBubbleBase;
class SaveAddressBubbleController;
class SaveCardBubbleController;
}  // namespace autofill

namespace content {
class WebContents;
}

namespace browser_core {

// Chrome's offers to save an address or a card that a page's form submitted, which the app shows
// in place of Chrome's bubbles (tab_offer_changed), as PasswordOffer shows the password manager's.
// Autofill's controllers ask the window's AutofillBubbleHandler for a bubble while the tab shows,
// and hide it when another tab shows or the page goes; the offer is that bubble. Update Address
// isn't offered: Chrome's own bubble for it is a list of changes this doesn't draw yet.
class AutofillOffer {
 public:
  // From the window's AutofillBubbleHandler: shows |controller|'s offer for |contents| and returns
  // the bubble Chrome hides to end it, or null where the app shows none, such as in an agent's tab.
  static autofill::AutofillBubbleBase* ShowAddress(
      content::WebContents* contents,
      std::unique_ptr<autofill::SaveAddressBubbleController> controller);
  static autofill::AutofillBubbleBase* ShowCard(content::WebContents* contents,
                                                autofill::SaveCardBubbleController* controller);
  // See bc_tab_answer_offer: answers |tab|'s address or card offer, and whether it had one.
  static bool Answer(bc_id tab, int answer);
};

}  // namespace browser_core

#endif  // BROWSER_CORE_AUTOFILL_OFFER_H_
