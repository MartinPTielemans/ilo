import Foundation
import NaturalLanguage
import Observation
import Translation
import WebKit

/// What reading a page is doing now: the article of the page on screen, and the sheet where it's
/// summarised and asked about. Nothing here outlives its page: no text, question or answer is
/// stored or logged.
@Observable
final class Reading {
  /// A summary and the questions after it, for as long as the sheet is open.
  struct Talk {
    let tab: WebTab
    let article: Article
    /// The reading of the page the article came from, which its passages are shown by.
    let reading: String
    var summary: Summary?
    var turns: [Asker.Turn] = []
    /// The question being answered.
    var asking: String?
    var answer: Answer?
    /// The one sentence for what went wrong.
    var failure: String?
    /// Put down to a strip at the foot, so the page shows the passage a point came from.
    var lowered = false
    /// The point whose passage the page is showing.
    var checking: String?

    var isWorking: Bool {
      asking != nil || (summary != nil && summary?.step != .done)
    }
  }

  /// The article of the page on screen, once it has loaded and been read.
  private(set) var article: Article?
  /// The language the article is written in, as its words show.
  private(set) var language: Locale.Language?
  private(set) var talk: Talk?
  /// The Reader, while it's open.
  private(set) var reader: Reader?
  /// The translation this page can have, if it's in a language the user doesn't read.
  private(set) var translation: PageTranslation?
  /// Goes up each time the followed tab's web view starts or ends a load, as WebKit says it. A
  /// reload can start and end between two frames, and leaves the same tab at the same address.
  private(set) var loads = 0

  /// Which reading of which page the article is: the name is the one the page's script holds
  /// its blocks under, so a document that came after it doesn't answer to it.
  @ObservationIgnored private var read: (tab: String, url: URL, name: String)?
  @ObservationIgnored private var heard: (tab: String, loading: NSKeyValueObservation)?
  /// The tab whose page is being translated, which is asked for its blocks.
  @ObservationIgnored private weak var translating: WebTab?
  @ObservationIgnored private var work: Task<Void, Never>?
  /// The model, for tests to put their own in.
  @ObservationIgnored var model: () -> (any ReadingModel)? = { ReadingModels.model }
  /// The translation a page in a language can have, for tests to put their own in.
  @ObservationIgnored var offers: (Locale.Language?) async -> PageTranslation? = { await PageTranslation.offer(for: $0) }

  /// Follows the page on screen: reads its article once it has loaded, and lets go of what
  /// belonged to the document before it.
  func watch(_ tab: WebTab?) async {
    // A place within the page is the same page.
    guard let tab, let url = tab.url.map(Self.page) else {
      hear(nil)
      forget()
      return
    }
    hear(tab)
    // A load on its way leaves this document, so nothing read of it is offered while it goes.
    guard !tab.isLoading else {
      forget()
      return
    }
    if let read, read.tab == tab.id {
      // The same address may be another document now, after a reload, or the same one redrawn.
      if read.url == url, await tab.holds(read.name) { return }
      guard !Task.isCancelled else { return }
    }
    forget()
    await take(tab, at: url)
  }

  /// Reads the page again if the first reading found no prose: a page may fill in after it loads.
  func look(_ tab: WebTab) async {
    guard !tab.isLoading, let url = tab.url.map(Self.page), article?.hasProse != true else { return }
    // Reading again numbers the blocks afresh and puts a translated page's own words back.
    guard translation?.isOn != true, talk == nil, reader == nil else { return }
    await take(tab, at: url)
  }

  /// Whether what was read is of the document the tab holds now. If it isn't, the page is read
  /// again first, so nothing is ever done with a page that has gone.
  private func fresh(_ tab: WebTab) async -> Bool {
    guard !tab.isLoading, let url = tab.url.map(Self.page) else { return false }
    if let read, read.tab == tab.id, read.url == url, await tab.holds(read.name) { return true }
    forget()
    await take(tab, at: url)
    return read?.tab == tab.id
  }

  private func take(_ tab: WebTab, at url: URL) async {
    let name = UUID().uuidString
    reads += 1
    let turn = reads
    let article = await tab.readArticle(as: name)
    let language = await Task.detached { article.flatMap(PageLanguage.of) }.value
    // The page may have gone elsewhere while it was read, or been read again since.
    guard turn == reads, !tab.isLoading, tab.url.map(Self.page) == url, !Task.isCancelled else { return }
    // WebKit can name the address it's going to before the document there has come: what was
    // read then is the page before it.
    guard let article, Self.isRead(article, at: url) else {
      forget()
      return
    }
    read = (tab.id, url, name)
    self.article = article
    self.language = language
    translation = nil
    let offer = await offers(language)
    if read?.name == name { translation = offer }
  }

