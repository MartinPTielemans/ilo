import PassKit
import QuickLook
import SwiftUI
import UniformTypeIdentifiers

/// How a downloaded file opens.
enum FileKind: Equatable {
  /// A pass Wallet can take, in Wallet's own sheet for adding one.
  case pass
  /// What Quick Look shows: a document, a picture, and a calendar file or a contact with the
  /// system's own Add.
  case looked
  /// Anything else goes to the system's sheet, for an app that opens it.
  case other

  static func of(_ file: URL) -> FileKind {
    if file.pathExtension.lowercased() == "pkpass" {
      // Wallet takes a pass that's whole and signed. Any other is a file like the rest.
      let whole = (try? Data(contentsOf: file)).flatMap { try? PKPass(data: $0) } != nil
      return whole && PKAddPassesViewController.canAddPasses() ? .pass : .other
    }
    return QLPreviewController.canPreview(file as NSURL) ? .looked : .other
  }

  /// The symbol a row shows a file by, from its name alone.
  nonisolated static func symbol(for name: String) -> String {
    let kind = (name as NSString).pathExtension.lowercased()
    if kind == "pkpass" { return "wallet.pass" }
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

/// Wallet's sheet for adding a pass.
struct PassSheet: UIViewControllerRepresentable {
  let file: URL

  func makeUIViewController(context: Context) -> UIViewController {
    let pass = (try? Data(contentsOf: file)).flatMap { try? PKPass(data: $0) }
    return pass.flatMap { PKAddPassesViewController(pass: $0) } ?? UIViewController()
  }

  func updateUIViewController(_ controller: UIViewController, context: Context) {}
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
  /// The system's print sheet for a file, and whether it can print one of its kind.
  @discardableResult
  static func print(_ file: URL, named name: String) -> Bool {
    guard UIPrintInteractionController.canPrint(file) else { return false }
    let info = UIPrintInfo.printInfo()
    info.jobName = name.isEmpty ? file.lastPathComponent : name
    let printer = UIPrintInteractionController.shared
    printer.printInfo = info
    printer.printingItem = file
    printer.present(animated: true)
    return true
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
