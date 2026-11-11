#ifndef BROWSER_CORE_ARRANGEMENT_H_
#define BROWSER_CORE_ARRANGEMENT_H_

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>

#include "base/values.h"

class PrefService;

namespace user_prefs {
class PrefRegistrySyncable;
}

// A site's arrangement (#2347), Boosts/<site>.arrange.json: data the core's own code places, the
// same for every site, never a script. Anything that can write the Boosts folder can change it, and
// one changed outside the browser could put a button where another is expected, so it's placed
// only while its bytes are the ones the user made in this profile's Arrange or turned on in Site
// Controls, as a Boost's script is (#2313).
//
// The file is checked whole before any of it is used: every field against the schema below, with
// no field the schema doesn't name, and under the limits. One that fails is refused whole, never
// half placed. What reaches the page is the file rebuilt from its checked fields, as JSON the page's
// script parses, so nothing from the file is ever joined into script or style text, and a
// description only ever reaches querySelector and attribute comparisons.
//
// {"version": 1, "changes": [change, …]}, with each change one of:
//   {"kind": "settle", <element>, "before" | "after": {<element>}}  placed before or after that one
//   {"kind": "trade", <element>, "with": {<element>}}                the two trade places
//   {"kind": "float", <element>, "at": {"x": n, "y": n}, "pin": "page" | "screen", "width": n?}
// where <element> is "name": its words, "role": its kind (bc_arrange_element), and
// "target": {"path": a selector, "fp": {"tag", "id"?, "role"?, "label"?, "name"?, "href"?,
// "src"?, "landmark"?, "text"?, "nth"?}}, a path, a fingerprint or both.
namespace browser_core::arrangement {

inline constexpr size_t kMaxBytes = 64 * 1024;
inline constexpr size_t kMaxChanges = 64;
inline constexpr size_t kMaxString = 2 * 1024;

enum class State {
  kOn,       // Its bytes are the ones the user made or turned on here: it's placed.
  kNew,      // Nothing was turned on for the site here: off until the user turns it on.
  kChanged,  // Its bytes differ from the ones turned on: off until the user turns it on again.
  kRefused,  // It fails the schema or a limit: off, and Turn On can't change that.
};

// |bytes| rebuilt from its checked fields, or nothing if any check fails.
std::optional<base::DictValue> Parse(std::string_view bytes);

// The bytes the core hands the app for |arrangement|, one Parse accepts.
std::string Write(const base::DictValue& arrangement);

// The profile pref the record keeps. It isn't syncable.
void RegisterPrefs(user_prefs::PrefRegistrySyncable* registry);

// What |bytes|, read from |site|'s file, may do in the profile of |prefs|.
State Check(const PrefService& prefs, std::string_view site, std::string_view bytes);

// Records |bytes| as the arrangement turned on for |site|: the user's change in Arrange and Turn On
// in Site Controls call it, never a page, an agent, sync, a link or an import. False, with nothing
// recorded, for bytes Parse refuses or a site that isn't one.
bool TurnOn(PrefService& prefs, std::string_view site, std::string_view bytes);

// Takes |site|'s record away, as Reset does once nothing is left: a file there later is new.
void Forget(PrefService& prefs, std::string_view site);

// |value| if it's from |low| to |high|, else |fallback|: how a look moves, from the app's tokens,
// keeps to what reads as motion, and a value left out, 0 in a zeroed struct, or NaN takes the
// default.
double Within(double value, double low, double high, double fallback);

}  // namespace browser_core::arrangement

#endif  // BROWSER_CORE_ARRANGEMENT_H_
