use super::*;

impl Bus {
    pub(super) fn read_control_register(&mut self, offset: usize) -> Option<u8> {
        match Self::decode_control_register(offset)? {
            ControlRegister::TimerCounter => Some(self.timer.read_counter()),
            ControlRegister::TimerControl => Some(self.timer.control()),
            ControlRegister::IrqMask => Some(self.interrupt_disable),
            ControlRegister::IrqStatus => {
                if let Some(force) = Self::env_irq_status_default() {
                    Some(self.interrupt_request | force)
                } else {
                    Some(self.interrupt_request)
                }
            }
        }
    }

    pub(super) fn write_control_register(&mut self, offset: usize, value: u8) -> bool {
        match Self::decode_control_register(offset) {
            Some(ControlRegister::TimerCounter) => {
                self.timer.write_reload(value);
                true
            }
            Some(ControlRegister::TimerControl) => {
                self.timer.write_control(value);
                true
            }
            Some(ControlRegister::IrqMask) => {
                let mask = IRQ_DISABLE_IRQ2 | IRQ_DISABLE_IRQ1 | IRQ_DISABLE_TIMER;
                self.interrupt_disable = value & mask;
                true
            }
            Some(ControlRegister::IrqStatus) => {
                // On real HuC6280, writing to $1403 always clears the timer
                // IRQ regardless of the written value (confirmed by Mednafen).
                self.interrupt_request &= !IRQ_REQUEST_TIMER;
                true
            }
            None => false,
        }
    }

