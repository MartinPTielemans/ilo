import Foundation
import FoundationModels

// The phone's own model, Apple's, which runs on the phone and nowhere else. ilo never uses the
// one in Apple's cloud, and gives the model no tools: it reads passages and writes, nothing more.

/// A point as the model writes it. The passage comes first, so the point is written about a
/// passage the model has already chosen.
@available(iOS 26.0, *)
@Generable
nonisolated struct WrittenPoint {
  @Guide(description: "The number in square brackets of the one passage this point mostly came from.")
  var passage: Int
  @Guide(description: "The point, in one plain sentence.")
  var text: String
}

@available(iOS 26.0, *)
@Generable
nonisolated struct WrittenSummary {
  @Guide(description: "The main points, the most important first.", .count(2...5))
  var points: [WrittenPoint]
}

/// An answer as the model writes it. Whether it answered comes last, as its verdict on what it
/// has just written: asked first, it has been seen to say yes and then write that the page
/// doesn't say.
@available(iOS 26.0, *)
@Generable
nonisolated struct WrittenAnswer {
  @Guide(description: "The number in square brackets of the one passage that answers the question, or 0 if none does.")
  var passage: Int
  @Guide(description: "The answer in your own words, in one to three short sentences, from the passages only. If they don't answer it, say that the page doesn't say.")
  var text: String
  @Guide(description: "True only if the passages answered the question. False if the page doesn't say.")
  var answered: Bool
}

@available(iOS 26.0, *)
nonisolated struct AppleModel: ReadingModel {
  private let model = SystemLanguageModel.default

  var contextSize: Int { model.contextSize }

  func tokens(in text: String) async -> Int? {
    if #available(iOS 26.4, *) { return try? await model.tokenCount(for: FoundationModels.Prompt(text)) }
    return nil
  }

  func points(instructions: String, prompt: String) -> AsyncThrowingStream<[DraftPoint], any Error> {
    stream(instructions) { session in
      session.streamResponse(to: FoundationModels.Prompt(prompt), generating: WrittenSummary.self, options: Self.options)
    } draft: { summary in
      (summary.points ?? []).map { DraftPoint(text: $0.text ?? "", passage: $0.passage) }
    }
  }

  func answer(instructions: String, prompt: String) -> AsyncThrowingStream<DraftAnswer, any Error> {
    stream(instructions) { session in
      session.streamResponse(to: FoundationModels.Prompt(prompt), generating: WrittenAnswer.self, options: Self.options)
    } draft: { answer in
      DraftAnswer(answered: answer.answered, text: answer.text ?? "", passage: answer.passage)
    }
  }

  /// The same words for the same page, and an end to an answer that runs away.
  private static var options: GenerationOptions {
    GenerationOptions(samplingMode: .greedy, maximumResponseTokens: Pieces.answerRoom)
  }

  /// One request in a session of its own, with no tools, whose drafts are passed on as they grow.
  private func stream<Content: Generable, Draft: Sendable>(
    _ instructions: String, _ respond: @escaping @Sendable (LanguageModelSession) -> LanguageModelSession.ResponseStream<Content>,
    draft: @escaping @Sendable (Content.PartiallyGenerated) -> Draft
  ) -> AsyncThrowingStream<Draft, any Error> {
    AsyncThrowingStream { continuation in
      let task = Task {
        do {
          let session = LanguageModelSession(model: model, tools: [], instructions: Instructions(instructions))
          for try await snapshot in respond(session) {
            continuation.yield(draft(snapshot.content))
          }
          continuation.finish()
        } catch {
          continuation.finish(throwing: Self.failure(error))
        }
      }
      continuation.onTermination = { _ in task.cancel() }
    }
  }

  /// The system's errors in the app's words.
  static func failure(_ error: any Error) -> any Error {
    if error is CancellationError { return error }
    if let error = error as? LanguageModelSession.GenerationError {
      switch error {
      case .guardrailViolation, .refusal: return ReadingFailure.refused
      case .exceededContextWindowSize: return ReadingFailure.tooLong
      case .unsupportedLanguageOrLocale: return ReadingFailure.language
      case .rateLimited, .concurrentRequests: return ReadingFailure.busy
      case .assetsUnavailable: return ReadingFailure.unavailable
      default: return ReadingFailure.other
      }
    }
    if #available(iOS 27.0, *) {
      if let error = error as? LanguageModelError {
        switch error {
        case .guardrailViolation, .refusal: return ReadingFailure.refused
        case .contextSizeExceeded: return ReadingFailure.tooLong
        case .unsupportedLanguageOrLocale: return ReadingFailure.language
        case .rateLimited, .timeout: return ReadingFailure.busy
        default: return ReadingFailure.other
        }
      }
      if error is SystemLanguageModel.Error { return ReadingFailure.unavailable }
      if error is LanguageModelSession.Error { return ReadingFailure.busy }
    }
    return ReadingFailure.other
  }
}

/// Where the app gets its model, and what the system says of it.
enum ReadingModels {
  /// Read in a view, this follows the system: the model arriving, or Apple Intelligence turned on.
  static var state: ModelState {
    #if DEBUG
      if let pretended = StandInModel.pretended { return pretended }
    #endif
    guard #available(iOS 26.0, *) else { return .absent }
    switch SystemLanguageModel.default.availability {
    case .available: return .ready
    case .unavailable(.modelNotReady): return .downloading
    case .unavailable(.appleIntelligenceNotEnabled): return .turnedOff
    default: return .absent
    }
  }

  static var model: (any ReadingModel)? {
    #if DEBUG
      if StandInModel.pretended == .ready { return StandInModel(pace: .milliseconds(30)) }
    #endif
    guard #available(iOS 26.0, *), SystemLanguageModel.default.isAvailable else { return nil }
    return AppleModel()
  }

  /// Whether the model reads and writes a language.
  static func reads(_ language: Locale.Language?) -> Bool {
    #if DEBUG
      if StandInModel.pretended != nil { return true }
    #endif
    guard #available(iOS 26.0, *), let code = language?.languageCode else { return false }
    return SystemLanguageModel.default.supportedLanguages.contains { $0.languageCode == code }
  }

  /// The language summaries and answers are written in, by its English name: the user's if the
  /// model writes it, else the page's.
  static func writes(page: Locale.Language?) -> String {
    let user = Locale.current.language
    let chosen = reads(user) ? user : (page ?? user)
    let name = chosen.languageCode.flatMap { Locale(identifier: "en").localizedString(forLanguageCode: $0.identifier) }
    return name ?? "English"
  }
}
