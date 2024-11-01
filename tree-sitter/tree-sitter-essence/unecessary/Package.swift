// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "TreeSitterEssence",
    products: [
        .library(name: "TreeSitterEssence", targets: ["TreeSitterEssence"]),
    ],
    dependencies: [
        .package(url: "https://github.com/ChimeHQ/SwiftTreeSitter", from: "0.8.0"),
    ],
    targets: [
        .target(
            name: "TreeSitterEssence",
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
            name: "TreeSitterEssenceTests",
            dependencies: [
                "SwiftTreeSitter",
                "TreeSitterEssence",
            ],
            path: "bindings/swift/TreeSitterEssenceTests"
        )
    ],
    cLanguageStandard: .c11
)
