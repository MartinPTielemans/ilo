#ifndef BROWSER_CORE_AGENT_PAGE_H_
#define BROWSER_CORE_AGENT_PAGE_H_

#include <cstdint>
#include <deque>
#include <memory>
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
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/values.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/devtools_agent_host_client.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/geometry/rect_f.h"
#include "url/gurl.h"

namespace content {
class DevToolsAgentHost;
class NavigationHandle;
class RenderFrameHost;
class ScopedAccessibilityMode;
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

  // |contents|' page went to |url|, which opens an app outside the browser, and the core stopped it
  // where it was. The action running there, as a click on such a link, answers that, as an error.
  static void AppRefused(content::WebContents* contents, const GURL& url);
  // A download the page in |contents| started answers the action that started it, in place of
  // the page: the request of the action running, or of one that answered while the download was
  // about to start, which then never answers. Else 0.
  static uint64_t TakeJob(content::WebContents* contents);

 private:
  friend class content::WebContentsUserData<AgentPage>;

  struct Job {
    uint64_t request;
    int action;
    int64_t element;
    std::string text;
    // When waiting for its element, or for the page to settle, gives up.
    base::TimeTicks deadline;
    // Whether the page was still busy when it stopped waiting for it to settle.
    bool busy = false;
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
  // How far a SELECT has got: reading the element's options, reading those clicking it showed, or
  // reading the tree after choosing one.
  enum class Choosing { kNo, kOptions, kOpened, kChosen };
  class WheelWatch;

  explicit AgentPage(content::WebContents* contents);

  // Runs the next job once the one before has answered.
  void Next();
  void Run();
  // Whether the tab's page is one agents may use, and if not, why, for the agent.
  bool IsWebPage();
  std::string NotWebPage();
  // Runs |then| once the page has stopped loading, or after a while.
  void AfterLoad(base::OnceClosure then);

  // Waits until the job's element is on screen, has held still for two frames and, with
  // |enabled|, isn't disabled, then gives its box; or at the deadline the box it has, if any. One
  // that's gone, or still disabled at the deadline, fails the job.
  void Ready(bool enabled, Then then);
  void Look(bool enabled, std::optional<gfx::RectF> last, int still, Then then);
  // Hears of the page's requests from now until the job answers, so an input's answer waits for
  // what it starts, then runs |then|.
  void Listen(base::OnceClosure then);
  void Unlisten();
  // A request started or ended in the session |session|, or "" for the main frame's.
  void Heard(const std::string& session, const std::string& method, const base::DictValue& event);
  // Waits for the page to settle after an input, then answers with the tree.
  void Settle();
  // Waits until the page has stopped loading and has had no request for a while.
  void Quiet();
  // Reads the main frame's tree until it's the same twice, a frame apart, then the snapshot.
  void Still(base::ListValue last);

  void Click();
  // Presses the job's element once it's ready. One outside the page's view, where scrolling
  // didn't bring it, is focused first unless it has been (|focused|), as a skip link is.
  void Press(bool focused);
  // Presses the element where it is in its frame's view, with the agent's pointer over it.
  void PressInView();
  // As a user's click at |at|, in the viewport of the frame of the session at |slot|: the pointer
  // comes, presses and lets go, from |step| on, each once the last is in, then the page settles.
  void ClickAt(int slot, gfx::PointF at, size_t step);
  void Mouse(const char* type, gfx::PointF at, int slot, base::OnceClosure then);
  // Chooses the job's option among those in |nodes|, the whole tree, whose frames' DevTools ids
  // |frames| has in order, or clicks the element to show them, or fails with the options there are.
  void Choose(const base::ListValue& nodes, const std::vector<std::string>& frames);
  // Chooses a select's |option|, the node in the frame |frames| has at |frame|, through its
  // accessibility action, as a screen reader does.
  void ChooseNative(const std::vector<std::string>& frames,
                    size_t frame,
                    const base::DictValue& option);
  // Whether the field in |nodes| shows the option the job chose.
  bool Shows(const base::ListValue& nodes);
  void EndChoosing();
  void Type(bool submit);
  void Focus(int64_t element, bool submit);
  // Types the job's text into |element|, which has the keyboard, with what it held selected.
  void TypeInto(int64_t element, bool submit);
  void TypeKeys(std::u16string text, bool submit);
  void PressKey(const std::string& key, base::OnceClosure then);
  // Hears the page's requests, then turns the wheel, so the answer waits for what the scroll
  // starts loading.
  void Scroll(bool down);
  void TurnWheel(bool down);
  // Turns the wheel at the middle of |area|, in the viewport's CSS pixels, by most of its height,
  // then answers with the tree, or that nothing scrolled.
  void Wheel(const gfx::RectF& area, bool down);
  // Turns it at |at|, by most of |height|, once nothing of an extension's is there.
  void TurnWheelAt(gfx::PointF at, bool down, float height);
  void Scrolled(bool down);
  void ScrollTo();
  void History(bool forward);
  void Screenshot();

