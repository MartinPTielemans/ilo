// What the app sees, which is the account's entities with the phone's unsent changes on them,
// and the calls that make those changes. Each writes the fields core/sync_spaces.cc's Current()
// writes for that kind of row, since the computer commits again whatever it would write
// differently.

import Foundation

extension Account {
  // MARK: The view

  /// An entity as the app shows it: the account's, or one the phone made, with what the phone
  /// changed on it.
  func effective(_ tag: String) -> SpaceEntity? {
    let change = state.changes[tag]
    if change?.deleted == true { return nil }
    guard var entity = change?.created ?? state.records[tag]?.entity ?? state.local?[tag] else {
      return nil
    }
    if let change {
      if let name = change.name { entity.name = name }
      if let url = change.url { entity.url = url }
      if let title = change.title { entity.title = title }
      if let seen = change.seen, Self.isLater(seen, than: entity.seen) { entity.seen = seen }
      if let space = change.space { entity.space = space }
      if let position = change.position { entity.position = position }
    }
    return entity
  }

  /// A showing is told to the account only an hour on from the last it has, as the computer's
  /// is (kSeenStep), or every look at a tab would be a commit.
  private static func isLater(_ seen: Int64, than known: Int64?) -> Bool {
    seen >= (known ?? 0) + 3600
  }

  /// Remakes what the app reads and saves the state. After every change to either.
  func changed() {
    rebuild()
    guard session != nil else { return }
    do {
      try store.save(JSONEncoder().encode(state))
    } catch {
      problem = Said.notKept
    }
  }

  func rebuild() {
    var entities: [SpaceEntity] = []
    for tag in Set(state.records.keys).union(state.changes.keys).union((state.local ?? [:]).keys) {
      guard let entity = effective(tag) else { continue }
      // A kind this ilo doesn't know isn't one to show as the default kind, a Space.
      if entity.kind == nil, state.records[tag]?.readOnly == true { continue }
      entities.append(entity)
    }
    let ordered = Self.ordered(entities)
    let spaces = ordered.filter { ($0.kind ?? .space) == .space }.map {
      SyncedSpace(
        id: $0.id ?? "", name: $0.name ?? "", color: $0.color, icon: $0.icon ?? "",
        theme: $0.theme ?? "")
    }
    let known = Set(spaces.map(\.id))
    let folders = Set(ordered.filter { $0.kind == .folder }.compactMap(\.id))
    var nodes: [String: SyncedNode] = [:]
    var pinned: [ListKey: [SyncedNode]] = [:]
    var today: [String: [SyncedNode]] = [:]
    // A row whose Space hasn't come waits for it, and one whose folder hasn't sits at the top
    // level, as on the computer.
    for entity in ordered {
      guard let id = entity.id, let kind = entity.kind, kind != .space,
        let space = entity.space, known.contains(space)
      else { continue }
      var node = SyncedNode(
        id: id, kind: kind == .today ? .today : kind == .folder ? .folder : .pin, space: space,
        name: entity.name ?? "", url: kind == .folder ? nil : entity.url.flatMap { URL(string: $0) },
        title: entity.title ?? "", icon: entity.icon ?? "")
      if kind == .today {
        node.seen = entity.seen.map { Date(timeIntervalSince1970: Double($0)) }
        today[space, default: []].append(node)
      } else {
        node.parent = entity.parent.flatMap { folders.contains($0) && $0 != id ? $0 : nil }
        pinned[ListKey(space: space, folder: node.parent), default: []].append(node)
      }
      nodes[id] = node
    }
    // Assigned only when different, so a view that reads one isn't redrawn for another.
    if self.spaces != spaces { self.spaces = spaces }
    if self.nodes != nodes { self.nodes = nodes }
    if pinnedLists != pinned { pinnedLists = pinned }
    if todayLists != today { todayLists = today }
  }

  /// In the sidebar's order: by position as Chromium compares them, one without a valid
  /// position after those with.
  private static func ordered(_ entities: [SpaceEntity]) -> [SpaceEntity] {
    entities.map { ($0, position(of: $0)) }.sorted { a, b in
      if a.1.isValid != b.1.isValid { return a.1.isValid }
      if a.1 != b.1 { return a.1.lessThan(b.1) }
      return (a.0.id ?? "") < (b.0.id ?? "")
    }.map(\.0)
  }

  private static func position(of entity: SpaceEntity) -> UniquePosition {
    UniquePosition(proto: entity.position ?? [])
  }

  // MARK: Changes

  /// Changes `tag`'s pending change and marks it newer than any commit on its way.
  private func change(_ tag: String, _ body: (inout Change) -> Void) {
    var change = state.changes[tag] ?? Change()
    body(&change)
    revision += 1
    change.revision = revision
    state.changes[tag] = change
  }

