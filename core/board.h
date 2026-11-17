// chrome://board/<id>, a Board (#3073): a page of the browser's own in the tab's native view, whose
// canvas is the app's code in its board/ folder, and whose cards are in the profile's Boards/<id>/
// (board_store.h). The page reads its Board with GETs from the host, and writes it through
// chrome.send in slices, each answered before the next is sent, so the UI thread only hands each
// to the Boards' sequence, where the hash and the writes run. A page writes the Board its address
// named when it committed, and no other.

#ifndef BROWSER_CORE_BOARD_H_
#define BROWSER_CORE_BOARD_H_

#include <cstdint>
#include <string_view>

#include "base/containers/span.h"
#include "base/functional/callback.h"

class Profile;

namespace browser_core {

// Makes chrome://board a host of the browser's, before the first profile is made.
void RegisterBoards();

// Puts |png|, a capture, into |profile|'s Board |id| as a stored file and into its arrivals, for
// its page to make a card of, after the call returns, then runs |done| with whether it's there.
// False, |done| dropped and nothing put there, for a profile that keeps no Boards, an id that isn't
// one, or bytes that aren't a PNG of at most board::kMaxFile.
bool AddCapture(Profile* profile,
                std::string_view id,
                base::span<const uint8_t> png,
                base::OnceCallback<void(bool)> done);

}  // namespace browser_core

#endif  // BROWSER_CORE_BOARD_H_
