import AbsoluteTouchCore

/// `UserDefaults` keys for the drawing area.
///
/// The home screen owns the `@AppStorage` properties; the editor receives a
/// binding, so these names appear in exactly one place.
enum ActiveAreaStorage {
	static let x = "absoluteTouch.area.x"
	static let y = "absoluteTouch.area.y"
	static let width = "absoluteTouch.area.width"
	static let height = "absoluteTouch.area.height"
	static let isLocked = "absoluteTouch.area.locked"
	static let aspectWidth = "absoluteTouch.area.aspectWidth"
	static let aspectHeight = "absoluteTouch.area.aspectHeight"
}
