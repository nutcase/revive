use crate::bus::{GbaBus, IRQ_TIMER0, IRQ_TIMER1, IRQ_TIMER2, IRQ_TIMER3};
use crate::state::{StateReader, StateWriter};
use std::sync::OnceLock;

const TIMER_COUNT: usize = 4;
const TIMER_CTRL_ENABLE: u16 = 1 << 7;
const TIMER_CTRL_IRQ: u16 = 1 << 6;
const TIMER_CTRL_CASCADE: u16 = 1 << 2;
const PRESCALERS: [u32; 4] = [1, 64, 256, 1024];
const TIMER_IRQ_MASKS: [u16; TIMER_COUNT] = [IRQ_TIMER0, IRQ_TIMER1, IRQ_TIMER2, IRQ_TIMER3];
const DEFAULT_AUDIO_TIMER_GRANULARITY: u32 = 1;
static AUDIO_TIMER_GRANULARITY: OnceLock<u32> = OnceLock::new();

#[derive(Debug, Default)]
pub struct GbaTimer {
    accumulators: [u32; TIMER_COUNT],
    enabled: [bool; TIMER_COUNT],
}

impl GbaTimer {
    pub fn reset(&mut self) {
        self.accumulators = [0; TIMER_COUNT];
        self.enabled = [false; TIMER_COUNT];
    }

    pub fn serialize_state(&self, w: &mut StateWriter) {
        for &acc in &self.accumulators {
            w.write_u32(acc);
        }
        for &en in &self.enabled {
            w.write_bool(en);
        }
    }

    pub fn deserialize_state(&mut self, r: &mut StateReader) -> Result<(), &'static str> {
        for acc in &mut self.accumulators {
            *acc = r.read_u32()?;
        }
        for en in &mut self.enabled {
            *en = r.read_bool()?;
        }
        Ok(())
    }

    pub(crate) fn cycles_until_event(&self, bus: &GbaBus) -> u32 {
        let mut next = u32::MAX;
        for channel in 0..TIMER_COUNT {
            let control = bus.timer_control(channel);
            if control & TIMER_CTRL_ENABLE == 0 {
                continue;
            }
            // First synchronize an enabled timer with its reload value.
            if !self.enabled[channel] {
                return 1;
            }
            if control & TIMER_CTRL_CASCADE != 0 {
                continue;
            }
            let prescaler = PRESCALERS[(control & 3) as usize];
            if self.accumulators[channel] >= prescaler {
                return 1;
            }
            let cycles = (0x1_0000 - u32::from(bus.timer_counter(channel))) * prescaler
                - self.accumulators[channel];
            next = next.min(cycles.max(1));
        }
        next
    }

    /// Keep the original ordering: advance audio for pre-overflow cycles,
    /// then latch FIFO/DMA at the overflow cycle, then mix that cycle's audio.
    fn step_exact(&mut self, mut cycles: u32, bus: &mut GbaBus) {
        while cycles != 0 {
            // Many active CPU instructions take one cycle. Avoid scheduling
            // overhead when there is nothing to batch.
            let chunk = if cycles == 1 {
                1
            } else {
                let timer_event = self.cycles_until_event(bus);
                let before_timer_event = timer_event.saturating_sub(1).max(1);
                cycles
                    .min(before_timer_event)
                    .min(bus.cycles_until_audio_sample())
            };
            self.step_timers_for_cycles(chunk, bus);
            bus.mix_audio_for_cycles(chunk);
            cycles -= chunk;
        }
    }

    pub(crate) fn step_halted(&mut self, cycles: u32, bus: &mut GbaBus) {
        self.step_exact(cycles, bus);
    }

    pub fn step(&mut self, cycles: u32, bus: &mut GbaBus) {
        let granularity = audio_timer_granularity();
        if granularity == 1 {
            self.step_exact(cycles, bus);
        } else {
            // Retain the opt-in experimental coarse audio mode.
            let mut remaining = cycles;
            while remaining != 0 {
                let chunk = remaining.min(granularity);
                remaining -= chunk;
                self.step_timers_for_cycles(chunk, bus);
                bus.mix_audio_for_cycles(chunk);
            }
        }
    }

    fn step_timers_for_cycles(&mut self, cycles: u32, bus: &mut GbaBus) {
        let mut previous_overflows = 0u32;

        for channel in 0..TIMER_COUNT {
            let control = bus.timer_control(channel);
            let reload = bus.timer_reload(channel);
            let is_enabled = (control & TIMER_CTRL_ENABLE) != 0;

            if !is_enabled {
                self.enabled[channel] = false;
                self.accumulators[channel] = 0;
                previous_overflows = 0;
                continue;
            }

            if !self.enabled[channel] {
                self.enabled[channel] = true;
                self.accumulators[channel] = 0;
                bus.set_timer_counter(channel, reload);
            }

            let uses_cascade = (control & TIMER_CTRL_CASCADE) != 0;
            let mut ticks = if uses_cascade {
                if channel == 0 { 0 } else { previous_overflows }
            } else {
                let prescaler = PRESCALERS[(control & 0x0003) as usize];
                self.accumulators[channel] = self.accumulators[channel].wrapping_add(cycles);
                let produced = self.accumulators[channel] / prescaler;
                self.accumulators[channel] %= prescaler;
                produced
            };

            let mut overflows = 0u32;
            while ticks != 0 {
                let current = bus.timer_counter(channel) as u32;
                let steps_until_overflow = 0x1_0000u32 - current;
                let step = ticks.min(steps_until_overflow);
                ticks -= step;

                let next = current + step;
                if next >= 0x1_0000 {
                    overflows += 1;
                    bus.set_timer_counter(channel, reload);
                } else {
                    bus.set_timer_counter(channel, next as u16);
                }
            }

            if overflows > 0 && (control & TIMER_CTRL_IRQ) != 0 {
                bus.request_irq(TIMER_IRQ_MASKS[channel]);
            }
            if overflows > 0 {
                bus.on_timer_overflow(channel, overflows);
            }

            previous_overflows = overflows;
        }
    }
}

