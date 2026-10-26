//! Linux `uinput` absolute tablet injector.
//!
//! The device is created when the injector is built and removed when the
//! process exits. Samples are already mapped to `0..=65535` and tilt arrives
//! in degrees, so this module only turns them into kernel events. `evdev`
//! appends `SYN_REPORT` to every emitted batch.

use std::io;

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    UinputAbsSetup,
};

use crate::inject::{InjectError, Injector};
use crate::mapping::{MappedSample, Tilt};

/// Name shown by `libinput list-devices`, `evtest`, and desktop settings.
const DEVICE_NAME: &str = "AbsoluteTouch Tablet";
/// Vendor ID from the `0x1209` community range.
const VENDOR_ID: u16 = 0x1209;
/// Product ID for the virtual tablet.
const PRODUCT_ID: u16 = 0x0001;
/// Device version.
const VERSION: u16 = 0x0001;

/// Largest value for a normalized axis.
const AXIS_MAX: i32 = u16::MAX as i32;
/// Smallest tilt in degrees.
const TILT_MIN: i32 = -90;
/// Largest tilt in degrees.
const TILT_MAX: i32 = 90;

/// Linux absolute tablet backed by `/dev/uinput`.
#[derive(Debug)]
pub struct UinputInjector {
    device: VirtualDevice,
    state: State,
}

impl UinputInjector {
    /// Create and register the virtual tablet.
    ///
    /// # Errors
    ///
    /// Returns [`InjectError::Unavailable`] when `/dev/uinput` cannot be
    /// opened, for example because the node is missing or the user lacks
    /// permission. Other setup failures return [`InjectError::Failed`].
    pub fn new() -> Result<Self, InjectError> {
        let device = build_device().map_err(|error| match error.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
                InjectError::Unavailable(format!(
                    "cannot create a uinput tablet: {error}; check that /dev/uinput exists and \
                     that this user may write to it"
                ))
            }
            _ => InjectError::Failed(format!("cannot create a uinput tablet: {error}")),
        })?;

        Ok(Self {
            device,
            state: State::default(),
        })
    }

    /// The first kernel event node created for this device.
    #[cfg(test)]
    fn device_node(&mut self) -> io::Result<std::path::PathBuf> {
        self.device
            .enumerate_dev_nodes_blocking()?
            .next()
            .transpose()?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no event node"))
    }
}

impl Injector for UinputInjector {
    fn inject(&mut self, sample: &MappedSample) -> Result<(), InjectError> {
        let events = self.state.sample_events(sample);
        self.device
            .emit(&events)
            .map_err(|error| write_error(&error))
    }

    fn release_all(&mut self) -> Result<(), InjectError> {
        let events = self.state.release_events();
        self.device
            .emit(&events)
            .map_err(|error| write_error(&error))
    }
}

/// One stylus or finger tool.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Tool {
    /// No tool is in range.
    #[default]
    OutOfRange,
    /// The pencil tip.
    Pen,
    /// The pencil eraser.
    Eraser,
    /// A finger.
    Finger,
}

/// Keys and tool state carried from one sample to the next.
#[derive(Debug, Clone, Copy, Default)]
struct State {
    tool: Tool,
    touch: bool,
    barrel: bool,
}

