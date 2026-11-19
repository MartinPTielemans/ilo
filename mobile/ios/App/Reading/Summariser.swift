import Foundation

/// A point of a summary, or an answer, on screen. Its block is the passage it came from, and is
/// only there when the model named a passage it was really given.
nonisolated struct Point: Equatable, Sendable, Identifiable {
  var id: Int
  var text: String
  var block: Int?
}

/// A summary as it's made.
nonisolated struct Summary: Equatable, Sendable {
  enum Step: Equatable, Sendable {
    /// Reading one part of a page too long to read at once.
    case reading(part: Int, of: Int)
    case writing
    case done
  }

  var step: Step
  var points: [Point] = []
  /// The page was longer than is read: the summary covers its first part.
  var cutShort = false
}

/// An answer to a question about the page, as it's written.
nonisolated struct Answer: Equatable, Sendable {
  var text = ""
  var block: Int?
  /// False once the model has said the page doesn't answer the question.
  var answered = true
  var done = false

  /// Whether there's anything of it yet: words, or the word that the page doesn't say.
  var isSomething: Bool {
    !text.isEmpty || !answered
  }
}

/// Measures text in the model's own tokens. Where the system can't count, it reckons three bytes
/// a token, which runs high for prose, and the caller cuts again if the model still says too long.
nonisolated struct Meter: Sendable {
  let model: any ReadingModel

  func count(_ text: String) async -> Int {
    if let exact = await model.tokens(in: text) { return exact }
    return Self.reckon(text)
  }

  static func reckon(_ text: String) -> Int {
    (text.utf8.count + 2) / 3
  }
}

/// Cuts a page's passages into pieces the model can hold.
nonisolated enum Pieces {
  /// The tokens kept back for the model's answer, and for the frame the system puts around a
  /// request: the shape of the answer and the marks between turns.
  static let answerRoom = 700
  static let frameRoom = 260

  /// How many tokens of passages fit beside these instructions.
  static func room(in model: any ReadingModel, instructions: String) async -> Int {
    let used = await Meter(model: model).count(instructions)
    return max(200, model.contextSize - used - answerRoom - frameRoom)
  }

  /// Packs passages in order into pieces of at most `room` tokens, each measured by the model.
  /// A passage too long for a piece of its own is cut to fit. No more than `most` pieces are
  /// made, so a page of any length is measured only as far as it's read.
  static func split(_ passages: [String], room: Int, meter: Meter, most: Int = .max) async -> [[String]] {
    guard !passages.isEmpty else { return [] }
    // The model's tokens to a byte for this text, from a first stretch of it, to pack by; every
    // piece is then measured whole.
    var opening = ""
    for passage in passages {
      opening += opening.isEmpty ? passage : "\n" + passage
      if opening.utf8.count >= 2400 { break }
    }
    let sample = String(opening.prefix(2400))
    let rate = Double(await meter.count(sample)) / Double(max(1, sample.utf8.count))
    func guess(_ passage: String) -> Double { Double(passage.utf8.count + 1) * rate }

    var pieces: [[String]] = []
    var rest = ArraySlice(passages)
    while !rest.isEmpty, pieces.count < most {
      // Measuring takes the model a while, and whoever asked may have stopped waiting.
      if Task.isCancelled { break }
      var take = 0
      var guessed = 0.0
      for passage in rest {
        let size = guess(passage)
        if take > 0, guessed + size > Double(room) * 0.94 { break }
        guessed += size
        take += 1
      }
      while take > 1, await meter.count(rest.prefix(take).joined(separator: "\n")) > room {
        take -= max(1, take / 8)
      }
      var piece = Array(rest.prefix(take))
      if take == 1 {
        var size = await meter.count(piece[0])
        while size > room, piece[0].count > 40 {
          piece[0] = String(piece[0].prefix(max(40, piece[0].count * room / size * 9 / 10)))
          size = await meter.count(piece[0])
        }
      }
      pieces.append(piece)
      rest = rest.dropFirst(take)
    }
    return pieces
  }
}

