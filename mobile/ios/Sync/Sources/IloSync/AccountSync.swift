// A sync cycle: what changed on the account comes down, then what the phone changed goes up, as
// Chromium's engine does for a type under a custom passphrase.

import Foundation

enum SyncFailure: Error {
  /// The account's data was cleared at the server, so what the phone has of it is of no use.
  case startOver
  /// The server wants nothing for this many seconds.
  case throttled(Int)
  /// The account was forgotten while the server was answering.
  case stale
  /// The access token ran out while the app was in the background, where none is refreshed.
  case background
}

extension Account {
  /// Runs a cycle, or one more after the cycle that's running.
  func requestSync() {
    guard session != nil else { return }
    if syncTask != nil {
      again = true
      return
    }
    syncTask = Task { [weak self] in
      while let self {
        self.again = false
        await self.cycle()
        if !self.again {
          self.syncTask = nil
          return
        }
      }
    }
  }

  public func refresh() async {
    // Asked for, so what was waiting for more changes goes now.
    settleTask?.cancel()
    settleTask = nil
    requestSync()
    await syncTask?.value
  }

  private func cycle() async {
    guard session != nil else { return }
    let generation = self.generation
    isSyncing = true
    defer { isSyncing = false }
    do {
      var said = false
      var later = false
      for pass in 1...3 {
        try await download(generation)
        if let meanwhile {
          self.meanwhile = nil
          await meanwhile()
        }
        if keys == nil {
          // The account's keys changed, or its data was cleared: the secret the phone has
          // opens the new ones, or it's another account's by now.
          if hasSecret, state.nigori?.isSetUp == true, let secret = secrets.read("secret") {
            let opened = await takeSecret(Bytes(secret))
            guard generation == self.generation else { return }
            if !opened {
              try? secrets.write("secret", nil)
              hasSecret = false
              updatePhase()
              problem = Said.wordsChanged
              said = true
            }
          }
          break
        }
        let result = try await commit(generation, last: pass == 3)
        said = said || result.undone
        later = result.later
        if !result.conflicted { break }
      }
      retryDelay = Self.firstDelay
      if keys == nil, state.nigori?.isSetUp != true {
        problem = Said.setUpFirst
      } else if !said {
        problem = nil
      }
      if later { retryLater() }
    } catch {
      guard generation == self.generation else { return }
      switch error {
      case SyncFailure.startOver:
        state = SyncState()
        loginIndex = nil
        keys = nil
        try? secrets.write("keys", nil)
        updatePhase()
        changed()
        again = true
      case SyncFailure.throttled(let seconds):
        // At least 5 s, so a server that says none isn't asked again at once and without end,
        // and at most an hour, so one that says a year doesn't keep the phone's changes for it.
        let wait = min(max(seconds, 5), 3600)
        heldUntil = Date(timeIntervalSinceNow: Double(wait))
        retry(after: .seconds(wait))
      case ServerFailure.unreachable:
        problem = Said.unreachable(server)
        retryLater()
      case AccountError.signedOut, SyncFailure.stale:
        // Forgetting the account said why.
        break
      case SyncFailure.background:
        // Nothing went wrong: what's unsent goes when the app is in front again.
        break
      default:
        problem = Said.unexpected
        retryLater()
      }
    }
  }

  /// Tries again by itself only while something waits on it: a change to send, or the first
  /// download. Otherwise the relay says when there's something to get.
  private func retryLater() {
    guard !state.changes.isEmpty || phase == .starting else { return }
    retry(after: retryDelay)
    retryDelay = min(retryDelay * 2, Self.longestDelay)
  }

  /// The one wake-up, which a later call replaces and which is never due before the server's
  /// hold ends: a retry that came sooner would find its changes held and leave them with no
  /// wake-up at all.
  private func retry(after delay: Duration) {
    let delay = max(delay, .seconds(heldUntil.timeIntervalSinceNow))
    retryTask?.cancel()
    retryTask = Task { [weak self] in
      try? await Task.sleep(for: delay)
      guard !Task.isCancelled, let self else { return }
      self.retryTask = nil
      self.requestSync()
    }
  }

