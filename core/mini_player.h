// The Mini Player: Chrome's Picture in Picture window for a video, moved, sized and left as Arc's
// is. A scroll moves it, a pinch or a scroll with ⌘ (Ctrl off the Mac) sizes it about its middle,
// keeping its shape, and a double-click goes back to its tab, as Chrome's back-to-tab button does.
// It stays on the display it's on. Its controls keep their clicks.
#ifndef BROWSER_CORE_MINI_PLAYER_H_
#define BROWSER_CORE_MINI_PLAYER_H_

namespace browser_core {

// Sets the hook that moves and sizes it, before Chrome starts.
void SetUpMiniPlayer();

}  // namespace browser_core

#endif  // BROWSER_CORE_MINI_PLAYER_H_
