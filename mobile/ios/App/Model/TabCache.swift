import Observation
import UIKit

/// What this phone remembers of a tab without a web view: a picture of its page for its card, and
/// the web view's state, which brings back its history and scroll position. Both are files named
/// after the tab's id, gone when the tab is.
@Observable
final class TabCache {
  nonisolated struct Saved: Codable, Sendable {
    /// The address the account had for the tab when the state was saved. Another device may
    /// have sent the tab elsewhere since.
    var url: URL?
    var state: Data
  }

  private var pictures: [String: UIImage] = [:]
  @ObservationIgnored private var asked: Set<String> = []
  @ObservationIgnored private let directory: URL
  /// The last write of typed text or move of a tab's files, which the next waits for: they
  /// land in the order made.
  @ObservationIgnored private var typedWrite: Task<Void, Never>?

  init(directory: URL = URL.cachesDirectory.appending(path: "tabs", directoryHint: .isDirectory)) {
    self.directory = directory
  }

  func picture(of id: String) -> UIImage? {
    pictures[id]
  }

  /// Reads a tab's picture from disk, once, when its card first shows.
  func load(_ id: String) async {
    guard pictures[id] == nil, asked.insert(id).inserted else { return }
    if let picture = await Self.read(file(id, "jpg")) {
      // One taken meanwhile is newer.
      if pictures[id] == nil { pictures[id] = picture }
    }
  }

  func keep(_ picture: UIImage, of id: String) {
    pictures[id] = picture
    let file = file(id, "jpg")
    Task { await Self.write(picture, to: file) }
  }

  /// Lets go of a tab's picture alone, when its page gave way to something the picture isn't of.
  func forgetPicture(of id: String) {
    pictures[id] = nil
    // Nothing reads back the file that's on its way out.
    asked.insert(id)
    let file = file(id, "jpg")
    Task { await Self.remove([file]) }
  }

  func keep(_ saved: Saved, of id: String) {
    let file = file(id, "state")
    Task { await Self.write(saved, to: file) }
  }

  func saved(of id: String) async -> Saved? {
    await Self.readSaved(file(id, "state"))
  }

  /// What the user typed into the tab's page and hasn't sent, or nil once it's sent or left. It
  /// outlives the app, in a file only readable while the phone is unlocked.
  func keep(_ typed: TypedText?, of id: String) {
    let file = file(id, "typed")
    let before = typedWrite
    typedWrite = Task {
      await before?.value
      await Self.write(typed, to: file)
    }
  }

  func typed(of id: String) async -> TypedText? {
    await typedWrite?.value
    return await Self.readTyped(file(id, "typed"))
  }

  /// A tab that was pinned or unpinned has a new id, and so has a pinned tab moved to another Space.
  func rename(_ id: String, to new: String) {
    pictures[new] = pictures.removeValue(forKey: id)
    let moves = ["jpg", "state", "typed"].map { (file(id, $0), file(new, $0)) }
    // After what was typed is written, or the text stays behind under the old id.
    let before = typedWrite
    typedWrite = Task {
      await before?.value
      await Self.move(moves)
    }
  }

  func forget(_ id: String) {
    pictures[id] = nil
    asked.remove(id)
    let files = ["jpg", "state", "typed"].map { file(id, $0) }
    Task { await Self.remove(files) }
  }

  /// Drops the files of tabs the account no longer has, such as ones closed on another device.
  func prune(keeping ids: some Sequence<String>) {
    let names = Set(ids.map(Self.name))
    for id in pictures.keys where !names.contains(Self.name(id)) { pictures[id] = nil }
    let directory = directory
    Task { await Self.prune(directory, keeping: names) }
  }

  private func file(_ id: String, _ ending: String) -> URL {
    directory.appending(path: Self.name(id) + "." + ending)
  }

  /// A tab's id as a file's name: a Today tab's has a colon.
  private nonisolated static func name(_ id: String) -> String {
    String(id.map { $0.isLetter || $0.isNumber || $0 == "-" ? $0 : "_" })
  }

  @concurrent private nonisolated static func read(_ file: URL) async -> UIImage? {
    guard let data = try? Data(contentsOf: file) else { return nil }
    return await UIImage(data: data)?.byPreparingForDisplay()
  }

  @concurrent private nonisolated static func write(_ picture: UIImage, to file: URL) async {
    guard let data = picture.jpegData(compressionQuality: 0.8) else { return }
    try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    try? data.write(to: file, options: .atomic)
  }

  @concurrent private nonisolated static func write(_ saved: Saved, to file: URL) async {
    guard let data = try? PropertyListEncoder().encode(saved) else { return }
    try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    try? data.write(to: file, options: .atomic)
  }

  @concurrent private nonisolated static func write(_ typed: TypedText?, to file: URL) async {
    guard let typed, let data = try? PropertyListEncoder().encode(typed) else {
      try? FileManager.default.removeItem(at: file)
      return
    }
    try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    try? data.write(to: file, options: [.atomic, .completeFileProtection])
  }

  @concurrent private nonisolated static func readTyped(_ file: URL) async -> TypedText? {
    guard let data = try? Data(contentsOf: file) else { return nil }
    if let typed = try? PropertyListDecoder().decode(TypedText.self, from: data) { return typed }
    // A file an older build wrote may hold what this one never keeps.
    try? FileManager.default.removeItem(at: file)
    return nil
  }

  @concurrent private nonisolated static func readSaved(_ file: URL) async -> Saved? {
    guard let data = try? Data(contentsOf: file) else { return nil }
    return try? PropertyListDecoder().decode(Saved.self, from: data)
  }

  @concurrent private nonisolated static func move(_ moves: [(URL, URL)]) async {
    for (from, to) in moves {
      try? FileManager.default.removeItem(at: to)
      try? FileManager.default.moveItem(at: from, to: to)
    }
  }

  @concurrent private nonisolated static func remove(_ files: [URL]) async {
    for file in files { try? FileManager.default.removeItem(at: file) }
  }

  @concurrent private nonisolated static func prune(_ directory: URL, keeping names: Set<String>) async {
    let files = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
    for file in files where !names.contains(file.deletingPathExtension().lastPathComponent) {
      try? FileManager.default.removeItem(at: file)
    }
  }
}
