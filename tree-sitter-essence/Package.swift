// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "TreeSitterEssenceTester",
    products: [
        .library(name: "TreeSitterEssenceTester", targets: ["TreeSitterEssenceTester"]),
    ],
    dependencies: [
        .package(url: "https://github.com/ChimeHQ/SwiftTreeSitter", from: "0.8.0"),
    ],
    targets: [
        .target(
            name: "TreeSitterEssenceTester",
            dependencies: [],
            path: ".",
            sources: [
                "src/parser.c",
                // NOTE: if your language has an external scanner, add it here.
            ],
            resources: [
                .copy("queries")
            ],
            publicHeadersPath: "bindings/swift",
            cSettings: [.headerSearchPath("src")]
        ),
        .testTarget(
            name: "TreeSitterEssenceTesterTests",
            dependencies: [
                "SwiftTreeSitter",
                "TreeSitterEssenceTester",
            ],
            path: "bindings/swift/TreeSitterEssenceTesterTests"
        )
    ],
    cLanguageStandard: .c11
)