  /// Makes an entity what `body` makes of it: on the whole of one the phone made and hasn't
  /// sent, else as the fields that differ from what the account has. A field the phone didn't
  /// change is never one of them, or it would be written over what another device changes
  /// there later.
  private func edit(_ tag: String, _ body: (inout SpaceEntity) -> Void) {
    guard let before = effective(tag) else { return }
    var wanted = before
    body(&wanted)
    guard wanted != before else { return }
    change(tag) { change in
      if change.created != nil, change.sent != true {
        change.created = wanted
        return
      }
      if wanted.name != before.name { change.name = wanted.name }
      if wanted.url != before.url { change.url = wanted.url }
      if wanted.title != before.title || wanted.url != before.url {
        change.title = wanted.title
        change.page = wanted.url
      }
      if wanted.seen != before.seen { change.seen = wanted.seen }
      if wanted.space != before.space {
        change.space = wanted.space
        change.position = wanted.position
      }
    }
    // While its commit is on the way, what the account has is about to change under it: the
    // fields are kept as they are until the answer says to what.
    if !sending.contains(tag) { rebase(tag) }
  }

  /// Fits a pending change to what the account has, after either changed: a field that's the
  /// account's own by now is let go, and so is a title whose page the tab has left, on this
  /// phone or on another device, since it would be put on a page it never was the title of.
  /// A change with nothing left is removed.
  func rebase(_ tag: String) {
    guard var change = state.changes[tag], !change.deleted else { return }
    // One the phone made and hasn't sent is whole, with no fields beside it.
    if change.created != nil, change.sent != true { return }
    guard let known = change.created ?? state.records[tag]?.entity else {
      state.changes[tag] = nil
      return
    }
    if change.name == known.name { change.name = nil }
    if change.url == known.url { change.url = nil }
    if change.title != nil, change.page != (change.url ?? known.url) || change.title == known.title {
      change.title = nil
    }
    if change.title == nil { change.page = nil }
    if let seen = change.seen, !Self.isLater(seen, than: known.seen) { change.seen = nil }
    // A tab another device moved to the same Space stays at the place it has there.
    if change.space == known.space {
      change.space = nil
      change.position = nil
    }
    if change.created == nil, change.name == nil, change.url == nil, change.title == nil, change.seen == nil,
      change.space == nil
    {
      state.changes[tag] = nil
    } else {
      state.changes[tag] = change
    }
  }

  /// The account said what it has of an entity: in a download, or in a commit's answer. One the
  /// phone made and sent is the account's by then, as the account has it, whatever became of
  /// the commit's own answer, and what the phone changed on it since goes on as fields.
  func heard(_ tag: String) {
    if state.changes[tag]?.created != nil, state.changes[tag]?.sent == true {
      state.changes[tag]?.created = nil
      state.changes[tag]?.sent = nil
    }
    rebase(tag)
  }

  /// Deletes an entity. One the account never got is deleted too, in case its commit is on the
  /// way; if it isn't, there's nothing to send.
  private func remove(_ tag: String) {
    change(tag) { $0 = Change(deleted: true) }
  }

  /// Shows the change now and sends it once no other has followed it for a moment, so a page
  /// that redirects twice and then gets its title is one commit.
  private func wrote() {
    changed()
    settleTask?.cancel()
    settleTask = Task { [weak self, settle] in
      try? await Task.sleep(for: settle)
      guard !Task.isCancelled, let self else { return }
      self.settleTask = nil
      self.requestSync()
    }
  }

  private func writable(_ tag: String) -> Bool {
    guard state.records[tag]?.writable == false else { return true }
    problem = Said.newer
    return false
  }

  /// A page's address as a tab's entity has it, for the pages the account takes: those the
  /// computer does. It reads a tab's address with Chromium's GURL, and a Today tab whose address
  /// that refuses is never made there and is then deleted from the account (PutToday and Look
  /// in core/sync_spaces.cc).
  static func address(_ url: URL) -> String? {
    guard let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https",
      var parts = URLComponents(url: url, resolvingAgainstBaseURL: true)
    else { return nil }
    // As Chromium writes an address with no path.
    if parts.path.isEmpty { parts.path = "/" }
    // GURL takes no address over 2 MB.
    guard let text = parts.string, text.utf8.count < 2 << 20, hasHostAndPort(text) else { return nil }
    return text
  }