  /// Counts the readings, so one that was overtaken by another is dropped.
  @ObservationIgnored private var reads = 0

  /// Whether an article was read from the page at `url`, as the page itself said where it was.
  static func isRead(_ article: Article, at url: URL) -> Bool {
    article.url.map(page) == page(url)
  }

  private func forget() {
    read = nil
    article = nil
    language = nil
    reader = nil
    endTranslation()
    translation = nil
    close()
  }

  /// Hears WebKit's own word on the tab's loading, each time it changes.
  private func hear(_ tab: WebTab?) {
    guard heard?.tab != tab?.id else { return }
    heard?.loading.invalidate()
    heard = tab.map { tab in
      let loading = tab.webView.observe(\.isLoading) { [weak self] _, _ in
        // WebKit changes it on the main thread.
        MainActor.assumeIsolated { self?.loads += 1 }
      }
      return (tab.id, loading)
    }
  }

  private static func page(_ url: URL) -> URL {
    guard url.fragment() != nil, var parts = URLComponents(url: url, resolvingAgainstBaseURL: false) else { return url }
    parts.fragment = nil
    return parts.url ?? url
  }

  /// What the action sheet does about the summary for this page.
  var offer: SummaryOffer {
    SummaryOffer(model: ReadingModels.state, hasProse: article?.hasProse == true, readsLanguage: ReadingModels.reads(language))
  }

  // MARK: The sheet

  /// Opens the sheet and starts the summary, which is only ever made when asked for.
  func summarise(_ tab: WebTab) async {
    guard await fresh(tab), let article, article.hasProse, let name = read?.name, let model = model() else { return }
    close()
    talk = Talk(tab: tab, article: article, reading: name, summary: Summary(step: .writing))
    let summariser = Summariser(model: model, language: ReadingModels.writes(page: language))
    work = Task {
      do {
        for try await summary in summariser.summarise(article) {
          talk?.summary = summary
        }
      } catch is CancellationError {
      } catch {
        fail(error)
      }
      // Stopped or failed, what was written so far stays.
      if let summary = talk?.summary, summary.step != .done {
        talk?.summary = summary.points.isEmpty ? nil : Summary(step: .done, points: summary.points, cutShort: summary.cutShort)
      }
    }
  }

  func ask(_ question: String) {
    let question = question.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !question.isEmpty, let talk, !talk.isWorking, let model = model() else { return }
    self.talk?.failure = nil
    self.talk?.asking = question
    self.talk?.answer = Answer()
    let asker = Asker(model: model, language: ReadingModels.writes(page: language))
    work = Task {
      do {
        for try await answer in asker.ask(question, about: talk.article, after: talk.turns) {
          self.talk?.answer = answer
        }
        // Stopped before its first word, an answer is no turn: there's nothing to show, and
        // nothing for the next question to refer to.
        if let answer = self.talk?.answer, answer.isSomething {
          self.talk?.turns.append(Asker.Turn(question: question, answer: answer))
        }
      } catch is CancellationError {
      } catch {
        fail(error)
      }
      self.talk?.asking = nil
      self.talk?.answer = nil
    }
  }

  private func fail(_ error: any Error) {
    talk?.failure = ((error as? ReadingFailure) ?? .other).sentence
  }

  /// Stops what the model is writing and keeps what it wrote.
  func stop() {
    work?.cancel()
  }

  /// What the model is writing, for a test to wait for its end.
  var working: Task<Void, Never>? { work }

  /// Puts the sheet away, and with it the summary and the conversation.
  func close() {
    work?.cancel()
    work = nil
    talk = nil
  }

  /// Shows the passage a point came from: the sheet goes down to a strip and the page scrolls.
  func check(_ point: Point, smooth: Bool) {
    guard let talk, let block = point.block else { return }
    self.talk?.lowered = true
    self.talk?.checking = point.text
    Task {
      // An anchor the page no longer has, after it rebuilt itself: the sheet comes back up.
      if await !talk.tab.show(block: block, of: talk.reading, smooth: smooth) { self.talk?.lowered = false }
    }
  }

  func raise() {
    talk?.lowered = false
  }

