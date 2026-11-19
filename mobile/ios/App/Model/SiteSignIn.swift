import Foundation

/// WebKit's handler for a challenge, called once whatever becomes of the challenge: WebKit
/// raises on a second call, and on a handler let go of uncalled.
final class ChallengeAnswer {
  typealias Handler = @MainActor @Sendable (URLSession.AuthChallengeDisposition, URLCredential?) -> Void

  private var handler: Handler?

  init(_ handler: @escaping Handler) {
    self.handler = handler
  }

  func give(_ disposition: URLSession.AuthChallengeDisposition, _ credential: URLCredential? = nil) {
    let handler = handler
    self.handler = nil
    handler?(disposition, credential)
  }

  // A challenge nobody answered, as one whose tab closed, is cancelled.
  isolated deinit {
    handler?(.cancelAuthenticationChallenge, nil)
  }
}

/// A site's question for a name and password, waiting on the user.
final class SiteSignIn: Identifiable {
  /// What a name and password are for: one realm of one server, asked for in one way.
  nonisolated struct Door: Hashable, Sendable {
    var host: String
    var port: Int
    var realm: String?
    var method: String
    var isProxy: Bool

    init(_ space: URLProtectionSpace) {
      host = space.host.lowercased()
      port = space.port
      realm = space.realm
      method = space.authenticationMethod
      isProxy = space.isProxy()
    }
  }

  let door: Door
  /// The page's own site, when it isn't the one that asks: a part of the page from elsewhere
  /// asks under its own name, never the page's.
  let page: String?
  /// Whether the password would travel where it can be read.
  let unencrypted: Bool
  /// Whether the last answer was turned down.
  let wrong: Bool
  /// The name given last time, to try another password with.
  let name: String

  private var answers: [ChallengeAnswer]

  init(_ challenge: URLAuthenticationChallenge, on page: URL?, name: String, answer: ChallengeAnswer) {
    let space = challenge.protectionSpace
    door = Door(space)
    let own = page?.host().map { Site.registrableDomain(of: $0) == Site.registrableDomain(of: space.host) } ?? true
    self.page = own || space.isProxy() ? nil : Site.name(of: page)
    unencrypted = !space.receivesCredentialSecurely
    wrong = challenge.previousFailureCount > 0
    self.name = name
    answers = [answer]
  }

  /// The server that asks, as the sheet names it.
  var host: String { door.host }

  /// The site's own words for what it guards, cut to what a line holds: they're quoted as its
  /// words, and can't run on into ilo's.
  var realm: String? {
    guard let realm = door.realm else { return nil }
    let plain = realm.unicodeScalars.filter { !CharacterSet.controlCharacters.contains($0) && !CharacterSet.newlines.contains($0) }
    let words = String(String.UnicodeScalarView(plain)).trimmingCharacters(in: .whitespaces)
    guard !words.isEmpty, words.lowercased() != host else { return nil }
    return words.count > Self.realmLength ? words.prefix(Self.realmLength) + "…" : words
  }

  static let realmLength = 60

  /// The same door asking again while the sheet is up gets the answer the sheet gives.
  func add(_ answer: ChallengeAnswer) {
    answers.append(answer)
  }

  func give(_ disposition: URLSession.AuthChallengeDisposition, _ credential: URLCredential? = nil) {
    let answers = answers
    self.answers = []
    for answer in answers { answer.give(disposition, credential) }
  }
}
