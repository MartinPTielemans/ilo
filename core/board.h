// chrome://board/<id>, a Board (#3073): a page of the browser's own in the tab's native view, whose
// canvas is the app's code in its board/ folder, and whose cards are in the profile's Boards/<id>/
// (board_store.h). The page reads its Board with GETs from the host, and writes it through
// chrome.send in slices, each answered before the next is sent, so the UI thread only hands each
// to the Boards' sequence, where the hash and the writes run. A page writes the Board its address
// named when it committed, and no other.

#ifndef BROWSER_CORE_BOARD_H_
#define BROWSER_CORE_BOARD_H_

namespace browser_core {

// Makes chrome://board a host of the browser's, before the first profile is made.
void RegisterBoards();

}  // namespace browser_core

#endif  // BROWSER_CORE_BOARD_H_
