// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "PromptCompanion",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "PromptCompanion", targets: ["PromptCompanion"])],
    targets: [
        .target(name: "CompanionCore"),
        .executableTarget(name: "PromptCompanion", dependencies: ["CompanionCore"]),
        .testTarget(name: "CompanionCoreTests", dependencies: ["CompanionCore", "PromptCompanion"])
    ],
    swiftLanguageModes: [.v5]
)