  // MARK: The Reader

  struct Reader {
    let tab: WebTab
    let article: Article
    /// The reading of the page the article came from.
    let reading: String
    /// The block the page had at its top, where the Reader opens.
    var start: Int?
  }

  /// Opens the article alone, at the place the page was scrolled to.
  func openReader(_ tab: WebTab) async {
    guard await fresh(tab), let article, article.isArticle, let name = read?.name else { return }
    close()
    let start = await tab.topBlock(of: name)
    guard read?.name == name else { return }
    reader = Reader(tab: tab, article: article, reading: name, start: start)
  }

  /// Goes back to the page, at the block the Reader had at its top.
  func closeReader(at block: Int?) {
    guard let reader else { return }
    self.reader = nil
    if let block { Task { await reader.tab.show(block: block, of: reader.reading, centred: false, smooth: false) } }
  }

  /// A link in the Reader goes where it says, as the same link in the page would.
  func follow(_ url: URL) {
    guard let reader else { return }
    self.reader = nil
    reader.tab.follow(url)
  }

  // MARK: Translation

  /// Translates the page where it stands, or puts its own words back.
  func toggleTranslation(_ tab: WebTab) async {
    let on = translation?.isOn != true
    // A page read again has its own words already, and its translation off.
    guard await fresh(tab), translation != nil, translation?.isOn != on else { return }
    translation?.isOn = on
    if !on { endTranslation() }
  }

  /// The words of every block to translate, by block, while translation is on.
  func translatable() -> [Int: String]? {
    guard let article, translation?.isOn == true else { return nil }
    return Dictionary(article.textBlocks.filter { $0.kind != .code }.map { ($0.id, $0.text) }, uniquingKeysWith: { first, _ in first })
  }

  /// The languages couldn't be had, or the user said no to downloading them.
  func declineTranslation() {
    translation?.isOn = false
    endTranslation()
  }

  /// Puts the page's own words back and answers whoever waits for its blocks, at once: the wait
  /// is the page's, and nothing else ends it.
  private func endTranslation() {
    translating?.webView.callAsyncJavaScript(TranslateScript.restore, arguments: [:], in: nil, in: ArticleScript.world)
    translating = nil
  }

  /// Has the page say which blocks come into view. Answers whether it will.
  func watchBlocks(_ ids: [Int], in tab: WebTab?) async -> Bool {
    guard let tab, translation?.isOn == true, let read, read.tab == tab.id else { return false }
    translating = tab
    let watching = try? await tab.webView.callAsyncJavaScript(
      TranslateScript.start, arguments: ["ids": ids, "reading": read.name], in: nil, contentWorld: ArticleScript.world)
    return watching as? Bool == true
  }

  /// The blocks that came into view untranslated. It waits in the page, with no timer, until
  /// there are some, and answers none once translation is off or the page has gone.
  func blocksDue() async -> [Int] {
    guard translation?.isOn == true else { return [] }
    let ids: [Int] = await withCheckedContinuation { answer in
      // The web view is asked and let go at once: holding it through a wait that may never end
      // would keep a closed tab's page alive.
      guard let web = translating?.webView else { return answer.resume(returning: []) }
      web.callAsyncJavaScript(TranslateScript.next, arguments: [:], in: nil, in: ArticleScript.world) { result in
        answer.resume(returning: (try? result.get()) as? [Int] ?? [])
      }
    }
    return translation?.isOn == true ? ids : []
  }

  func put(_ text: String, in block: Int) async {
    guard translation?.isOn == true, let web = translating?.webView else { return }
    _ = try? await web.callAsyncJavaScript(
      TranslateScript.apply, arguments: ["id": block, "text": text], in: nil, contentWorld: ArticleScript.world)
  }
}

/// The language a page is written in, from its words rather than from what it says of itself.
nonisolated enum PageLanguage {
  static func of(_ article: Article) -> Locale.Language? {
    let sample = String(article.textBlocks.lazy.filter { $0.kind != .code }.map(\.text).joined(separator: "\n").prefix(3000))
    guard sample.count >= 120 else { return nil }
    let recogniser = NLLanguageRecognizer()
    recogniser.processString(sample)
    guard let language = recogniser.dominantLanguage, language != .undetermined,
      (recogniser.languageHypotheses(withMaximum: 1)[language] ?? 0) >= 0.6
    else { return nil }
    return Locale.Language(identifier: language.rawValue)
  }
}
