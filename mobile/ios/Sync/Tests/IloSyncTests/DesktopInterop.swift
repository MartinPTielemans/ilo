// The phone beside a real desktop ilo on one account: with ILO_INTEROP naming a file, the phone
// signs in to the account that file says a desktop made on a local ilo-server, then does what
// it's told, one step a line, so a script can read the desktop between steps
// (~/.cache/ilo-ios-spike/interop/run.sh is that script). Without ILO_INTEROP this is skipped.
//
// The file's lines: "server <url>", "email <address>", "words <the 24>", "log <the server's log,
// for the sign-in code>" and "dir <a folder>". Each line of <dir>/phone.in is a JSON object with
// "n" and "do"; its answer is a line of <dir>/phone.out with the same "n". "do" is one of
// snapshot, refresh, openToday, updateToday, sawToday, closeToday, pin, unpin, rename, nameAnyway,
// age, logins and quit.

import Foundation
import XCTest

@testable import IloSync

final class DesktopInterop: XCTestCase {
  @MainActor
  func testBesideADesktop() async throws {
    guard let path = ProcessInfo.processInfo.environment["ILO_INTEROP"] else {
      throw XCTSkip("ILO_INTEROP names no file")
    }
    var given: [String: String] = [:]
    for line in try String(contentsOfFile: path, encoding: .utf8).split(separator: "\n") {
      let parts = line.split(separator: " ", maxSplits: 1)
      if parts.count == 2 { given[String(parts[0])] = String(parts[1]) }
    }
    let server = try XCTUnwrap(given["server"].flatMap { URL(string: $0) })
    let email = try XCTUnwrap(given["email"])
    let words = try XCTUnwrap(given["words"])
    let log = URL(fileURLWithPath: try XCTUnwrap(given["log"]))
    let directory = URL(fileURLWithPath: try XCTUnwrap(given["dir"]))
    // Never the hosted server or a real account.
    guard ["localhost", "127.0.0.1"].contains(server.host() ?? ""), email.hasSuffix("@example.org") else {
      throw XCTSkip("\(server) as \(email) isn't a local server's throwaway account")
    }

    let state = directory.appendingPathComponent("phone-state-\(UUID().uuidString.lowercased())")
    defer { try? FileManager.default.removeItem(at: state) }
    // As the app makes it: a change waits a second for the next before it's sent.
    let account = Account(
      secrets: MemorySecretStore(), state: FileStateStore(directory: state), deviceName: "ilo on iPhone")
    defer {
      account.settleTask?.cancel()
      account.retryTask?.cancel()
      account.active = false
      account.closeRelay()
    }
    let page = account.beginSignIn(server: server)
    try await account.finishSignIn(redirect: try await Self.signedIn(page, email: email, log: log))
    XCTAssertEqual(account.phase, .needsSecret)
    try await account.enterRecoveryWords(words)
    XCTAssertEqual(account.phase, .ready)

    let asked = directory.appendingPathComponent("phone.in")
    let answered = directory.appendingPathComponent("phone.out")
    FileManager.default.createFile(atPath: answered.path, contents: nil)
    let answers = try FileHandle(forWritingTo: answered)
    func answer(_ n: Any, _ body: [String: Any]) throws {
      var line = body
      line["n"] = n
      line["state"] = Self.snapshot(account)
      try answers.seekToEnd()
      try answers.write(contentsOf: JSONSerialization.data(withJSONObject: line, options: [.sortedKeys]) + Data("\n".utf8))
    }
    try answer(0, ["did": "signed in"])

    var read = 0
    while FileManager.default.fileExists(atPath: path) {
      let lines = ((try? String(contentsOf: asked, encoding: .utf8)) ?? "").split(separator: "\n")
      guard read < lines.count else {
        try await Task.sleep(for: .milliseconds(50))
        continue
      }
      let line = lines[read]
      read += 1
      guard let ask = try JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any],
        let what = ask["do"] as? String
      else { continue }
      let n = ask["n"] ?? read
      func text(_ name: String) -> String { ask[name] as? String ?? "" }
      var said: [String: Any] = ["did": what]
      switch what {
      case "snapshot":
        break
      case "refresh":
        await account.refresh()
      case "openToday":
        let node = account.openToday(url: try XCTUnwrap(URL(string: text("url"))), title: text("title"), in: text("space"))
        said["id"] = node.id
      case "updateToday":
        account.updateToday(text("id"), url: try XCTUnwrap(URL(string: text("url"))), title: text("title"))
      case "sawToday":
        account.sawToday(text("id"))
      case "closeToday":
        account.closeToday(text("id"))
      case "pin":
        said["id"] = account.pin(today: text("id"))?.id ?? NSNull()
      case "unpin":
        said["id"] = account.unpin(text("id"))?.id ?? NSNull()
      case "move":
        said["id"] = account.move(text("id"), to: text("space"))?.id ?? NSNull()
      case "rename":
        account.rename(text("id"), to: text("name"))
      case "nameAnyway":
        // A name written as a computer writes one, past rename(_:to:), which sends none for a tab
        // while the computer puts the old one back: what the computer does with it is the question.
        var change = account.state.changes[text("id")] ?? Change()
        change.name = text("name")
        account.revision += 1
        change.revision = account.revision
        account.state.changes[text("id")] = change
        account.changed()
        account.requestSync()
      case "age":
        // An hour or more passes for the phone alone: what it has of a Today tab's last showing
        // is that much older, so its next showing is one to tell the account of. Nothing is sent.
        let by = (ask["seconds"] as? NSNumber)?.int64Value ?? 7200
        if let seen = account.state.records[text("id")]?.entity.seen {
          account.state.records[text("id")]?.entity.seen = seen - by
          account.rebuild()
        }
      case "logins":
        // The logins the account has for a page, after a refresh, and each one's password as the
        // phone decrypts it: a made-up login of the run's own, written out for the script to compare.
        await account.refresh()
        let page = try XCTUnwrap(URL(string: text("url")))
        let logins = account.logins(for: page)
        said["logins"] = logins.map(\.username)
        said["passwords"] = logins.map { (try? account.password(for: $0)) ?? "" }
        said["realm"] = PasswordForm.realm(of: page) ?? ""
        said["entities"] = account.state.passwords?.count ?? 0
      case "quit":
        try answer(n, said)
        return
      default:
        said["error"] = "no such step"
      }
      // What the change left to send, before anything is.
      said["pending"] = account.state.changes.keys.sorted()
      try answer(n, said)
    }
  }

  /// What the phone shows, by the calls the app's views make, and beside each row what the
  /// account has of it as the phone last heard: its version there and its place.
  @MainActor
  static func snapshot(_ account: Account) -> [String: Any] {
    func row(_ node: SyncedNode, in space: String) -> [String: Any] {
      var said: [String: Any] = [
        "id": node.id, "kind": node.kind == .today ? "today" : node.kind == .folder ? "folder" : "pin",
        "name": node.name, "url": node.url?.absoluteString ?? "", "title": node.title,
        "parent": node.parent ?? "",
      ]
      if let seen = node.seen { said["seen"] = Int(seen.timeIntervalSince1970) }
      if node.kind == .folder {
        said["rows"] = account.pinned(in: space, folder: node.id).map { row($0, in: space) }
      }
      return said
    }
    var records: [String: Any] = [:]
    for (id, record) in account.state.records {
      records[id] = [
        "version": record.version, "hash": record.hash,
        "position": Data(record.entity.position ?? []).base64EncodedString(),
        "bytes": Data(record.entity.serialized).base64EncodedString(),
      ]
    }
    return [
      "phase": "\(account.phase)", "problem": account.problem ?? "", "syncing": account.isSyncing,
      "unsent": account.state.changes.keys.sorted(),
      "spaces": account.spaces.map { space -> [String: Any] in
        [
          "id": space.id, "name": space.name, "color": space.color.map { Int($0) } ?? -1,
          "icon": space.icon, "theme": space.theme,
          "pinned": account.pinned(in: space.id, folder: nil).map { row($0, in: space.id) },
          "today": account.today(in: space.id).map { row($0, in: space.id) },
        ]
      },
      "records": records,
    ]
  }

  /// The server's sign-in page walked as the phone's web view walks it, with the code the
  /// server logs, to the address the page sends the view on to (Support.swift's signedIn, for a
  /// server this process didn't start).
  @MainActor
  static func signedIn(_ page: SignInPage, email: String, log: URL) async throws -> URL {
    final class NoRedirects: NSObject, URLSessionTaskDelegate {
      func urlSession(
        _ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest
      ) async -> URLRequest? { nil }
    }
    let browser = URLSession(configuration: .ephemeral, delegate: NoRedirects(), delegateQueue: nil)
    defer { browser.invalidateAndCancel() }
    func load(_ path: String, form: [(String, String)]? = nil) async throws -> (String, HTTPURLResponse) {
      var request = URLRequest(url: URL(string: path, relativeTo: page.url)!.absoluteURL)
      if let form {
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        request.httpBody = Data(Server.form(form).utf8)
      }
      let (data, response) = try await browser.data(for: request)
      return (String(decoding: data, as: UTF8.self), response as! HTTPURLResponse)
    }
    func codes() -> [String] {
      ((try? String(contentsOf: log, encoding: .utf8)) ?? "").split(separator: "\n")
        .filter { $0.contains(email) }.compactMap { line in
          line.range(of: " is your ilo sign-in code").map { String(line[..<$0.lowerBound].suffix(6)) }
        }
    }
    let first = try await load(page.url.absoluteString)
    XCTAssertEqual(first.1.statusCode, 200)
    let before = codes().count
    let asked = try await load("/authorize/email", form: [("email", email)])
    XCTAssertTrue(asked.0.contains("Check your email"))
    let deadline = Date(timeIntervalSinceNow: 20)
    while codes().count <= before, Date() < deadline { try await Task.sleep(for: .milliseconds(50)) }
    var answer = try await load("/authorize/code", form: [("code", codes().last ?? "")])
    if answer.1.statusCode == 200 {
      // An account with no passkey offers one: not now.
      answer = try await load("/authorize/continue", form: [])
    }
    XCTAssertEqual(answer.1.statusCode, 303)
    let done = try await load("/authorize/done")
    let marker = #"http-equiv="refresh" content="1; url="#
    guard let start = done.0.range(of: marker)?.upperBound, let end = done.0[start...].firstIndex(of: "\"")
    else { throw XCTSkip("the sign-in page didn't finish") }
    return try XCTUnwrap(URL(string: done.0[start..<end].replacingOccurrences(of: "&amp;", with: "&")))
  }
}