  /// Whether an http(s) address has a host and port GURL is sure to take: a name of letters,
  /// digits, "-", "_" and ".", an IPv4 or bracketed IPv6 address, and a port from 1 to 65535.
  /// Stricter than GURL where being exact would mean being Chromium's canonicalizer, since one
  /// refused here only stays on the phone, and one wrongly sent is deleted for every device.
  private static func hasHostAndPort(_ address: String) -> Bool {
    guard let start = address.range(of: "://")?.upperBound else { return false }
    let authority = address[start...].prefix { !"/?#\\".contains($0) }
    var host = authority.split(separator: "@", omittingEmptySubsequences: false).last ?? ""
    var port: Substring?
    if host.hasPrefix("[") {
      guard let close = host.firstIndex(of: "]") else { return false }
      let after = host[host.index(after: close)...]
      guard after.isEmpty || after.hasPrefix(":") else { return false }
      if !after.isEmpty { port = after.dropFirst() }
      host = host[...close]
    } else if let colon = host.lastIndex(of: ":") {
      port = host[host.index(after: colon)...]
      host = host[..<colon]
    }
    if let port {
      guard port.count <= 5, port.utf8.allSatisfy({ $0 >= 0x30 && $0 <= 0x39 }),
        let number = Int(port), (1...65535).contains(number)
      else { return false }
    }
    if host.hasPrefix("[") {
      var parsed = in6_addr()
      return inet_pton(AF_INET6, String(host.dropFirst().dropLast()), &parsed) == 1
    }
    let named = host.utf8.allSatisfy {
      ($0 >= 0x30 && $0 <= 0x39) || ($0 >= 0x41 && $0 <= 0x5a) || ($0 >= 0x61 && $0 <= 0x7a)
        || $0 == 0x2d || $0 == 0x2e || $0 == 0x5f
    }
    var labels = host.split(separator: ".", omittingEmptySubsequences: false)
    // A name may end in the root's dot.
    if labels.count > 1, labels.last == "" { labels.removeLast() }
    guard named, let last = labels.last, !labels.contains("") else { return false }
    // GURL reads a host whose last label is a number as an IPv4 address, in forms this doesn't
    // follow it through: only four plain numbers.
    let numeric = last.utf8.allSatisfy { $0 >= 0x30 && $0 <= 0x39 } || last.lowercased().hasPrefix("0x")
    guard numeric else { return true }
    return labels.count == 4
      && labels.allSatisfy { label in
        label.count <= 3 && (label == "0" || !label.hasPrefix("0")) && label.utf8.allSatisfy { $0 >= 0x30 && $0 <= 0x39 }
          && Int(label).map { $0 <= 255 } == true
      }
  }

  private static func now() -> Int64 {
    Int64(Date().timeIntervalSince1970)
  }

  // Both lists are in order with the valid positions first, and Before and After give a first
  // position when there's none to go by.

  /// A position before a Space's first Today tab, where a new one goes.
  private func firstToday(in space: String) -> UniquePosition {
    let first = todayLists[space]?.first.flatMap { effective($0.id) }.map(Self.position)
    return .before(first ?? UniquePosition(), suffix: UniquePosition.randomSuffix())
  }

  /// A position after a Space's last pinned tab or folder at the top level.
  private func lastPinned(in space: String) -> UniquePosition {
    let last = (pinnedLists[ListKey(space: space, folder: nil)] ?? [])
      .compactMap { effective($0.id) }.map(Self.position).last(where: \.isValid)
    return .after(last ?? UniquePosition(), suffix: UniquePosition.randomSuffix())
  }

  @discardableResult
  public func openToday(url: URL, title: String, in space: String) -> SyncedNode {
    let id = SpaceEntity.todayPrefix + UUID().uuidString.lowercased()
    guard spaces.contains(where: { $0.id == space }) else {
      // Shown where it was opened, and not the account's.
      problem = Said.notAPage
      return SyncedNode(id: id, kind: .today, space: space, url: url, title: title, seen: Date())
    }
    var tab = SpaceEntity()
    tab.id = id
    tab.kind = .today
    tab.space = space
    tab.position = firstToday(in: space).proto
    tab.name = ""
    tab.title = title
    tab.icon = ""
    tab.seen = Self.now()
    if let address = Self.address(url) {
      tab.url = address
      change(id) { $0.created = tab }
      wrote()
    } else {
      // A tab of this phone alone until it's at a page the computers can open.
      tab.url = url.absoluteString
      var local = state.local ?? [:]
      local[id] = tab
      state.local = local
      changed()
    }
    return nodes[id] ?? SyncedNode(id: id, kind: .today, space: space, url: url, title: title)
  }

  public func updateToday(_ id: String, url: URL, title: String) {
    guard let tab = effective(id), tab.kind == .today else { return }
    let address = Self.address(url)
    if var local = state.local?[id] {
      local.url = address ?? url.absoluteString
      local.title = title
      guard address != nil else {
        state.local?[id] = local
        changed()
        return
      }
      // Its first page the account takes, where the tab is made now.
      state.local?[id] = nil
      change(id) { $0 = Change(created: local) }
      wrote()
      return
    }
    // One the account has stays at its last page the computers can open, as theirs do.
    guard let address else { return }
    // A page that hasn't said its title yet doesn't take away the one it has.
    let title = title.isEmpty && address == tab.url ? tab.title ?? "" : title
    guard tab.url != address || (tab.title ?? "") != title, writable(id) else { return }
    edit(id) {
      $0.url = address
      $0.title = title
    }
    wrote()
  }

