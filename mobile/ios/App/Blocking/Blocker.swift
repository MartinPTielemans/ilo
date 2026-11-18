import Foundation
import Observation
import WebKit
import os

/// Ads, trackers and cookie notices that don't load: WebKit's content rule lists, made of the
/// filter lists the desktop blocks with (Blocking/build.sh), on every web view the app makes.
///
/// WebKit compiles the rules once, in a process of its own, and keeps them by the version the
/// build gave them, so only the first launch after an install or an update compiles, for a few
/// seconds. Nothing waits for it: a page opened meanwhile loads unblocked and stays as it is, and
/// the pages after it are blocked.
@Observable
final class Blocker {
  static let shared = Blocker()

  /// The switch in Settings.
  var isOn: Bool {
    get { choice.isOn }
    set { choice.isOn = newValue }
  }

  private(set) var choice: BlockingChoice {
    didSet {
      guard choice != oldValue else { return }
      defaults.set(choice.isOn, forKey: Key.isOn)
      defaults.set(choice.sitesOff.sorted(), forKey: Key.sitesOff)
      apply()
    }
  }

  @ObservationIgnored private let defaults: UserDefaults
  @ObservationIgnored private var lists: [WKContentRuleList] = []
  @ObservationIgnored private var views: [ObjectIdentifier: Attached] = [:]
  @ObservationIgnored private var started = false
  /// What a private tab's user chose for a site in that tab: whether it blocks there, by site,
  /// for the tab's web view. Never written, since the choice names a site the tab was on.
  private var held: [ObjectIdentifier: [String: Bool]] = [:]

  private enum Key {
    static let isOn = "blocking"
    static let sitesOff = "blockingOffSites"
  }

  /// A web view, the watch on its address, and how many of the lists it has.
  private struct Attached {
    weak var view: WKWebView?
    var watch: NSKeyValueObservation
    var count = 0
  }

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
    choice = BlockingChoice(
      isOn: defaults.object(forKey: Key.isOn) as? Bool ?? true, sitesOff: Set(defaults.stringArray(forKey: Key.sitesOff) ?? []))
  }

  /// Whether ads and trackers are kept from the page at `url`: in a `view` that holds a choice
  /// of its own for the page's site, that one.
  func blocks(_ url: URL?, in view: WKWebView? = nil) -> Bool {
    guard choice.isOn else { return false }
    if let view, let site = BlockingChoice.site(of: url), let own = held[ObjectIdentifier(view)]?[site] { return own }
    return choice.blocks(url)
  }

  /// A link without its tracking parameters, or nil when it has none, its site isn't blocked for,
  /// or it only moves within the page it was followed `from`.
  func withoutTracking(_ url: URL, from page: URL? = nil, in view: WKWebView? = nil) -> URL? {
    blocks(url, in: view) ? TrackingParameters.stripped(url, followedFrom: page) : nil
  }

  /// Turns blocking on or off for the site of the page at `url`. The page shows it once reloaded.
  func set(_ blocks: Bool, at url: URL?) {
    choice.set(blocks, at: url)
  }

  /// Turns blocking on or off for the site of the page at `url` in one web view alone, a private
  /// tab's, for as long as the view is there.
  func hold(_ blocks: Bool, at url: URL?, in view: WKWebView) {
    guard let site = BlockingChoice.site(of: url) else { return }
    let id = ObjectIdentifier(view)
    held[id, default: [:]][site] = blocks
    apply(to: id)
  }

  /// Has `view` follow the user's choice for each page it goes to. WebKit says where a view is
  /// going as soon as the page's own request starts, before anything the page asks for.
  func attach(_ view: WKWebView) {
    start()
    views = views.filter { $0.value.view != nil }
    // A choice goes with the view that held it.
    if !held.isEmpty { held = held.filter { views[$0.key] != nil } }
    let id = ObjectIdentifier(view)
    let watch = view.observe(\.url) { [weak self] view, _ in
      // WebKit changes it on the main thread.
      MainActor.assumeIsolated { self?.apply(to: id) }
    }
    views[id] = Attached(view: view, watch: watch)
    apply(to: id)
  }

  private func apply() {
    for id in views.keys { apply(to: id) }
  }

  private func apply(to id: ObjectIdentifier) {
    guard var attached = views[id], let view = attached.view else { return }
    let content = view.configuration.userContentController
    if blocks(view.url, in: view) {
      for list in lists.dropFirst(attached.count) { content.add(list) }
      attached.count = lists.count
    } else {
      for list in lists.prefix(attached.count) { content.remove(list) }
      attached.count = 0
    }
    views[id] = attached
  }

  // MARK: The rules

  /// What Blocking/build.sh wrote beside the rules.
  private nonisolated struct Manifest: Decodable, Sendable {
    struct List: Decodable, Sendable {
      var file: String
      var rules: Int
    }
    var version: String
    var lists: [List]
  }

  private static let log = Logger(subsystem: Bundle.main.bundleIdentifier ?? "app.ilo.browser.ios", category: "blocking")

  /// Readies the rules, once: those WebKit has from an earlier launch, and the rest compiled.
  /// Each list blocks as soon as it's ready.
  func start() {
    guard !started else { return }
    started = true
    Task {
      guard let folder = Bundle.main.url(forResource: "Rules", withExtension: nil),
        let manifest = await Self.manifest(in: folder), let store = WKContentRuleListStore.default()
      else {
        Self.log.error("No rules in the app, so nothing is blocked.")
        return
      }
      let began = ContinuousClock.now
      var compiled = 0
      var current: Set<String> = []
      for (index, entry) in manifest.lists.enumerated() {
        let name = "ilo-\(manifest.version)-\(index)"
        current.insert(name)
        var list = try? await store.contentRuleList(forIdentifier: name)
        if list == nil, let rules = await Self.rules(at: folder.appending(path: entry.file)) {
          do {
            list = try await store.compileContentRuleList(forIdentifier: name, encodedContentRuleList: rules)
            compiled += 1
          } catch {
            Self.log.error("WebKit didn't take \(entry.file, privacy: .public): \(error.localizedDescription, privacy: .public)")
          }
        }
        if let list {
          lists.append(list)
          apply()
        }
      }
      let rules = manifest.lists.reduce(0) { $0 + $1.rules }
      Self.log.notice(
        "\(self.lists.count) lists of \(rules) rules ready in \(ContinuousClock.now - began, privacy: .public), \(compiled) compiled now.")
      // An earlier version's lists, which nothing reads any more.
      for name in await store.availableIdentifiers() ?? [] where name.hasPrefix("ilo-") && !current.contains(name) {
        try? await store.removeContentRuleList(forIdentifier: name)
      }
    }
  }

  @concurrent private nonisolated static func manifest(in folder: URL) async -> Manifest? {
    guard let data = try? Data(contentsOf: folder.appending(path: "manifest.json")) else { return nil }
    return try? JSONDecoder().decode(Manifest.self, from: data)
  }

  /// A list's rules as WebKit reads them, out of the compressed file the app comes with.
  @concurrent private nonisolated static func rules(at file: URL) async -> String? {
    guard let packed = try? Data(contentsOf: file), let data = try? (packed as NSData).decompressed(using: .lzfse) else { return nil }
    return String(data: data as Data, encoding: .utf8)
  }
}
