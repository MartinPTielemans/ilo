#ifndef BROWSER_CORE_SPACE_H_
#define BROWSER_CORE_SPACE_H_

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <vector>

#include "base/functional/function_ref.h"
#include "base/uuid.h"
#include "browser_core/public/browser_core.h"
#include "components/sync/base/unique_position.h"
#include "url/gurl.h"

namespace browser_core {

// A tab container: its pinned tabs and its Today tabs, each top to bottom (decision 5). Spaces are
// process-wide, and a window shows one.
//
// Tabs in a split view show side by side, or stacked top to bottom, and sit together in their
// list, in their order, as one row. Each of the Space's calls that moves a tab moves its split with
// it.
//
// Folders hold pinned tabs and other folders. Each sits in the pinned list followed by what's in
// it, and moves with it.
//
// A Space is the user's, or an agent's (decision 8), for as long as it lives: an agent may act only
// on the tabs of its own Space.
//
// Each Space, pinned tab and folder is a node, as sync knows them, with a random id and a place among
// its siblings (Node), which the session keeps.
class Space {
 public:
  // The most tabs a split shows.
  static constexpr size_t kMaxPanes = 3;

  // What sync knows a Space, a pinned tab or a folder by: an id, random, that it keeps for as long
  // as it lives, across launches, and its place among its siblings, the Spaces or what's in its
  // folder or outside one. Their places are in their order, and only what moves gets a new one, so
  // one moved between two others changes none of theirs. A tab moved to another Space is a new
  // node there.
  struct Node {
    base::Uuid id;
    syncer::UniquePosition position;
  };

  // An agent a Space belongs to, as its client names itself.
  struct Agent {
    // Stable across the agent's connections, and never empty.
    std::string id;
    // What the user sees, such as "Claude Code".
    std::string name;
  };

  // With the id |id|, a deleted Space's, if it isn't 0. The agent's if there's |agent|, else the
  // user's. At |at| in the Spaces' order, else after the others.
  static Space* Create(std::string name,
                       bc_id id = 0,
                       std::optional<Agent> agent = std::nullopt,
                       size_t at = SIZE_MAX);
  // What a window's own Space is for.
  enum class Own {
    // A private window's: its tabs in the profile's off-the-record profile.
    kPrivate,
    // A Little ilo window's (Arc's Little Arc): its tabs in the profile itself.
    kLittle,
  };
  // A window's own Space, its tabs in the profile in the directory |profile|, or for a private
  // window in its off-the-record profile. It holds Today tabs alone, and isn't among All(), so
  // it's never saved, archived or listed with the Spaces. It goes with its window.
  static Space* CreateOwn(std::string profile, Own own);
  // Deletes the Space, which windows no longer show.
  static void Delete(bc_id id);
  static Space* FromId(bc_id id);
  // Every Space but the windows' own, in the switcher's order, which sync's places set.
  static std::vector<Space*> All();
  // The Space holding |tab|, pinned or in Today, if any.
  static Space* Holding(bc_id tab);
  // The Space holding |folder|, if any.
  static Space* HoldingFolder(bc_id folder);
  // Whether the agent with the id |agent| may act on |tab|: only if its own Space holds it. Never
  // a favorite, which is in no Space.
  static bool AgentMayAct(const std::string& agent, bc_id tab);

  Space(const Space&) = delete;
  Space& operator=(const Space&) = delete;

