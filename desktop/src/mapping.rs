//! Normalized coordinate mapping.
//!
//! The iPad sends positions in `0..=65535` over its active area. The host maps
//! them into a target rectangle in the same normalized space. Backends (such
//! as Linux `uinput`) consume that rectangle directly.

use serde::{Deserialize, Serialize};

/// A pointer state as received from the iPad.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag mirrors one distinct wire bit"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabletSample {
    /// X position in `0..=65535`.
    pub x: u16,
    /// Y position in `0..=65535`.
    pub y: u16,
    /// Pressure in `0..=65535`; meaningful only when the iPad supports it.
    pub pressure: u16,
    /// Tilt altitude in `0..=65535`.
    pub altitude: u16,
    /// Tilt azimuth in `0..=65535`.
    pub azimuth: u16,
    /// Twist in `0..=65535`; reserved on current hardware.
    pub twist: u16,
    /// The tip is touching the screen.
    pub contact: bool,
    /// The Pencil is in range but not touching.
    pub hover: bool,
    /// The eraser tool is selected.
    pub eraser: bool,
    /// The barrel button is pressed.
    pub barrel: bool,
    /// The sample came from a finger.
    pub finger: bool,
    /// The pointer left the active area.
    pub out_of_range: bool,
}

/// A sample mapped into the target desktop rectangle.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag mirrors one distinct wire bit"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedSample {
    /// X position in `0..=65535` within the target rectangle.
    pub x: u16,
    /// Y position in `0..=65535` within the target rectangle.
    pub y: u16,
    /// Pressure in `0..=65535`, after capability fallback.
    pub pressure: u16,
    /// Tilt altitude in `0..=65535`.
    pub altitude: u16,
    /// Tilt azimuth in `0..=65535`.
    pub azimuth: u16,
    /// Twist in `0..=65535`.
    pub twist: u16,
    /// The tip is touching the screen.
    pub contact: bool,
    /// The Pencil is in range but not touching.
    pub hover: bool,
    /// The eraser tool is selected.
    pub eraser: bool,
    /// The barrel button is pressed.
    pub barrel: bool,
    /// The sample came from a finger.
    pub finger: bool,
}

/// A rectangle in normalized `0..=65535` coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rect {
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Width; the right edge is `x + width`.
    pub width: u16,
    /// Height; the bottom edge is `y + height`.
    pub height: u16,
}

impl Rect {
    /// The full normalized desktop.
    pub const FULL: Self = Self {
        x: 0,
        y: 0,
        width: u16::MAX,
        height: u16::MAX,
    };

    /// Return the right edge, `x + width`.
    ///
    /// Mapping is closed: a normalized `u16::MAX` on the X axis maps to this
    /// value.
    #[must_use]
    pub const fn right(self) -> u32 {
        self.x as u32 + self.width as u32
    }

    /// Return the bottom edge, `y + height`.
    ///
    /// Mapping is closed: a normalized `u16::MAX` on the Y axis maps to this
    /// value.
    #[must_use]
    pub const fn bottom(self) -> u32 {
        self.y as u32 + self.height as u32
    }

    /// Whether the rectangle fits inside `0..=65535` on both axes.
    #[must_use]
    pub const fn is_inside_full_range(self) -> bool {
        self.right() <= u16::MAX as u32 && self.bottom() <= u16::MAX as u32
    }

    /// Whether the rectangle has a usable size.
    #[must_use]
    pub const fn is_nonempty(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self::FULL
    }
}

/// How the active area is fitted into the target rectangle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AspectMode {
    /// Fill the target rectangle, ignoring the aspect ratio.
    Stretch,
    /// Fit inside the target rectangle and center the result.
    #[default]
    Preserve,
}

/// Maps device coordinates into the configured target rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    target: Rect,
    aspect: AspectMode,
    source_aspect: Option<f64>,
    target_aspect: Option<f64>,
}

impl Mapping {
    /// Create a mapping.
    ///
    /// Aspect preservation needs both aspect ratios; when either is missing
    /// the mapping stretches to the target rectangle.
    #[must_use]
    pub const fn new(
        target: Rect,
        aspect: AspectMode,
        source_aspect: Option<f64>,
        target_aspect: Option<f64>,
    ) -> Self {
        Self {
            target,
            aspect,
            source_aspect,
            target_aspect,
        }
    }

    /// A mapping that stretches across the full desktop.
    #[must_use]
    pub fn full() -> Self {
        Self::new(Rect::FULL, AspectMode::Stretch, None, None)
    }

    /// The configured target rectangle, before aspect fitting.
    #[must_use]
    pub const fn target(&self) -> Rect {
        self.target
    }

    /// The rectangle actually used, after aspect fitting and centering.
    #[must_use]
    pub fn content_rect(&self) -> Rect {
        let (AspectMode::Preserve, Some(source), Some(target)) =
            (self.aspect, self.source_aspect, self.target_aspect)
        else {
            return self.target;
        };

        if !source.is_finite() || !target.is_finite() || source <= 0.0 || target <= 0.0 {
            return self.target;
        }

        let (width, height) = if source >= target {
            (
                f64::from(self.target.width),
                f64::from(self.target.width) * target / source,
            )
        } else {
            (
                f64::from(self.target.height) * source / target,
                f64::from(self.target.height),
            )
        };

        let width = width.round().clamp(1.0, f64::from(self.target.width)) as u16;
        let height = height.round().clamp(1.0, f64::from(self.target.height)) as u16;

        Rect {
            x: self.target.x + (self.target.width - width) / 2,
            y: self.target.y + (self.target.height - height) / 2,
            width,
            height,
        }
    }