  /// Sends one message. An access token the server refuses is refreshed once.
  private func send(_ message: Bytes, _ generation: Int) async throws -> SyncResponse {
    guard let api, let cacheGUID = session?.cacheGUID else { throw SyncFailure.stale }
    let access = try await accessToken()
    var response: SyncResponse
    do {
      response = try await api.command(message, access: access, cacheGUID: cacheGUID)
    } catch ServerFailure.unauthorized {
      let fresh = try await accessToken(refused: access)
      response = try await api.command(message, access: fresh, cacheGUID: cacheGUID)
    }
    guard generation == self.generation else { throw SyncFailure.stale }
    switch response.error {
    case .success: break
    case .notMyBirthday, .obsolete: throw SyncFailure.startOver
    case .throttled: throw SyncFailure.throttled(response.throttle ?? 60)
    default: throw ServerFailure.unexpected
    }
    if state.birthday.isEmpty {
      state.birthday = response.birthday
    } else if !response.birthday.isEmpty, response.birthday != state.birthday {
      throw SyncFailure.startOver
    }
    return response
  }

  // MARK: Down

  private func download(_ generation: Int) async throws {
    guard let email = session?.email else { throw SyncFailure.stale }
    // Whether the last request asked for the Spaces: the keys may come while one that didn't is
    // answered, and that one has downloaded none.
    var askedSpaces = false
    while true {
      // The Spaces and passwords only once the phone can read them, so none is passed by
      // undecrypted.
      askedSpaces = keys != nil
      let types =
        askedSpaces ? [DataType.nigori, DataType.iloSpace, DataType.passwords] : [DataType.nigori]
      let before = state.markers
      let response = try await send(
        SyncMessage.getUpdates(
          share: email, birthday: state.birthday, markers: types.map { ($0, before[$0] ?? []) }),
        generation)
      apply(response, asked: types)
      if response.remaining <= 0 || state.markers == before { break }
    }
    if askedSpaces, keys != nil { state.downloaded = true }
    updatePhase()
    changed()
  }

  private func apply(_ response: SyncResponse, asked: [Int]) {
    // The Nigori first: the keys it names are the ones the rest is encrypted with.
    for update in response.updates {
      guard let bytes = update.nigori, let nigori = NigoriSpecifics(bytes), nigori != state.nigori
      else { continue }
      state.nigori = nigori
      if let keys, !nigori.isSetUp || nigori.keybag?.keyName != keys.defaultName {
        self.keys = nil
        try? secrets.write("keys", nil)
      }
    }
    for marker in response.markers where marker.type == DataType.nigori {
      state.markers[marker.type] = marker.token
    }
    if asked.contains(DataType.passwords) { applyPasswords(response) }
    guard let keys, asked.contains(DataType.iloSpace) else { return }
    var tags: [String: String] = [:]
    for (tag, record) in state.records { tags[record.hash] = tag }
    // An entity the phone made and sent may be the account's without the phone having heard.
    for (tag, change) in state.changes where change.created != nil && change.sent == true {
      tags[clientTagHash(tag: tag)] = tag
    }
    for update in response.updates where update.isSpace && update.serverTag == nil {
      guard let hash = update.hash else { continue }
      let known = tags[hash]
      // Chromium's processor takes an update only if it's newer than what it has.
      if let known, let record = state.records[known], update.version <= record.version { continue }
      if update.deleted {
        guard let known else { continue }
        state.records[known] = nil
        tags[hash] = nil
        // What the phone changed on it has nothing left to change. One it made again stays,
        // unless that was sent: then this is the account's word after it, and sending it
        // again would bring back what another device closed.
        if let change = state.changes[known], change.created == nil || change.sent == true {
          state.changes[known] = nil
        }
        continue
      }
      // As the computer's bridge: an entity that isn't encrypted, isn't valid or isn't under
      // its own id's hash is passed over.
      guard let encrypted = update.encrypted, let plain = keys.decrypt(encrypted),
        let space = protoFields(plain)?.last(DataType.iloSpace)?.bytes.flatMap(protoFields),
        let parsed = space.last(1)?.bytes.flatMap(SpaceEntity.parse), parsed.entity.isValid,
        let id = parsed.entity.id, clientTagHash(tag: id) == hash
      else {
        // The marker moves past it all the same, so what the phone has of it is no longer
        // what the account has, and a change made on that could only be refused.
        if let known { state.records[known]?.stale = true }
        continue
      }
      state.records[id] = Record(
        serverID: update.id, version: update.version, created: update.created, hash: hash,
        entity: parsed.entity, readOnly: !parsed.understood)
      tags[hash] = id
      heard(id)
    }
    for marker in response.markers where marker.type == DataType.iloSpace {
      state.markers[marker.type] = marker.token
    }
  }

  // MARK: Up

  private struct Sent {
    var tag: String
    var revision: Int
    /// What the account has once the server takes the entry: nil for a deletion.
    var entity: SpaceEntity?
    var entry: CommitEntry
  }

