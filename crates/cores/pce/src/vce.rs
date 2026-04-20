#[derive(Clone, bincode::Encode, bincode::Decode)]
pub(crate) struct Vce {
    pub(crate) palette: [u16; 0x200],
    control: u16,
    address: u16,
    data_latch: u16,
    write_phase: VcePhase,
    read_phase: VcePhase,
}

#[derive(Clone, Copy, PartialEq, Eq, bincode::Encode, bincode::Decode)]
enum VcePhase {
    Low,
    High,
}

impl Vce {
    pub(crate) fn new() -> Self {
        Self {
            palette: [0; 0x200],
            control: 0,
            address: 0,
            data_latch: 0,
            write_phase: VcePhase::Low,
            read_phase: VcePhase::Low,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.palette.fill(0);
        self.control = 0;
        self.address = 0;
        self.data_latch = 0;
        self.write_phase = VcePhase::Low;
        self.read_phase = VcePhase::Low;
    }

    fn index(&self) -> usize {
        (self.address as usize) & 0x01FF
    }

    /// Current palette address index (9-bit masked).
    pub(crate) fn address_index(&self) -> usize {
        self.index()
    }

    /// Set the palette address (for CRAM DMA).
    pub(crate) fn set_address(&mut self, addr: u16) {
        self.address = addr;
    }

    pub(crate) fn write_control_low(&mut self, value: u8) {
        self.control = (self.control & 0xFF00) | (value as u16 & 0x0087);
        self.read_phase = VcePhase::Low;
        self.write_phase = VcePhase::Low;
        #[cfg(feature = "trace_hw_writes")]
        eprintln!("  VCE control low <= {:02X}", value);
    }

    pub(crate) fn write_control_high(&mut self, _value: u8) {
        self.read_phase = VcePhase::Low;
        self.write_phase = VcePhase::Low;
        #[cfg(feature = "trace_hw_writes")]
        eprintln!("  VCE control high <= {:02X}", _value);
    }

    pub(crate) fn read_control_low(&self) -> u8 {
        0xFF
    }

    pub(crate) fn read_control_high(&self) -> u8 {
        0xFF
    }

    pub(crate) fn write_address_low(&mut self, value: u8) {
        self.address = (self.address & 0x0100) | value as u16;
        self.read_phase = VcePhase::Low;
        self.write_phase = VcePhase::Low;
        #[cfg(feature = "trace_hw_writes")]
        eprintln!("  VCE address low <= {:02X}", value);
    }

    pub(crate) fn write_address_high(&mut self, value: u8) {
        self.address = (self.address & 0x00FF) | (((value as u16) & 0x01) << 8);
        self.read_phase = VcePhase::Low;
        self.write_phase = VcePhase::Low;
        #[cfg(feature = "trace_hw_writes")]
        eprintln!("  VCE address high <= {:02X}", value);
    }

    pub(crate) fn read_address_low(&self) -> u8 {
        0xFF
    }

    pub(crate) fn read_address_high(&self) -> u8 {
        0xFF
    }

    pub(crate) fn write_data_low(&mut self, value: u8) {
        // Per MAME huc6260.cpp: writing the low port directly modifies the
        // low byte of the current palette entry, preserving the high byte.
        // No phase tracking needed — each port write takes effect immediately.
        let idx = self.index();
        if let Some(slot) = self.palette.get_mut(idx) {
            *slot = (*slot & 0xFF00) | value as u16;
        }
        // Keep latch in sync for reads
        self.data_latch = self.palette.get(idx).copied().unwrap_or(0);
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VCE palette[{idx:03X}] low <= {:02X} => {:04X}",
            value, self.data_latch
        );
    }

    pub(crate) fn write_data_high(&mut self, value: u8) {
        // Per MAME huc6260.cpp: writing the high port directly modifies the
        // high byte of the current palette entry, preserving the low byte,
        // then auto-increments the address.  No phase tracking.
        let idx = self.index();
        let high = (value as u16) & 0x01;
        if let Some(slot) = self.palette.get_mut(idx) {
            *slot = (*slot & 0x00FF) | (high << 8);
        }
        // Keep latch in sync for reads
        self.data_latch = self.palette.get(idx).copied().unwrap_or(0);
        #[cfg(feature = "trace_hw_writes")]
        eprintln!(
            "  VCE palette[{idx:03X}] high <= {:02X} => {:04X}",
            value, self.data_latch
        );
        self.increment_index();
    }

    pub(crate) fn read_data_low(&mut self) -> u8 {
        if self.read_phase == VcePhase::Low {
            self.data_latch = self.palette.get(self.index()).copied().unwrap_or(0);
        }
        self.read_phase = VcePhase::High;
        (self.data_latch & 0x00FF) as u8
    }

    pub(crate) fn read_data_high(&mut self) -> u8 {
        if self.read_phase == VcePhase::Low {
            self.data_latch = self.palette.get(self.index()).copied().unwrap_or(0);
        }
        let value = ((self.data_latch >> 8) as u8 & 0x01) | 0xFE;
        self.increment_index();
        self.read_phase = VcePhase::Low;
        value
    }

    fn increment_index(&mut self) {
        let next = (self.index() + 1) & 0x01FF;
        self.address = (next as u16) & 0x01FF;
    }

    pub(crate) fn palette_access_stall_pixels(&self, cpu_high_speed: bool) -> usize {
        let cpu_master_cycles = if cpu_high_speed { 1usize } else { 4usize };
        let dot_divider = match self.control & 0x0003 {
            0x00 => 4usize,
            0x01 => 3usize,
            _ => 2usize,
        };
        cpu_master_cycles.div_ceil(dot_divider).max(1)
    }

    #[inline]
    fn brightness_override() -> Option<u8> {
        use std::sync::OnceLock;
        static OVERRIDE: OnceLock<Option<u8>> = OnceLock::new();
        *OVERRIDE.get_or_init(|| {
            std::env::var("PCE_FORCE_BRIGHTNESS")
                .ok()
                .and_then(|s| u8::from_str_radix(&s, 16).ok())
                .map(|v| v & 0x0F)
        })
    }

    pub(crate) fn palette_word(&self, index: usize) -> u16 {
        self.palette.get(index).copied().unwrap_or(0)
    }

    pub(crate) fn palette_rgb(&self, index: usize) -> u32 {
        let raw = self.palette.get(index).copied().unwrap_or(0);
        // HuC6260 palette words are 9-bit RGB (3 bits/channel).
        let blue = (raw & 0x0007) as u8;
        let red = ((raw >> 3) & 0x0007) as u8;
        let green = ((raw >> 6) & 0x0007) as u8;

        let scale = Self::brightness_override()
            .map(|v| v as u16)
            .unwrap_or(0x07);
        let component = |value: u8| -> u8 {
            if scale == 0 {
                return 0;
            }
            let expanded = (value as u16 * 255) / 0x07;
            let scaled = (expanded * scale) / 0x07;
            scaled.min(255) as u8
        };

        let r = component(red);
        let g = component(green);
        let b = component(blue);
        ((r as u32) << 16) | ((g as u32) << 8) | b as u32
    }
}