  bc_id id() const { return id_; }
  const std::string& name() const { return name_; }
  // 0xRRGGBB in sRGB, if it has one.
  std::optional<uint32_t> color() const { return color_; }
  // The emoji the user gave it to show in place of its dot and before its name, or empty.
  const std::string& icon() const { return icon_; }
  const std::string& theme() const { return theme_; }
  // Whether it was ever given a name or icon other than those it was made with, which keeps a
  // device's only Space when sync first brings the account's (space_default.h). A colour or theme,
  // as onboarding gives the first Space, is setup and doesn't count.
  bool touched() const { return touched_; }
  // Puts back what the session saved of touched().
  void SetTouched(bool touched) { touched_ = touched; }
  // The directory of the profile its tabs open in, which its favorites are in (decision 5).
  const std::string& profile() const { return profile_; }
  // The agent it belongs to, or none for the user's own.
  const std::optional<Agent>& agent() const { return agent_; }
  // Whether it's a window's own (CreateOwn), which no other window shows.
  bool own() const { return own_.has_value(); }
  // Whether it's a private window's, whose tabs open in that profile's off-the-record profile
  // instead.
  bool off_the_record() const { return own_ == Own::kPrivate; }
  // Whether it's a Little ilo window's.
  bool little() const { return own_ == Own::kLittle; }
  // The Space's node. A window's own has none.
  const Node& node() const { return node_; }
  // The pinned tab's or folder's |id|'s node, or null if it isn't one of the Space's.
  const Node* NodeOf(bc_id id) const;
  // Gives the Space and its pinned tabs and folders the nodes the session kept for them, each that
  // has an id: |rows| by the pinned tab's or folder's id. Those whose places are out of their
  // order get new ones.
  void RestoreNodes(const Node& space, const std::map<bc_id, Node>& rows);
  // The pinned tabs and folders, top to bottom, each folder followed by what's in it.
  const std::vector<bc_id>& pinned_rows() const { return pinned_; }
  // The pinned tabs alone.
  std::vector<bc_id> pinned() const;
  const std::vector<bc_id>& today() const { return today_; }
  bool Has(bc_id tab) const;
  bool IsPinned(bc_id tab) const;
  // Where |tab| was when it was pinned, which Back to Pinned URL goes to. Empty if it isn't pinned.
  GURL PinnedUrl(bc_id tab) const;
  // The tabs shown with |tab|, in their order, |tab| among them: its split's, else |tab| alone.
  // Empty if the Space hasn't |tab|.
  std::vector<bc_id> Panes(bc_id tab) const;
  bool IsFolder(bc_id id) const { return folders_.contains(id); }
  // The folder holding the pinned tab or folder |id|, else 0.
  bc_id FolderOf(bc_id id) const;
  // A folder's name, its icon, empty for none, and whether it shows what's in it.
  std::string FolderName(bc_id folder) const;
  std::string FolderIcon(bc_id folder) const;
  bool IsOpen(bc_id folder) const;
  // Whether the pinned tabs and folders show, which collapsing them hides.
  bool pinned_open() const { return pinned_open_; }

  void SetName(std::string name);
  void SetColor(std::optional<uint32_t> color);
  // Gives the Space |icon|, and tells the app. Empty takes its icon away.
  void SetIcon(std::string icon);
  // Gives the Space |theme|, and tells the app. Empty takes its theme away.
  void SetTheme(std::string theme);
  void SetProfile(std::string profile);

