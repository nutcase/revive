use std::time::{Duration, Instant};

pub(crate) struct FrameClock {
    last_frame: Instant,
    frame_duration: Duration,
}

impl FrameClock {
    pub(crate) fn with_rate(hz: f64) -> Self {
        Self {
            last_frame: Instant::now(),
            frame_duration: Duration::from_secs_f64(1.0 / hz),
        }
    }

    pub(crate) fn wait(&mut self) {
        let target = self.last_frame + self.frame_duration;
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target - now);
            self.last_frame = target;
        } else {
            // Keep the cadence after a short overrun. Resetting on every late
            // frame permanently loses emulated time and starves the audio stream.
            // A long stall (pause/debugger) must not trigger an unbounded catch-up.
            self.last_frame = if now.duration_since(target) < self.frame_duration {
                target
            } else {
                now
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_overrun_preserves_cadence() {
        let period = Duration::from_secs(1);
        let mut clock = FrameClock {
            last_frame: Instant::now() - period - Duration::from_millis(50),
            frame_duration: period,
        };
        let expected = clock.last_frame + period;
        clock.wait();
        assert_eq!(clock.last_frame, expected);
    }

    #[test]
    fn long_stall_does_not_accumulate_catch_up_frames() {
        let before = Instant::now();
        let mut clock = FrameClock {
            last_frame: before - Duration::from_secs(5),
            frame_duration: Duration::from_millis(16),
        };
        clock.wait();
        assert!(clock.last_frame >= before);
    }
}
