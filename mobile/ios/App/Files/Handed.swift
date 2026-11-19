import Compression
import Foundation
import PassKit

/// A file the system has a sheet of its own for: a link to one opens that sheet, as Safari's
/// does, and nothing is asked or listed on the way.
nonisolated enum Handed: Equatable, Sendable {
  /// A calendar file, for the system's sheet that adds an event.
  case invite
  /// A pass or a bundle of passes, for Wallet's sheet that adds them.
  case pass

  /// The most such a file may be, which is the most Wallet takes in a bundle. A file is
  /// fetched for its sheet without a question, so not one of any size.
  static let most: Int64 = 150_000_000

  /// What a server's answer is, by the type it says. Only an answer that says nothing of its
  /// kind is taken by its name.
  init?(type: String?, name: String) {
    let kind = (name as NSString).pathExtension.lowercased()
    switch type?.lowercased() {
    case "text/calendar": self = .invite
    case "application/vnd.apple.pkpass", "application/vnd.apple.pkpasses": self = .pass
    case nil, "", "application/octet-stream", "binary/octet-stream", "application/zip", "application/x-zip-compressed":
      switch kind {
      case "ics": self = .invite
      case "pkpass", "pkpasses": self = .pass
      default: return nil
      }
    default: return nil
    }
  }

  /// The name its copy is kept under, which says what it is whatever the site called it.
  func named(_ name: String) -> String {
    let kind = (name as NSString).pathExtension.lowercased()
    switch self {
    case .invite: return kind == "ics" ? name : name + ".ics"
    case .pass: return kind == "pkpass" || kind == "pkpasses" ? name : name + ".pkpass"
    }
  }
}

enum Passes {
  /// The passes in a file that Wallet accepts: the file itself, or each pass of a bundle. None
  /// for a pass that isn't whole and signed.
  static func read(_ file: URL) -> [PKPass] {
    guard let data = try? Data(contentsOf: file, options: .mappedIfSafe), data.count <= Handed.most else { return [] }
    if let pass = try? PKPass(data: data) { return [pass] }
    return Zip.files(in: data) { $0.lowercased().hasSuffix(".pkpass") }.compactMap { try? PKPass(data: $0) }
  }
}

/// A zip's files, read as far as a bundle of passes needs: files stored or deflated, by the
/// directory at its end. iOS has no reader of its own for an app to call.
nonisolated enum Zip {
  /// The files whose name is `wanted`, each no bigger than `most` and no more than `count` of them.
  static func files(in data: Data, most: Int = 50_000_000, count: Int = 16, wanted: (String) -> Bool) -> [Data] {
    let bytes = [UInt8](data)
    func number(_ at: Int, _ width: Int) -> Int? {
      guard at >= 0, at + width <= bytes.count else { return nil }
      return (0..<width).reduce(0) { $0 | Int(bytes[at + $1]) << (8 * $1) }
    }
    // The directory's end is the last thing in the file, but for a comment.
    guard bytes.count >= 22,
      let end = stride(from: bytes.count - 22, through: max(0, bytes.count - 22 - 0xFFFF), by: -1).first(where: { number($0, 4) == 0x0605_4B50 }),
      let entries = number(end + 10, 2), var at = number(end + 16, 4)
    else { return [] }
    var files: [Data] = []
    for _ in 0..<entries where files.count < count {
      guard number(at, 4) == 0x0201_4B50, let method = number(at + 10, 2), let packed = number(at + 20, 4), let size = number(at + 24, 4),
        let nameLength = number(at + 28, 2), let extra = number(at + 30, 2), let comment = number(at + 32, 2), let header = number(at + 42, 4),
        at + 46 + nameLength <= bytes.count
      else { break }
      let name = String(decoding: bytes[(at + 46)..<(at + 46 + nameLength)], as: UTF8.self)
      at += 46 + nameLength + extra + comment
      // The file's own header says where its bytes begin.
      guard wanted(name), size <= most, number(header, 4) == 0x0403_4B50, let headerName = number(header + 26, 2),
        let headerExtra = number(header + 28, 2)
      else { continue }
      let start = header + 30 + headerName + headerExtra
      guard start + packed <= bytes.count else { continue }
      switch method {
      case 0 where packed == size:
        files.append(Data(bytes[start..<(start + packed)]))
      case 8:
        // Into room for what the directory says it holds and no more, whatever the bytes say.
        var whole = [UInt8](repeating: 0, count: size + 1)
        let got = bytes[start..<(start + packed)].withUnsafeBufferPointer { source in
          source.baseAddress.map { compression_decode_buffer(&whole, size + 1, $0, packed, nil, COMPRESSION_ZLIB) } ?? 0
        }
        if got == size { files.append(Data(whole[..<size])) }
      default:
        continue
      }
    }
    return files
  }
}
