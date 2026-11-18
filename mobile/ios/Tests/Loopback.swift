import Foundation
import Network

@testable import ilo

/// A web server on this machine alone, for the pages a test makes up: it answers each request as
/// the test says, and keeps what was asked of it.
nonisolated final class Loopback: @unchecked Sendable {
  struct Request: Sendable {
    var method: String
    var path: String
    var headers: [String: String]
    var body: Data
  }

  struct Response: Sendable {
    var status = 200
    var headers: [String: String] = [:]
    var body = Data()
    /// Keeps the body's end back until a test says, as a slow file's is.
    var held: Held?

    /// A file to keep, under a name: what a link to a download answers with.
    static func file(_ name: String, _ body: Data, type: String = "application/octet-stream", held: Held? = nil) -> Response {
      Response(headers: ["Content-Type": type, "Content-Disposition": "attachment; filename=\"\(name)\""], body: body, held: held)
    }

    static func html(_ body: String, headers: [String: String] = [:]) -> Response {
      let page = "<!doctype html><meta name=viewport content='width=device-width'>" + body
      return Response(headers: headers.merging(["Content-Type": "text/html; charset=utf-8"]) { mine, _ in mine }, body: Data(page.utf8))
    }
  }

  /// The first bytes of a body go at once; the rest when the gate opens, or the connection is
  /// dropped then, as a network that went away drops it.
  struct Held: Sendable {
    var first: Int
    var gate: Gate
    var drops = false
  }

  /// Opened once by a test, to let what waits on it go on.
  final class Gate: @unchecked Sendable {
    private let lock = NSLock()
    private var isOpen = false
    private var waiting: [@Sendable () -> Void] = []

    func open() {
      let work = lock.withLock {
        isOpen = true
        defer { waiting = [] }
        return waiting
      }
      work.forEach { $0() }
    }

    func whenOpen(_ work: @escaping @Sendable () -> Void) {
      let now = lock.withLock {
        if !isOpen { waiting.append(work) }
        return isOpen
      }
      if now { work() }
    }
  }

  private let listener: NWListener
  private let queue = DispatchQueue(label: "loopback")
  private let lock = NSLock()
  private var seen: [Request] = []
  /// What each request gets. Nothing is a connection closed with no answer.
  private let answer: @Sendable (Request) -> Response?
  var port: UInt16 { listener.port?.rawValue ?? 0 }
  /// Whether it speaks TLS, with a certificate a test made.
  private let secure: Bool

  /// With an `identity`, the server speaks TLS and proves itself with it.
  init(identity: SecIdentity? = nil, _ answer: @escaping @Sendable (Request) -> Response?) async throws {
    self.answer = answer
    let parameters: NWParameters
    if let identity, let local = sec_identity_create(identity) {
      let tls = NWProtocolTLS.Options()
      sec_protocol_options_set_local_identity(tls.securityProtocolOptions, local)
      parameters = NWParameters(tls: tls)
    } else {
      parameters = .tcp
    }
    secure = identity != nil
    parameters.requiredLocalEndpoint = .hostPort(host: .ipv4(.loopback), port: .any)
    let listener = try NWListener(using: parameters)
    self.listener = listener
    let queue = queue
    listener.newConnectionHandler = { [weak self] connection in
      connection.start(queue: queue)
      Self.read(connection, Data()) { [weak self] request in self?.serve(request, on: connection) }
    }
    try await withCheckedThrowingContinuation { (ready: CheckedContinuation<Void, any Error>) in
      listener.stateUpdateHandler = { state in
        switch state {
        case .ready:
          listener.stateUpdateHandler = nil
          ready.resume()
        case .failed(let error):
          listener.stateUpdateHandler = nil
          ready.resume(throwing: error)
        default: break
        }
      }
      listener.start(queue: queue)
    }
  }

  deinit {
    listener.cancel()
  }

  /// An address here, by the machine's number or by another `host` that means this machine.
  func url(_ path: String, host: String = "127.0.0.1") -> URL {
    URL(string: "\(secure ? "https" : "http")://\(host):\(port)\(path)")!
  }

  /// Every request so far, in the order they came.
  var requests: [Request] {
    lock.withLock { seen }
  }

  func requests(to path: String, method: String = "GET") -> Int {
    requests.count { $0.path == path && $0.method == method }
  }

  private func serve(_ request: Request, on connection: NWConnection) {
    lock.withLock { seen.append(request) }
    guard let response = answer(request) else { return connection.cancel() }
    var head = "HTTP/1.1 \(response.status) OK\r\nContent-Length: \(response.body.count)\r\nConnection: close\r\n"
    for (name, value) in response.headers { head += "\(name): \(value)\r\n" }
    guard let held = response.held else {
      return connection.send(content: Data((head + "\r\n").utf8) + response.body, completion: .contentProcessed { _ in connection.cancel() })
    }
    let rest = response.body.dropFirst(held.first)
    connection.send(content: Data((head + "\r\n").utf8) + response.body.prefix(held.first), completion: .contentProcessed { _ in })
    held.gate.whenOpen {
      if held.drops { return connection.cancel() }
      connection.send(content: Data(rest), completion: .contentProcessed { _ in connection.cancel() })
    }
  }

  /// Reads one request: its head, then as much body as the head says there is.
  private static func read(_ connection: NWConnection, _ had: Data, then: @escaping @Sendable (Request) -> Void) {
    connection.receive(minimumIncompleteLength: 1, maximumLength: 1 << 16) { data, _, done, error in
      let all = had + (data ?? Data())
      if let request = parse(all) { return then(request) }
      if done || error != nil { return connection.cancel() }
      read(connection, all, then: then)
    }
  }

  private static func parse(_ data: Data) -> Request? {
    guard let end = data.range(of: Data("\r\n\r\n".utf8)) else { return nil }
    let lines = String(decoding: data[..<end.lowerBound], as: UTF8.self).components(separatedBy: "\r\n")
    let first = lines[0].split(separator: " ")
    guard first.count >= 2 else { return nil }
    var headers: [String: String] = [:]
    for line in lines.dropFirst() {
      let parts = line.split(separator: ":", maxSplits: 1)
      if parts.count == 2 { headers[parts[0].lowercased()] = parts[1].trimmingCharacters(in: .whitespaces) }
    }
    let body = data[end.upperBound...]
    guard body.count >= Int(headers["content-length"] ?? "") ?? 0 else { return nil }
    return Request(method: String(first[0]), path: String(first[1]), headers: headers, body: Data(body))
  }
}

