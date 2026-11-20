import Foundation
import SQLite3

/// One connection to an SQLite file, through the system's library and its C API. It isn't safe
/// to share: its owner keeps it to one thread at a time.
nonisolated final class Database {
  /// SQLite's code and nothing more: its message can quote what was being written.
  struct Failure: Error, Equatable {
    let code: Int32
  }

  enum Value {
    case text(String)
    case number(Int64)
  }

  /// A row a statement returned, read by column.
  struct Row {
    fileprivate let statement: OpaquePointer

    func text(_ column: Int32) -> String {
      sqlite3_column_text(statement, column).map { String(cString: $0) } ?? ""
    }

    /// Nil for a column with no value, as the newest of no rows is.
    func number(_ column: Int32) -> Int64? {
      sqlite3_column_type(statement, column) == SQLITE_NULL ? nil : sqlite3_column_int64(statement, column)
    }
  }

  /// A function SQLite calls with one value, on any row a statement looks at.
  typealias Function = @convention(c) (OpaquePointer?, Int32, UnsafeMutablePointer<OpaquePointer?>?) -> Void

  /// Tells SQLite to copy a text it's handed, which is Swift's only for the length of the call.
  static let copied = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

  private let handle: OpaquePointer

  /// Opens the file, making it if it isn't there. Its folder has to be.
  init(file: URL) throws {
    var opened: OpaquePointer?
    // Readable after the first unlock, as the account's own file is, and so is what SQLite keeps
    // beside it. No mutex of SQLite's: the owner already keeps the connection to one thread.
    let flags =
      SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_NOMUTEX
      | SQLITE_OPEN_FILEPROTECTION_COMPLETEUNTILFIRSTUSERAUTHENTICATION
    let code = sqlite3_open_v2(file.path, &opened, flags, nil)
    guard code == SQLITE_OK, let opened else {
      sqlite3_close_v2(opened)
      throw Failure(code: code)
    }
    handle = opened
  }

  deinit {
    sqlite3_close_v2(handle)
  }

  /// Runs statements that take no values and return nothing, such as the ones that make tables.
  func script(_ sql: String) throws {
    try check(sqlite3_exec(handle, sql, nil, nil, nil))
  }

  /// Runs one statement with `values` for its `?1`, `?2`…, and reads each row it returns.
  func rows<Read>(_ sql: String, _ values: [Value] = [], _ read: (Row) -> Read) throws -> [Read] {
    var prepared: OpaquePointer?
    try check(sqlite3_prepare_v2(handle, sql, -1, &prepared, nil))
    defer { sqlite3_finalize(prepared) }
    guard let prepared else { return [] }
    for (index, value) in values.enumerated() {
      switch value {
      case .text(let text): try check(sqlite3_bind_text(prepared, Int32(index + 1), text, -1, Self.copied))
      case .number(let number): try check(sqlite3_bind_int64(prepared, Int32(index + 1), number))
      }
    }
    var rows: [Read] = []
    while true {
      switch sqlite3_step(prepared) {
      case SQLITE_ROW: rows.append(read(Row(statement: prepared)))
      case SQLITE_DONE: return rows
      case let code: throw Failure(code: code)
      }
    }
  }

  /// Runs one statement that changes rows, and says how many.
  @discardableResult func run(_ sql: String, _ values: [Value] = []) throws -> Int {
    _ = try rows(sql, values) { _ in }
    return Int(sqlite3_changes(handle))
  }

  /// Adds `name(value)` to what statements can call. The same value always gives the same answer.
  func define(_ name: String, _ function: Function) throws {
    try check(sqlite3_create_function_v2(handle, name, 1, SQLITE_UTF8 | SQLITE_DETERMINISTIC, nil, function, nil, nil, nil))
  }

  private func check(_ code: Int32) throws {
    guard code == SQLITE_OK else { throw Failure(code: code) }
  }
}
