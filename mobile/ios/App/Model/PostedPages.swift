import WebKit

/// The pages of a tab's history that a POST brought. WebKit keeps what such a request sent with
/// the page it brought, to send it again at a reload, and hands it out in the state a tab is
/// saved with: a password, a card's number, a message. So a tab with one of them in its history
/// keeps the state it was saved with before.
final class PostedPages {
  /// The request the tab's own frame was last let go to was a POST.
  private var posts = false
  private let brought = NSHashTable<WKBackForwardListItem>.weakObjects()

  /// A frame is let go somewhere while `entry` shows, by a request that `posts` or doesn't. A
  /// frame inside the page brings its page into that frame alone, which WebKit keeps with the
  /// page it's in.
  func asked(posts: Bool, mainFrame: Bool, on entry: WKBackForwardListItem?) {
    if mainFrame { self.posts = posts } else if posts, let entry { brought.add(entry) }
  }

  /// The tab's own frame committed `entry`. A page the server sent the browser on to was asked
  /// for anew, without what the POST sent.
  func committed(_ entry: WKBackForwardListItem?) {
    if posts, let entry { brought.add(entry) }
    posts = false
  }

  /// Whether the state WebKit hands out for `list` holds what a POST sent.
  func holds(in list: WKBackForwardList) -> Bool {
    (list.backList + [list.currentItem].compactMap { $0 } + list.forwardList).contains(where: brought.contains)
  }
}
