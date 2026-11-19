import Foundation

/// The addresses "Open in ilo" was handed in another app's share sheet and ilo hasn't opened
/// yet. iOS lets an action open no app, so the action leaves each here, in the folder the two
/// share, and ilo takes them the next time it's on screen. The action is built from this file
/// too (project.yml), so what it writes is what the app reads.
nonisolated struct PendingLinks {
  /// The App Group the app and the action are both signed into: the one folder both can reach.
  static let group = "group.app.ilo.browser.ios"

  /// What the action asks iOS to open, to bring ilo forward. It carries no address: the page
  /// comes from here either way, so a page or an app that opens it hands ilo nothing.
  static let wake = URL(string: "ilo://open")!

  /// One file for each address, so the action never reads or rewrites what's waiting.
  let folder: URL

  /// The shared folder's, or nil in a build signed without the group.
  static var shared: PendingLinks? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group).map { PendingLinks(folder: $0.appending(path: "Pending")) }
  }

  /// Whether an address is a web page's. The action takes nothing else, and ilo opens nothing else.
  static func isWeb(_ url: URL) -> Bool {
    ["http", "https"].contains(url.scheme?.lowercased()) && url.host() != nil
  }

  /// Leaves a web page's address for ilo, and says whether it's there. The address alone, with
  /// nothing about where it came from or when.
  @discardableResult func add(_ url: URL) -> Bool {
    guard Self.isWeb(url) else { return false }
    do {
      var folder = folder
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      // It waits for the next look at ilo, on this phone: a backup has no use for it.
      var values = URLResourceValues()
      values.isExcludedFromBackup = true
      try? folder.setResourceValues(values)
      // Whole or not there: ilo may look while this is written.
      try Data(url.absoluteString.utf8).write(to: folder.appending(path: UUID().uuidString), options: .atomic)
      return true
    } catch {
      return false
    }
  }

  /// The addresses left since the last look, oldest first, each removed as it's read: nothing
  /// stays behind, and a second look finds none.
  func take() -> [URL] {
    let files = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: [.creationDateKey])) ?? []
    let dated = files.map { ($0, (try? $0.resourceValues(forKeys: [.creationDateKey]).creationDate) ?? .distantPast) }
    return dated.sorted { $0.1 < $1.1 }.compactMap { file, _ in
      defer { try? FileManager.default.removeItem(at: file) }
      return (try? String(contentsOf: file, encoding: .utf8)).flatMap { URL(string: $0) }
    }
  }
}