    /// Map one sample into the target rectangle.
    #[must_use]
    pub fn map(&self, sample: &TabletSample) -> MappedSample {
        let rect = self.content_rect();

        MappedSample {
            x: map_axis(sample.x, rect.x, rect.width),
            y: map_axis(sample.y, rect.y, rect.height),
            pressure: sample.pressure,
            altitude: sample.altitude,
            azimuth: sample.azimuth,
            twist: sample.twist,
            contact: sample.contact,
            hover: sample.hover,
            eraser: sample.eraser,
            barrel: sample.barrel,
            finger: sample.finger,
        }
    }
}

impl Default for Mapping {
    fn default() -> Self {
        Self::full()
    }
}

fn map_axis(value: u16, offset: u16, span: u16) -> u16 {
    let scaled =
        (u64::from(value) * u64::from(span) + u64::from(u16::MAX) / 2) / u64::from(u16::MAX);
    (u64::from(offset) + scaled.min(u64::from(span))) as u16
}

#[cfg(test)]
mod tests {
    use super::{AspectMode, MappedSample, Mapping, Rect, TabletSample};

    fn sample(x: u16, y: u16) -> TabletSample {
        TabletSample {
            x,
            y,
            pressure: 1_234,
            altitude: 5_678,
            azimuth: 9_012,
            twist: 0,
            contact: true,
            hover: false,
            eraser: false,
            barrel: false,
            finger: false,
            out_of_range: false,
        }
    }

    #[test]
    fn stretch_maps_the_full_range() {
        let mapping = Mapping::full();

        assert_eq!(mapping.map(&sample(0, 0)).x, 0);
        assert_eq!(mapping.map(&sample(u16::MAX, u16::MAX)).x, u16::MAX);
        assert_eq!(mapping.map(&sample(u16::MAX, u16::MAX)).y, u16::MAX);
    }

    #[test]
    fn stretch_keeps_the_sample_fields() {
        let mapped: MappedSample = Mapping::full().map(&sample(1, 2));

        assert_eq!(mapped.pressure, 1_234);
        assert_eq!(mapped.altitude, 5_678);
        assert_eq!(mapped.azimuth, 9_012);
        assert!(mapped.contact);
        assert!(!mapped.hover);
    }

    #[test]
    fn target_rectangle_offsets_and_scales() {
        let mapping = Mapping::new(
            Rect {
                x: 1_000,
                y: 2_000,
                width: 10_000,
                height: 20_000,
            },
            AspectMode::Stretch,
            None,
            None,
        );

        assert_eq!(mapping.map(&sample(0, 0)).x, 1_000);
        assert_eq!(mapping.map(&sample(0, 0)).y, 2_000);
        assert_eq!(mapping.map(&sample(u16::MAX, u16::MAX)).x, 11_000);
        assert_eq!(mapping.map(&sample(u16::MAX, u16::MAX)).y, 22_000);
    }

    #[test]
    fn preserve_letterboxes_a_square_source_into_a_wide_target() {
        let mapping = Mapping::new(Rect::FULL, AspectMode::Preserve, Some(1.0), Some(2.0));
        let rect = mapping.content_rect();

        assert_eq!(rect.height, u16::MAX);
        assert_eq!(rect.width, 32_768);
        assert_eq!(rect.x, (u16::MAX - 32_768) / 2);
        assert_eq!(mapping.map(&sample(u16::MAX, 0)).x, rect.x + rect.width);
    }

    #[test]
    fn preserve_pillarboxes_a_wide_source_into_a_tall_target() {
        let mapping = Mapping::new(Rect::FULL, AspectMode::Preserve, Some(2.0), Some(1.0));
        let rect = mapping.content_rect();

        assert_eq!(rect.width, u16::MAX);
        assert_eq!(rect.height, 32_768);
        assert_eq!(rect.y, (u16::MAX - 32_768) / 2);
    }

    #[test]
    fn preserve_without_aspects_stretches() {
        let mapping = Mapping::new(Rect::FULL, AspectMode::Preserve, Some(4.0 / 3.0), None);

        assert_eq!(mapping.content_rect(), Rect::FULL);
    }

    #[test]
    fn preserve_ignores_invalid_aspects() {
        let mapping = Mapping::new(Rect::FULL, AspectMode::Preserve, Some(0.0), Some(1.0));

        assert_eq!(mapping.content_rect(), Rect::FULL);
    }

    #[test]
    fn rect_bounds_are_checked() {
        assert!(Rect::FULL.is_inside_full_range());
        assert!(
            !Rect {
                x: 60_000,
                y: 0,
                width: 10_000,
                height: 1,
            }
            .is_inside_full_range()
        );
        assert!(
            !Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 1,
            }
            .is_nonempty()
        );
    }
}
