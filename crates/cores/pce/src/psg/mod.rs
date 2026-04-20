mod audio;
mod channel;
mod tables;

pub(crate) use channel::PsgChannel;
pub(crate) use tables::*;

use tables::{PSG_STATUS_IRQ, phase_step_for_period};

#[derive(Clone, Copy, Default)]
struct TransientF64(f64);

impl bincode::Encode for TransientF64 {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        _encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        Ok(())
    }
}

impl<Context> bincode::Decode<Context> for TransientF64 {
    fn decode<D: bincode::de::Decoder>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(0.0))
    }
}

impl<'de, Context> bincode::BorrowDecode<'de, Context> for TransientF64 {
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        _decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        Ok(Self(0.0))
    }
}

impl core::ops::Deref for TransientF64 {
    type Target = f64;
    fn deref(&self) -> &f64 {
        &self.0
    }
}

impl core::ops::DerefMut for TransientF64 {
    fn deref_mut(&mut self) -> &mut f64 {
        &mut self.0
    }
}

#[derive(Clone, bincode::Encode, bincode::Decode)]
pub(crate) struct Psg {
    regs: [u8; PSG_REG_COUNT],
    select: u8,
    current_channel: usize,
    pub(crate) main_balance: u8,
    lfo_frequency: u8,
    lfo_control: u8,
    accumulator: u32,
    irq_pending: bool,
    pub(crate) channels: [PsgChannel; PSG_CHANNEL_COUNT],
    pub(crate) waveform_ram: [u8; PSG_CHANNEL_COUNT * PSG_WAVE_SIZE],
    post_filter_state: f64,
    dc_prev_input: TransientF64,
}

