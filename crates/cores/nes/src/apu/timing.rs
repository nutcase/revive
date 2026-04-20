use super::*;

impl Apu {
    pub fn step(&mut self) {
        self.cycle_count += 1;
        self.frame_counter += 1;

        // Triangle: clocked every CPU cycle (timer always runs)
        self.triangle.step();

        // Pulse and Noise: clocked every 2 CPU cycles (timers always run)
        if self.cycle_count & 1 == 0 {
            self.pulse1.step();
            self.pulse2.step();
            self.noise.step();
        }

        self.dmc.step();

        // Frame sequencer with proper 4-step/5-step timing
        // Values are in CPU cycles (APU cycle * 2, since step() is called per CPU cycle)
        // APU 3728.5 = CPU 7457, APU 7456.5 = CPU 14913, etc.
        if !self.frame_mode {
            // 4-step mode
            match self.frame_counter {
                7457 => self.clock_quarter_frame(),
                14913 => self.clock_half_frame(),
                22371 => self.clock_quarter_frame(),
                29829 => {
                    self.clock_half_frame();
                    if !self.irq_disable {
                        self.frame_irq = true;
                    }
                    self.frame_counter = 0;
                }
                _ => {}
            }
        } else {
            // 5-step mode (no IRQ)
            match self.frame_counter {
                7457 => self.clock_quarter_frame(),
                14913 => self.clock_half_frame(),
                22371 => self.clock_quarter_frame(),
                29829 => {} // nothing
                37281 => {
                    self.clock_half_frame();
                    self.frame_counter = 0;
                }
                _ => {}
            }
        }

        // Anti-aliasing: filter raw mixer output at CPU rate, then accumulate.
        let raw = self.raw_mix() + self.expansion_audio;
        let aa = self.aa_filter1.process(raw);
        let aa = self.aa_filter2.process(aa);
        self.sample_accumulator += aa;
        self.sample_accumulator_count += 1;

        // Fractional sample accumulator for accurate 44100 Hz sampling
        self.sample_counter += self.sample_rate;
        if self.sample_counter >= self.cpu_clock_rate {
            self.sample_counter -= self.cpu_clock_rate;
            let sample = self.produce_sample();
            // Push directly to ring buffer for jitter-free delivery,
            // fall back to Vec when no ring buffer is attached.
            if let Some(ref ring) = self.audio_ring {
                ring.push_one(sample);
            } else {
                self.output_buffer.push(sample);
            }
        }
    }

    /// Quarter frame: envelopes + triangle linear counter
    pub(super) fn clock_quarter_frame(&mut self) {
        self.pulse1.clock_envelope();
        self.pulse2.clock_envelope();
        self.triangle.clock_linear_counter();
        self.noise.clock_envelope();
    }

    /// Half frame: quarter frame + length counters + sweeps
    pub(super) fn clock_half_frame(&mut self) {
        self.clock_quarter_frame();
        self.pulse1.clock_length_counter();
        self.pulse1.clock_sweep();
        self.pulse2.clock_length_counter();
        self.pulse2.clock_sweep();
        self.triangle.clock_length_counter();
        self.noise.clock_length_counter();
    }
}