  // The tree, as bc_agent_page answers it: the main frame's, with each frame agents may see under
  // the element it's in.
  void Snapshot();
  // The snapshot, from the main frame's tree in |nodes|.
  void Read(base::ListValue nodes);
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
  // Gives the answer kept while a download started, as none took its action.
  void AnswerHeld();

  // Scrolls the job's element into view and gives its box, in the main frame's viewport's CSS
  // pixels.
  void Center(Then then);
  // The same in the viewport of the frame whose session has the element.
  void Locate(Then then);
  // Moves |box|, in the session at |slot|, into the main frame's viewport, through the elements
  // its frame is in.
  void Offset(int slot, gfx::RectF box, Then then);
  // Tells the app where the element the job acts on is, for the agent's pointer.
  void Point(const gfx::RectF& box);
  // Whether the element is a password field, from its attributes.
  void IsPassword(int64_t element, base::OnceCallback<void(std::optional<bool>)> then);
  // Closes Chrome's autofill and password suggestions, which the keys would otherwise pick from.
  void HideSuggestions();
  // Whether an extension's control is what input at |at|, in the main frame's viewport, would
  // land on, in whichever frame it lands: a frame that isn't a web page's, as an extension's own
  // is, or what an extension's content script put in the page (engine/patches/ours/
  // chromium-agent-extension-nodes.patch). Agents never act on an extension's controls.
  void Covered(gfx::PointF at, base::OnceCallback<void(bool)> then);
  // The same, at |at| in the viewport of the frame of the session at |slot|, |depth| frames down.
  void CoveredIn(int slot, gfx::PointF at, int depth, base::OnceCallback<void(bool)> then);
  void CoveredBy(int slot,
                 gfx::PointF at,
                 int depth,
                 int node,
                 base::OnceCallback<void(bool)> then,
                 const base::DictValue* described);
  // Whether the keyboard is in a frame that isn't a web page's, as an extension's menu is when it
  // has taken focus. An agent's keys go nowhere then.
  bool KeysInExtension();
  // Refuses the job, as what it would act on is an extension's: the agent hears |why|, and the
  // activity log has it.
  void RefuseExtension(std::string why);

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
  void DidFinishNavigation(content::NavigationHandle* navigation) override;
  void WebContentsDestroyed() override;

  const bc_id tab_;
  std::deque<Job> jobs_;
  // The job running, which has yet to answer.
  std::optional<Job> job_;
  // Whether a download took the job running, so its answer goes nowhere.
  bool taken_ = false;
  // An answer kept while a navigation's download starts, as it may take the job (#1446).
  struct Held {
    uint64_t request;
    std::string text;
    bool error;
  };
  std::optional<Held> held_;
  base::OneShotTimer download_starting_;
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
  // Whether the job hears of the page's requests, and waits for them to end.
  bool listening_ = false;
  bool settling_ = false;
  // The requests under way that the job heard start, by session and id, and the streams, as an
  // EventSource's, which never end, so the job doesn't wait for them.
  std::set<std::string> requests_;
  std::set<std::string> streams_;
  // When a request last started or ended, or the job started listening.
  base::TimeTicks last_request_;
  Choosing choosing_ = Choosing::kNo;
  // The options the page had before a SELECT clicked its element, so that those that show after
  // are the element's, wherever the page puts them.
  std::set<int64_t> shown_;
  // The element a SELECT chooses in, and the option it chose, with its name.
  int64_t field_ = 0;
  int64_t chosen_ = 0;
  std::string chosen_name_;
  // Whether the field shows it, for the answer.
  bool shows_ = false;
  // The page's renderer takes accessibility actions only while it serves the accessibility tree,
  // which a SELECT turns on for its tab until it answers.
  std::unique_ptr<content::ScopedAccessibilityMode> accessibility_;
  // Whether the wheel the scroll running turned moved anything.
  std::unique_ptr<WheelWatch> wheel_;
  // Keeps the page drawing while it's hidden, for the agent's input and screenshots.
  base::ScopedClosureRunner drawing_;
  // The app link the job running followed, and the app the system opens it in, once it's found.
  std::optional<GURL> app_link_;
  std::u16string app_name_;
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