  public func sawToday(_ id: String) {
    let now = Self.now()
    guard let tab = effective(id), tab.kind == .today, Self.isLater(now, than: tab.seen) else { return }
    if state.local?[id] != nil {
      state.local?[id]?.seen = now
      changed()
      return
    }
    guard writable(id) else { return }
    edit(id) { $0.seen = now }
    wrote()
  }

  public func closeToday(_ id: String) {
    guard effective(id)?.kind == .today else { return }
    if state.local?[id] != nil {
      state.local?[id] = nil
      changed()
      return
    }
    guard writable(id) else { return }
    remove(id)
    wrote()
  }

  @discardableResult
  public func pin(today id: String) -> SyncedNode? {
    let key = SpaceEntity.key(id)
    guard let tab = effective(id), tab.kind == .today, let space = tab.space, writable(id),
      writable(key)
    else { return nil }
    // A pinned tab is the account's, so one the account can't take stays a tab of the phone.
    guard state.local?[id] == nil else {
      problem = Said.notAPage
      return nil
    }
    // The node keeps its key and loses the "today:" before it, which to the account is one
    // entity deleted and another put.
    var pin = SpaceEntity()
    pin.id = key
    pin.kind = .pin
    pin.space = space
    pin.position = lastPinned(in: space).proto
    pin.url = tab.url
    pin.name = tab.name ?? ""
    pin.title = tab.title ?? ""
    pin.icon = tab.icon ?? ""
    remove(id)
    change(key) { $0 = Change(created: pin) }
    wrote()
    return nodes[key]
  }

  @discardableResult
  public func unpin(_ id: String) -> SyncedNode? {
    let today = SpaceEntity.todayPrefix + id
    guard let pin = effective(id), pin.kind == .pin, let space = pin.space, writable(id),
      writable(today)
    else { return nil }
    // A Today tab is a page, so a pinned tab without one the account takes stays pinned.
    guard let address = pin.url.flatMap({ URL(string: $0) }).flatMap(Self.address) else {
      problem = Said.notAPage
      return nil
    }
    var tab = SpaceEntity()
    tab.id = today
    tab.kind = .today
    tab.space = space
    tab.position = firstToday(in: space).proto
    tab.url = address
    tab.name = pin.name ?? ""
    tab.title = pin.title ?? ""
    tab.icon = pin.icon ?? ""
    tab.seen = Self.now()
    remove(id)
    change(today) { $0 = Change(created: tab) }
    wrote()
    return nodes[today]
  }

  @discardableResult
  public func move(_ id: String, to space: String) -> SyncedNode? {
    guard let tab = effective(id), tab.kind == .today || tab.kind == .pin, tab.space != space,
      spaces.contains(where: { $0.id == space })
    else { return nil }
    if tab.kind == .today {
      let position = firstToday(in: space).proto
      if state.local?[id] != nil {
        // A tab of this phone alone moves here alone.
        state.local?[id]?.space = space
        state.local?[id]?.position = position
        changed()
        return nodes[id]
      }
      guard writable(id) else { return nil }
      // The same entity in its new Space, as the computer writes one it moved (Window::MoveToSpace
      // in core/window.cc), so the other devices move the tab with its page instead of closing it
      // and opening another (Put in core/sync_spaces.cc).
      edit(id) {
        $0.space = space
        $0.position = position
      }
      wrote()
      return nodes[id]
    }
    guard writable(id) else { return nil }
    // A pinned tab is a new node in another Space, as the computer's is there: to the account
    // one entity deleted and another put under a new id, outside any folder.
    var pin = SpaceEntity()
    pin.id = UUID().uuidString.lowercased()
    pin.kind = .pin
    pin.space = space
    pin.position = lastPinned(in: space).proto
    pin.url = tab.url
    pin.name = tab.name ?? ""
    pin.title = tab.title ?? ""
    pin.icon = tab.icon ?? ""
    let key = pin.id ?? ""
    remove(id)
    change(key) { $0 = Change(created: pin) }
    wrote()
    return nodes[key]
  }

  public func rename(_ id: String, to name: String) {
    // A Space or a folder. The computer takes those names, and never a tab's: it leaves the tab
    // as it was and writes the old name back (Put and PutToday in core/sync_spaces.cc).
    guard let entity = effective(id), (entity.kind ?? .space) == .space || entity.kind == .folder,
      (entity.name ?? "") != name, writable(id)
    else { return }
    edit(id) { $0.name = name }
    wrote()
  }
}
