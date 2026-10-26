//! Recording injector used by tests and dry runs.

use crate::inject::{InjectError, Injector};
use crate::mapping::MappedSample;

/// One recorded injector call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A sample was injected.
    Sample(MappedSample),
    /// Every pressed button was released.
    ReleaseAll,
}

/// An injector that records calls instead of touching the OS.
#[derive(Debug, Default)]
pub struct MockInjector {
    events: Vec<Event>,
    failures: usize,
}

impl MockInjector {
    /// Recorded events, oldest first.
    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Take the recorded events, leaving the recorder empty.
    #[must_use]
    pub fn take(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Forget recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Make the next `count` calls fail, to exercise error paths.
    pub fn fail_next(&mut self, count: usize) {
        self.failures = count;
    }

    fn record(&mut self, event: Event) -> Result<(), InjectError> {
        if self.failures > 0 {
            self.failures -= 1;
            return Err(InjectError::Failed("injected test failure".to_owned()));
        }
        self.events.push(event);
        Ok(())
    }
}

impl Injector for MockInjector {
    fn inject(&mut self, sample: &MappedSample) -> Result<(), InjectError> {
        self.record(Event::Sample(*sample))
    }

    fn release_all(&mut self) -> Result<(), InjectError> {
        self.record(Event::ReleaseAll)
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, MockInjector};
    use crate::inject::Injector;
    use crate::mapping::MappedSample;

    fn sample() -> MappedSample {
        MappedSample {
            x: 1,
            y: 2,
            pressure: 3,
            altitude: 4,
            azimuth: 5,
            twist: 6,
            contact: true,
            hover: false,
            eraser: false,
            barrel: false,
            finger: false,
        }
    }

    #[test]
    fn records_samples_and_release() {
        let mut injector = MockInjector::default();

        injector.inject(&sample()).expect("inject");
        injector.release_all().expect("release");

        assert_eq!(
            injector.events(),
            &[Event::Sample(sample()), Event::ReleaseAll]
        );
    }

    #[test]
    fn take_empties_the_recorder() {
        let mut injector = MockInjector::default();
        injector.inject(&sample()).expect("inject");

        assert_eq!(injector.take().len(), 1);
        assert!(injector.events().is_empty());
    }

    #[test]
    fn injected_failures_are_reported_once() {
        let mut injector = MockInjector::default();
        injector.fail_next(1);

        assert!(injector.inject(&sample()).is_err());
        assert!(injector.inject(&sample()).is_ok());
        assert_eq!(injector.events().len(), 1);
    }
}
