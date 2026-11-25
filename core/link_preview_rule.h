// Which link preview is a page's own link, apart so browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_LINK_PREVIEW_RULE_H_
#define BROWSER_CORE_LINK_PREVIEW_RULE_H_

#include <string>
#include <string_view>

namespace browser_core {

// The link a tab's page asked the app to open in a link preview, until a preview opens from the
// tab. Only that link is the page's own, loaded with what the page would have sent with it: a
// preview the user opens from elsewhere, as the Library's, is no page's link.
class LinkPreviewAsk {
 public:
  // The app said yes to a preview of |url|, which it opens after.
  void Keep(std::string_view url);
  // Whether the preview opening at |url| is the link kept. Forgets it either way: one ask is one
  // preview.
  bool Take(std::string_view url);

 private:
  std::string asked_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_LINK_PREVIEW_RULE_H_
