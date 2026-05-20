// swift-tools-version: 6.3
// ActantDB Swift SDK — vertical slice.
// Swift 6.3, .macOS(.v26)/.iOS(.v26) to match the Swoosh consumer (planning/sdk-swift.md
// lists 5.9/14 as the public floor — that lift is a follow-up).

import Foundation
import PackageDescription

let localActantFFI = ProcessInfo.processInfo.environment["ACTANTDB_LOCAL_FFI_XCFRAMEWORK"]
    .flatMap { $0.isEmpty ? nil : $0 }
    .map(localBinaryTargetPath)

var targets: [Target] = []
var actantDBDependencies: [Target.Dependency] = []
var actantDBSwiftSettings: [SwiftSetting] = []

if let localActantFFI {
    targets.append(.binaryTarget(name: "actant_ffiFFI", path: localActantFFI))
    targets.append(.target(
        name: "ActantFFI",
        dependencies: ["actant_ffiFFI"],
        path: ".actantffi/Sources/ActantFFI",
        linkerSettings: [.linkedFramework("SystemConfiguration", .when(platforms: [.macOS]))]
    ))
    actantDBDependencies.append("ActantFFI")
    actantDBSwiftSettings.append(.define("ACTANTDB_LOCAL_FFI"))
}

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
    targets: targets + [
        // TODO: uncomment after the first `vX.Y.Z` tag ships an XCFramework via
        // `.github/workflows/ios-xcframework.yml`. Until then the embedded path
        // throws ActantError.transport("ActantFFI binary target not linked …").
        // URL pattern: https://github.com/Prompt-or-Die-Labs/actantdb/releases/download/vX.Y.Z/ActantFFI.xcframework.zip
        // Checksum is the contents of ActantFFI.checksum produced by the workflow.
        // .binaryTarget(
        //     name: "ActantFFI",
        //     url: "https://github.com/Prompt-or-Die-Labs/actantdb/releases/download/v0.0.X/ActantFFI.xcframework.zip",
        //     checksum: "<paste-the-sha256-from-ActantFFI.checksum-here>"
        // ),
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
