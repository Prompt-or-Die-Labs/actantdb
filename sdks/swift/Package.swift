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
    ],
    targets: [
        .target(
            name: "ActantDB",
            path: "Sources/ActantDB"
        ),
        .testTarget(
            name: "ActantDBTests",
            dependencies: ["ActantDB"],
            path: "Tests/ActantDBTests"
        ),
    ]
)