  // Puts |tab| in Today. One the user opened (|opener| 0) goes at the top, under "+ New Tab". One a
  // page in this Space opened goes in Chrome's place: right under its opener if it opened in front,
  // or under the tabs its opener already opened behind it. A pinned opener's tabs go at the top.
  // One opened in front makes the Space forget where its other tabs came from, as Chrome's tab
  // strip does, so closing it goes back to its opener rather than to a tab opened earlier.
  void Add(bc_id tab, bc_id opener, bool foreground);
  // The user showed |shown| after |was|, either of them perhaps another Space's. Unless one opened
  // the other or one tab opened both, the user went on to something else, and the Space forgets
  // where its tabs came from, as Chrome's tab strip does on a switch the user makes.
  void UserShowed(bc_id was, bc_id shown);
  // Forgets where each tab came from, so closing one shows its neighbour, as Chrome's tab strip
  // does once the user goes somewhere other than by a link.
  void ForgetOpeners();
  // Puts |tab| at |index| in Today, else at the bottom, and below a split |index| is inside.
  void AddAt(bc_id tab, size_t index);
  // Puts |tab| at |index| in |folder|, else in the pinned tabs, counted as Move counts, else at
  // the bottom, and below a split |index| is inside, pinned at |url|.
  void AddPinned(bc_id tab, bc_id folder, size_t index, GURL url);
  // Pins |tab|, a Today tab, at the bottom of the pinned tabs, each of its split at its |url_of|.
  void Pin(bc_id tab, base::FunctionRef<GURL(bc_id)> url_of);
  // Unpins |tab| to the top of Today.
  void Unpin(bc_id tab);
  // Changes where the pinned |tab| was pinned, which Back to Pinned URL goes to, to |url|. Does
  // nothing for a tab that isn't pinned.
  void SetPinnedUrl(bc_id tab, GURL url);
  // Moves |tab| to |index| in Today, without it, or in the pinned tabs, inside |folder| if it isn't
  // 0, else to the bottom. In the pinned tabs or a folder, |index| counts the tabs and folders
  // directly in it. A Today tab moved to the pinned tabs is pinned at its |url_of|, and a pinned
  // tab moved to Today is unpinned. |tab| may be a folder, which moves with what's in it, but not
  // into itself.
  void Move(bc_id tab,
            bool pinned,
            bc_id folder,
            size_t index,
            base::FunctionRef<GURL(bc_id)> url_of);
  // Takes |tab| out of the Space, and out of its split.
  void Remove(bc_id tab);

  // Makes an open folder named |name| at |index| in |folder|, else in the pinned tabs, counted as
  // Move counts. Returns it.
  bc_id AddFolder(std::string name, bc_id folder, size_t index);
  void RenameFolder(bc_id folder, std::string name);
  void SetFolderIcon(bc_id folder, std::string icon);
  void SetOpen(bc_id folder, bool open);
  void SetPinnedOpen(bool open);
  // Deletes |folder|, leaving what was in it where it was, in the folder that held it.
  void DeleteFolder(bc_id folder);

  // Puts |tab| in a split view with |with|, at its left end if |left|, else its right end, making
  // one if |with| is in none. |tab| leaves any split it was in, and goes before or after the split
  // in its list, pinned at |url| if that's the pinned tabs and it wasn't pinned. False, doing
  // nothing, if |with|'s split is full, or either tab isn't the Space's.
  bool Split(bc_id tab, bc_id with, GURL url, bool left = false);
  // Takes |tab| out of its split, to just below the tabs left in it. The split goes once one is.
  void Separate(bc_id tab);
  // Moves |tab| to |index| in its split, and in its list with it. False, doing nothing, if it's in
  // no split or |index| is past its end.
  bool MoveInSplit(bc_id tab, size_t index);
  // Whether |tab|'s split shows its tabs stacked, top to bottom, rather than side by side.
  bool Stacked(bc_id tab) const;
  // Stacks |tab|'s split's tabs top to bottom, or puts them side by side. False, doing nothing, if
  // it's in no split.
  bool SetStacked(bc_id tab, bool stacked);
  // The share of its length each of |tab|'s split's tabs takes, in their order, adding up to 1, or
  // none while they're even, as a split starts and goes back to whenever a tab joins or leaves it.
  std::vector<double> Shares(bc_id tab) const;
  // Gives |tab|'s split's tabs |shares| of its length, in their order, scaled to add up to 1, or
  // makes them even if there are none. False, doing nothing, if it's in no split or |shares| aren't
  // one above 0 for each of its tabs.
  bool SetShares(bc_id tab, std::vector<double> shares);
  // The tab of |tab|'s split whose page took the card by SetExpanded, else 0: the others keep 220
  // pt each however wide the card is, which a window works out when it lays the pages out. A tab
  // joining or leaving the split, and SetShares, end it.
  bc_id Expanded(bc_id tab) const;
  // Gives |tab|'s page the card with its split's other pages kept at 220 pt each, or if |expanded|
  // is false ends that, leaving the pages even. False, doing nothing, if it's in no split.
  bool SetExpanded(bc_id tab, bool expanded);

