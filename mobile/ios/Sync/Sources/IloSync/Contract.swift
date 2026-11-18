// What the app sees of the account: the Spaces as the account's devices keep them (ILO_SPACE,
// core/sync_spaces.proto), and the calls that change them. The app's views read an IloAccount and
// nothing else of this package.

import Foundation
import Observation

/// One of the account's Spaces.
public struct SyncedSpace: Identifiable, Hashable, Sendable {
  /// Its node's id, a lowercase UUID.
  public let id: String
  public var name: String
  /// 0xRRGGBB, if the Space has a colour.
  public var color: UInt32?
  /// An emoji, or empty.
  public var icon: String
  public var theme: String

  public init(id: String, name: String, color: UInt32? = nil, icon: String = "", theme: String = "") {
    self.id = id
    self.name = name
    self.color = color
    self.icon = icon
    self.theme = theme
  }
}

/// A pinned tab, a folder of them, or a Today tab, in a Space.
public struct SyncedNode: Identifiable, Hashable, Sendable {
  public enum Kind: Sendable { case pin, folder, today }

  /// The entity's id as the account has it: a Today tab's keeps its "today:" prefix.
  public let id: String
  public var kind: Kind
  public var space: String
  /// The folder holding a pinned tab or folder, if one does.
  public var parent: String?
  /// A folder's name, or the name the user gave a tab.
  public var name: String
  /// A pinned tab's pinned URL or a Today tab's page. A folder has none.
  public var url: URL?
  /// The page's own title.
  public var title: String
  /// An emoji, or empty.
  public var icon: String
  /// When a device last showed a Today tab.
  public var seen: Date?

  public init(
    id: String, kind: Kind, space: String, parent: String? = nil, name: String = "", url: URL? = nil,
    title: String = "", icon: String = "", seen: Date? = nil
  ) {
    self.id = id
    self.kind = kind
    self.space = space
    self.parent = parent
    self.name = name
    self.url = url
    self.title = title
    self.icon = icon
    self.seen = seen
  }

  /// What a row shows: the user's name for it, else the page's title, else its host.
  public var label: String {
    if !name.isEmpty { return name }
    if !title.isEmpty { return title }
    return url?.host() ?? ""
  }
}

public enum AccountPhase: Equatable, Sendable {
  /// No account on this phone.
  case signedOut
  /// Signed in, but the account's data can't be read until the phone has its secret: the 24
  /// recovery words, or a pairing another device approves.
  case needsSecret
  /// The first download of the account's Spaces.
  case starting
  case ready
}

/// The server's sign-in page to show, and the address it sends the page on to when it's done,
/// which is taken instead of loaded and handed to `finishSignIn`.
public struct SignInPage: Equatable, Sendable {
  public let url: URL
  public let redirect: URL

  public init(url: URL, redirect: URL) {
    self.url = url
    self.redirect = redirect
  }
}

/// A pairing this phone asked for, which another of the account's devices approves.
public enum PairingState: Equatable, Sendable {
  case none
  /// Waiting for another device, which shows the same `code` for the user to compare.
  case waiting(code: String)
  case refused
}

public enum AccountError: Error, Equatable, Sendable {
  /// The server couldn't be reached, or answered with something else than the protocol's.
  case server(String)
  /// The words aren't 24 of the list with their checksum, or don't open the account's data.
  case wrongWords
  case signedOut
}

@MainActor
public protocol IloAccount: AnyObject, Observable {
  var phase: AccountPhase { get }
  var email: String { get }
  var server: URL { get }
  /// The last thing that went wrong, in a sentence for the user, until something works again.
  var problem: String? { get }
  var isSyncing: Bool { get }
  var pairing: PairingState { get }

  /// The Spaces in the sidebar's order.
  var spaces: [SyncedSpace] { get }
  /// A Space's pinned tabs and folders in order, those in `folder` or at the top level.
  func pinned(in space: String, folder: String?) -> [SyncedNode]
  /// A Space's Today tabs in the sidebar's order, newest first.
  func today(in space: String) -> [SyncedNode]
  func node(_ id: String) -> SyncedNode?
  /// Whether a Today tab is this phone's alone, which no other device has: one at an address
  /// the account doesn't take.
  func isOnlyHere(_ id: String) -> Bool

