// The Mini Player: Chrome's Picture in Picture window for a video, placed, moved, sized and left as
// Arc's is. The first shows 10 pt from the top right of the screen, and the next where the last was
// left. A scroll moves it, a pinch or a scroll with ⌘ (Ctrl off the Mac) sizes it about its middle,
// keeping its shape, and a double-click goes back to its tab, as Chrome's back-to-tab button does.
// It stays on the display it's on. Its controls keep their clicks. Coming back to its tab's window
// takes it back to the tab. It looks as Dia's does: its corners, rim, scrim and top buttons.
#ifndef BROWSER_CORE_MINI_PLAYER_H_
#define BROWSER_CORE_MINI_PLAYER_H_

class PrefRegistrySimple;

namespace browser_core {

class Window;

// Sets the hooks that place, move and size it, before Chrome starts.
void SetUpMiniPlayer();

// Registers where the last one was left, in Local State, app-wide.
void RegisterMiniPlayerPrefs(PrefRegistrySimple* registry);

// Whether the Mini Player has the keyboard, as after a click on it.
bool MiniPlayerHasKeyboard();

// How often a video's Mini Player has shown, which Chrome does with the same window each time the
// tab floats a video again.
int MiniPlayerShows();

// The user came back to |window|, which is key again: a video's Mini Player whose tab it shows
// closes, its video playing on in the page, as Arc's does, one the page floated itself too. A call's
// floating page stays (#700).
void CameBackTo(Window& window);

}  // namespace browser_core

#endif  // BROWSER_CORE_MINI_PLAYER_H_
