// Apple's Translation for a page in another language (#671), with the models macOS keeps on the
// Mac, so the page's words never leave it. Swift's alone: the framework has no Objective-C API. A
// session without SwiftUI's view comes with macOS 26, so before it the Mac offers none.

import AppKit
import Foundation
import Translation

// What host_translate answers, as translate_host.rs reads it.
private let translated: Int32 = 0
private let missing: Int32 = 1
private let failed: Int32 = 2

// The user's language, the first the system prefers, as a language alone ("en" of "en-US"), where
// the Mac can translate into it.
@_cdecl("host_translation_target")
public func hostTranslationTarget(_ out: UnsafeMutablePointer<CChar>, _ capacity: Int) -> Bool {
  guard #available(macOS 26.0, *) else { return false }
  guard let preferred = Locale.preferredLanguages.first,
    let code = Locale.Language(identifier: preferred).languageCode?.identifier
  else { return false }
  return copy(code, out, capacity)
}

// The language's name in the user's own, such as "Spanish" for "es".
@_cdecl("host_language_name")
public func hostLanguageName(
  _ language: UnsafePointer<CChar>, _ out: UnsafeMutablePointer<CChar>, _ capacity: Int
) -> Bool {
  let code = String(cString: language)
  guard let name = Locale.current.localizedString(forIdentifier: code) else { return false }
  return copy(name, out, capacity)
}

// Translates |texts|, a JSON array of strings, from |from| to |to|, and calls |done| with
// |context| on the main thread: with the translations as another array, or with why not. An empty
// text stays empty. A language whose model isn't on the Mac is missing, for System Settings to add.
@_cdecl("host_translate")
public func hostTranslate(
  _ from: UnsafePointer<CChar>, _ to: UnsafePointer<CChar>, _ texts: UnsafePointer<CChar>,
  _ context: UnsafeMutableRawPointer?,
  _ done: @convention(c) (UnsafeMutableRawPointer?, Int32, UnsafePointer<CChar>?) -> Void
) {
  let source = Locale.Language(identifier: String(cString: from))
  let target = Locale.Language(identifier: String(cString: to))
  let list = String(cString: texts).data(using: .utf8).flatMap {
    try? JSONDecoder().decode([String].self, from: $0)
  }
  // The context crosses to the task as a number, handed back on the main thread it came from.
  let handle = UInt(bitPattern: context)
  let answer = { (status: Int32, json: String?) in
    DispatchQueue.main.async {
      let context = UnsafeMutableRawPointer(bitPattern: handle)
      guard let json else { return done(context, status, nil) }
      json.withCString { done(context, status, $0) }
    }
  }
  guard #available(macOS 26.0, *), let list else { return answer(failed, nil) }
  Task {
    switch await LanguageAvailability().status(from: source, to: target) {
    case .installed: break
    case .supported: return answer(missing, nil)
    default: return answer(failed, nil)
    }
    let session = TranslationSession(installedSource: source, target: target)
    let requests = list.enumerated().compactMap { index, text in
      text.isEmpty ? nil : TranslationSession.Request(sourceText: text, clientIdentifier: "\(index)")
    }
    do {
      var out = list
      for response in try await session.translations(from: requests) {
        if let index = response.clientIdentifier.flatMap(Int.init), out.indices.contains(index) {
          out[index] = response.targetText
        }
      }
      let json = try String(decoding: JSONEncoder().encode(out), as: UTF8.self)
      answer(translated, json)
    } catch {
      answer(failed, nil)
    }
  }
}

// System Settings at Language & Region, where Translation Languages adds a language's model.
@_cdecl("host_open_translation_settings")
public func hostOpenTranslationSettings() {
  if let url = URL(string: "x-apple.systempreferences:com.apple.Localization-Settings.extension") {
    NSWorkspace.shared.open(url)
  }
}

private func copy(_ text: String, _ out: UnsafeMutablePointer<CChar>, _ capacity: Int) -> Bool {
  let bytes = Array(text.utf8CString)
  guard bytes.count <= capacity else { return false }
  bytes.withUnsafeBufferPointer { out.update(from: $0.baseAddress!, count: bytes.count) }
  return true
}