  /// The default server's address, which the sign-in screen offers.
  static var defaultServer: URL { get }
  func beginSignIn(server: URL) -> SignInPage
  func finishSignIn(redirect: URL) async throws
  func enterRecoveryWords(_ words: String) async throws
  /// The words of the list that start with `prefix`, for the keyboard's suggestions.
  nonisolated static func recoveryWords(startingWith prefix: String) -> [String]
  func requestPairing() async
  func cancelPairing()
  func signOut() async
  /// Asks the server for what changed. The account also hears of changes by itself while the app
  /// is in front.
  func refresh() async

  // Changes. Each shows at once and goes to the account in the background; one the server never
  // takes is undone and said in `problem`. One on something this ilo can't write, which a newer
  // ilo made or whose latest it couldn't read, does nothing but say so there.

  /// A page the phone opened, as a Today tab of `space`, first in it. The account takes web
  /// pages only: http or https, with a host its computers can read. A tab at any other address
  /// is this phone's alone, returned and listed as any Today tab and never sent, until
  /// `updateToday` says it's at a page the account takes.
  @discardableResult func openToday(url: URL, title: String, in space: String) -> SyncedNode
  /// A Today tab's page went elsewhere, or got its title. Only what differs from the tab as it
  /// is goes to the account, and an empty title never replaces one unless the address changed
  /// too. An address the account doesn't take is kept for a tab of this phone alone, and left
  /// out for one the account has, which stays at its last page there.
  func updateToday(_ id: String, url: URL, title: String)
  /// The phone is showing a Today tab now.
  func sawToday(_ id: String)
  func closeToday(_ id: String)
  /// Pins a Today tab at the end of its Space's pinned tabs, where it stops being a Today tab.
  @discardableResult func pin(today id: String) -> SyncedNode?
  /// Unpins a pinned tab, which becomes a Today tab again, first in its Space.
  @discardableResult func unpin(_ id: String) -> SyncedNode?
  /// Moves a Today tab or a pinned tab to another of the account's Spaces. A Today tab goes
  /// first among that Space's Today tabs and keeps its id. A pinned tab goes to the end of its
  /// pinned tabs, outside any folder, and is a new node there: the one returned, with a new id,
  /// in place of the old. Nil, and nothing done, for the Space the tab is in already, for a
  /// folder or a Space, and for an id or a Space the account doesn't have.
  @discardableResult func move(_ id: String, to space: String) -> SyncedNode?
  /// Renames a Space or a folder. For a pinned or Today tab this does nothing yet: the
  /// account's computers don't take a tab's name from the account, and would put the old one
  /// back.
  func rename(_ id: String, to name: String)
}

/// A login the account has saved for a site, without its password.
public struct SavedLogin: Identifiable, Hashable, Sendable {
  /// Names the entry to the account and says nothing of what's in it.
  public let id: String
  /// May be empty: a password saved with no username.
  public let username: String
  /// The site it was saved for: its scheme, host and port.
  public let realm: String

  public init(id: String, username: String, realm: String) {
    self.id = id
    self.username = username
    self.realm = realm
  }
}

public enum SavedLoginError: Error, Equatable, Sendable {
  /// The account no longer has the login as it was listed.
  case gone
}

/// The passwords the account's computers saved, which the phone reads and never changes. Apart
/// from IloAccount, so that what stands in for an account needn't have any.
@MainActor
public protocol IloPasswords: AnyObject {
  /// The logins saved for exactly this page's scheme, host and port, most recently used first.
  /// None until the account's data is open on this phone.
  func logins(for page: URL) -> [SavedLogin]
  /// A login's password, decrypted for this call and kept nowhere.
  func password(for login: SavedLogin) throws -> String
}

extension IloAccount {
  /// An account's tabs are every device's unless it says otherwise.
  public func isOnlyHere(_ id: String) -> Bool { false }
}
