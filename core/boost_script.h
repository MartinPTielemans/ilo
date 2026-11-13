#ifndef BROWSER_CORE_BOOST_SCRIPT_H_
#define BROWSER_CORE_BOOST_SCRIPT_H_

#include <cstddef>
#include <string>
#include <string_view>

class PrefService;

namespace user_prefs {
class PrefRegistrySyncable;
}

// A Boost's script, Boosts/<site>.js, runs only while its bytes are the ones the user last turned
// on (#2297's 2.1). Anything that can write the folder (an editor, a git pull, a copy from another
// computer, an agent with file tools) can change them, and the browser can't tell who did, so any
// changed byte turns the script off until the user turns it on again in the browser's own dialog.
//
// The record is the SHA-256 of those bytes, per site, in the profile's Preferences: never in or
// beside the Boosts folder, so no copy of the folder carries it, and never synced. Its one writer
// is Turn On in that dialog. The app keeps a copy of the text turned on beside it, so the dialog
// can show what changed since, and shows the copy only while Check says it's the text turned on;
// what runs is always the bytes that were hashed, never the copy.
namespace browser_core::boost_script {

// The largest script that runs, a round figure the user is told: "Script over 256 KB, off".
inline constexpr size_t kMaxBytes = 256 * 1024;

enum class State {
  kOn,        // Its bytes are the ones turned on: it runs.
  kNew,       // Nothing was turned on for the site here: off until the user turns it on.
  kChanged,   // Its bytes differ from the ones turned on: off until the user turns it on again.
  kTooLarge,  // Over kMaxBytes: off, and Turn On can't change that.
};

// The profile pref the record keeps. It isn't syncable.
void RegisterPrefs(user_prefs::PrefRegistrySyncable* registry);

// What |script|, the bytes read from |site|'s file, may do in the profile of |prefs|.
State Check(const PrefService& prefs, std::string_view site, std::string_view script);

// Records |script| as the one turned on for |site|: the record's only writer, which Turn On in the
// browser's turn-on dialog calls, never a page, an agent, sync, a link or an import. False, with
// nothing recorded, for a script too large or a site that isn't one.
bool TurnOn(PrefService& prefs, std::string_view site, std::string_view script);

// Takes |site|'s record away, as Turn Off and Delete… do: its script is new again.
void Forget(PrefService& prefs, std::string_view site);

}  // namespace browser_core::boost_script

#endif  // BROWSER_CORE_BOOST_SCRIPT_H_
