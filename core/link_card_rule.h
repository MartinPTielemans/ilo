// What a link card (#3080) asks for and refuses, apart so browser_core_unittests tests the rules
// alone. link_card.cc does the asking.

#ifndef BROWSER_CORE_LINK_CARD_RULE_H_
#define BROWSER_CORE_LINK_CARD_RULE_H_

#include <cstddef>
#include <string_view>

class GURL;

namespace browser_core {

// The redirects a card follows, as most link shorteners need one or two.
inline constexpr int kLinkCardMaxRedirects = 5;
// The most of a page a card reads: its tags are in its head, and the largest head of #3080's
// sample of 101 pages ended at 839 KB.
inline constexpr size_t kLinkCardPageCap = 1 << 20;
// The largest picture a card takes; the share images of the sample were under 300 KB.
inline constexpr size_t kLinkCardPictureCap = 2 << 20;

// Whether a card may ask |url|: http or https, for the link and for every redirect.
bool LinkCardAsks(const GURL& url);

// Whether a card follows a redirect to |to| as its |hop|th, counting from 1.
bool LinkCardFollows(const GURL& to, int hop);

// Whether a card asks through the connection a proxy lookup answered with |net_error| and, if it
// found one, whether that's |direct|. Only a direct connection is checked for a private network's
// address, so a fetch the profile's proxy would carry could reach the proxy's.
bool LinkCardDirect(int net_error, bool found, bool direct);

// Whether an answer of |mime| is what the card reads: a page, or for its picture, an image the app
// decodes.
bool LinkCardPage(std::string_view mime);
bool LinkCardPicture(std::string_view mime);

// How much of |read|, a page's body so far, the card keeps: up to its head's end, "</head>" in
// any case, or the cap. Less than |read|'s size, or the cap, ends the read.
size_t LinkCardKeep(std::string_view read);

}  // namespace browser_core

#endif  // BROWSER_CORE_LINK_CARD_RULE_H_
