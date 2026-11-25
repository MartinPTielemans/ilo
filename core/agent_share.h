#ifndef BROWSER_CORE_AGENT_SHARE_H_
#define BROWSER_CORE_AGENT_SHARE_H_

#include "content/public/browser/render_frame_host.h"
#include "mojo/public/cpp/bindings/binder_map.h"

namespace browser_core {

// Puts the core's binder for a page's share service in place of Chrome's, where Chrome has one
// (the Mac and Windows). A tab of the user's gets Chrome's own, as ever. A page in an agent's
// Space gets one that asks at each share whether the user holds the tab, as a page keeps the
// service it first asked for while a hold begins and ends: not held, the page hears that it
// wasn't allowed and no sheet of the system's is drawn; held, Chrome's own shares it.
void AddShareBinder(mojo::BinderMapWithContext<content::RenderFrameHost*>* map);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SHARE_H_
