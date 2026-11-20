// The server's HTTP: the sign-in's token endpoints and sync's one command, as core/sync.cc and
// Chromium's sync engine call them.

import Foundation

/// What a sign-in or a refresh gives.
struct Tokens: Sendable {
  var access: String
  var refresh: String
  var expires: Date
  var account: String
  var device: String
  var email: String
}

enum ServerFailure: Error {
  /// No answer: the network's own words for why.
  case unreachable(String)
  /// The sign-in's code or the refresh token was refused, as it is once the device is removed
  /// or the token was used: /token's 400 with invalid_grant, and nothing else.
  case refused
  /// The access token was refused.
  case unauthorized
  case unexpected
}

/// A test's hand on the wire: it gets each request and the call that sends it, and gives the
/// answer the phone sees, so it can lose an answer the server gave or answer in its place.
typealias Wire = @Sendable (URLRequest, @Sendable () async throws -> (Data, Int)) async throws -> (Data, Int)

/// The phone's requests carry its tokens, which are for the server it signed in to and never
/// for wherever an answer sends them on to: a redirect is the answer, not followed.
private final class NoRedirects: NSObject, URLSessionTaskDelegate {
  func urlSession(
    _ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
    newRequest request: URLRequest
  ) async -> URLRequest? { nil }
}

struct Server: Sendable {
  let base: URL
  let session: URLSession
  var wire: Wire?

  /// The phone's own URLSession, which keeps nothing, no cookies and no cache, and follows no
  /// redirect.
  static func session() -> URLSession {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.httpCookieStorage = nil
    configuration.urlCache = nil
    configuration.timeoutIntervalForRequest = 30
    return URLSession(configuration: configuration, delegate: NoRedirects(), delegateQueue: nil)
  }

  private static let unreserved = CharacterSet(
    charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")

  private static func escape(_ text: String) -> String {
    text.addingPercentEncoding(withAllowedCharacters: unreserved) ?? ""
  }

  static func form(_ fields: [(String, String)]) -> String {
    fields.map { "\(escape($0.0))=\(escape($0.1))" }.joined(separator: "&")
  }

  /// `path` from the server's root, whatever path its own address has.
  func url(_ path: String, query: [(String, String)] = [], scheme: String? = nil) -> URL {
    var parts = URLComponents(url: base, resolvingAgainstBaseURL: false) ?? URLComponents()
    parts.path = path
    parts.percentEncodedQuery = query.isEmpty ? nil : Self.form(query)
    parts.fragment = nil
    if let scheme { parts.scheme = scheme }
    return parts.url ?? base
  }

  var relay: URL {
    url("/relay", scheme: base.scheme == "https" ? "wss" : "ws")
  }

  private func send(_ request: URLRequest) async throws -> (Data, Int) {
    guard let wire else { return try await transport(request) }
    return try await wire(request) { try await transport(request) }
  }

  private func transport(_ request: URLRequest) async throws -> (Data, Int) {
    do {
      let (data, response) = try await session.data(for: request)
      return (data, (response as? HTTPURLResponse)?.statusCode ?? 0)
    } catch {
      throw ServerFailure.unreachable(error.localizedDescription)
    }
  }

  private func post(_ path: String, form: [(String, String)]) -> URLRequest {
    var request = URLRequest(url: url(path))
    request.httpMethod = "POST"
    request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
    request.httpBody = Data(Self.form(form).utf8)
    return request
  }

  /// POST /token, for a sign-in's code or a refresh token.
  func token(_ form: [(String, String)]) async throws -> Tokens {
    let (data, status) = try await send(post("/token", form: form))
    let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    // Only this answer is the server saying the code or token isn't one it takes. A 401, a 403
    // or a 500, its own or a proxy's on the way, is something to try again after.
    if status == 400, json["error"] as? String == "invalid_grant" { throw ServerFailure.refused }
    guard status == 200, let access = json["access_token"] as? String,
      let refresh = json["refresh_token"] as? String, let lifetime = json["expires_in"] as? Int,
      let claims = (json["id_token"] as? String).flatMap(Self.claims),
      let account = claims["sub"] as? String, let device = claims["sid"] as? String,
      let email = claims["email"] as? String
    else { throw ServerFailure.unexpected }
    return Tokens(
      access: access, refresh: refresh, expires: Date(timeIntervalSinceNow: Double(lifetime)),
      account: account, device: device, email: email)
  }

  /// What an ID token says. Its signature isn't checked: it came from the server itself, over
  /// TLS, in answer to this request.
  private static func claims(_ token: String) -> [String: Any]? {
    let parts = token.split(separator: ".")
    guard parts.count == 3, let payload = Bytes(base64URL: String(parts[1])) else { return nil }
    return (try? JSONSerialization.jsonObject(with: Data(payload))) as? [String: Any]
  }

  /// Signs the device out at the server, which is told once and not asked again.
  func revoke(_ refresh: String) async {
    _ = try? await send(post("/revoke", form: [("token", refresh)]))
  }

  /// One of sync's messages, and its answer.
  func command(_ message: Bytes, access: String, cacheGUID: String) async throws -> SyncResponse {
    let answer = try await exchange(message, access: access, cacheGUID: cacheGUID)
    guard let response = SyncResponse(answer) else { throw ServerFailure.unexpected }
    return response
  }

  /// A message's answer as the server sent it.
  func exchange(_ message: Bytes, access: String, cacheGUID: String) async throws -> Bytes {
    var request = URLRequest(
      url: url("/command/", query: [("client", "Chromium"), ("client_id", cacheGUID)]))
    request.httpMethod = "POST"
    request.setValue("Bearer \(access)", forHTTPHeaderField: "Authorization")
    request.setValue("application/octet-stream", forHTTPHeaderField: "Content-Type")
    request.httpBody = Data(message)
    let (data, status) = try await send(request)
    if status == 401 { throw ServerFailure.unauthorized }
    guard status == 200 else { throw ServerFailure.unexpected }
    return Bytes(data)
  }
}
