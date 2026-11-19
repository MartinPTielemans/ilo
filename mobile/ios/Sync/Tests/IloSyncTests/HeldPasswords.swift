// Saved passwords for the account HeldAccount holds, to try the app's sign-in offer by hand:
// with ILO_HELD naming that account's file and ILO_LOGIN_SITES the made-up logins to save, as
// "realm|username|password" separated by commas, a second computer signs in to the held server
// and saves them as a desktop does. Without ILO_HELD this is skipped.

import Foundation
import XCTest

@testable import IloSync

final class HeldPasswords: XCTestCase {
  @MainActor
  func testSavesPasswordsToTheHeldAccount() async throws {
    let environment = ProcessInfo.processInfo.environment
    guard let hold = environment["ILO_HELD"], let sites = environment["ILO_LOGIN_SITES"] else {
      throw XCTSkip("ILO_HELD names no file")
    }
    let line: @Sendable (String) -> [String] = { name in
      ((try? String(contentsOfFile: hold, encoding: .utf8)) ?? "").split(separator: "\n")
        .filter { $0.hasPrefix(name + " ") }.map { String($0.dropFirst(name.count + 1)) }
    }
    let server = try XCTUnwrap(line("server").first.flatMap(URL.init))
    XCTAssertEqual(server.host, "localhost", "only ever the held local server")
    let email = try XCTUnwrap(line("email").first)
    let computer = try await Computer(at: server, email: email) { line("code") }
    try await computer.join(words: try XCTUnwrap(line("words").first))
    var saved = 0
    for (index, site) in sites.split(separator: ",").enumerated() {
      let parts = site.split(separator: "|", omittingEmptySubsequences: false).map(String.init)
      guard parts.count == 3 else { continue }
      let data = passwordData(realm: parts[0], username: parts[1], password: parts[2], used: Int64(1000 - index))
      // A new tag each run: this computer doesn't know what an earlier one saved.
      let answer = try await computer.put(password: data, tag: "held-\(UUID().uuidString)")
      XCTAssertEqual(answer.result, .success)
      saved += 1
    }
    print("HELD PASSWORDS: saved \(saved)")
  }
}
