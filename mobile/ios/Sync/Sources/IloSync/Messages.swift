// Sync's messages (components/sync/protocol/sync.proto), the fields of them the phone sends and
// reads, by their numbers there.

import CryptoKit
import Foundation

func randomBytes(_ count: Int) -> Bytes {
  var generator = SystemRandomNumberGenerator()
  return (0..<count).map { _ in UInt8.random(in: .min ... .max, using: &generator) }
}

extension Bytes {
  /// Base64 for a URL, without padding, as the server's tokens and the relay's frames are.
  var base64URL: String {
    Data(self).base64EncodedString().replacingOccurrences(of: "+", with: "-")
      .replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
  }

  init?(base64URL text: String) {
    var text = text.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
    while text.hasSuffix("=") { text.removeLast() }
    text += String(repeating: "=", count: (4 - text.count % 4) % 4)
    guard let data = Data(base64Encoded: text) else { return nil }
    self.init(data)
  }
}

enum DataType {
  /// EntitySpecifics' field for each type, which is also its id in a progress marker.
  static let nigori = 47745
  static let passwords = 45873
  static let iloSpace = 500_000_001
}

/// What names an entity to the server: the hash of its type and its client tag, which for a
/// Space's entity is its id (components/sync/base/client_tag_hash.cc).
func clientTagHash(type: Int = DataType.iloSpace, tag: String) -> String {
  var specifics = ProtoWriter()
  specifics.bytes(type, [])
  return Data(Insecure.SHA1.hash(data: specifics.bytes + Bytes(tag.utf8))).base64EncodedString()
}

/// Milliseconds since 1970, sync's time.
func protoTime(_ date: Date = Date()) -> Int64 {
  Int64((date.timeIntervalSince1970 * 1000).rounded())
}

/// One entry of a commit for ILO_SPACE, as Chromium writes one for a type that's encrypted
/// (CommitContributionImpl::PopulateCommitProto, DataTypeWorker::EncryptSpecifics).
struct CommitEntry {
  /// The server's id for it, or any new one for an entity it hasn't got.
  var id: String
  var hash: String
  /// The version this change was made on: 0 for a new entity.
  var version: Int64
  var created: Int64
  var modified: Int64
  /// Its SpaceEntity encrypted, or nil to delete it.
  var encrypted: EncryptedData?

  func write(_ writer: inout ProtoWriter) {
    writer.string(1, id)
    writer.int(4, version)
    writer.int(5, modified)
    if encrypted != nil { writer.int(6, created) }
    // The real name would tell the server what's inside. A deletion's is empty.
    writer.string(7, encrypted == nil ? "" : "encrypted")
    writer.bool(18, encrypted == nil)
    writer.message(21) { specifics in
      if let encrypted { specifics.message(1, encrypted.write) }
      // The type's own field, empty, says which type the entity is.
      specifics.bytes(DataType.iloSpace, [])
    }
    writer.string(23, hash)
  }

  /// What `encrypted` holds before it's encrypted: the entity's EntitySpecifics.
  static func plaintext(_ entity: SpaceEntity) -> Bytes {
    var specifics = ProtoWriter()
    specifics.message(DataType.iloSpace) { $0.bytes(1, entity.serialized) }
    return specifics.bytes
  }
}

enum SyncMessage {
  static let protocolVersion: UInt64 = 99

  private static func envelope(
    share: String, birthday: String, contents: UInt64, _ body: (inout ProtoWriter) -> Void
  ) -> Bytes {
    var message = ProtoWriter()
    message.string(1, share)
    message.varint(2, protocolVersion)
    message.varint(3, contents)
    body(&message)
    if !birthday.isEmpty { message.string(7, birthday) }
    return message.bytes
  }

  static func commit(share: String, birthday: String, cacheGUID: String, entries: [CommitEntry])
    -> Bytes
  {
    envelope(share: share, birthday: birthday, contents: 1) { message in
      message.message(4) { commit in
        for entry in entries { commit.message(1, entry.write) }
        commit.string(2, cacheGUID)
      }
    }
  }

  /// `markers` are each type's id and how far the phone has it, empty for none of it.
  static func getUpdates(share: String, birthday: String, markers: [(type: Int, token: Bytes)])
    -> Bytes
  {
    envelope(share: share, birthday: birthday, contents: 2) { message in
      message.message(5) { updates in
        updates.bool(3, true)
        for marker in markers {
          updates.message(6) { each in
            each.int(1, Int64(marker.type))
            each.bytes(2, marker.token)
          }
        }
        // NEW_CLIENT for a first download, GU_TRIGGER after.
        updates.varint(9, markers.contains { $0.token.isEmpty } ? 9 : 12)
      }
    }
  }
}