impl State {
    /// Build the key and axis changes for one sample.
    fn sample_events(&mut self, sample: &MappedSample) -> Vec<InputEvent> {
        let tool = if sample.finger {
            Tool::Finger
        } else if sample.eraser {
            Tool::Eraser
        } else {
            Tool::Pen
        };

        let mut events = Vec::with_capacity(10);
        if self.tool != tool {
            events.push(key(KeyCode::BTN_TOOL_PEN, tool == Tool::Pen));
            events.push(key(KeyCode::BTN_TOOL_RUBBER, tool == Tool::Eraser));
            events.push(key(KeyCode::BTN_TOOL_FINGER, tool == Tool::Finger));
            self.tool = tool;
        }

        if self.touch != sample.contact {
            events.push(key(KeyCode::BTN_TOUCH, sample.contact));
            self.touch = sample.contact;
        }

        let barrel = sample.barrel && !sample.finger;
        if self.barrel != barrel {
            events.push(key(KeyCode::BTN_STYLUS, barrel));
            self.barrel = barrel;
        }

        events.push(axis(AbsoluteAxisCode::ABS_X, i32::from(sample.x)));
        events.push(axis(AbsoluteAxisCode::ABS_Y, i32::from(sample.y)));
        events.push(axis(
            AbsoluteAxisCode::ABS_PRESSURE,
            if sample.contact {
                i32::from(sample.pressure)
            } else {
                0
            },
        ));

        let tilt = sample.tilt.unwrap_or(Tilt { x: 0, y: 0 });
        events.push(axis(AbsoluteAxisCode::ABS_TILT_X, i32::from(tilt.x)));
        events.push(axis(AbsoluteAxisCode::ABS_TILT_Y, i32::from(tilt.y)));

        events
    }

    /// Build a full release, independent of the tracked state.
    fn release_events(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::with_capacity(8);
        for code in [
            KeyCode::BTN_TOOL_PEN,
            KeyCode::BTN_TOOL_RUBBER,
            KeyCode::BTN_TOOL_FINGER,
            KeyCode::BTN_TOUCH,
            KeyCode::BTN_STYLUS,
        ] {
            events.push(key(code, false));
        }
        events.push(axis(AbsoluteAxisCode::ABS_PRESSURE, 0));
        events.push(axis(AbsoluteAxisCode::ABS_TILT_X, 0));
        events.push(axis(AbsoluteAxisCode::ABS_TILT_Y, 0));

        *self = Self::default();
        events
    }
}

fn key(code: KeyCode, pressed: bool) -> InputEvent {
    InputEvent::new(EventType::KEY.0, code.0, i32::from(pressed))
}

fn axis(code: AbsoluteAxisCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::ABSOLUTE.0, code.0, value)
}

fn write_error(error: &io::Error) -> InjectError {
    InjectError::Failed(format!("uinput write failed: {error}"))
}

fn build_device() -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for code in [
        KeyCode::BTN_TOOL_PEN,
        KeyCode::BTN_TOOL_RUBBER,
        KeyCode::BTN_TOOL_FINGER,
        KeyCode::BTN_TOUCH,
        KeyCode::BTN_STYLUS,
    ] {
        keys.insert(code);
    }

    let mut builder = VirtualDevice::builder()?
        .name(DEVICE_NAME)
        .input_id(InputId::new(
            BusType::BUS_VIRTUAL,
            VENDOR_ID,
            PRODUCT_ID,
            VERSION,
        ))
        .with_keys(&keys)?;

    for axis in axes() {
        builder = builder.with_absolute_axis(&axis)?;
    }

    builder.build()
}

fn axes() -> [UinputAbsSetup; 5] {
    [
        abs_setup(AbsoluteAxisCode::ABS_X, 0, AXIS_MAX),
        abs_setup(AbsoluteAxisCode::ABS_Y, 0, AXIS_MAX),
        abs_setup(AbsoluteAxisCode::ABS_PRESSURE, 0, AXIS_MAX),
        abs_setup(AbsoluteAxisCode::ABS_TILT_X, TILT_MIN, TILT_MAX),
        abs_setup(AbsoluteAxisCode::ABS_TILT_Y, TILT_MIN, TILT_MAX),
    ]
}

fn abs_setup(code: AbsoluteAxisCode, minimum: i32, maximum: i32) -> UinputAbsSetup {
    UinputAbsSetup::new(code, AbsInfo::new(0, minimum, maximum, 0, 0, 0))
}

#[cfg(test)]
mod tests {
    use super::{State, Tool, UinputInjector, axis, key};
    use crate::inject::InjectError;
    use crate::inject::Injector;
    use crate::mapping::{MappedSample, Tilt};
    use evdev::{AbsoluteAxisCode, EventType, InputEvent, KeyCode, SynchronizationCode};
    use std::time::Duration;

