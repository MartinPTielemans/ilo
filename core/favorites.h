#ifndef BROWSER_CORE_FAVORITES_H_
#define BROWSER_CORE_FAVORITES_H_

#include <string>
#include <vector>

#include "browser_core/public/browser_core.h"
#include "url/gurl.h"

namespace browser_core {

class Space;

// The favorites: the tiles above a Space's own tabs. A favorite is a site the user keeps there, and
// each profile that shows it has a tab of its own for it, in no Space, as a profile's logins are
// its own. The scope (BC_FAVORITES_) says who shares a list: every Space, whatever its profile (the
// same sites, each profile opening them in its own tabs), the Spaces of one profile (Arc), or no
// two Spaces. A tab is a favorite until it's taken out of the favorites or closes. Each keeps the
// URL it was made a favorite at, which Back to Favorite URL goes to.
class Favorites {
 public:
  Favorites() = delete;

  // A BC_FAVORITES_ value: the user's choice, which the session keeps.
  static int scope();
  // Makes the scope |scope|, without losing a favorite. Narrowing gives each Space or profile its
  // own copy of the list, its tabs unloaded but for those already loaded, which stay with one
  // Space. Widening merges the lists by URL, the list of the Space the window active last shows
  // first and then each other's extras in the order of the Spaces, and closes a duplicate's extra
  // tabs, keeping a loaded one over an unloaded one. Does nothing for a value that isn't one.
  static void SetScope(int scope);

  // Every favorite's tab: each profile's, and each Space's under Each Space, each list left to
  // right. Not those being closed.
  static std::vector<bc_id> All();
  // The favorites in the profile in the directory |profile|, in any Space's list.
  static std::vector<bc_id> Of(const std::string& profile);
  // The favorites shown above |space|, left to right.
  static std::vector<bc_id> In(const Space& space);
  // The favorites that belong to the Space |space| under Each Space, left to right.
  static std::vector<bc_id> OwnedBy(bc_id space);
  // Whether |space| shows |tab| among its favorites.
  static bool ShownIn(bc_id tab, const Space& space);
  // The directory of the favorite |tab|'s profile, or empty if it isn't a favorite.
  static std::string ProfileOf(bc_id tab);
  // The Space the favorite |tab| belongs to, under Each Space, else 0.
  static bc_id SpaceOf(bc_id tab);
  // What names |tab|'s site, shared by the tabs of one site in each profile under Every Space.
  // 0 if it isn't a favorite.
  static int SiteOf(bc_id tab);
  // Whether |tab| is a favorite, or one being closed.
  static bool Has(bc_id tab);
  // Empty if |tab| isn't a favorite.
  static GURL Url(bc_id tab);

  // Makes |tab|, a tab of |space| and in no Space now, the favorite at |index| of the list |space|
  // shows, else the last, at |url|. Under Every Space the other profiles that show favorites get
  // the site, at the same place, in tabs of their own.
  static void Add(bc_id tab, GURL url, size_t index, bc_id space);
  // Puts back |tab|, a favorite of the last launch, at the end of its list. It belongs to |space|
  // under Each Space, else 0, and |site| names its site, else 0.
  static void Restore(bc_id tab, GURL url, bc_id space, int site);
  // Gives the favorite |tab| the URL it goes back to, |url|, and under Every Space the site's tabs
  // in the other profiles too, so an undo restores the site as Add made it. Does nothing if it
  // isn't one.
  static void SetUrl(bc_id tab, GURL url);
  // Moves the favorite |tab| to |index| of its list without it, else to the end. Under Every
  // Space, the site moves in every profile.
  static void Move(bc_id tab, size_t index);
  // |tab| is no longer a favorite, or is gone.
  static void Remove(bc_id tab);
  // Takes |tab| out of the favorites, as Remove does, and under Every Space closes the site's tabs
  // in the other profiles.
  static void Unfavorite(bc_id tab);
  // The Space |space| was given another profile, which has loaded: its favorites, under Each Space,
  // go to tabs there, and under Every Space the profile gets the sites it lacks.
  static void SpaceMoved(bc_id space);
  // The Space |space| is going: under Each Space, its favorites close with it.
  static void SpaceDeleting(bc_id space);
  // The profile in the directory |profile| is going: its favorites' tabs close with it. Under
  // Every Space the other profiles keep the sites.
  static void ProfileDeleting(const std::string& profile);
  // Under Every Space, gives each profile that shows favorites a tab for each site it lacks.
  static void Complete();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_FAVORITES_H_
