use super::super::{DmcChannel, DMC_RATE_TABLE};

impl DmcChannel {
    pub(in crate::apu) fn new() -> Self {
        DmcChannel {
            irq_enabled: false,
            irq_pending: false,
            loop_flag: false,
            timer: DMC_RATE_TABLE[0],
            timer_reload: DMC_RATE_TABLE[0],
            output_level: 0,
            sample_address: 0xC000,
            sample_length: 1,
            current_address: 0xC000,
            bytes_remaining: 0,
            sample_buffer: None,
            shift_register: 0,
            bits_remaining: 8,
            silence: true,
            dma_delay: 0,
            pending_dma_stall_cycles: 0,
        }
    }

    pub(in crate::apu) fn write_control(&mut self, data: u8) {
        self.irq_enabled = (data & 0x80) != 0;
        self.loop_flag = (data & 0x40) != 0;
        self.timer_reload = DMC_RATE_TABLE[(data & 0x0F) as usize];
        if !self.irq_enabled {
            self.irq_pending = false;
        }
    }

    pub(in crate::apu) fn write_direct_load(&mut self, data: u8) {
        self.output_level = data & 0x7F;
    }

    pub(in crate::apu) fn write_sample_address(&mut self, data: u8) {
        self.sample_address = 0xC000 | ((data as u16) << 6);
    }

    pub(in crate::apu) fn write_sample_length(&mut self, data: u8) {
        self.sample_length = ((data as u16) << 4) | 1;
    }

    pub(in crate::apu) fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.bytes_remaining = 0;
            self.dma_delay = 0;
            self.pending_dma_stall_cycles = 0;
            return;
        }

        if self.bytes_remaining == 0 {
            self.restart_sample();
            self.schedule_dma(2, 3);
        }
    }

    pub(in crate::apu) fn restart_sample(&mut self) {
        self.current_address = self.sample_address;
        self.bytes_remaining = self.sample_length;
    }

    pub(in crate::apu) fn schedule_dma(&mut self, delay: u8, stall_cycles: u8) {
        if self.sample_buffer.is_some()
            || self.bytes_remaining == 0
            || self.pending_dma_stall_cycles != 0
        {
            return;
        }

        self.dma_delay = delay;
        self.pending_dma_stall_cycles = stall_cycles;
    }

    pub(in crate::apu) fn pull_sample_request(&mut self) -> Option<(u16, u8)> {
        if self.pending_dma_stall_cycles == 0
            || self.sample_buffer.is_some()
            || self.bytes_remaining == 0
        {
            return None;
        }

        if self.dma_delay > 0 {
            self.dma_delay -= 1;
            return None;
        }

        let stall_cycles = self.pending_dma_stall_cycles;
        self.pending_dma_stall_cycles = 0;
        let addr = self.current_address;
        self.current_address = if self.current_address == 0xFFFF {
            0x8000
        } else {
            self.current_address + 1
        };

        self.bytes_remaining -= 1;
        if self.bytes_remaining == 0 {
            if self.loop_flag {
                self.restart_sample();
            } else if self.irq_enabled {
                self.irq_pending = true;
            }
        }

        Some((addr, stall_cycles))
    }

    pub(in crate::apu) fn push_sample(&mut self, data: u8) {
        self.sample_buffer = Some(data);
    }

    pub(in crate::apu) fn step(&mut self) {
        if self.timer == 0 {
            self.timer = self.timer_reload;
            self.clock_output();
        } else {
            self.timer -= 1;
        }
    }

    pub(in crate::apu) fn clock_output(&mut self) {
        if !self.silence {
            if (self.shift_register & 0x01) != 0 {
                if self.output_level <= 125 {
                    self.output_level += 2;
                }
            } else if self.output_level >= 2 {
                self.output_level -= 2;
            }
        }

        self.shift_register >>= 1;
        if self.bits_remaining > 0 {
            self.bits_remaining -= 1;
        }

        if self.bits_remaining == 0 {
            self.bits_remaining = 8;
            if let Some(sample) = self.sample_buffer.take() {
                self.shift_register = sample;
                self.silence = false;
                self.schedule_dma(0, 4);
            } else {
                self.silence = true;
            }
        }
    }

    pub(in crate::apu) fn output(&self) -> f32 {
        self.output_level as f32
    }
}
