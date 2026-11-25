// core/sync_spaces.proto's SpaceEntity. A field keeps whether it was set, since the computer
// compares an entity's bytes with what it would write and commits again on any difference.

import Foundation

struct SpaceEntity: Equatable, Codable, Sendable {
  enum Kind: Int, Codable, Sendable { case space = 0, pin, folder, today }

  static let todayPrefix = "today:"

  var id: String?
  var kind: Kind?
  var space: String?
  var parent: String?
  /// A serialized sync_pb.UniquePosition.
  var position: Bytes?
  var name: String?
  var url: String?
  var title: String?
  var color: UInt32?
  var icon: String?
  var theme: String?
  /// Seconds since 1970.
  var seen: Int64?
  /// The node key of the today's folder a Today tab is in on the computers, kept and written back
  /// as it came so a write here never takes the tab out of it there. The app draws no today's
  /// folder, so it's never read.
  var todayFolder: String?

  /// The node's key: a Today tab's id without its prefix.
  static func key(_ id: String) -> String {
    id.hasPrefix(todayPrefix) ? String(id.dropFirst(todayPrefix.count)) : id
  }

  /// An entity and whether every field of it was one of these, with the type the schema gives
  /// it. One with more was written by a newer ilo, and is never written back.
  static func parse(_ bytes: Bytes) -> (entity: SpaceEntity, understood: Bool)? {
    guard let fields = protoFields(bytes) else { return nil }
    var entity = SpaceEntity()
    var understood = true
    for field in fields {
      switch (field.number, field.value) {
      case (1, .bytes), (3, .bytes), (4, .bytes), (6, .bytes), (7, .bytes), (8, .bytes),
        (10, .bytes), (11, .bytes), (13, .bytes):
        guard let text = field.string else {
          understood = false
          continue
        }
        switch field.number {
        case 1: entity.id = text
        case 3: entity.space = text
        case 4: entity.parent = text
        case 6: entity.name = text
        case 7: entity.url = text
        case 8: entity.title = text
        case 10: entity.icon = text
        case 11: entity.theme = text
        default: entity.todayFolder = text
        }
      case (2, .varint(let value)):
        // A kind this build doesn't know is kept as an unknown field by the computer's parser.
        if let kind = Int32(exactly: Int64(bitPattern: value)).flatMap({ Kind(rawValue: Int($0)) }) {
          entity.kind = kind
        } else {
          understood = false
        }
      case (5, .bytes(let value)):
        entity.position = value
      case (9, .varint(let value)):
        entity.color = UInt32(truncatingIfNeeded: value)
        understood = understood && value <= UInt64(UInt32.max)
      case (12, .varint(let value)):
        entity.seen = Int64(bitPattern: value)
      default:
        understood = false
      }
    }
    return (entity, understood)
  }

  /// The fields that are set, in their numbers' order, as protobuf writes them.
  var serialized: Bytes {
    var writer = ProtoWriter()
    if let id { writer.string(1, id) }
    if let kind { writer.int(2, Int64(kind.rawValue)) }
    if let space { writer.string(3, space) }
    if let parent { writer.string(4, parent) }
    if let position { writer.bytes(5, position) }
    if let name { writer.string(6, name) }
    if let url { writer.string(7, url) }
    if let title { writer.string(8, title) }
    if let color { writer.varint(9, UInt64(color)) }
    if let icon { writer.string(10, icon) }
    if let theme { writer.string(11, theme) }
    if let seen { writer.int(12, seen) }
    if let todayFolder { writer.string(13, todayFolder) }
    return writer.bytes
  }

  /// What core/sync_spaces.cc's Parse asks of an entity before it takes it.
  var isValid: Bool {
    guard let id else { return false }
    return id.hasPrefix(Self.todayPrefix) == (kind == .today) && Self.isLowercaseUUID(Self.key(id))
  }

  static func isLowercaseUUID(_ text: String) -> Bool {
    let bytes = Bytes(text.utf8)
    guard bytes.count == 36 else { return false }
    for (i, byte) in bytes.enumerated() {
      let dash = i == 8 || i == 13 || i == 18 || i == 23
      let hex = (byte >= 0x30 && byte <= 0x39) || (byte >= 0x61 && byte <= 0x66)
      if dash ? byte != 0x2d : !hex { return false }
    }
    return true
  }
}
