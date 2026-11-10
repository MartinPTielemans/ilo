#ifndef BROWSER_CORE_AD_BLOCK_PAGE_AGENT_H_
#define BROWSER_CORE_AD_BLOCK_PAGE_AGENT_H_

#include <cstdint>

namespace content {
class RenderFrame;
}  // namespace content

namespace browser_core {

// Gives |frame| the browser's own ad blocker's agent, which hides and runs on each of its documents
// what the browser sent for it (ad_block_page.mojom). Chrome's renderer calls it for each frame
// (engine/patches/ours/chromium-core-ad-block-page.patch); the agent goes with the frame. It
// watches a document's classes and ids for the generic rules that hide by them in |world|, an
// isolated world Chrome keeps for it.
void CreateAdBlockPageAgent(content::RenderFrame* frame, int32_t world);

// Runs the scriptlets the browser sent for |frame|'s new document, if any. Chrome's renderer calls
// it where extensions' document_start scripts run, before the page's first script; it isn't done at
// DidCreateDocumentElement, where Blink forbids script.
void RunAdBlockScriptsAtDocumentStart(content::RenderFrame* frame);

}  // namespace browser_core

#endif  // BROWSER_CORE_AD_BLOCK_PAGE_AGENT_H_
