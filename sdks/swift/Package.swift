// swift-tools-version: 6.3
// ActantDB Swift SDK — vertical slice.
// Swift 6.3, .macOS(.v26)/.iOS(.v26) to match the Swoosh consumer (planning/sdk-swift.md
// lists 5.9/14 as the public floor — that lift is a follow-up).

import Foundation
import PackageDescription

let localActantFFI = ProcessInfo.processInfo.environment["ACTANTDB_LOCAL_FFI_XCFRAMEWORK"]
    .flatMap { $0.isEmpty ? nil : $0 }
    .map(localBinaryTargetPath)

let releasedActantFFI: (url: String, checksum: String)? = nil

let actantFFIBinaryTarget: Target?
if let localActantFFI {
    actantFFIBinaryTarget = .binaryTarget(name: "actant_ffiFFI", path: localActantFFI)
} else if let releasedActantFFI {
    actantFFIBinaryTarget = .binaryTarget(
        name: "actant_ffiFFI",
        url: releasedActantFFI.url,
        checksum: releasedActantFFI.checksum
    )
} else {
    actantFFIBinaryTarget = nil
}

let actantFFITargets: [Target] = actantFFIBinaryTarget.map {
    [
        $0,
        .target(
            name: "ActantFFI",
            dependencies: ["actant_ffiFFI"],
            path: "Sources/ActantFFI",
            linkerSettings: [.linkedFramework("SystemConfiguration", .when(platforms: [.macOS]))]
        ),
    ]
} ?? []

let actantDBDependencies: [Target.Dependency] = actantFFIBinaryTarget == nil ? [] : ["ActantFFI"]
let actantDBSwiftSettings: [SwiftSetting] = actantFFIBinaryTarget == nil ? [] : [.define("ACTANTDB_FFI")]

let package = Package(
    name: "ActantDB",
    platforms: [
        .macOS(.v26),
        .iOS(.v26),
    ],
    products: [
        .library(name: "ActantDB", targets: ["ActantDB"]),
        .library(name: "ActantAgent", targets: ["ActantAgent"]),
    ],
    targets: actantFFITargets + [
        .target(
            name: "ActantDB",
            dependencies: actantDBDependencies,
            path: "Sources/ActantDB",
            swiftSettings: actantDBSwiftSettings
        ),
        .target(
            name: "ActantAgent",
            dependencies: ["ActantDB"],
            path: "Sources/ActantAgent"
        ),
        // Shared test-support target. Holds MockURLProtocol + any other
        // cross-suite test fixtures. Previously the mock was byte-duplicated
        // into both test targets; a fix in one would silently fail to land
        // in the other.
        .target(
            name: "ActantTestSupport",
            path: "Tests/ActantTestSupport"
        ),
        .testTarget(
            name: "ActantDBTests",
            dependencies: ["ActantDB", "ActantTestSupport"],
            path: "Tests/ActantDBTests",
            swiftSettings: actantDBSwiftSettings
        ),
        .testTarget(
            name: "ActantAgentTests",
            dependencies: ["ActantAgent", "ActantTestSupport"],
            path: "Tests/ActantAgentTests"
        ),
    ]
)

func localBinaryTargetPath(_ rawPath: String) -> String {
    let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().standardizedFileURL
    let artifact = URL(fileURLWithPath: rawPath).standardizedFileURL
    let rootPath = packageRoot.path.hasSuffix("/") ? packageRoot.path : "\(packageRoot.path)/"
    if artifact.path.hasPrefix(rootPath) {
        return String(artifact.path.dropFirst(rootPath.count))
    }
    return rawPath
}