impl Psg {
    pub(crate) fn new() -> Self {
        Self {
            regs: [0; PSG_REG_COUNT],
            select: 0,
            current_channel: 0,
            main_balance: 0xFF,
            lfo_frequency: 0,
            lfo_control: 0,
            accumulator: 0,
            irq_pending: false,
            channels: [PsgChannel::default(); PSG_CHANNEL_COUNT],
            waveform_ram: [0; PSG_CHANNEL_COUNT * PSG_WAVE_SIZE],
            post_filter_state: 0.0,
            dc_prev_input: TransientF64(0.0),
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(crate) fn post_load_fixup(&mut self) {
        // Save states should preserve PSG register/channel state, but the
        // host-side DC blocker history is output pipeline state. Restoring
        // only part of that history causes audible garbage immediately after
        // load, so restart the filter cleanly.
        self.post_filter_state = 0.0;
        self.dc_prev_input = TransientF64(0.0);
    }

    pub(crate) fn write_address(&mut self, value: u8) {
        self.select = value;
    }

    pub(crate) fn write_data(&mut self, value: u8) {
        let index = self.select as usize;
        if index < PSG_REG_COUNT {
            self.regs[index] = value;
            self.write_register(index, value);
        }
        if index >= PSG_REG_COUNT {
            self.write_wave_ram(index - PSG_REG_COUNT, value);
        }
        self.select = self.select.wrapping_add(1);
    }

    pub(crate) fn read_address(&self) -> u8 {
        self.select
    }

    pub(crate) fn read_data(&mut self) -> u8 {
        let index = self.select as usize;
        let value = if index < PSG_REG_COUNT {
            self.regs[index]
        } else {
            let wave_index = index - PSG_REG_COUNT;
            self.waveform_ram[wave_index % self.waveform_ram.len()]
        };
        self.select = self.select.wrapping_add(1);
        value
    }

    pub(crate) fn write_direct(&mut self, index: usize, value: u8) {
        if index < PSG_REG_COUNT {
            self.regs[index] = value;
            self.write_register(index, value);
        } else {
            self.write_wave_ram(index - PSG_REG_COUNT, value);
        }
    }

    pub(crate) fn read_direct(&mut self, index: usize) -> u8 {
        if index < PSG_REG_COUNT {
            self.regs[index]
        } else {
            let wave_index = index - PSG_REG_COUNT;
            self.waveform_ram[wave_index % self.waveform_ram.len()]
        }
    }

    pub(crate) fn read_status(&mut self) -> u8 {
        let mut status = 0;
        if self.irq_pending {
            status |= PSG_STATUS_IRQ;
        }
        status
    }

    fn write_register(&mut self, index: usize, value: u8) {
        match index {
            PSG_REG_CH_SELECT => {
                self.current_channel = (value as usize) & 0x07;
                if self.current_channel >= PSG_CHANNEL_COUNT {
                    self.current_channel = PSG_CHANNEL_COUNT - 1;
                }
            }
            PSG_REG_MAIN_BALANCE => {
                self.main_balance = value;
            }
            PSG_REG_FREQ_LO => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                channel.frequency = (channel.frequency & 0x0F00) | value as u16;
                channel.phase_step = phase_step_for_period(channel.frequency);
            }
            PSG_REG_FREQ_HI => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                channel.frequency = (channel.frequency & 0x00FF) | (((value & 0x0F) as u16) << 8);
                channel.phase_step = phase_step_for_period(channel.frequency);
            }
            PSG_REG_CH_CONTROL => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                let previous = channel.control;
                channel.control = value;
                if previous & PSG_CH_CTRL_DDA != 0 && value & PSG_CH_CTRL_DDA == 0 {
                    // Hardware resets the waveform index when DDA is cleared.
                    channel.wave_write_pos = 0;
                    channel.wave_pos = 0;
                }
                if previous & PSG_CH_CTRL_KEY_ON == 0 && value & PSG_CH_CTRL_KEY_ON != 0 {
                    channel.phase = 0;
                    channel.wave_pos = channel.wave_write_pos;
                    channel.noise_phase = 0;
                    channel.noise_lfsr = 1;
                }
            }
            PSG_REG_CH_BALANCE => {
                self.channels[self.current_channel].balance = value;
            }
            PSG_REG_WAVE_DATA => {
                let ch = self.current_channel;
                let channel = &mut self.channels[ch];
                let sample = value & 0x1F;
                if channel.control & PSG_CH_CTRL_DDA != 0 {
                    channel.dda_sample = sample;
                }
                if channel.control & PSG_CH_CTRL_KEY_ON == 0 {
                    // Games commonly upload wave tables with KEY OFF and DDA toggled.
                    // Accept writes whenever KEY is off so both patterns work.
                    let write_pos = channel.wave_write_pos as usize & (PSG_WAVE_SIZE - 1);
                    let index = ch * PSG_WAVE_SIZE + write_pos;
                    self.waveform_ram[index] = sample;
                    channel.wave_write_pos = channel.wave_write_pos.wrapping_add(1) & 0x1F;
                }
            }
            PSG_REG_NOISE_CTRL => {
                if self.current_channel >= 4 {
                    self.channels[self.current_channel].noise_control = value;
                }
            }
            PSG_REG_LFO_FREQ => {
                self.lfo_frequency = value;
            }
            PSG_REG_LFO_CTRL => {
                if value & 0x80 != 0 {
                    let channel = &mut self.channels[1];
                    channel.wave_pos = 0;
                    channel.phase = 0;
                }
                self.lfo_control = value;
            }
            PSG_REG_TIMER_LO | PSG_REG_TIMER_HI => {
                self.accumulator = 0;
            }
            PSG_REG_TIMER_CTRL => {
                if value & PSG_CTRL_ENABLE == 0 {
                    self.irq_pending = false;
                }
            }
            _ => {}
        }
    }

    fn timer_period(&self) -> u16 {
        let lo = self.regs[PSG_REG_TIMER_LO] as u16;
        let hi = self.regs[PSG_REG_TIMER_HI] as u16;
        (hi << 8) | lo
    }

    fn enabled(&self) -> bool {
        let ctrl = self.regs[PSG_REG_TIMER_CTRL];
        self.timer_period() != 0 && (ctrl & PSG_CTRL_ENABLE != 0)
    }

    pub(crate) fn tick(&mut self, cycles: u32) -> bool {
        if !self.enabled() {
            return false;
        }
        if self.irq_pending {
            return false;
        }

        self.accumulator = self.accumulator.saturating_add(cycles);
        let period = self.timer_period() as u32;
        if period == 0 {
            return false;
        }
        if self.accumulator >= period {
            self.accumulator %= period.max(1);
            if self.regs[PSG_REG_TIMER_CTRL] & PSG_CTRL_IRQ_ENABLE != 0 {
                self.irq_pending = true;
                return true;
            }
        }
        false
    }

    pub(crate) fn acknowledge(&mut self) {
        self.irq_pending = false;
    }

    fn write_wave_ram(&mut self, addr: usize, value: u8) {
        let index = addr % self.waveform_ram.len();
        self.waveform_ram[index] = value & 0x1F;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_load_fixup_resets_output_filter_history() {
        let mut psg = Psg::new();
        psg.post_filter_state = 123.0;
        psg.dc_prev_input = TransientF64(45.0);

        psg.post_load_fixup();

        assert_eq!(psg.post_filter_state, 0.0);
        assert_eq!(*psg.dc_prev_input, 0.0);
    }
}