  /// The entries the pending changes come to. A change that changes nothing by now is dropped.
  private func plan(_ keys: Keybag) -> [Sent] {
    var sent: [Sent] = []
    let now = protoTime()
    // By node, so a tab's delete under one id and its put under the other are side by side.
    let tags = state.changes.keys.sorted {
      (SpaceEntity.key($0), $0) < (SpaceEntity.key($1), $1)
    }
    for tag in tags {
      guard let change = state.changes[tag] else { continue }
      let record = state.records[tag]
      let entity = effective(tag)
      // Nothing to send: it's as the account has it, or deleted before the account got it.
      if record?.writable == false || entity == record?.entity {
        state.changes[tag] = nil
        continue
      }
      var entry = CommitEntry(
        // Chromium sends a new entity under a random id, which the server replaces.
        id: record?.serverID ?? UUID().uuidString.lowercased(),
        hash: record?.hash ?? clientTagHash(tag: tag), version: record?.version ?? 0,
        created: record?.created ?? now, modified: now)
      if let entity {
        if let record, Self.onlyTitle(record.entity, entity) { continue }
        guard let encrypted = keys.encrypt(CommitEntry.plaintext(entity)) else { continue }
        entry.encrypted = encrypted
      }
      sent.append(Sent(tag: tag, revision: change.revision, entity: entity, entry: entry))
    }
    return sent
  }

  /// A Today tab that differs only in its title isn't sent for that alone, as the computer
  /// doesn't: a page's title changes often, and goes with the next real change. A tab the
  /// account has no title for gets its first one.
  private static func onlyTitle(_ known: SpaceEntity, _ now: SpaceEntity) -> Bool {
    guard now.kind == .today, known.title?.isEmpty == false else { return false }
    var same = now
    same.title = known.title
    return same == known
  }

  /// `last` when there's no download left after it in this cycle.
  private func commit(_ generation: Int, last: Bool) async throws
    -> (conflicted: Bool, undone: Bool, later: Bool)
  {
    var result = (conflicted: false, undone: false, later: false)
    // Not while a change waits for the next one, nor while the server asked for quiet, which
    // what's unsent then waits out.
    guard let keys, let session, settleTask == nil else { return result }
    guard Date() >= heldUntil else {
      result.later = !state.changes.isEmpty
      return result
    }
    let pending = plan(keys)
    var at = 0
    while at < pending.count {
      // Chromium's batch is 25. A pinned or unpinned tab's two entries go in one, as the
      // computer moves the tab only when it gets both at once, and closes it otherwise.
      var end = min(at + 25, pending.count)
      if end < pending.count, SpaceEntity.key(pending[end].tag) == SpaceEntity.key(pending[end - 1].tag) {
        end += 1
      }
      let batch = pending[at..<end]
      at = end
      // Marked and saved before the commit leaves: iOS may suspend the app before the answer
      // comes, and the next launch must know the account may have these.
      var marked = false
      for sent in batch where state.changes[sent.tag]?.created != nil && state.changes[sent.tag]?.sent != true {
        state.changes[sent.tag]?.sent = true
        marked = true
      }
      if marked { changed() }
      sending = Set(batch.map(\.tag))
      let response: SyncResponse
      do {
        response = try await send(
          SyncMessage.commit(
            share: session.email, birthday: state.birthday, cacheGUID: session.cacheGUID,
            entries: batch.map(\.entry)), generation)
      } catch {
        sending = []
        for sent in batch { rebase(sent.tag) }
        throw error
      }
      sending = []
      guard response.entries.count == batch.count else { throw ServerFailure.unexpected }
      for (sent, answer) in zip(batch, response.entries) {
        switch answer.result {
        case .success:
          if let entity = sent.entity {
            state.records[sent.tag] = Record(
              serverID: answer.id.isEmpty ? sent.entry.id : answer.id, version: answer.version,
              created: sent.entry.created, hash: sent.entry.hash, entity: entity, readOnly: false)
          } else {
            state.records[sent.tag] = nil
          }
          // A change made since it was sent goes with the next commit, as what it changes on
          // what the account has now.
          if state.changes[sent.tag]?.revision == sent.revision {
            state.changes[sent.tag] = nil
          } else {
            heard(sent.tag)
          }
        case .conflict where !last:
          // Another device changed it first: the next download brings what it has now, and
          // the change is made on that.
          result.conflicted = true
        case .invalid, .conflict:
          // A conflict no download settles is over an entity this ilo can't read as the
          // account has it now, so the change has nothing to be made on.
          state.changes[sent.tag] = nil
          problem = Said.undone
          result.undone = true
        case .overQuota, .transient, nil:
          result.later = true
        }
      }
      for sent in batch { rebase(sent.tag) }
      changed()
    }
    return result
  }
}
