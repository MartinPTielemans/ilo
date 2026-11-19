// Where an Account keeps what outlives the app: what must stay secret behind SecretStore, and
// what it has of the account's data behind SyncStateStore.

import Foundation

#if os(iOS)
  import Security
#endif

/// Holds the account's secret, the sign-in's tokens and the keys made from them, each under a
/// name. A read of a name never written gives nil, and writing nil removes it.
public protocol SecretStore: Sendable {
  func read(_ name: String) -> Data?
  func write(_ name: String, _ value: Data?) throws
}

/// Secrets for as long as the process runs: for tests, previews and the Mac.
public final class MemorySecretStore: SecretStore, @unchecked Sendable {
  private let lock = NSLock()
  private var values: [String: Data] = [:]

  public init() {}

  public func read(_ name: String) -> Data? {
    lock.withLock { values[name] }
  }

  public func write(_ name: String, _ value: Data?) {
    lock.withLock { values[name] = value }
  }
}

#if os(iOS)
  /// The phone's Keychain. Only the iPhone has this: a development process that reads the Mac's
  /// Keychain puts a password prompt on its owner's screen.
  public struct KeychainSecretStore: SecretStore {
    public struct Failure: Error {
      public let status: OSStatus
    }

    private let service: String

    /// `service` is the app's bundle id.
    public init(service: String) {
      self.service = service
    }

    private func query(_ name: String) -> [CFString: Any] {
      [kSecClass: kSecClassGenericPassword, kSecAttrService: service, kSecAttrAccount: name]
    }

    public func read(_ name: String) -> Data? {
      var query = query(name)
      query[kSecReturnData] = true
      query[kSecMatchLimit] = kSecMatchLimitOne
      var found: CFTypeRef?
      guard SecItemCopyMatching(query as CFDictionary, &found) == errSecSuccess else { return nil }
      return found as? Data
    }

    public func write(_ name: String, _ value: Data?) throws {
      let query = query(name) as CFDictionary
      guard let value else {
        let status = SecItemDelete(query)
        guard status == errSecSuccess || status == errSecItemNotFound else {
          throw Failure(status: status)
        }
        return
      }
      // Readable after the first unlock, so a change waiting to go out can be sent while the
      // phone is locked, and never copied to another device.
      let attributes: [CFString: Any] = [
        kSecValueData: value,
        kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
      ]
      var status = SecItemUpdate(query, attributes as CFDictionary)
      if status == errSecItemNotFound {
        status = SecItemAdd(self.query(name).merging(attributes) { $1 } as CFDictionary, nil)
      }
      guard status == errSecSuccess else { throw Failure(status: status) }
    }
  }
#endif

/// Holds what the phone has of the account's data and the changes it hasn't sent yet, as one
/// value that is replaced whole, and a mark that this install of the app signed in. The mark
/// goes only with the app's own files: `erase` leaves it. The secrets can outlive those files, as
/// the Keychain's do when the app is deleted, and without the mark they aren't this install's.
public protocol SyncStateStore: Sendable {
  func load() -> Data?
  func save(_ state: Data) throws
  func erase() throws
  var isMarked: Bool { get }
  func mark() throws
}

/// The state as a file in a directory of the app's, which is made when first written.
public struct FileStateStore: SyncStateStore {
  private let file: URL
  private let marker: URL

  public init(directory: URL) {
    file = directory.appendingPathComponent("spaces.json")
    marker = directory.appendingPathComponent("installed")
  }

  public var isMarked: Bool {
    FileManager.default.fileExists(atPath: marker.path)
  }

  public func mark() throws {
    guard !isMarked else { return }
    try FileManager.default.createDirectory(
      at: marker.deletingLastPathComponent(), withIntermediateDirectories: true)
    try Data().write(to: marker)
  }

  public func load() -> Data? {
    try? Data(contentsOf: file)
  }

  public func save(_ state: Data) throws {
    try FileManager.default.createDirectory(
      at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    // Atomic, so a crash while writing leaves the last state whole. The file holds the
    // account's tabs decrypted, so on the phone it's protected as the Keychain's items are.
    #if os(iOS)
      try state.write(to: file, options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    #else
      try state.write(to: file, options: .atomic)
    #endif
  }

  public func erase() throws {
    do {
      try FileManager.default.removeItem(at: file)
    } catch CocoaError.fileNoSuchFile {
    }
  }
}

/// The state for as long as the process runs.
public final class MemoryStateStore: SyncStateStore, @unchecked Sendable {
  private let lock = NSLock()
  private var state: Data?
  private var marked = false

  public init() {}

  public var isMarked: Bool {
    lock.withLock { marked }
  }

  public func mark() {
    lock.withLock { marked = true }
  }

  public func load() -> Data? {
    lock.withLock { state }
  }

  public func save(_ state: Data) {
    lock.withLock { self.state = state }
  }

  public func erase() {
    lock.withLock { state = nil }
  }
}
