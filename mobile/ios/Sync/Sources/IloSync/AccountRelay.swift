// The relay: a websocket the server tells the account's devices of each other's changes on,
// and carries a pairing over. Open only while the app is in front, so nothing wakes the phone.

import Foundation

/// A relay connection, closed when the account lets go of it.
final class Socket: Sendable {
  let task: URLSessionWebSocketTask

  init(_ task: URLSessionWebSocketTask) {
    self.task = task
  }

  deinit {
    task.cancel(with: .goingAway, reason: nil)
  }
}

/// A pairing the phone asked for.
struct PairingAsk {
  var keys = PairingRequest()
  var sent = false
  /// The server's id for it, once it has the request.
  var id: String?
  var approver: Bytes?
  var expiry: Task<Void, Never>?
}

extension Account {
  // MARK: The connection

  func watchForeground() {
    #if os(iOS)
      // UIKit's notifications by name, as the package doesn't link UIKit.
      for (name, active) in [
        ("UIApplicationDidBecomeActiveNotification", true),
        ("UIApplicationDidEnterBackgroundNotification", false),
      ] {
        NotificationCenter.default.addObserver(
          forName: Notification.Name(name), object: nil, queue: .main
        ) { [weak self] _ in
          MainActor.assumeIsolated { self?.setActive(active) }
        }
      }
    #endif
  }

  func setActive(_ active: Bool) {
    guard active != self.active else { return }
    self.active = active
    if active {
      refreshAhead()
      openRelay()
      // What the last time in the background couldn't send.
      if !state.changes.isEmpty { requestSync() }
    } else {
      closeRelay()
      failPairing(Said.pairingExpired, ifSent: true)
      // What waits to be sent goes while iOS still lets the app run, if the phone's token is
      // still good: one that ran out isn't refreshed now, and the changes wait for the app.
      if settleTask != nil {
        settleTask?.cancel()
        settleTask = nil
        requestSync()
      }
    }
  }

  func openRelay() {
    guard session != nil, active, socket == nil, !relayOpening else { return }
    relayOpening = true
    relayRetry?.cancel()
    relayRetry = nil
    let generation = self.generation
    Task { [weak self] in
      let token = try? await self?.accessToken()
      guard let self else { return }
      self.relayOpening = false
      guard generation == self.generation, self.active, let api = self.api else { return }
      guard let token else {
        self.retryRelay()
        return
      }
      var request = URLRequest(url: api.relay)
      request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
      let socket = Socket(self.http.webSocketTask(with: request))
      self.socket = socket
      socket.task.resume()
      self.listen(to: socket.task)
    }
  }

  private func listen(to task: URLSessionWebSocketTask) {
    Task { [weak self] in
      while true {
        do {
          let message = try await task.receive()
          guard let self, self.socket?.task === task else { return }
          if case .string(let text) = message { self.relayFrame(text) }
        } catch {
          self?.relayClosed(task)
          return
        }
      }
    }
  }

  func closeRelay() {
    relayRetry?.cancel()
    relayRetry = nil
    socket = nil
    relayOpen = false
    relayUp = nil
    // Leaving was the phone's doing, so the next connection starts from the first delay.
    relayDelay = Self.firstDelay
  }

  private func relayClosed(_ task: URLSessionWebSocketTask) {
    guard socket?.task === task else { return }
    socket = nil
    relayOpen = false
    // Only a connection that held earns a quick return. A server that takes one and drops it
    // would otherwise be asked for it, and for the account's changes, every 2 s.
    if let relayUp, Date().timeIntervalSince(relayUp) >= 30 { relayDelay = Self.firstDelay }
    relayUp = nil
    failPairing(Said.pairingExpired, ifSent: true)
    let refused =
      task.closeCode.rawValue == 4001 || (task.response as? HTTPURLResponse)?.statusCode == 401
    guard refused, let access = session?.access else {
      retryRelay()
      return
    }
    // The server no longer takes the token: another one, or the news that the device was
    // removed, before trying again.
    Task { [weak self] in
      _ = try? await self?.accessToken(refused: access)
      self?.retryRelay()
    }
  }

  private func retryRelay() {
    guard session != nil, active, socket == nil else { return }
    let delay = relayDelay
    relayDelay = min(relayDelay * 2, Self.longestDelay)
    relayRetry?.cancel()
    relayRetry = Task { [weak self] in
      try? await Task.sleep(for: delay)
      guard !Task.isCancelled, let self else { return }
      self.relayRetry = nil
      self.openRelay()
    }
  }