fn audio_timer_granularity() -> u32 {
    *AUDIO_TIMER_GRANULARITY.get_or_init(|| {
        std::env::var("GBA_AUDIO_TIMER_GRANULARITY")
            .ok()
            .and_then(|raw| raw.trim().parse::<u32>().ok())
            .map(|value| value.clamp(1, 256))
            .unwrap_or(DEFAULT_AUDIO_TIMER_GRANULARITY)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::IRQ_TIMER0;

    fn audio_timer_fixture(prescaler: u16) -> (GbaTimer, GbaBus) {
        let mut bus = GbaBus::default();
        bus.reset();
        bus.write16(0x0400_0084, 0x0080); // master enable
        bus.write16(0x0400_0082, 0x730C); // A: timer0; B: timer1, stereo
        for i in 0..256u32 {
            bus.write32(0x0300_2000 + i * 4, i.wrapping_mul(0x17315977));
        }
        for (dma, fifo) in [(0xBC, 0xA0), (0xC8, 0xA4)] {
            bus.write32(0x0400_0000 + dma, 0x0300_2000);
            bus.write32(0x0400_0000 + dma + 4, 0x0400_0000 + fifo);
            bus.write16(0x0400_0000 + dma + 10, 0xB640);
        }
        bus.write16(0x0400_0100, 0xFFF9);
        bus.write16(0x0400_0102, 0xC0 | prescaler);
        bus.write16(0x0400_0104, 0xFFFD);
        bus.write16(0x0400_0106, 0xC4); // cascade with IRQ
        bus.write16(0x0400_0108, 0xFFFE);
        bus.write16(0x0400_010A, 0xC4);
        bus.write16(0x0400_010C, 0xFFFB);
        bus.write16(0x0400_010E, 0xC1);
        (GbaTimer::default(), bus)
    }

    fn state_bytes(timer: &GbaTimer, bus: &GbaBus) -> Vec<u8> {
        let mut writer = StateWriter::new();
        timer.serialize_state(&mut writer);
        bus.serialize_state(&mut writer);
        writer.into_vec()
    }

    #[test]
    fn event_batches_match_single_cycle_timers_fifo_dma_and_pcm() {
        for prescaler in 0..4 {
            let (mut fast, mut fast_bus) = audio_timer_fixture(prescaler);
            let (mut reference, mut reference_bus) = audio_timer_fixture(prescaler);
            for phase in 0..4 {
                for cycles in [1, 3, 17, 511, 1024, 4097] {
                    fast.step_exact(cycles, &mut fast_bus);
                    for _ in 0..cycles {
                        reference.step_timers_for_cycles(1, &mut reference_bus);
                        reference_bus.mix_audio_for_cycles(1);
                    }
                }
                assert_eq!(
                    fast_bus.take_audio_samples(),
                    reference_bus.take_audio_samples(),
                    "PCM prescaler={prescaler} phase={phase}"
                );
                let a = state_bytes(&fast, &fast_bus);
                let b = state_bytes(&reference, &reference_bus);
                assert_eq!(a.len(), b.len());
                assert_eq!(
                    a.iter().zip(&b).position(|(a, b)| a != b),
                    None,
                    "state prescaler={prescaler} phase={phase}"
                );
                // Reconfigure a running prescaler, disable/re-enable a timer,
                // and change sound sample cadence between instruction batches.
                for bus in [&mut fast_bus, &mut reference_bus] {
                    bus.write16(
                        0x0400_0102,
                        if phase == 1 {
                            0
                        } else {
                            0xC0 | ((prescaler + phase + 1) & 3)
                        },
                    );
                    bus.write16(0x0400_0088, 0x200 | ((phase & 3) << 14));
                }
            }
        }
    }

    #[test]
    #[ignore = "manual release benchmark"]
    fn benchmark_timer_event_batches() {
        use std::{hint::black_box, time::Instant};
        for cycles in [1, 4, 1000] {
            for batched in [false, true] {
                let (mut timer, mut bus) = audio_timer_fixture(1);
                let start = Instant::now();
                for _ in 0..(2_000_000 / cycles) {
                    if batched {
                        timer.step_exact(black_box(cycles), &mut bus);
                    } else {
                        for _ in 0..cycles {
                            timer.step_timers_for_cycles(1, &mut bus);
                            bus.mix_audio_for_cycles(1);
                        }
                    }
                }
                eprintln!(
                    "timer chunk={cycles} batched={batched}: {:?}",
                    start.elapsed()
                );
                black_box(bus.take_audio_samples());
            }
        }
    }

    #[test]
    fn timer0_overflow_sets_irq_flag_when_enabled() {
        let mut bus = GbaBus::default();
        let mut timer = GbaTimer::default();
        bus.reset();
        timer.reset();

        bus.write16(0x0400_0100, 0xFFFE); // TM0CNT_L (reload)
        bus.write16(0x0400_0102, 0x00C0); // start + IRQ + prescale=1

        timer.step(2, &mut bus);
        assert_ne!(bus.read16(0x0400_0202) & IRQ_TIMER0, 0);
        assert_eq!(bus.read16(0x0400_0100), 0xFFFE);
    }

    #[test]
    fn timer_prescaler_divides_cpu_cycles() {
        let mut bus = GbaBus::default();
        let mut timer = GbaTimer::default();
        bus.reset();
        timer.reset();

        bus.write16(0x0400_0100, 0);
        bus.write16(0x0400_0102, 0x0081); // start + prescale=64

        timer.step(63, &mut bus);
        assert_eq!(bus.read16(0x0400_0100), 0);

        timer.step(1, &mut bus);
        assert_eq!(bus.read16(0x0400_0100), 1);
    }

    #[test]
    fn timer1_can_cascade_from_timer0_overflow() {
        let mut bus = GbaBus::default();
        let mut timer = GbaTimer::default();
        bus.reset();
        timer.reset();

        bus.write16(0x0400_0100, 0xFFFF); // TM0 reload
        bus.write16(0x0400_0102, 0x0080); // TM0 start
        bus.write16(0x0400_0104, 0x0000); // TM1 reload
        bus.write16(0x0400_0106, 0x0084); // TM1 start + cascade

        timer.step(1, &mut bus);
        assert_eq!(bus.read16(0x0400_0104), 1);
    }

    #[test]
    fn timer_overflow_can_trigger_special_fifo_dma() {
        let mut bus = GbaBus::default();
        let mut timer = GbaTimer::default();
        bus.reset();
        timer.reset();

        bus.write32(0x0300_2000, 0xCCCC_0001);
        bus.write32(0x0300_2004, 0xCCCC_0002);
        bus.write32(0x0300_2008, 0xCCCC_0003);
        bus.write32(0x0300_200C, 0xCCCC_0004);

        bus.write32(0x0400_00BC, 0x0300_2000); // DMA1SAD
        bus.write32(0x0400_00C0, 0x0400_00A0); // DMA1DAD FIFO A
        bus.write16(0x0400_00C6, 0xB640); // enable + special + repeat + 32-bit + dest fixed

        bus.write16(0x0400_0100, 0xFFFF); // TM0 reload
        bus.write16(0x0400_0102, 0x0080); // TM0 start, prescale=1
        timer.step(1, &mut bus);

        assert_eq!(bus.read32(0x0400_00A0), 0xCCCC_0004);
        // DMA source register is write-only; IO latch retains CPU-written value.
        assert_eq!(bus.read32(0x0400_00BC), 0x0300_2000);
    }
}