  // The lowest pinned tab still |loaded|, else 0: the one to show once no Today tab is left.
  bc_id LastLoadedPinned(base::FunctionRef<bool(bc_id)> loaded) const;
  // The tab to show once |leaving| closes or, pinned, unloads. For a Today tab it's Chrome's order:
  // a tab it opened, then another tab its opener opened, then its opener, then the tab below it,
  // then the one above, and after the last Today tab the lowest pinned tab still |loaded|. For a
  // pinned tab it's the top Today tab, else the nearest pinned tab still |loaded|, below it first.
  // In a split it's the tab beside it, the one on its right first. For a tab not in the Space, a
  // favorite, it's the top Today tab, else the lowest pinned tab still |loaded|. 0 if there's none.
  bc_id NextAfterLeaving(bc_id leaving, base::FunctionRef<bool(bc_id)> loaded) const;

 private:
  Space(bc_id id, std::string name, std::optional<Agent> agent);

  bc_id OpenerOf(bc_id tab) const;
  // Makes the tabs |tab| opened its opener's, and |tab| no one's, as Chrome does when a tab moves.
  void ForgetOpener(bc_id tab);
  // The first tab below |index| opened by |opener|, else the nearest above it. 0 if none.
  bc_id NearestOpenedBy(bc_id opener, size_t index) const;
  struct SplitView {
    // In their order, as they are in their list.
    std::vector<bc_id> tabs;
    bool stacked = false;
    // Each tab's share of its length, or none while they're even.
    std::vector<double> shares;
    // The tab that took the card, else 0, while the others are kept at 220 pt.
    bc_id expanded = 0;
  };

  // The split holding |tab|, or splits_.end().
  std::vector<SplitView>::iterator SplitOf(bc_id tab);
  std::vector<SplitView>::const_iterator SplitOf(bc_id tab) const;
  // |index| in |tabs|, or the index past the split it's inside, so a tab put there isn't in one.
  size_t OutsideSplits(const std::vector<bc_id>& tabs, size_t index) const;
  // Takes |tab| out of its split, leaving it where it is in its list.
  void LeaveSplit(bc_id tab);
  // Gives each pinned tab and folder without a node one, and a new place to each out of its order.
  void PlaceRows();
  // The same for the Spaces.
  static void PlaceSpaces();
  // Whether |id| is |folder| or inside it.
  bool Within(bc_id id, bc_id folder) const;
  // Past the end of the rows of |folder|, else of the pinned list, in |pinned_|.
  size_t EndOf(bc_id folder) const;
  // Where in |pinned_| what goes at |index| in |folder| goes, counted as Move counts.
  size_t PinnedAt(bc_id folder, size_t index) const;
  void ReportChanged();

  const bc_id id_;
  const std::optional<Agent> agent_;
  std::string name_;
  std::optional<uint32_t> color_;
  std::string icon_;
  std::string theme_;
  std::string profile_;
  bool touched_ = false;
  std::optional<Own> own_;
  Node node_;
  // Pinned tabs and folders.
  std::vector<bc_id> pinned_;
  // Each pinned tab's and folder's.
  std::map<bc_id, Node> nodes_;
  std::map<bc_id, GURL> pinned_urls_;
  struct Folder {
    std::string name;
    std::string icon;
    bool open = true;
  };
  std::map<bc_id, Folder> folders_;
  bool pinned_open_ = true;
  // Each pinned tab or folder inside a folder, to the folder.
  std::map<bc_id, bc_id> folder_of_;
  std::vector<bc_id> today_;
  // Each tab a page opened, to the tab it came from.
  std::map<bc_id, bc_id> openers_;
  std::vector<SplitView> splits_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SPACE_H_