  private func relaySend(_ frame: [String: String]) {
    guard relayOpen, let socket, let data = try? JSONEncoder().encode(frame) else { return }
    socket.task.send(.string(String(decoding: data, as: UTF8.self))) { _ in }
  }

  private func relayFrame(_ text: String) {
    guard let frame = (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any],
      let type = frame["type"] as? String
    else { return }
    let id = frame["pairing"] as? String
    switch type {
    case "hello":
      relayOpen = true
      relayUp = Date()
      // Whatever changed while the phone wasn't listening.
      requestSync()
      sendPairingRequest()
    case "changed":
      requestSync()
    case "pair_requested":
      guard let id, ask?.sent == true, ask?.id == nil else { return }
      ask?.id = id
      // The server ends a pairing only at its next step, so the phone gives up at its time.
      var left = 300.0
      if let expires = frame["expires_at"] as? Double {
        left = min(max(expires - Date().timeIntervalSince1970, 0), 300)
      }
      expirePairing(after: .seconds(left), saying: Said.pairingExpired)
    case "pair_accept":
      guard let id, id == ask?.id,
        let approver = (frame["public_key"] as? String).flatMap(Bytes.init(base64URL:)),
        approver.count == 32, let keys = ask?.keys
      else { return }
      guard ask?.approver == nil else {
        // A second key, from someone between the devices trying to make the codes agree.
        relaySend(["type": "pair_cancel", "pairing": id])
        failPairing(Said.pairingFailed)
        return
      }
      ask?.approver = approver
      pairing = .waiting(code: keys.code(approver: approver))
      relaySend(["type": "pair_reveal", "pairing": id, "public_key": keys.publicKey.base64URL])
    case "pair_sealed":
      guard let id, id == ask?.id, let ask else { return }
      let secret = (frame["sealed"] as? String).flatMap(Bytes.init(base64URL:)).flatMap { sealed in
        ask.approver.flatMap { ask.keys.open(sealed: sealed, approver: $0, pairing: id) }
      }
      endPairing()
      guard let secret else {
        failPairing(Said.pairingFailed)
        return
      }
      Task { [weak self] in
        // The pairing gave a secret, which is the account's only if it opens its keys.
        let opened = await self?.takeSecret(secret)
        self?.pairing = .none
        if opened == false { self?.problem = Said.pairingFailed }
      }
    case "pair_closed":
      guard let id, id == ask?.id else { return }
      if frame["reason"] as? String == "cancelled" {
        endPairing()
        pairing = .refused
      } else {
        failPairing(Said.pairingExpired)
      }
    case "error":
      // About the request the server hasn't named yet, or about the pairing by its name.
      guard let ask, ask.id == nil || ask.id == id else { return }
      failPairing(Said.pairingFailed)
    default:
      break
    }
  }

  // MARK: Pairing

  public func requestPairing() async {
    guard session != nil, keys == nil else { return }
    if state.nigori == nil { await refresh() }
    guard state.nigori?.isSetUp == true else {
      problem = Said.setUpFirst
      return
    }
    endPairing()
    ask = PairingAsk()
    pairing = .waiting(code: "")
    problem = nil
    // The relay gets half a minute to open and take the request.
    expirePairing(after: .seconds(30), saying: Said.pairingUnreachable)
    openRelay()
    sendPairingRequest()
  }

  public func cancelPairing() {
    if let id = ask?.id { relaySend(["type": "pair_cancel", "pairing": id]) }
    endPairing()
    pairing = .none
  }

  private func sendPairingRequest() {
    guard relayOpen, let ask, !ask.sent else { return }
    self.ask?.sent = true
    relaySend([
      "type": "pair_request", "commitment": ask.keys.commitment.base64URL, "name": deviceName,
      "os": "iOS",
    ])
  }

  private func expirePairing(after delay: Duration, saying why: String) {
    ask?.expiry?.cancel()
    ask?.expiry = Task { [weak self] in
      try? await Task.sleep(for: delay)
      guard !Task.isCancelled, let self else { return }
      if let id = self.ask?.id { self.relaySend(["type": "pair_cancel", "pairing": id]) }
      self.failPairing(why)
    }
  }

  func endPairing() {
    ask?.expiry?.cancel()
    ask = nil
  }

  /// Ends the pairing with a reason. With `ifSent`, only one the relay had been told of, which
  /// is lost with the connection; one still waiting for the relay goes on waiting.
  private func failPairing(_ why: String, ifSent: Bool = false) {
    guard ask != nil || !ifSent, !ifSent || ask?.sent == true else { return }
    endPairing()
    pairing = .none
    problem = why
  }
}