/// Summarises an article with the phone's model: at once if it fits, else part by part and then
/// the parts' points together.
nonisolated struct Summariser: Sendable {
  let model: any ReadingModel
  /// The language to write in, by its English name.
  var language = "English"
  /// The most parts of a page that are read. Past them the summary says it was cut short.
  var mostParts = 12

  func summarise(_ article: Article) -> AsyncThrowingStream<Summary, any Error> {
    AsyncThrowingStream { continuation in
      let task = Task {
        do {
          try await run(article) { continuation.yield($0) }
          continuation.finish()
        } catch let failure as ReadingFailure {
          continuation.finish(throwing: failure)
        } catch is CancellationError {
          continuation.finish(throwing: CancellationError())
        } catch {
          continuation.finish(throwing: ReadingFailure.other)
        }
      }
      continuation.onTermination = { _ in task.cancel() }
    }
  }

  private func run(_ article: Article, tell: (Summary) -> Void) async throws {
    let instructions = ReadingPrompt.summary(language: language)
    let room = await Self.room(in: model, instructions: instructions, title: article.title)
    do {
      try await run(article, instructions: instructions, room: room, tell: tell)
    } catch ReadingFailure.tooLong {
      // The measure said it fit and the model said not: once more, in pieces half the size.
      try await run(article, instructions: instructions, room: room / 2, tell: tell)
    }
  }

  /// How many tokens of passages fit in a request: what the instructions leave, less the words
  /// every request puts around its passages, the page's title among them.
  static func room(in model: any ReadingModel, instructions: String, title: String) async -> Int {
    let meter = Meter(model: model)
    let around = [ReadingPrompt.summarise(title: title, passages: "", part: (99, of: 99)), ReadingPrompt.combine(title: title, notes: "")]
    var frame = 0
    for words in around { frame = max(frame, await meter.count(words)) }
    return max(200, await Pieces.room(in: model, instructions: instructions) - frame)
  }

  private func run(_ article: Article, instructions: String, room: Int, tell: (Summary) -> Void) async throws {
    let meter = Meter(model: model)
    let blocks = article.textBlocks
    // One part more than is read says the page was cut short.
    var pieces = await Pieces.split(blocks.map(ReadingPrompt.passage), room: room, meter: meter, most: mostParts + 1)
    try Task.checkCancellation()
    guard !pieces.isEmpty else { throw ReadingFailure.other }
    let cutShort = pieces.count > mostParts
    if cutShort { pieces = Array(pieces.prefix(mostParts)) }
    let real = Set(blocks.map(\.id))

    if pieces.count == 1 {
      tell(Summary(step: .writing))
      let prompt = ReadingPrompt.summarise(title: article.title, passages: pieces[0].joined(separator: "\n"))
      let points = try await write(instructions: instructions, prompt: prompt, known: real) { tell(Summary(step: .writing, points: $0)) }
      tell(Summary(step: .done, points: points))
      return
    }

    // Each part's points are notes for the summary, not shown: they'd be replaced a moment later.
    var notes: [Point] = []
    for (index, piece) in pieces.enumerated() {
      try Task.checkCancellation()
      tell(Summary(step: .reading(part: index + 1, of: pieces.count)))
      let prompt = ReadingPrompt.summarise(
        title: article.title, passages: piece.joined(separator: "\n"), part: (index + 1, of: pieces.count))
      notes += try await write(instructions: instructions, prompt: prompt, known: real) { _ in }
    }

    tell(Summary(step: .writing))
    // Notes too many for the model to hold are themselves made fewer, a group at a time.
    var lines = notes.map(Self.note)
    for _ in 0..<3 {
      let groups = await Pieces.split(lines, room: room, meter: meter)
      guard groups.count > 1 else { break }
      notes = []
      for group in groups {
        try Task.checkCancellation()
        let prompt = ReadingPrompt.combine(title: article.title, notes: group.joined(separator: "\n"))
        notes += try await write(instructions: instructions, prompt: prompt, known: real) { _ in }
      }
      lines = notes.map(Self.note)
    }
    let cited = Set(notes.compactMap(\.block))
    let prompt = ReadingPrompt.combine(title: article.title, notes: lines.joined(separator: "\n"))
    let points = try await write(instructions: instructions, prompt: prompt, known: cited) {
      tell(Summary(step: .writing, points: $0, cutShort: cutShort))
    }
    tell(Summary(step: .done, points: points, cutShort: cutShort))
  }

  private static func note(_ point: Point) -> String {
    "[\(point.block.map(String.init) ?? "-")] \(point.text)"
  }

  /// One request for points. A passage the model names counts only if it's one it was given.
  private func write(instructions: String, prompt: String, known: Set<Int>, draft: ([Point]) -> Void) async throws -> [Point] {
    var points: [Point] = []
    do {
      for try await drafts in model.points(instructions: instructions, prompt: prompt) {
        try Task.checkCancellation()
        points = Self.points(drafts, known: known)
        draft(points)
      }
    } catch let failure as ReadingFailure {
      throw failure
    } catch is CancellationError {
      throw CancellationError()
    } catch {
      throw ReadingFailure.other
    }
    guard !points.isEmpty else { throw ReadingFailure.other }
    return points
  }

  static func points(_ drafts: [DraftPoint], known: Set<Int>) -> [Point] {
    drafts.enumerated().compactMap { index, draft in
      let text = draft.text.trimmingCharacters(in: .whitespacesAndNewlines)
      guard !text.isEmpty else { return nil }
      return Point(id: index, text: text, block: draft.passage.flatMap { known.contains($0) ? $0 : nil })
    }
  }
}