    fn sample() -> MappedSample {
        MappedSample {
            x: 1_000,
            y: 2_000,
            pressure: 30_000,
            tilt: Some(Tilt { x: 12, y: -34 }),
            twist: 0,
            contact: true,
            hover: false,
            eraser: false,
            barrel: false,
            finger: false,
        }
    }

    fn key_event(code: KeyCode, pressed: bool) -> InputEvent {
        key(code, pressed)
    }

    fn axis_event(code: AbsoluteAxisCode, value: i32) -> InputEvent {
        axis(code, value)
    }

    #[test]
    fn a_contact_frame_selects_the_pen_and_reports_axes() {
        let mut state = State::default();
        let events = state.sample_events(&sample());

        assert_eq!(
            events,
            vec![
                key_event(KeyCode::BTN_TOOL_PEN, true),
                key_event(KeyCode::BTN_TOOL_RUBBER, false),
                key_event(KeyCode::BTN_TOOL_FINGER, false),
                key_event(KeyCode::BTN_TOUCH, true),
                axis_event(AbsoluteAxisCode::ABS_X, 1_000),
                axis_event(AbsoluteAxisCode::ABS_Y, 2_000),
                axis_event(AbsoluteAxisCode::ABS_PRESSURE, 30_000),
                axis_event(AbsoluteAxisCode::ABS_TILT_X, 12),
                axis_event(AbsoluteAxisCode::ABS_TILT_Y, -34),
            ]
        );
        assert_eq!(state.tool, Tool::Pen);
        assert!(state.touch);
    }

    #[test]
    fn a_hover_frame_lifts_contact_but_keeps_the_tool() {
        let mut state = State::default();
        state.sample_events(&sample());

        let mut hover = sample();
        hover.contact = false;
        hover.hover = true;
        hover.pressure = 12_345;
        let events = state.sample_events(&hover);

        assert_eq!(
            events,
            vec![
                key_event(KeyCode::BTN_TOUCH, false),
                axis_event(AbsoluteAxisCode::ABS_X, 1_000),
                axis_event(AbsoluteAxisCode::ABS_Y, 2_000),
                axis_event(AbsoluteAxisCode::ABS_PRESSURE, 0),
                axis_event(AbsoluteAxisCode::ABS_TILT_X, 12),
                axis_event(AbsoluteAxisCode::ABS_TILT_Y, -34),
            ]
        );
    }

    #[test]
    fn switching_to_the_eraser_releases_the_pen_tool() {
        let mut state = State::default();
        state.sample_events(&sample());

        let mut eraser = sample();
        eraser.eraser = true;
        let events = state.sample_events(&eraser);

        assert_eq!(
            events[..3],
            [
                key_event(KeyCode::BTN_TOOL_PEN, false),
                key_event(KeyCode::BTN_TOOL_RUBBER, true),
                key_event(KeyCode::BTN_TOOL_FINGER, false),
            ]
        );
        assert_eq!(state.tool, Tool::Eraser);
    }

    #[test]
    fn a_finger_is_a_finger_and_cannot_press_the_barrel() {
        let mut state = State::default();
        let mut finger = sample();
        finger.finger = true;
        finger.barrel = true;
        finger.tilt = None;

        let events = state.sample_events(&finger);

        assert_eq!(
            events[..4],
            [
                key_event(KeyCode::BTN_TOOL_PEN, false),
                key_event(KeyCode::BTN_TOOL_RUBBER, false),
                key_event(KeyCode::BTN_TOOL_FINGER, true),
                key_event(KeyCode::BTN_TOUCH, true),
            ]
        );
        assert!(!events.contains(&key_event(KeyCode::BTN_STYLUS, true)));
        assert_eq!(state.tool, Tool::Finger);
        assert!(!state.barrel);
        assert_eq!(
            &events[events.len() - 5..],
            [
                axis_event(AbsoluteAxisCode::ABS_X, 1_000),
                axis_event(AbsoluteAxisCode::ABS_Y, 2_000),
                axis_event(AbsoluteAxisCode::ABS_PRESSURE, 30_000),
                axis_event(AbsoluteAxisCode::ABS_TILT_X, 0),
                axis_event(AbsoluteAxisCode::ABS_TILT_Y, 0),
            ]
        );
    }

