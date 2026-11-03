#ifndef BROWSER_CORE_AGENT_PAGE_H_
#define BROWSER_CORE_AGENT_PAGE_H_

#include <cstdint>
#include <deque>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/containers/flat_map.h"
#include "base/functional/callback.h"
#include "base/functional/callback_helpers.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/timer/timer.h"
#include "base/values.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/devtools_agent_host_client.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "ui/gfx/geometry/rect_f.h"

namespace content {
class DevToolsAgentHost;
class RenderFrameHost;
}  // namespace content

namespace browser_core {

// The page of a tab an agent works in, as bc_agent_page acts on it: its accessibility tree, input
// and screenshots, through Chromium's DevTools protocol from inside the browser, as Capture's
// PageCapture is. Only the commands here are ever sent, so an agent never runs script of its own.
// One action runs at a time, in the order asked. The page's frames are part of it, those from
// http(s) origins: a frame in a process of its own, as Chromium isolates sites, is a DevTools
// session of its own, and an element's id carries its session's slot above its DOM node's id.
class AgentPage : public content::WebContentsObserver,
                  public content::WebContentsUserData<AgentPage>,
                  public content::DevToolsAgentHostClient {
 public:
  AgentPage(const AgentPage&) = delete;
  AgentPage& operator=(const AgentPage&) = delete;
  ~AgentPage() override;

  // See bc_agent_page.
  void Act(uint64_t request, int action, int64_t element, std::string text);

  // The agent's session with |tab| ended: its page stops drawing while hidden, and DevTools lets
  // it go. What's still asked is answered as an error.
  static void SessionEnded(bc_id tab);

 private:
  friend class content::WebContentsUserData<AgentPage>;

  struct Job {
    uint64_t request;
    int action;
    int64_t element;
    std::string text;
  };
  using Reply = base::OnceCallback<void(const base::DictValue* result)>;
  using Then = base::OnceCallback<void(std::optional<gfx::RectF>)>;
  // A frame whose tree a snapshot reads, through the session at |slot|, below the frame at
  // |parent| in the snapshot's list.
  struct Frame {
    std::string id;
    int slot;
    size_t parent;
    base::ListValue nodes;
  };
  // A DevTools session the page's frames are in: the main frame's, or a frame's in a process of
  // its own. A frame's slot stays its own, so an element's id names the same frame even after its
  // session ends, and then nothing.
  struct Target {
    std::string frame;
    std::string session;
  };

  explicit AgentPage(content::WebContents* contents);

  // Runs the next job once the one before has answered.
  void Next();
  void Run();
  // Whether the tab's page is one agents may use.
  bool IsWebPage();
  // Runs |then| once the page has stopped loading, or after a while.
  void AfterLoad(base::OnceClosure then);
  // Waits a moment for what an input starts, such as a navigation, then for it to load, then
  // answers with the tree.
  void Settle();

  void Click();
  void Mouse(const char* type, std::pair<double, double> at, base::OnceClosure then);
  void Type(bool submit);
  void Focus(int64_t element, bool submit);
  void PressKey(const std::string& key, base::OnceClosure then);
  void Scroll(bool down);
  void History(bool forward);
  void Screenshot();

  // The tree, as bc_agent_page answers it: the main frame's, with each frame agents may see under
  // the element it's in.
  void Snapshot();
  // The frames whose trees a snapshot reads, each after the frame it's in, the main frame first.
  std::vector<Frame> Frames();
  // Reads the tree of each frame from |next| on, then puts each under its frame's element.
  void ReadTree(size_t next);
  void StitchTree(size_t next);
  // Marks the text fields in |fields| that are for passwords, one at a time, then answers.
  void CheckPasswords(base::ListValue nodes,
                      std::vector<int64_t> fields,
                      std::set<int64_t> passwords);
  void Answer(std::string text, bool error);
  void Fail(std::string why);

  // Scrolls the job's element into view and gives its box, in the viewport's CSS pixels.
  void Center(Then then);
  // Moves |box|, in the session at |slot|, into the main frame's viewport, through the elements
  // its frame is in.
  void Offset(int slot, gfx::RectF box, Then then);
  // Tells the app where the element the job acts on is, for the agent's pointer.
  void Point(const gfx::RectF& box);
  // Whether the element is a password field, from its attributes.
  void IsPassword(int64_t element, base::OnceCallback<void(std::optional<bool>)> then);
  // Closes Chrome's autofill and password suggestions, which the keys would otherwise pick from.
  void HideSuggestions();

  // The slot of the session |frame| is in, or -1 if there's none.
  int SlotOf(content::RenderFrameHost* frame);
  content::RenderFrameHost* FrameOf(int slot);

  // Sends the command to the session at |slot|.
  void Send(const char* method, base::DictValue params, Reply reply, int slot = 0);
  // Has the session at |slot| bring each frame in another process into a session of its own.
  void AutoAttach(int slot);
  void Detach();
  void LetGo();

  // content::DevToolsAgentHostClient:
  void DispatchProtocolMessage(content::DevToolsAgentHost* agent_host,
                               base::span<const uint8_t> message) override;
  void AgentHostClosed(content::DevToolsAgentHost* agent_host) override;
  // A frame's session started, as Target.attachedToTarget tells, or ended.
  void Attached(const std::string& session, const base::DictValue& event);
  void Detached(const std::string& session);

  // content::WebContentsObserver:
  void DidStopLoading() override;
  void WebContentsDestroyed() override;

  const bc_id tab_;
  std::deque<Job> jobs_;
  // The job running, which has yet to answer.
  std::optional<Job> job_;
  base::OnceClosure loaded_;
  base::OneShotTimer wait_;
  base::OneShotTimer rest_;
  scoped_refptr<content::DevToolsAgentHost> agent_host_;
  int next_command_ = 1;
  // Each command's reply, with the slot of the session it was sent to.
  base::flat_map<int, std::pair<int, Reply>> replies_;
  std::vector<Target> targets_;
  // The frames the snapshot running reads.
  std::vector<Frame> frames_;
  // Keeps the page drawing while it's hidden, for the agent's input and screenshots.
  base::ScopedClosureRunner drawing_;
  base::WeakPtrFactory<AgentPage> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

// Whether the frame holds a web page's content, from an http(s) origin, as do the about:blank and
// sandboxed frames a page makes. An extension's or the browser's frame, as a password manager puts
// in pages, is never an agent's.
bool IsWebFrame(content::RenderFrameHost* frame);

// What a page's accessibility tree, as DevTools' Accessibility.getFullAXTree gives it, may show
// an agent: each node without its value, and without the nodes under a field, where Chromium puts
// what was typed or filled into it, and with "password": true on the nodes in |passwords|.
base::ListValue AgentTree(base::ListValue nodes, const std::set<int64_t>& passwords);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_PAGE_H_
