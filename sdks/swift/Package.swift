// swift-tools-version: 6.3
// ActantDB Swift SDK — vertical slice.
// Swift 6.3, .macOS(.v26)/.iOS(.v26) to match the Swoosh consumer (planning/sdk-swift.md
// lists 5.9/14 as the public floor — that lift is a follow-up).

import PackageDescription

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
    targets: [
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
            // dependencies: ["ActantFFI"],   // <- enable alongside the
            // binaryTarget above; the FFI bridge already guards on
            // `#if canImport(ActantFFI)`.
            path: "Sources/ActantDB"
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
            path: "Tests/ActantDBTests"
        ),
        .testTarget(
            name: "ActantAgentTests",
            dependencies: ["ActantAgent", "ActantTestSupport"],
            path: "Tests/ActantAgentTests"
        ),
    ]
)