    #[test]
    fn barrel_state_is_sent_once_per_change() {
        let mut state = State::default();
        let mut barrel = sample();
        barrel.barrel = true;
        let first = state.sample_events(&barrel);
        let second = state.sample_events(&barrel);

        assert!(first.contains(&key_event(KeyCode::BTN_STYLUS, true)));
        assert!(!second.contains(&key_event(KeyCode::BTN_STYLUS, true)));
        assert!(state.barrel);
    }

    #[test]
    fn release_clears_every_button_and_pressure_without_moving() {
        let mut state = State::default();
        state.sample_events(&sample());
        let events = state.release_events();

        assert_eq!(
            events,
            vec![
                key_event(KeyCode::BTN_TOOL_PEN, false),
                key_event(KeyCode::BTN_TOOL_RUBBER, false),
                key_event(KeyCode::BTN_TOOL_FINGER, false),
                key_event(KeyCode::BTN_TOUCH, false),
                key_event(KeyCode::BTN_STYLUS, false),
                axis_event(AbsoluteAxisCode::ABS_PRESSURE, 0),
                axis_event(AbsoluteAxisCode::ABS_TILT_X, 0),
                axis_event(AbsoluteAxisCode::ABS_TILT_Y, 0),
            ]
        );
        assert_eq!(state.tool, Tool::OutOfRange);
        assert!(!state.touch);
        assert!(!state.barrel);
    }

    #[test]
    fn release_is_idempotent() {
        let mut state = State::default();
        let first = state.release_events();
        let second = state.release_events();

        assert_eq!(first, second);
    }

    #[test]
    fn a_kernel_device_is_created_when_uinput_is_available() {
        let mut injector = match UinputInjector::new() {
            Ok(injector) => injector,
            Err(InjectError::Unavailable(reason)) => {
                eprintln!("skipping uinput device test: {reason}");
                return;
            }
            Err(error) => panic!("uinput setup failed: {error}"),
        };

        let node = injector.device_node().expect("event node");
        let mut reader = match evdev::Device::open(&node) {
            Ok(reader) => reader,
            Err(error) => {
                eprintln!("skipping readback: cannot open {}: {error}", node.display());
                return;
            }
        };
        reader.set_nonblocking(true).expect("nonblocking");

        assert_eq!(reader.name(), Some(super::DEVICE_NAME));
        let axes = reader.supported_absolute_axes().expect("absolute axes");
        for axis in [
            AbsoluteAxisCode::ABS_X,
            AbsoluteAxisCode::ABS_Y,
            AbsoluteAxisCode::ABS_PRESSURE,
            AbsoluteAxisCode::ABS_TILT_X,
            AbsoluteAxisCode::ABS_TILT_Y,
        ] {
            assert!(axes.contains(axis), "missing {axis:?}");
        }

        injector.inject(&sample()).expect("inject");
        injector.release_all().expect("release");

        let mut events = Vec::new();
        for _ in 0..50 {
            match reader.fetch_events() {
                Ok(batch) => {
                    events.extend(batch);
                    if events
                        .iter()
                        .any(|event| event.event_type() == EventType::SYNCHRONIZATION)
                    {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("readback failed: {error}"),
            }
        }

        assert!(
            events
                .iter()
                .any(|event| event.event_type() == EventType::ABSOLUTE
                    && event.code() == AbsoluteAxisCode::ABS_X.0
                    && event.value() == i32::from(sample().x)),
            "no ABS_X event in {events:?}"
        );
        assert!(
            events.iter().any(|event| {
                event.event_type() == EventType::SYNCHRONIZATION
                    && event.code() == SynchronizationCode::SYN_REPORT.0
            }),
            "no SYN_REPORT in {events:?}"
        );
    }
}
