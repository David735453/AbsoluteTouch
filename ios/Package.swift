// swift-tools-version: 6.0
import PackageDescription

// SwiftPM manifest for the xtool build of the iPad app.
//
// xtool builds exactly one library product into the app bundle, so the app is
// a library target rather than an executable. The Xcode/XcodeGen project in
// `project.yml` stays the release path; this package exists so the same sources
// can be built and run from Linux without Xcode.
let package = Package(
	name: "AbsoluteTouch",
	platforms: [
		.iOS(.v17)
	],
	products: [
		.library(
			name: "AbsoluteTouch",
			targets: ["AbsoluteTouch"]
		)
	],
	dependencies: [
		.package(path: "AbsoluteTouchCore")
	],
	targets: [
		.target(
			name: "AbsoluteTouch",
			dependencies: [
				.product(name: "AbsoluteTouchCore", package: "AbsoluteTouchCore")
			],
			path: "App",
			// Xcode builds these through project.yml; xtool reads the icon and
			// resource paths from xtool.yml instead of the asset catalog.
			exclude: [
				"Assets.xcassets",
				"Info.plist",
				"Info-xtool.plist",
			],
			swiftSettings: [.swiftLanguageMode(.v6)]
		)
	]
)
