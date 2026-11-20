// An account to try the app on by hand: with ILO_HOLD naming a file, a local ilo-server stays up
// with an account a computer made, the file says how to sign in to it, and it all ends when the
// file is removed. Without ILO_HOLD this is skipped.

import Foundation
import XCTest

@testable import IloSync

final class HeldAccount: XCTestCase {
  @MainActor
  func testHoldsAnAccount() async throws {
    guard let hold = ProcessInfo.processInfo.environment["ILO_HOLD"] else {
      throw XCTSkip("ILO_HOLD names no file")
    }
    let server = try TestServer()
    defer { server.stop() }
    let email = "check-\(Int(Date().timeIntervalSince1970))@example.org"
    let computer = try await Computer(server: server, email: email)
    try await computer.setUpAccount()
    var last = UniquePosition.initial(suffix: UniquePosition.randomSuffix())
    func row(_ kind: SpaceEntity.Kind, in space: String? = nil, name: String = "", url: String? = nil, title: String = "")
      -> SpaceEntity
    {
      last = .after(last, suffix: UniquePosition.randomSuffix())
      var entity = SpaceEntity()
      let id = UUID().uuidString.lowercased()
      entity.id = kind == .today ? SpaceEntity.todayPrefix + id : id
      entity.kind = kind
      entity.space = space
      entity.position = last.proto
      entity.name = name
      entity.url = url
      entity.title = title
      if kind == .today { entity.seen = Int64(Date().timeIntervalSince1970) - 600 }
      return entity
    }
    var personal = row(.space, name: "Personal")
    personal.color = 0x289054
    try await computer.put(personal)
    var work = row(.space, name: "Work")
    work.color = 0xEF8C63
    try await computer.put(work)
    let home = personal.id!
    try await computer.put(row(.pin, in: home, url: "https://en.wikipedia.org/wiki/Main_Page", title: "Wikipedia"))
    try await computer.put(row(.pin, in: home, url: "https://news.ycombinator.com/", title: "Hacker News"))
    let reading = row(.folder, in: home, name: "Reading")
    try await computer.put(reading)
    var inside = row(.pin, in: home, url: "https://www.theguardian.com/international", title: "The Guardian")
    inside.parent = reading.id
    try await computer.put(inside)
    try await computer.put(row(.pin, in: home, url: "https://www.openstreetmap.org/", title: "OpenStreetMap"))
    try await computer.put(row(.today, in: home, url: "https://example.org/", title: "Example Domain"))
    try await computer.put(row(.today, in: home, url: "https://developer.mozilla.org/en-US/", title: "MDN Web Docs"))
    try await computer.put(row(.today, in: home, url: "https://www.rust-lang.org/", title: "Rust Programming Language"))
    try await computer.put(row(.pin, in: work.id!, url: "https://github.com/explore", title: "Explore GitHub"))
    try await computer.put(row(.today, in: work.id!, url: "https://www.swift.org/", title: "Swift.org"))

    let info = "server \(server.url.absoluteString)\nemail \(email)\nwords \(computer.words)\n"
    try info.write(toFile: hold, atomically: true, encoding: .utf8)
    var said = 0
    while FileManager.default.fileExists(atPath: hold) {
      let codes = server.codes(for: email)
      if codes.count > said {
        said = codes.count
        try? (info + "code \(codes.last!)\n").write(toFile: hold, atomically: true, encoding: .utf8)
      }
      try await Task.sleep(for: .milliseconds(300))
    }
  }
}