    pub(super) fn decode_control_register(offset: usize) -> Option<ControlRegister> {
        if (HW_TIMER_BASE..=HW_TIMER_BASE + 0x03FF).contains(&offset) {
            match offset & 0x01 {
                0x00 => Some(ControlRegister::TimerCounter),
                0x01 => Some(ControlRegister::TimerControl),
                _ => None,
            }
        } else if (HW_IRQ_BASE..=HW_IRQ_BASE + 0x03FF).contains(&offset) {
            match offset & 0x03 {
                0x02 => Some(ControlRegister::IrqMask),
                0x03 => Some(ControlRegister::IrqStatus),
                _ => None,
            }
        } else if (HW_CPU_CTRL_BASE..=HW_CPU_CTRL_BASE + 0x03FF).contains(&offset) {
            match offset & 0xFF {
                0x10 => Some(ControlRegister::TimerCounter),
                0x11 => Some(ControlRegister::TimerControl),
                0x12 => Some(ControlRegister::IrqMask),
                0x13 => Some(ControlRegister::IrqStatus),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Map a $FF00-$FF7F address to the I/O page offset.
    pub(super) fn mpr_index_for_addr(addr: u16) -> Option<usize> {
        if !(0xFF80..=0xFFBF).contains(&addr) {
            return None;
        }
        let offset = (addr - 0xFF80) as usize;
        Some(offset & 0x07)
    }

    pub(super) fn vdc_port_kind(offset: usize) -> Option<VdcPort> {
        // VDC is mirrored over the 0x0000–0x03FF IO window. Only A1..A0 select
        // control/data; A2+ are ignored by the chip. Many HuCARDs stream writes
        // via 0x2002/0x2003/0x200A/0x200B, so ensure any offset whose low two
        // bits are 0/1 goes to Control, 2/3 goes to Data.
        // For debug `PCE_VDC_ULTRA_MIRROR`, widen to the entire hardware page.
        if Bus::env_vdc_force_hot_ports() && Self::force_map_candidates(offset) {
            return Some(Self::vdc_port_from_low_bits(offset));
        }
        let mirrored = offset & 0x1FFF;
        let ultra = Self::env_vdc_ultra_mirror();
        let catchall = Self::env_vdc_catchall();
        if Self::env_vdc_force_hot_ports() && Self::force_map_candidates(offset) {
            return Some(Self::vdc_port_from_low_bits(offset));
        }
        if !catchall {
            if !Self::env_extreme_mirror() && !ultra && mirrored >= 0x0400 {
                return None;
            }
            if Self::env_extreme_mirror() && !ultra && mirrored >= 0x1000 {
                return None;
            }
            if ultra && mirrored >= 0x2000 {
                return None;
            }
        }
        match mirrored & 0x03 {
            0x00 | 0x01 => Some(VdcPort::Control),
            0x02 | 0x03 => Some(VdcPort::Data),
            _ => None,
        }
    }

    #[inline]
    pub(super) fn vdc_port_from_low_bits(offset: usize) -> VdcPort {
        if offset & 0x02 != 0 {
            VdcPort::Data
        } else {
            VdcPort::Control
        }
    }

    pub(super) fn force_map_candidates(offset: usize) -> bool {
        // Small list of hot addresses observed in HuCARD traces (0x2200/2211,
        // 0x2002/200A, 0x2017..0x201D, 0x0800..) that may mirror VDC ports.
        const HOT: &[usize] = &[
            0x0000, 0x0002, 0x0003, 0x0800, 0x0802, 0x0803, 0x0804, 0x0805, 0x0807, 0x2000, 0x2001,
            0x2002, 0x2003, 0x200A, 0x200B, 0x2010, 0x2011, 0x2012, 0x2016, 0x2017, 0x2018, 0x2019,
            0x201A, 0x201B, 0x201C, 0x201D, 0x2048, 0x2049, 0x204A, 0x204B, 0x204D, 0x2200, 0x2201,
            0x2202, 0x2209, 0x220A, 0x220B, 0x220C, 0x220D, 0x220F, 0x2210, 0x2211, 0x2212, 0x2215,
            0x2217, 0x2219, 0x221A, 0x221D, 0x2220, 0x2226, 0x2227, 0x2228, 0x2229, 0x222A, 0x222B,
            0x222D, 0x222E, 0x0A3A, 0x0A3B, 0x0A3C, 0x0A3D,
        ];
        HOT.iter().any(|&h| (offset & 0x3FFF) == h)
    }

    #[cfg(feature = "trace_hw_writes")]
    pub(super) fn st0_hold_enabled() -> bool {
        use std::sync::OnceLock;
        static ENABLED: OnceLock<bool> = OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var("PCE_TRACE_DISABLE_ST0_HOLD").is_err())
    }

    pub(super) fn normalized_io_offset(offset: usize) -> usize {
        // Optional: fold 0x0200–0x03FF down to 0x0000–0x01FF when debugging
        // HuCARDs that stream hardware writes through the wider mirror region.
        if Self::env_fold_io_02xx() && offset >= 0x0200 && offset < 0x0400 {
            offset & 0x01FF
        } else {
            offset
        }
    }

    pub(super) fn io_offset_targets_vdc_or_vce(raw_offset: usize) -> bool {
        let mut offset = raw_offset & 0x1FFF;
        offset = Self::normalized_io_offset(offset);
        if Self::env_route_02xx_hw() && offset >= 0x0200 && offset < 0x0220 {
            offset &= 0x01FF;
        }
        Self::vdc_port_kind(offset).is_some() || matches!(offset, 0x0400..=0x07FF | 0x1C40..=0x1C47)
    }

    pub(super) fn read_io_internal(&mut self, raw_offset: usize) -> u8 {
        // The HuC6280 only decodes A0–A10 for the hardware page; fold everything
        // into 0x0000–0x1FFF first, then optional 0x0200 folding for debug.
        let mut offset = raw_offset & 0x1FFF;
        offset = Self::normalized_io_offset(offset);
        if Self::env_route_02xx_hw() && offset >= 0x0200 && offset < 0x0220 {
            offset &= 0x01FF; // map 0x0200–0x021F to 0x0000–0x001F
        }
        if let Some(port) = Self::vdc_port_kind(offset) {
            #[cfg(feature = "trace_hw_writes")]
            {
                self.vdc.last_io_addr = offset as u16;
            }
            return match port {
                VdcPort::Control => self.vdc.read_status(),
                VdcPort::Data => {
                    let port_index = if offset & 0x01 != 0 { 2 } else { 1 };
                    self.vdc.read_port(port_index)
                }
            };
        }
        match offset {
            0x0400..=0x07FF | 0x1C40..=0x1C47 => {
                let sub = (offset & 0x0007) as u16;
                self.read_vce_port(sub)
            }
            // HuC6280 PSG native map is direct registers at $0800-$080F.
            0x0800..=0x0BFF => self.psg.read_direct(offset & 0x0F),
            // Keep legacy 4-port mirror behavior for older tests/tooling.
            0x1C60..=0x1C63 => match offset & 0x03 {
                0x00 => self.psg.read_address(),
                0x01 => self.io[offset],
                0x02 => self.psg.read_data(),
                _ => self.psg.read_status(),
            },
            0x0C00..=0x0FFF => {
                if let Some(value) = self.read_control_register(offset) {
                    value
                } else {
                    self.io[offset]
                }
            }
            0x1000..=0x13FF => {
                if let Some(value) = self.io_port.read(offset - HW_JOYPAD_BASE) {
                    value
                } else {
                    self.io[offset]
                }
            }
            0x1400..=0x17FF | 0x1C10..=0x1C13 => {
                if let Some(value) = self.read_control_register(offset) {
                    value
                } else {
                    self.io[offset]
                }
            }
            0x1800..=0x1BFF => match offset {
                BRAM_LOCK_PORT => {
                    *self.bram_unlocked = false;
                    0xFF
                }
                _ => 0xFF,
            },
            0x1C00..=0x1FFF => {
                if let Some(value) = self.read_control_register(offset) {
                    value
                } else {
                    self.io[offset]
                }
            }
            _ => self.io[offset],
        }
    }

    pub(super) fn write_io_internal(&mut self, raw_offset: usize, value: u8) {
        // Fold to 0x0000–0x1FFF to mirror HuC6280 hardware page decode.
        let mut offset = raw_offset & 0x1FFF;
        offset = Self::normalized_io_offset(offset);
        if Self::env_route_02xx_hw() && offset >= 0x0200 && offset < 0x0220 {
            offset &= 0x01FF; // map 0x0200–0x021F to 0x0000–0x001F
        }
        if let Some(port) = Self::vdc_port_kind(offset) {
            #[cfg(feature = "trace_hw_writes")]
            {
                self.vdc.last_io_addr = offset as u16;
            }
            match port {
                VdcPort::Control => self.write_st_port_internal(0, value),
                VdcPort::Data => {
                    let port_index = if offset & 0x01 != 0 { 2 } else { 1 };
                    self.write_st_port_internal(port_index, value)
                }
            }
            return;
        }
        #[cfg(feature = "trace_hw_writes")]
        if (offset & 0x1FFF) >= 0x2400 && (offset & 0x1FFF) < 0x2800 {
            eprintln!(
                "  IO write HIGH mirror offset {:04X} -> {:02X}",
                offset, value
            );
        }
        #[cfg(feature = "trace_hw_writes")]
        if (offset & 0xE000) == 0 && value != 0 {
            eprintln!("  HW page data write {:04X} -> {:02X}", offset, value);
        }
        match offset {
            // VCE mirrors also appear at 0x1C40–0x1C43 in some docs; treat them the same.
            0x0400..=0x07FF | 0x1C40..=0x1C47 => {
                let sub = (offset & 0x0007) as u16;
                self.write_vce_port(sub, value);
            }
            // HuC6280 PSG native map is direct registers at $0800-$080F.
            0x0800..=0x0BFF => self.psg.write_direct(offset & 0x0F, value),
            // Keep legacy 4-port mirror behavior for older tests/tooling.
            0x1C60..=0x1C63 => match offset & 0x03 {
                0x00 => self.psg.write_address(value),
                0x01 => self.psg.write_data(value),
                _ => self.io[offset] = value,
            },
            0x0C00..=0x0FFF | 0x1400..=0x17FF | 0x1C10..=0x1C13 => {
                // Timer/IRQ registers (mirrored)
                if !self.write_control_register(offset, value) {
                    self.io[offset] = value;
                }
            }
            0x1000..=0x13FF => {
                if !self.io_port.write(offset - HW_JOYPAD_BASE, value) {
                    self.io[offset] = value;
                }
            }
            0x1800..=0x1BFF => {
                if offset == BRAM_UNLOCK_PORT && (value & 0x80) != 0 {
                    *self.bram_unlocked = true;
                }
                self.io[offset] = value;
            }
            0x1C00..=0x1FFF => {
                // Treat as additional mirror for control/TIMER/IRQ/PSG status
                if (offset & 0x3F) >= 0x40 && (offset & 0x3F) <= 0x43 {
                    // Mirror of VCE control area? leave as IO
                    self.io[offset] = value;
                } else if !self.write_control_register(offset, value) {
                    self.io[offset] = value;
                }
            }
            _ => {
                self.io[offset] = value;
            }
        }
    }

    #[inline]
    pub(super) fn read_vce_port(&mut self, addr: u16) -> u8 {
        if matches!(addr & 0x0007, 0x04 | 0x05) && self.vdc.in_active_display_period() {
            self.note_vce_palette_access_flicker();
        }
        match addr & 0x0007 {
            0x00 => self.vce.read_control_low(),
            0x01 => self.vce.read_control_high(),
            0x02 => self.vce.read_address_low(),
            0x03 => self.vce.read_address_high(),
            0x04 => self.vce.read_data_low(),
            0x05 => self.vce.read_data_high(),
            _ => 0xFF,
        }
    }

    #[inline]
    pub(super) fn write_vce_port(&mut self, addr: u16, value: u8) {
        if matches!(addr & 0x0007, 0x04 | 0x05) && self.vdc.in_active_display_period() {
            self.note_vce_palette_access_flicker();
        }
        match addr & 0x0007 {
            0x00 => self.vce.write_control_low(value),
            0x01 => self.vce.write_control_high(value),
            0x02 => self.vce.write_address_low(value),
            0x03 => self.vce.write_address_high(value),
            0x04 => self.vce.write_data_low(value),
            0x05 => self.vce.write_data_high(value),
            _ => {}
        }
    }
}
