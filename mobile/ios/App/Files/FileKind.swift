import PassKit
import QuickLook
import SwiftUI
import UniformTypeIdentifiers

/// How a downloaded file opens.
enum FileKind: Equatable {
  /// The passes of a file that Wallet can take, in Wallet's own sheet for adding them.
  case passes([PKPass])
  /// A calendar file's one event, in the system's sheet for adding one.
  case invite(Invite)
  /// What Quick Look shows: a document, a picture, a contact with the system's own Add, and a
  /// calendar file that isn't one event read exactly, which Quick Look shows and can't add.
  case looked
  /// Anything else goes to the system's sheet, for an app that opens it.
  case other

  static func of(_ file: URL) -> FileKind {
    switch file.pathExtension.lowercased() {
    case "pkpass", "pkpasses":
      // Wallet takes a pass that's whole and signed. Any other is a file like the rest.
      let passes = PKAddPassesViewController.canAddPasses() ? Passes.read(file) : []
      return passes.isEmpty ? .other : .passes(passes)
    case "ics":
      if let invite = (try? Data(contentsOf: file)).flatMap({ Invite.read($0) }) { return .invite(invite) }
    default:
      break
    }
    return QLPreviewController.canPreview(file as NSURL) ? .looked : .other
  }

  /// The symbol a row shows a file by, from its name alone.
  nonisolated static func symbol(for name: String) -> String {
    let kind = (name as NSString).pathExtension.lowercased()
    if kind == "pkpass" || kind == "pkpasses" { return "wallet.pass" }
    guard let type = UTType(filenameExtension: kind) else { return "doc" }
    if type.conforms(to: .image) { return "photo" }
    if type.conforms(to: .archive) { return "doc.zipper" }
    if type.conforms(to: .movie) { return "film" }
    if type.conforms(to: .audio) { return "music.note" }
    if type.conforms(to: .calendarEvent) { return "calendar" }
    if type.conforms(to: .vCard) { return "person.crop.square" }
    return "doc"
  }
}

/// Wallet's sheet for adding a pass, or several.
struct PassSheet: UIViewControllerRepresentable {
  let passes: [PKPass]
  let done: () -> Void

  func makeCoordinator() -> Coordinator {
    Coordinator(done: done)
  }

  func makeUIViewController(context: Context) -> UIViewController {
    let sheet = PKAddPassesViewController(passes: passes)
    sheet?.delegate = context.coordinator
    return sheet ?? UIViewController()
  }

  func updateUIViewController(_ controller: UIViewController, context: Context) {}

  final class Coordinator: NSObject, PKAddPassesViewControllerDelegate {
    let done: () -> Void

    init(done: @escaping () -> Void) {
      self.done = done
    }

    func addPassesViewControllerDidFinish(_ controller: PKAddPassesViewController) {
      done()
    }
  }
}

/// The system's picker of places in Files, to copy a file to. The file in ilo's folder stays.
struct CopyPicker: UIViewControllerRepresentable {
  let file: URL

  func makeUIViewController(context: Context) -> UIDocumentPickerViewController {
    UIDocumentPickerViewController(forExporting: [file], asCopy: true)
  }

  func updateUIViewController(_ controller: UIDocumentPickerViewController, context: Context) {}
}

/// The system's sheet for a file or for a link: save it, open it, send it on.
struct FileSheet: UIViewControllerRepresentable {
  let file: URL

  func makeUIViewController(context: Context) -> UIActivityViewController {
    UIActivityViewController(activityItems: [file], applicationActivities: nil)
  }

  func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}

enum Printing {
  /// The system's print sheet for a tab's page, until the sheet is done: a PDF as the document
  /// it is, from a copy fetched as the page was, and any other page as it prints.
  static func print(_ tab: WebTab, copying downloads: Downloads) async {
    let info = UIPrintInfo.printInfo()
    info.jobName = tab.title.isEmpty ? Site.name(of: tab.url) : tab.title
    let printer = UIPrintInteractionController.shared
    printer.printInfo = info
    var copy: URL?
    if tab.isPDF {
      if let url = tab.url { copy = await downloads.copy(of: url, from: tab.webView) }
      guard let copy, UIPrintInteractionController.canPrint(copy) else { return downloads.discard(copy) }
      printer.printingItem = copy
    } else {
      printer.printFormatter = tab.webView.viewPrintFormatter()
    }
    await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
      // The sheet says it's done once, or never comes up.
      if !printer.present(animated: true, completionHandler: { _, _, _ in done.resume() }) { done.resume() }
    }
    downloads.discard(copy)
  }
}

/// Where the files are, in the Files app: the app's own folder under On My iPhone.
enum FilesApp {
  static func open(_ folder: URL) {
    var address = URLComponents()
    address.scheme = "shareddocuments"
    address.host = ""
    address.path = folder.path
    if let url = address.url { UIApplication.shared.open(url) }
  }
}
