import CryptoKit
import Foundation
import Security

/// One certificate at one address: all that Visit Anyway trusts. Another host, another port's
/// other certificate and a new certificate at the same address are each something else.
nonisolated struct CertificateException: Hashable, Sendable {
  let host: String
  let port: Int
  /// The SHA-256 of the site's own certificate, as it sent it.
  let fingerprint: Data

  init(host: String, port: Int, fingerprint: Data) {
    self.host = host.lowercased()
    self.port = port
    self.fingerprint = fingerprint
  }
}

/// What a certificate that can't be trusted says of itself, and what's wrong with it.
nonisolated struct CertificateFacts: Equatable, Sendable {
  enum Fault: Equatable, Sendable {
    case ranOut(Date)
    case notValidUntil(Date)
    /// Made out to another name than the site's.
    case wrongName
    case unknownSigner
    /// Something else the phone doesn't accept of it, which it doesn't say in words to pass on.
    case other
  }

  /// One fact, as a label and what it's a label for: "Ran out on", "12 April 2015".
  struct Line: Equatable, Sendable {
    var label: String
    var value: String?
  }

  var madeOutTo: String
  /// Who signed it, where the site sent that certificate along.
  var signedBy: String?
  var signedItself: Bool
  var faults: [Fault]

  /// What's wrong, in a sentence: the fault that says most, where there are several.
  var sentence: String {
    if faults.contains(.wrongName) { return "Its certificate is for another site." }
    for fault in faults {
      switch fault {
      case .ranOut: return "Its certificate ran out."
      case .notValidUntil: return "Its certificate isn't valid yet."
      default: continue
      }
    }
    if faults.contains(.unknownSigner) {
      return signedItself ? "It signed its own certificate, which anyone can do." : "Nobody the phone trusts signed its certificate."
    }
    return "The phone doesn't accept its certificate."
  }

  /// The facts, for the site called `host`: who the certificate is for, who signed it, and each
  /// thing wrong with it.
  func lines(for host: String) -> [Line] {
    var lines = [Line(label: "Made out to", value: madeOutTo)]
    if signedItself {
      lines.append(Line(label: "Signed by", value: "itself"))
    } else if let signedBy {
      lines.append(Line(label: "Signed by", value: signedBy))
    }
    for fault in faults {
      switch fault {
      case .ranOut(let day): lines.append(Line(label: "Ran out on", value: Self.day(day)))
      case .notValidUntil(let day): lines.append(Line(label: "Not valid until", value: Self.day(day)))
      case .wrongName: lines.append(Line(label: "The name doesn't match", value: host))
      // One that signed itself has said so on the line above.
      case .unknownSigner where !signedItself:
        lines.append(Line(label: signedBy == nil ? "Signed by nobody the phone trusts" : "Nobody the phone trusts signed that"))
      case .unknownSigner: break
      case .other: lines.append(Line(label: "The phone doesn't accept it"))
      }
    }
    return lines
  }

  static func day(_ date: Date) -> String {
    date.formatted(.dateTime.day().month(.wide).year())
  }
}

/// A certificate the phone doesn't trust: what going on would trust, and what to say of it first.
nonisolated struct CertificateFault: Equatable, Sendable {
  let exception: CertificateException
  let facts: CertificateFacts
}

/// The certificates the user went on past, until ilo quits. They're in memory alone: nothing
/// here is written to a file, to a log or to the account, and a private tab has a set of its own.
final class CertificateExceptions {
  private var trusted: Set<CertificateException> = []
  /// Every one trusted since launch, the ones no longer trusted too: a page that still comes
  /// over a connection made before is told by it, and gets its warning back.
  private var marked: Set<CertificateException> = []

  /// Whether any was made for an address, which costs nothing to ask for a page with none.
  func knows(host: String, port: Int) -> Bool {
    guard !marked.isEmpty else { return false }
    let host = host.lowercased()
    return marked.contains { $0.host == host && $0.port == port }
  }

  func trust(_ exception: CertificateException) {
    trusted.insert(exception)
    marked.insert(exception)
  }

  func stop(_ exception: CertificateException) {
    trusted.remove(exception)
  }

  func trusts(_ exception: CertificateException) -> Bool {
    trusted.contains(exception)
  }

  /// Whether a page with this certificate got here past a warning, one since taken back or not.
  func marks(_ exception: CertificateException) -> Bool {
    marked.contains(exception)
  }
}

