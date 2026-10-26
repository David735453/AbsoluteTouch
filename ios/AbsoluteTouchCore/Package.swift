// swift-tools-version: 6.4
import PackageDescription

let package = Package(
	name: "AbsoluteTouchCore",
	platforms: [
		.iOS(.v17),
		.macOS(.v14),
	],
	products: [
		.library(name: "AbsoluteTouchCore", targets: ["AbsoluteTouchCore"])
	],
	targets: [
		.target(
			name: "AbsoluteTouchCore",
			swiftSettings: [.swiftLanguageMode(.v6)]
		),
		.testTarget(
			name: "AbsoluteTouchCoreTests",
			dependencies: ["AbsoluteTouchCore"],
			swiftSettings: [.swiftLanguageMode(.v6)]
		),
	]
)
