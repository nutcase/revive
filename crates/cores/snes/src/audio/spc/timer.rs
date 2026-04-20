pub struct Timer {
    resolution: i32,
    is_running: bool,
    ticks: i32,
    // Target value written via $FA-$FC. Note: value 0 means 256 ticks.
    target: u8,
    counter_low: u8,
    counter_high: u8,
    // Set when counter_high increments; used to wake SPC700 from SLEEP.
    pub fired: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TimerState {
    pub resolution: i32,
    pub is_running: bool,
    pub ticks: i32,
    pub target: u8,
    pub counter_low: u8,
    pub counter_high: u8,
}

impl Timer {
    pub fn new(resolution: i32) -> Timer {
        Timer {
            resolution: resolution,
            is_running: false,
            ticks: 0,
            target: 0,
            counter_low: 0,
            counter_high: 0,
            fired: false,
        }
    }

    pub fn reset(&mut self) {
        self.is_running = false;
        self.ticks = 0;
        self.target = 0;
        self.counter_low = 0;
        self.counter_high = 0;
        self.fired = false;
    }

    pub fn get_state(&self) -> TimerState {
        TimerState {
            resolution: self.resolution,
            is_running: self.is_running,
            ticks: self.ticks,
            target: self.target,
            counter_low: self.counter_low,
            counter_high: self.counter_high,
        }
    }

    pub fn set_state(&mut self, state: &TimerState) {
        self.resolution = state.resolution;
        self.is_running = state.is_running;
        self.ticks = state.ticks;
        self.target = state.target;
        self.counter_low = state.counter_low;
        self.counter_high = state.counter_high;
        self.fired = false;
    }

    pub fn cpu_cycles_callback(&mut self, num_cycles: i32) {
        if !self.is_running {
            return;
        }
        self.ticks += num_cycles;
        // Timers tick when the internal divider reaches the configured resolution.
        // Using `>=` avoids an off-by-one that would slow timers (e.g., 33 instead of 32 cycles).
        while self.ticks >= self.resolution {
            self.ticks -= self.resolution;

            self.counter_low = self.counter_low.wrapping_add(1);
            // Stage 2: post-increment compare against target. A target value of 0
            // corresponds to 256 ticks, which naturally matches on wrap to 0.
            if self.counter_low == self.target {
                self.counter_high = self.counter_high.wrapping_add(1);
                self.counter_low = 0;
                self.fired = true;
            }
        }
    }

    pub fn set_start_stop_bit(&mut self, value: bool) {
        // A transition from clear to set (0 -> 1) resets the timer.
        if value && !self.is_running {
            self.ticks = 0;
            self.counter_low = 0;
            self.counter_high = 0;
        }
        self.is_running = value;
    }

    pub fn set_target(&mut self, value: u8) {
        self.target = value;
    }

    pub fn debug_state(&self) -> (i32, bool, u8, u8, u8, i32) {
        (
            self.ticks,
            self.is_running,
            self.target,
            self.counter_low,
            self.counter_high,
            self.resolution,
        )
    }

    pub fn read_counter(&mut self) -> u8 {
        let ret = self.counter_high & 0x0f;
        self.counter_high = 0;
        ret
    }
}
