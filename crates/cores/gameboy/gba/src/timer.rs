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

    pub fn step(&mut self, cycles: u32, bus: &mut GbaBus) {
        let granularity = audio_timer_granularity();
        let mut remaining = cycles;
        while remaining != 0 {
            let chunk = remaining.min(granularity);
            remaining -= chunk;
            self.step_timers_for_cycles(chunk, bus);
            bus.mix_audio_for_cycles(chunk);
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
