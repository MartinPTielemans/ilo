import Foundation
import SQLite3

/// The phone's history: the pages reached on it, in one SQLite file among the app's own. It stays
/// on the phone. Nothing here is sent anywhere, and the store knows of no server.
///
/// The table has every field of the record an account's devices will share (#3451), so sharing
/// adds a table of its own beside this one and fills `chunk`, and nothing here changes shape.
actor HistoryStore {
  /// How long a visit is kept. None: everything stays, until the owner picks a window (#3451).
  static let keeps: Duration? = nil

  nonisolated let file: URL

  // An actor on a serial queue of its own. The actor has the compiler keep the one connection to
  // one caller at a time, and the queue is where its waits on the disk happen: never on the main
  // thread, which only hands work over, nor on Swift's shared threads, which are as few as cores.
  private nonisolated let queue = DispatchSerialQueue(label: "app.ilo.history")
  nonisolated var unownedExecutor: UnownedSerialExecutor { queue.asUnownedSerialExecutor() }

  private var opened: Database?
  private var own: String?

  /// Nothing is read or made until the store is first used, so launch never waits for it.
  init(file: URL) {
    self.file = file
  }

  private func open() throws -> Database {
    if let opened { return opened }
    try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
    let database = try Database(file: file)
    // What's deleted is overwritten in the file, not only unlisted: a cleared history is gone.
    // `chunk` has no type: which of a chunk's names goes there is sync's to say.
    try database.script(
      """
      PRAGMA journal_mode = WAL;
      PRAGMA synchronous = NORMAL;
      PRAGMA secure_delete = ON;
      CREATE TABLE IF NOT EXISTS visits (
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        time INTEGER NOT NULL,
        typed INTEGER NOT NULL,
        device TEXT NOT NULL,
        chunk,
        UNIQUE (device, time)
      );
      CREATE INDEX IF NOT EXISTS visits_by_time ON visits (time);
      CREATE INDEX IF NOT EXISTS visits_by_page ON visits (url, time);
      CREATE TABLE IF NOT EXISTS store (name TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
      """)
    try database.define("fold", foldForSearch)
    if let keeps = Self.keeps {
      let oldest = Visit.time(of: .now) - Int64(keeps.components.seconds) * 1_000_000
      try database.run("DELETE FROM visits WHERE time < ?1", [.number(oldest)])
    }
    opened = database
    return database
  }

  /// This phone's name on its visits: random, made with the store and kept in it. The app has no
  /// other to give: the one sync uses is made anew at each sign-in, and there's none signed out.
  func device() throws -> String {
    if let own { return own }
    let database = try open()
    // In the form of a sync client's id: 16 random bytes.
    let made = Data((0..<16).map { _ in UInt8.random(in: .min ... .max) }).base64EncodedString()
    try database.run("INSERT INTO store (name, value) VALUES ('device', ?1) ON CONFLICT (name) DO NOTHING", [.text(made)])
    guard let kept = try database.rows("SELECT value FROM store WHERE name = 'device'", [], { $0.text(0) }).first else {
      throw Database.Failure(code: SQLITE_NOTFOUND)
    }
    own = kept
    return kept
  }

  // MARK: Writing

  /// Writes a visit, and says whether history took it. This is the only way in, so what history
  /// leaves out is left out here: what isn't a web page's address, an address with a login in
  /// it, one too long to be a page's. A visit already there, by its device and time, is the
  /// same visit. A private tab, when the phone has them, has no history to call this on.
  @discardableResult
  func write(_ url: URL, title: String = "", at time: Int64, typed: Bool, by device: String? = nil) throws -> Bool {
    guard Visit.keeps(url) else { return false }
    let device = try device ?? self.device()
    return try open().run(
      "INSERT INTO visits (url, title, time, typed, device) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (device, time) DO NOTHING",
      [.text(url.absoluteString), .text(title), .number(time), .number(typed ? 1 : 0), .text(device)]) == 1
  }

  /// The page of this phone's visit at `time` has said its title, or changed it.
  func retitle(_ time: Int64, to title: String) throws {
    try open().run(
      "UPDATE visits SET title = ?1 WHERE device = ?2 AND time = ?3 AND title <> ?1", [.text(title), .text(try device()), .number(time)])
  }

  // MARK: Reading

  /// Every visit, newest first, `limit` at a time: the next lot is the one `before` the last's time.
  func visits(before: Int64? = nil, limit: Int) throws -> [Visit] {
    try open().rows(
      "SELECT url, title, time, typed, device FROM visits WHERE time < ?1 ORDER BY time DESC LIMIT ?2",
      [.number(before ?? .max), .number(Int64(limit))], Self.visit
    ).compactMap(\.self)
  }

  /// History as a list shows it, newest first: each page once a day, as its last visit that day
  /// with the title it had then, which is how the computer's Library lists it. `limit` at a
  /// time; the next lot is the one `before` the last's time. A day is the `calendar`'s.
  func pages(before: Int64? = nil, limit: Int, calendar: Calendar = .current) throws -> [Visit] {
    let database = try open()
    var pages: [Visit] = []
    var before = before ?? .max
    // A day at a time, since only the calendar knows where a day ends: each is found by the
    // newest visit not yet listed, so days with nothing in them cost nothing.
    while pages.count < limit {
      let newest = try database.rows("SELECT MAX(time) FROM visits WHERE time < ?1", [.number(before)]) { $0.number(0) }
      guard let newest = newest.first ?? nil, let day = calendar.dateInterval(of: .day, for: Date(timeIntervalSince1970: Double(newest) / 1_000_000))
      else { break }
      let start = Visit.time(of: day.start)
      pages += try database.rows(
        """
        SELECT url, title, time, typed, device FROM visits AS visit
        WHERE time >= ?1 AND time < ?2
          AND NOT EXISTS (SELECT 1 FROM visits WHERE url = visit.url AND time > visit.time AND time < ?3)
        ORDER BY time DESC LIMIT ?4
        """,
        [.number(start), .number(before), .number(Visit.time(of: day.end)), .number(Int64(limit - pages.count))], Self.visit
      ).compactMap(\.self)
      before = start
    }
    return pages
  }

  /// The pages whose title or address has every word of `text`, for the search field's matches:
  /// each address once, as its newest visit that matches, newest first. Capitals and accents
  /// don't count. It reads every visit, which a phone's history is small enough for.
  func search(_ text: String, limit: Int = 5) throws -> [Visit] {
    // A search asked for as someone types is given up when the next letter comes, before it reads.
    try Task.checkCancellation()
    let words = Visit.folded(text).split(whereSeparator: \.isWhitespace).prefix(8).map(String.init)
    guard !words.isEmpty else { return [] }
    let each = words.indices.map { "(instr(fold(title), ?\($0 + 1)) > 0 OR instr(fold(url), ?\($0 + 1)) > 0)" }
    // SQLite gives the other columns of the row MAX picked.
    return try open().rows(
      """
      SELECT url, title, MAX(time), typed, device FROM visits WHERE \(each.joined(separator: " AND "))
      GROUP BY url ORDER BY MAX(time) DESC LIMIT ?\(words.count + 1)
      """,
      words.map { .text($0) } + [.number(Int64(limit))], Self.visit
    ).compactMap(\.self)
  }

  private static func visit(_ row: Database.Row) -> Visit? {
    guard let url = URL(string: row.text(0)), let time = row.number(2) else { return nil }
    return Visit(url: url, title: row.text(1), time: time, typed: row.number(3) == 1, device: row.text(4))
  }

  // MARK: Forgetting

  // Each answers with the visits it forgot, which is all an Undo has to put back: once the
  // caller lets go of them, nothing of them is left.

  /// Forgets a page: every visit to its address.
  @discardableResult func forget(_ url: URL) throws -> [Visit] {
    try forget("DELETE FROM visits WHERE url = ?1", [.text(url.absoluteString)])
  }

  /// Forgets every visit made from the start of `range` up to its end.
  @discardableResult func forget(_ range: Range<Date>) throws -> [Visit] {
    try forget(
      "DELETE FROM visits WHERE time >= ?1 AND time < ?2", [.number(Visit.time(of: range.lowerBound)), .number(Visit.time(of: range.upperBound))])
  }

  /// Forgets every visit. The phone keeps its name.
  @discardableResult func forgetEverything() throws -> [Visit] {
    try forget("DELETE FROM visits", [])
  }

  private func forget(_ sql: String, _ values: [Database.Value]) throws -> [Visit] {
    let database = try open()
    let forgotten = try database.rows(sql + " RETURNING url, title, time, typed, device", values, Self.visit).compactMap(\.self)
    // The log beside the file still has what was deleted, until it's written back and emptied.
    try database.script("PRAGMA wal_checkpoint(TRUNCATE);")
    return forgotten
  }

  /// Puts back visits a forgetting answered with, each as it was: its page, title, time and
  /// device. All of them or none.
  func restore(_ visits: [Visit]) throws {
    let database = try open()
    try database.script("BEGIN")
    do {
      for visit in visits {
        try write(visit.url, title: visit.title, at: visit.time, typed: visit.typed, by: visit.device)
      }
      try database.script("COMMIT")
    } catch {
      try? database.script("ROLLBACK")
      throw error
    }
  }
}

/// `fold(text)` for the search's statement: `Visit.folded` of a title or an address.
private nonisolated func foldForSearch(_ context: OpaquePointer?, _ count: Int32, _ values: UnsafeMutablePointer<OpaquePointer?>?) {
  guard let value = values?[0], let start = sqlite3_value_text(value) else { return sqlite3_result_null(context) }
  let text = UnsafeBufferPointer(start: start, count: Int(sqlite3_value_bytes(value)))
  guard text.allSatisfy({ $0 < 0x80 }) else {
    return sqlite3_result_text(context, Visit.folded(String(decoding: text, as: UTF8.self)), -1, Database.copied)
  }
  // Every address and most titles are plain ASCII, where folding is only A to Z made small:
  // done on the bytes, since a search asks this of every visit.
  let folded = text.map { (0x41...0x5A).contains($0) ? $0 | 0x20 : $0 }
  folded.withUnsafeBufferPointer { bytes in
    bytes.withMemoryRebound(to: CChar.self) { sqlite3_result_text(context, $0.baseAddress, Int32($0.count), Database.copied) }
  }
}
