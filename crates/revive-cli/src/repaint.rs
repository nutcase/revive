use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// egui can request immediate or delayed repaints (caret, hover, animations).
/// Keep the earliest deadline; SDL input wakes the event wait independently.
#[derive(Clone, Default)]
pub(crate) struct RepaintSchedule(Arc<Mutex<Option<Instant>>>);

impl RepaintSchedule {
    pub(crate) fn request_after(&self, delay: Duration) {
        self.request_at(Instant::now().checked_add(delay));
    }

    fn request_at(&self, deadline: Option<Instant>) {
        let mut pending = self.0.lock().unwrap();
        if let Some(deadline) = deadline {
            *pending = Some(pending.map_or(deadline, |old| old.min(deadline)));
        }
    }

    pub(crate) fn due(&self) -> bool {
        self.0.lock().unwrap().is_some_and(|t| t <= Instant::now())
    }

    pub(crate) fn begin_frame(&self) {
        *self.0.lock().unwrap() = None;
    }

    pub(crate) fn wait_timeout(&self, minimized: bool) -> Duration {
        // Also bound responsiveness to requests from another thread while SDL
        // waits. Window/input events themselves return immediately.
        let maximum = Duration::from_millis(100);
        if minimized {
            return maximum;
        }
        self.0.lock().unwrap().map_or(maximum, |deadline| {
            deadline
                .saturating_duration_since(Instant::now())
                .min(maximum)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earliest_request_wins_and_render_consumes_it() {
        let schedule = RepaintSchedule::default();
        let now = Instant::now();
        schedule.request_at(now.checked_add(Duration::from_secs(10)));
        schedule.request_at(Some(now));
        schedule.request_at(None); // Duration::MAX must not overflow or cancel.
        schedule.request_at(now.checked_add(Duration::from_secs(20)));
        assert!(schedule.due());
        assert_eq!(schedule.wait_timeout(false), Duration::ZERO);
        assert_eq!(schedule.wait_timeout(true), Duration::from_millis(100));
        schedule.begin_frame();
        assert!(!schedule.due());
        assert_eq!(schedule.wait_timeout(false), Duration::from_millis(100));
    }
}
