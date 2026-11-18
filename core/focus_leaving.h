#ifndef BROWSER_CORE_FOCUS_LEAVING_H_
#define BROWSER_CORE_FOCUS_LEAVING_H_

#include <memory>

namespace content {
class WebContents;
class WebContentsViewDelegate;
}  // namespace content

namespace browser_core {

// Chrome's view delegate for |contents|, |chrome|, answering as Tab leaves a tab's page at either
// end by focus_leaving_rule's rule (#3220): Chrome's Browser leaves that to the delegate, which
// moves the keyboard to a toolbar ilo doesn't draw, so it went nowhere. Everything else, and a
// page of no tab's, such as docked DevTools, stays Chrome's. Every platform's page view asks it.
std::unique_ptr<content::WebContentsViewDelegate> AnswerFocusLeaving(
    content::WebContents* contents,
    std::unique_ptr<content::WebContentsViewDelegate> chrome);

}  // namespace browser_core

#endif  // BROWSER_CORE_FOCUS_LEAVING_H_