/// A server's trust, to read off the main thread: the system guards a trust's state itself.
nonisolated struct ServerTrust: @unchecked Sendable {
  let trust: SecTrust

  init(_ trust: SecTrust) {
    self.trust = trust
  }
}

/// Reads a server's trust: which certificate it is, and why the phone doesn't trust it.
nonisolated enum Certificates {
  /// What an exception for the certificate a server sent would be.
  @concurrent static func exception(for server: ServerTrust, host: String, port: Int) async -> CertificateException? {
    leaf(of: server.trust).map { CertificateException(host: host, port: port, fingerprint: fingerprint(of: $0)) }
  }

  /// What's wrong with the certificate a server at `host` sent, judged as of `now`.
  @concurrent static func fault(of server: ServerTrust, host: String, port: Int, now: Date = .now) async -> CertificateFault? {
    read(server.trust, host: host, port: port, now: now)
  }

  static func fingerprint(of certificate: SecCertificate) -> Data {
    Data(SHA256.hash(data: SecCertificateCopyData(certificate) as Data))
  }

  private static func leaf(of trust: SecTrust) -> SecCertificate? {
    // Copying the chain may evaluate the trust, which never goes to the network for it here.
    SecTrustSetNetworkFetchAllowed(trust, false)
    return (SecTrustCopyCertificateChain(trust) as? [SecCertificate])?.first
  }

  static func read(_ trust: SecTrust, host: String, port: Int, now: Date) -> CertificateFault? {
    SecTrustSetNetworkFetchAllowed(trust, false)
    guard let chain = SecTrustCopyCertificateChain(trust) as? [SecCertificate], let leaf = chain.first else { return nil }
    let itself =
      SecCertificateCopyNormalizedIssuerSequence(leaf) as Data? == SecCertificateCopyNormalizedSubjectSequence(leaf) as Data?
    let signer = itself ? nil : chain.dropFirst().first.flatMap { SecCertificateCopySubjectSummary($0) as String? }

    var faults: [CertificateFacts.Fault] = []
    let from = SecCertificateCopyNotValidBeforeDate(leaf) as Date?
    let until = SecCertificateCopyNotValidAfterDate(leaf) as Date?
    if let until, now > until { faults.append(.ranOut(until)) }
    if let from, now < from { faults.append(.notValidUntil(from)) }
    // The rest is judged on a day the certificate holds for, so its dates aren't counted twice.
    var day = now
    if let from, let until, now < from || now > until {
      day = from.addingTimeInterval(until.timeIntervalSince(from) / 2)
    }
    // With what the site sent taken as trusted, only the name is left to be wrong.
    if !passes(chain, for: host, anchors: chain, on: day), passes(chain, for: "", anchors: chain, on: day) { faults.append(.wrongName) }
    if !passes(chain, for: nil, anchors: nil, on: day) { faults.append(.unknownSigner) }
    if faults.isEmpty { faults.append(.other) }

    let facts = CertificateFacts(
      madeOutTo: SecCertificateCopySubjectSummary(leaf) as String? ?? host, signedBy: signer, signedItself: itself, faults: faults)
    return CertificateFault(exception: CertificateException(host: host, port: port, fingerprint: fingerprint(of: leaf)), facts: facts)
  }

  /// Whether a chain holds on a day: as a server's for `host`, as any server's for an empty
  /// name, or with none as a chain alone. With `anchors`, they're trusted in the phone's place.
  private static func passes(_ chain: [SecCertificate], for host: String?, anchors: [SecCertificate]?, on day: Date) -> Bool {
    let policy = host.map { SecPolicyCreateSSL(true, $0.isEmpty ? nil : $0 as CFString) } ?? SecPolicyCreateBasicX509()
    var made: SecTrust?
    guard SecTrustCreateWithCertificates(chain as CFArray, policy, &made) == errSecSuccess, let trust = made else { return false }
    SecTrustSetNetworkFetchAllowed(trust, false)
    SecTrustSetVerifyDate(trust, day as CFDate)
    if let anchors {
      SecTrustSetAnchorCertificates(trust, anchors as CFArray)
      SecTrustSetAnchorCertificatesOnly(trust, true)
    }
    return SecTrustEvaluateWithError(trust, nil)
  }
}