/// Answers a question from the article alone. A long article gives only its passages nearest
/// the question, since the model can't hold it all.
nonisolated struct Asker: Sendable {
  struct Turn: Equatable, Sendable {
    var question: String
    var answer: Answer
  }

  let model: any ReadingModel
  var language = "English"

  /// The most tokens of passages a question is answered from, however much the model holds. A
  /// small model answers better, and in a third of the time, from the few passages nearest the
  /// question than from a page's worth.
  static let mostPassages = 1800

  func ask(_ question: String, about article: Article, after earlier: [Turn] = []) -> AsyncThrowingStream<Answer, any Error> {
    AsyncThrowingStream { continuation in
      let task = Task {
        do {
          try await run(question, article, earlier) { continuation.yield($0) }
          continuation.finish()
        } catch let failure as ReadingFailure {
          continuation.finish(throwing: failure)
        } catch is CancellationError {
          continuation.finish(throwing: CancellationError())
        } catch {
          continuation.finish(throwing: ReadingFailure.other)
        }
      }
      continuation.onTermination = { _ in task.cancel() }
    }
  }

  private func run(_ question: String, _ article: Article, _ earlier: [Turn], tell: (Answer) -> Void) async throws {
    let instructions = ReadingPrompt.answers(language: language)
    let meter = Meter(model: model)
    // The last two turns ride along, so "and why?" has something to refer to.
    let said = earlier.suffix(2).map { (question: $0.question, answer: $0.answer.text) }
    let frame = ReadingPrompt.ask(title: article.title, passages: "", earlier: said, question: question)
    let fits = await Pieces.room(in: model, instructions: instructions) - (await meter.count(frame))
    guard fits > 120 else { throw ReadingFailure.tooLong }
    let room = min(fits, Self.mostPassages)

    let chosen = await Self.passages(
      for: question, in: article, near: earlier.suffix(2).compactMap(\.answer.block), room: room, meter: meter)
    let known = Set(chosen.map(\.id))
    let prompt = ReadingPrompt.ask(
      title: article.title, passages: chosen.map(ReadingPrompt.passage).joined(separator: "\n"), earlier: said, question: question)

    var answer = Answer()
    for try await draft in model.answer(instructions: instructions, prompt: prompt) {
      try Task.checkCancellation()
      answer = Self.answer(draft, known: known)
      tell(answer)
    }
    guard !answer.text.isEmpty || !answer.answered else { throw ReadingFailure.other }
    answer.done = true
    tell(answer)
  }

  static func answer(_ draft: DraftAnswer, known: Set<Int>) -> Answer {
    Answer(
      text: draft.text.trimmingCharacters(in: .whitespacesAndNewlines),
      // The passage is linked once the model has said it answered: an answer the page doesn't
      // give has no passage to show.
      block: draft.answered == true ? draft.passage.flatMap { known.contains($0) ? $0 : nil } : nil,
      answered: draft.answered ?? true)
  }

  /// The passages to answer from, in the page's order: all of them if they fit, else those that
  /// share the most of the question's words, with the ones beside them, and the page's opening.
  static func passages(for question: String, in article: Article, near: [Int], room: Int, meter: Meter) async -> [Article.Block] {
    let blocks = article.textBlocks
    let whole = blocks.map(ReadingPrompt.passage).joined(separator: "\n")
    if await meter.count(whole) <= room { return blocks }

    let asked = words(question)
    var score = [Double](repeating: 0, count: blocks.count)
    for (index, block) in blocks.enumerated() {
      let has = words(block.text)
      let shared = Double(asked.intersection(has).count)
      score[index] = shared / (1 + Double(has.count).squareRoot() / 8)
      if near.contains(block.id) { score[index] += 1.5 }
    }
    // A passage beside a good one is often what the good one refers to.
    var ranked = score
    for index in blocks.indices {
      if index > 0 { ranked[index] += score[index - 1] * 0.35 }
      if index + 1 < blocks.count { ranked[index] += score[index + 1] * 0.35 }
    }
    // The opening says what the page is about, whatever is asked.
    for index in blocks.indices.prefix(2) { ranked[index] += 0.6 }

    let rate = Double(await meter.count(String(whole.prefix(2400)))) / Double(max(1, min(2400, whole.count)))
    var taken: Set<Int> = []
    var used = 0.0
    for index in blocks.indices.sorted(by: { ranked[$0] > ranked[$1] || (ranked[$0] == ranked[$1] && $0 < $1) }) {
      let size = Double(ReadingPrompt.passage(blocks[index]).count + 1) * rate
      if used + size > Double(room) * 0.9 { continue }
      used += size
      taken.insert(index)
    }
    var chosen = blocks.indices.filter { taken.contains($0) }.map { blocks[$0] }
    // The reckoning by rate can run over: the model's own count has the last word.
    while chosen.count > 1, await meter.count(chosen.map(ReadingPrompt.passage).joined(separator: "\n")) > room {
      let weakest = chosen.indices.min { a, b in
        let ia = blocks.firstIndex { $0.id == chosen[a].id } ?? 0
        let ib = blocks.firstIndex { $0.id == chosen[b].id } ?? 0
        return ranked[ia] < ranked[ib]
      }
      chosen.remove(at: weakest ?? chosen.count - 1)
    }
    return chosen
  }

  /// A text's words that carry meaning: folded, and without the short ones every text shares.
  static func words(_ text: String) -> Set<String> {
    let folded = text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
    return Set(folded.split { !$0.isLetter && !$0.isNumber }.map(String.init).filter { $0.count > 2 && !common.contains($0) })
  }

  private static let common: Set<String> = [
    "the", "and", "for", "are", "was", "were", "what", "when", "where", "which", "who", "why", "how", "does", "did", "that", "this",
    "with", "from", "have", "has", "had", "its", "about", "into", "page", "article", "say", "says", "said", "tell", "there", "their",
    "they", "you", "your", "can", "could", "would", "should", "been", "being", "not", "any", "all", "but",
  ]
}
