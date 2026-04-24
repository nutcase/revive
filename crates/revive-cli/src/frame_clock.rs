use std::time::{Duration, Instant};

use revive_core::SystemKind;

pub(crate) struct FrameClock {
    last_frame: Instant,
    frame_duration: Duration,
}

impl FrameClock {
    pub(crate) fn new(system: SystemKind) -> Self {
        Self {
            last_frame: Instant::now(),
            frame_duration: Duration::from_secs_f64(1.0 / system.frame_rate_hz()),
        }
    }

    pub(crate) fn wait(&mut self) {
        let target = self.last_frame + self.frame_duration;
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target - now);
            self.last_frame = target;
        } else {
            self.last_frame = now;
        }
    }
}
