// A link card (#3080): the page behind the link the pointer is on, asked as no page and no user
// would ask it, for the app to show what it says about itself (bc_tab_link_card). The rules for
// what it asks and refuses are in link_card_rule.h.

#ifndef BROWSER_CORE_LINK_CARD_H_
#define BROWSER_CORE_LINK_CARD_H_

#include <cstdint>
#include <string>

#include "browser_core/public/browser_core.h"

namespace browser_core {

// bc_tab_link_card: a card for the link the pointer is on in |tab|, or 0.
uint64_t StartLinkCard(bc_id tab);

// bc_link_card_picture: asks |url| for |card|.
void AskLinkCardPicture(uint64_t card, const std::string& url);

// bc_link_card_cancel.
void CancelLinkCard(uint64_t card);

}  // namespace browser_core

#endif  // BROWSER_CORE_LINK_CARD_H_
