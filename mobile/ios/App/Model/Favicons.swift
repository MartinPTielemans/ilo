import ImageIO
import Observation
import UIKit

/// Sites' icons, asked of the site itself and of nobody else: the icon its page links to, or its
/// /favicon.ico. Kept on disk by host.
@Observable
final class Favicons {
  private var icons: [String: UIImage] = [:]
  /// Hosts already looked for, so a site without an icon is asked once a launch.
  @ObservationIgnored private var asked: Set<String> = []
  /// Hosts whose page has said which icon is theirs, which beats /favicon.ico.
  @ObservationIgnored private var told: Set<String> = []
  /// Hosts already looked for on disk alone.
  @ObservationIgnored private var recalled: Set<String> = []
  @ObservationIgnored private let directory: URL
  @ObservationIgnored private let session: URLSession

  init(
    directory: URL = URL.cachesDirectory.appending(path: "favicons", directoryHint: .isDirectory),
    session: URLSession = URLSession(configuration: .ephemeral)
  ) {
    self.directory = directory
    self.session = session
  }

  func icon(for url: URL?) -> UIImage? {
    url?.host().flatMap { icons[$0] }
  }

  /// Finds the icon of a site that's on screen: from disk, else its /favicon.ico.
  func find(for url: URL?) async {
    guard let url, let host = url.host(), ["http", "https"].contains(url.scheme), icons[host] == nil,
      asked.insert(host).inserted
    else { return }
    let file = file(host)
    if let icon = await Self.read(file) {
      if icons[host] == nil { icons[host] = icon }
    } else if let guess = URL(string: "/favicon.ico", relativeTo: url)?.absoluteURL {
      await fetch(guess, for: host)
    }
  }

  /// Finds the icon of a site been to, from disk alone: reading history asks no site for anything.
  func recall(for url: URL?) async {
    guard let host = url?.host(), icons[host] == nil, recalled.insert(host).inserted else { return }
    if let icon = await Self.read(file(host)), icons[host] == nil { icons[host] = icon }
  }

  /// A loaded page linked to its icon.
  func learn(_ icon: URL, for page: URL) async {
    guard let host = page.host(), told.insert(host).inserted else { return }
    asked.insert(host)
    await fetch(icon, for: host)
  }

  /// The icon at an address for a private tab, which holds it itself: nothing here remembers the
  /// site, in a file or for the launch. Asked as any icon is, in a session that keeps nothing.
  func passing(_ address: URL) async -> UIImage? {
    guard let data = await data(at: address) else { return nil }
    return await Self.decode(data, saveTo: nil)
  }

  private func fetch(_ address: URL, for host: String) async {
    guard let data = await data(at: address), let icon = await Self.decode(data, saveTo: file(host)) else { return }
    icons[host] = icon
  }

  private func data(at address: URL) async -> Data? {
    var request = URLRequest(url: address, timeoutInterval: 15)
    request.httpShouldHandleCookies = false
    guard let (data, response) = try? await session.data(for: request),
      (response as? HTTPURLResponse)?.statusCode == 200, data.count < 2_000_000
    else { return nil }
    return data
  }

  private func file(_ host: String) -> URL {
    directory.appending(path: host + ".png")
  }

  @concurrent private nonisolated static func read(_ file: URL) async -> UIImage? {
    guard let data = try? Data(contentsOf: file) else { return nil }
    return await UIImage(data: data)?.byPreparingForDisplay()
  }

  /// The image at the size the largest icon is drawn, 56 pt at 3×, so a 1024-pixel touch icon
  /// isn't held whole for a 20 pt mark.
  @concurrent private nonisolated static func decode(_ data: Data, saveTo file: URL?) async -> UIImage? {
    let options: [CFString: Any] = [
      kCGImageSourceCreateThumbnailFromImageAlways: true,
      kCGImageSourceCreateThumbnailWithTransform: true,
      kCGImageSourceThumbnailMaxPixelSize: 168,
    ]
    guard let source = CGImageSourceCreateWithData(data as CFData, nil),
      let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    else { return nil }
    let icon = UIImage(cgImage: plated(image))
    if let file {
      try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
      try? icon.pngData()?.write(to: file, options: .atomic)
    }
    return icon
  }

  /// A mark that's dark ink on nothing can't be seen on a dark Space, so it's put on a light
  /// plate, as a tab strip's own ground does for it on a computer.
  nonisolated static func plated(_ image: CGImage) -> CGImage {
    let width = image.width, height = image.height
    let whole = CGRect(x: 0, y: 0, width: width, height: height)
    guard
      let context = CGContext(
        data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
    else { return image }
    context.draw(image, in: whole)
    guard let data = context.data else { return image }
    let pixels = data.bindMemory(to: UInt8.self, capacity: width * height * 4)
    var clear = 0
    var light = 0.0
    var ink = 0.0
    for at in stride(from: 0, to: width * height * 4, by: 4) {
      if pixels[at + 3] < 128 { clear += 1 }
      // The colours are already multiplied by how solid the pixel is.
      light += 0.299 * Double(pixels[at]) + 0.587 * Double(pixels[at + 1]) + 0.114 * Double(pixels[at + 2])
      ink += Double(pixels[at + 3])
    }
    guard clear * 8 > width * height, ink > 0, light / ink < 0.3 else { return image }
    context.clear(whole)
    context.setFillColor(gray: 0.93, alpha: 1)
    context.addPath(CGPath(roundedRect: whole, cornerWidth: CGFloat(width) / 4, cornerHeight: CGFloat(height) / 4, transform: nil))
    context.fillPath()
    context.draw(image, in: whole.insetBy(dx: CGFloat(width) * 0.14, dy: CGFloat(height) * 0.14))
    return context.makeImage() ?? image
  }
}