/// An entity as GetUpdates gives it.
struct SyncUpdate {
  var id = ""
  var version: Int64 = 0
  var created: Int64 = 0
  var deleted = false
  var serverTag: String?
  var hash: String?
  var encrypted: EncryptedData?
  /// NigoriSpecifics, on the account's Nigori.
  var nigori: Bytes?
  /// Whether the entity is one of ILO_SPACE.
  var isSpace = false
  /// Whether the entity is one of PASSWORDS, and its PasswordSpecifics' own `encrypted`: a
  /// password is encrypted there, never in the specifics' as other types are.
  var isPassword = false
  var password: EncryptedData?

  init?(_ bytes: Bytes) {
    guard let fields = protoFields(bytes) else { return nil }
    id = fields.last(1)?.string ?? ""
    version = fields.last(4)?.varint.map(Int64.init(bitPattern:)) ?? 0
    created = fields.last(6)?.varint.map(Int64.init(bitPattern:)) ?? 0
    deleted = fields.last(18)?.varint == 1
    serverTag = fields.last(10)?.string
    hash = fields.last(23)?.string
    if let specifics = fields.last(21)?.bytes.flatMap(protoFields) {
      encrypted = specifics.last(1)?.bytes.flatMap(EncryptedData.init)
      nigori = specifics.last(DataType.nigori)?.bytes
      isSpace = specifics.last(DataType.iloSpace) != nil
      if let own = specifics.last(DataType.passwords) {
        isPassword = true
        password = own.bytes.flatMap(protoFields)?.last(1)?.bytes.flatMap(EncryptedData.init)
      }
    }
  }
}

/// ClientToServerResponse.
struct SyncResponse {
  enum ErrorCode: Int {
    case success = 0, notMyBirthday = 2, throttled = 3, transient = 8, obsolete = 13
    case unknown = 100
  }

  struct Entry {
    enum Result: Int { case success = 1, conflict = 2, invalid = 4, overQuota = 5, transient = 6 }
    var result: Result?
    var id = ""
    var version: Int64 = 0
  }

  var error = ErrorCode.unknown
  var message = ""
  var birthday = ""
  var throttle: Int?
  var updates: [SyncUpdate] = []
  var remaining = 0
  var markers: [(type: Int, token: Bytes)] = []
  /// A commit's results, one for each entry sent and in their order.
  var entries: [Entry] = []

  init?(_ bytes: Bytes) {
    guard let fields = protoFields(bytes) else { return nil }
    // The field's default is UNKNOWN, so a response without one is an error.
    if let code = fields.last(4)?.varint {
      error = ErrorCode(rawValue: Int(truncatingIfNeeded: code)) ?? .unknown
    }
    message = fields.last(5)?.string ?? ""
    birthday = fields.last(6)?.string ?? ""
    if let command = fields.last(7)?.bytes.flatMap(protoFields) {
      throttle = command.last(5)?.varint.map(Int.init(truncatingIfNeeded:))
    }
    if let got = fields.last(2)?.bytes.flatMap(protoFields) {
      for field in got {
        switch field.number {
        case 1:
          if let update = field.bytes.flatMap(SyncUpdate.init) { updates.append(update) }
        case 4:
          remaining = field.varint.map(Int.init(truncatingIfNeeded:)) ?? 0
        case 5:
          if let marker = field.bytes.flatMap(protoFields), let type = marker.last(1)?.varint {
            markers.append((Int(truncatingIfNeeded: type), marker.last(2)?.bytes ?? []))
          }
        default: break
        }
      }
    }
    if let commit = fields.last(1)?.bytes.flatMap(protoFields) {
      // EntryResponse is a group, the only one sync still has.
      for field in commit where field.number == 1 {
        guard let inner = field.message.flatMap(protoFields) else { return nil }
        var entry = Entry()
        entry.result = inner.last(2)?.varint.flatMap { Entry.Result(rawValue: Int(truncatingIfNeeded: $0)) }
        entry.id = inner.last(3)?.string ?? ""
        entry.version = inner.last(6)?.varint.map(Int64.init(bitPattern:)) ?? 0
        entries.append(entry)
      }
    }
  }
}
