// swift-tools-version: 6.0
import PackageDescription

// ilo's account and sync for the phone: ilo-server's sign-in, Chromium's sync protocol and its
// encryption, with no dependency but the system's, so it builds and tests on the Mac too.
let package = Package(
  name: "IloSync",
  platforms: [.iOS(.v18), .macOS(.v15)],
  products: [.library(name: "IloSync", targets: ["IloSync"])],
  targets: [
    .target(name: "IloSync"),
    .testTarget(name: "IloSyncTests", dependencies: ["IloSync"]),
  ]
)
