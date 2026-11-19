// Compiles each rules-N.json in a folder with WebKit's own compiler, as the phone will, so a rule
// WebKit refuses fails the build and not the phone: swift check.swift FOLDER. The compiled lists
// go in a temporary folder that's removed.
import Foundation
import WebKit

let folder = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let scratch = FileManager.default.temporaryDirectory.appendingPathComponent("ilo-rules-\(ProcessInfo.processInfo.processIdentifier)", isDirectory: true)
try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: scratch) }
guard let store = WKContentRuleListStore(url: scratch) else { fatalError("no store at \(scratch.path)") }

let files = try FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)
  .filter { $0.pathExtension == "json" && $0.lastPathComponent.hasPrefix("rules-") }
  .sorted { $0.lastPathComponent < $1.lastPathComponent }
var failed = false
for file in files {
  let rules = try String(contentsOf: file, encoding: .utf8)
  let start = Date()
  do {
    _ = try await store.compileContentRuleList(forIdentifier: file.lastPathComponent, encodedContentRuleList: rules)
    print("\(file.lastPathComponent): compiled in \(String(format: "%.1f", Date().timeIntervalSince(start))) s")
  } catch {
    failed = true
    print("\(file.lastPathComponent): \((error as NSError).userInfo[NSHelpAnchorErrorKey] ?? error.localizedDescription)")
  }
}
exit(failed ? 1 : 0)
