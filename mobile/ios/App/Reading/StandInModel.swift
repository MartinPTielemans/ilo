#if DEBUG
  import Foundation

  /// A stand-in for the phone's model, in debug builds only: the tests' model, and with the
  /// `-reading-model stand-in` launch argument the sheet's, where the real one isn't installed.
  /// `-reading-model downloading` and `off` pretend the system's two reasons for a line instead.
  /// It writes no summary worth reading: its points are the opening sentences of passages it was
  /// given, which is enough to see the pieces, the streaming and the links work.
  nonisolated final class StandInModel: ReadingModel, @unchecked Sendable {
    let contextSize: Int
    /// The wait between words, so the streaming shows.
    let pace: Duration
    /// What every request throws, for the failures' sentences.
    var failing: ReadingFailure?
    /// The passage every point names, for a model that names one it wasn't given.
    var citing: Int?
    var counts = true

    private let lock = NSLock()
    private var asked: [String] = []

    init(contextSize: Int = 4096, pace: Duration = .zero) {
      self.contextSize = contextSize
      self.pace = pace
    }

    /// Every prompt it was given, in order.
    var prompts: [String] {
      lock.withLock { asked }
    }

    static let pretended: ModelState? = {
      let arguments = ProcessInfo.processInfo.arguments
      guard let flag = arguments.firstIndex(of: "-reading-model"), arguments.indices.contains(flag + 1) else { return nil }
      switch arguments[flag + 1] {
      case "stand-in": return .ready
      case "downloading": return .downloading
      case "off": return .turnedOff
      default: return nil
      }
    }()

    /// A word and a third to the token, near enough to what real models make of English.
    func tokens(in text: String) async -> Int? {
      counts ? Self.count(text) : nil
    }

    static func count(_ text: String) -> Int {
      (text.split(whereSeparator: \.isWhitespace).count * 4 + 2) / 3
    }

    func points(instructions: String, prompt: String) -> AsyncThrowingStream<[DraftPoint], any Error> {
      let fits = Self.count(instructions) + Self.count(prompt) <= contextSize
      let passages = Self.passages(in: prompt)
      // A few passages spread over what it was given.
      let wanted = min(passages.count, passages.count > 6 ? 4 : 2)
      let picked = (0..<wanted).map { passages[$0 * passages.count / max(1, wanted)] }
      let points = picked.map { DraftPoint(text: Self.sentence(of: $0.text), passage: citing ?? $0.id) }
      return stream(prompt, fits: fits) { continuation in
        var written: [DraftPoint] = []
        for point in points {
          written.append(DraftPoint(text: "", passage: point.passage))
          for word in point.text.split(separator: " ") {
            try await Task.sleep(for: self.pace)
            written[written.count - 1].text += (written[written.count - 1].text.isEmpty ? "" : " ") + word
            continuation.yield(written)
          }
        }
        if written.isEmpty { continuation.yield([]) }
      }
    }

    func answer(instructions: String, prompt: String) -> AsyncThrowingStream<DraftAnswer, any Error> {
      let fits = Self.count(instructions) + Self.count(prompt) <= contextSize
      let question = prompt.components(separatedBy: "\nQuestion: ").last ?? ""
      let asked = Asker.words(question)
      let best = Self.passages(in: prompt)
        .map { (passage: $0, shared: asked.intersection(Asker.words($0.text)).count) }
        .max { $0.shared < $1.shared }
      return stream(prompt, fits: fits) { continuation in
        guard let best, best.shared > 0 else {
          continuation.yield(DraftAnswer(answered: false, text: "The page doesn't say.", passage: 0))
          return
        }
        var answer = DraftAnswer(answered: true, text: "", passage: self.citing ?? best.passage.id)
        for word in Self.sentence(of: best.passage.text).split(separator: " ") {
          try await Task.sleep(for: self.pace)
          answer.text += (answer.text.isEmpty ? "" : " ") + word
          continuation.yield(answer)
        }
      }
    }

    private func stream<Draft: Sendable>(
      _ prompt: String, fits: Bool, write: @escaping @Sendable (AsyncThrowingStream<Draft, any Error>.Continuation) async throws -> Void
    ) -> AsyncThrowingStream<Draft, any Error> {
      lock.withLock { asked.append(prompt) }
      let failing = failing
      return AsyncThrowingStream { continuation in
        let task = Task {
          do {
            if let failing { throw failing }
            if !fits { throw ReadingFailure.tooLong }
            try await write(continuation)
            continuation.finish()
          } catch {
            continuation.finish(throwing: error)
          }
        }
        continuation.onTermination = { _ in task.cancel() }
      }
    }

    /// The numbered passages in a prompt, as the model would read them.
    static func passages(in prompt: String) -> [(id: Int, text: String)] {
      prompt.split(separator: "\n").compactMap { line in
        guard line.hasPrefix("["), let close = line.firstIndex(of: "]"), let id = Int(line[line.index(after: line.startIndex)..<close])
        else { return nil }
        return (id, line[line.index(after: close)...].trimmingCharacters(in: .whitespaces))
      }
    }

    private static func sentence(of text: String) -> String {
      let end = text.firstRange(of: ". ")?.lowerBound ?? text.endIndex
      return String(text[..<end].prefix(160)) + "."
    }
  }
#endif