/// A browser of its own for a test, with the phone's own Space and nothing kept from another run.
@MainActor func testBrowser() -> Browser {
  let directory = URL.temporaryDirectory.appending(path: "path-tests-\(UUID().uuidString)")
  return Browser(
    account: PhoneAccount(account: FixtureAccount(signedIn: false), directory: directory.appending(path: "phone")),
    settings: Settings(defaults: UserDefaults(suiteName: "path-tests-\(UUID().uuidString)")!),
    cache: TabCache(directory: directory.appending(path: "tabs")),
    favicons: Favicons(directory: directory.appending(path: "favicons")),
    downloads: testDownloads(in: directory))
}

/// A downloads list of a test's own: its folder, its file and its parts under `directory`.
@MainActor func testDownloads(in directory: URL) -> Downloads {
  Downloads(
    folder: directory.appending(path: "files"), record: directory.appending(path: "downloads/list.json"),
    parts: directory.appending(path: "parts"))
}

/// Waits for something a page or the network brings about, and says whether it came.
@MainActor func eventually(_ came: @MainActor () async -> Bool) async -> Bool {
  // Half a minute at most: CI's iOS 27 simulator takes ten seconds over a page that an iOS 26
  // one loads in a third of one, and what came is answered at once either way.
  for _ in 0..<3000 {
    if await came() { return true }
    try? await Task.sleep(for: .milliseconds(10))
  }
  return false
}

extension WebTab {
  /// What a script answers in the page, run as the user's own tap would run it.
  func run(_ script: String) async -> Any? {
    try? await webView.callAsyncJavaScript(script, contentWorld: .page)
  }

  /// Whether the page that's showing is the one at `path`, loaded. The page says so itself: the
  /// web view names an address before the page at it has replaced the one before.
  func shows(_ path: String) async -> Bool {
    await run("return document.readyState === 'complete' ? location.pathname : ''") as? String == path
  }
}
