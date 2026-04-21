// SNES DMA and HDMA implementation
use crate::debug_flags;
use crate::savestate::{DmaChannelSaveState, DmaControllerSaveState};

#[derive(Debug, Clone)]
pub struct DmaChannel {
    pub control: u8,      // DMA制御レジスタ ($43X0)
    pub dest_address: u8, // 転送先アドレス ($43X1) - PPUレジスタ
    pub src_address: u32, // 転送元アドレス ($43X2-$43X4)
    pub size: u16,        // 転送サイズ ($43X5-$43X6)
    pub dasb: u8,         // Indirect HDMA bank / DMA reg ($43X7)
    pub a2a: u16,         // HDMA table current address ($43X8-$43X9)
    pub nltr: u8,         // HDMA line counter/reload ($43XA)
    pub unused: u8,       // Unused shared byte ($43XB and $43XF)

    // HDMA関連
    pub hdma_table_addr: u32,   // HDMAテーブルアドレス ($43X2-$43X4)
    pub hdma_line_counter: u8,  // HDMAライン残数 ($43X0の下位7ビット)
    pub hdma_repeat_flag: bool, // HDMAリピートフラグ ($43X0の7ビット目)
    pub hdma_do_transfer: bool, // HDMAがこのラインで転送するか（repeat=0時の「最初の1回」制御）
    pub hdma_enabled: bool,     // HDMAが有効か
    pub hdma_terminated: bool,  // HDMAが終了したか
    // HDMAデータ（リピート用ラッチ）
    pub hdma_latched: [u8; 4],
    pub hdma_latched_len: u8,
    // HDMA indirect addressing support
    pub hdma_indirect: bool,
    pub hdma_indirect_addr: u32,
    pub configured: bool,

    // Debug/config tracking (for INIT summaries)
    pub cfg_ctrl: bool,
    pub cfg_dest: bool,
    pub cfg_src: bool,
    pub cfg_size: bool,
}

impl Default for DmaChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl DmaChannel {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            // Power-on defaults per SNESdev wiki:
            // - DMAPn  = $FF
            // - BBADn  = $FF
            // - A1Tn   = $FFFFFF
            // - DASn   = $FFFF
            control: 0xFF,
            dest_address: 0xFF,
            src_address: 0x00FF_FFFF,
            size: 0xFFFF,
            dasb: 0xFF,
            a2a: 0xFFFF,
            nltr: 0xFF,
            unused: 0xFF,
            hdma_table_addr: 0,
            hdma_line_counter: 0,
            hdma_repeat_flag: false,
            hdma_do_transfer: false,
            hdma_enabled: false,
            hdma_terminated: false,
            hdma_latched: [0; 4],
            hdma_latched_len: 0,
            hdma_indirect: false,
            hdma_indirect_addr: 0,
            configured: false,
            cfg_ctrl: false,
            cfg_dest: false,
            cfg_src: false,
            cfg_size: false,
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // DMA転送方向を取得
    #[allow(dead_code)]
    pub fn is_ppu_to_cpu(&self) -> bool {
        self.control & 0x80 != 0
    }

    fn sync_indirect_addr_from_regs(&mut self) {
        self.hdma_indirect_addr = ((self.dasb as u32) << 16) | self.size as u32;
    }

    fn decode_nltr(value: u8) -> (u8, bool) {
        let mut line_count = value & 0x7F;
        if line_count == 0 {
            line_count = 128;
        }
        let repeat = value == 0x00 || (value != 0x80 && (value & 0x80) != 0);
        (line_count, repeat)
    }

    fn write_nltr(&mut self, value: u8) {
        self.nltr = value;
        let (line_count, repeat_flag) = Self::decode_nltr(value);
        self.hdma_line_counter = line_count;
        self.hdma_repeat_flag = repeat_flag;
        if !self.hdma_enabled {
            self.hdma_do_transfer = false;
        }
    }

    // DMA転送単位を取得
    pub fn get_transfer_unit(&self) -> u8 {
        self.control & 0x07
    }

    // アドレス増減設定を取得
    pub fn get_address_mode(&self) -> u8 {
        (self.control >> 3) & 0x03
    }

    pub fn to_save_state(&self) -> DmaChannelSaveState {
        DmaChannelSaveState {
            control: self.control,
            dest_address: self.dest_address,
            src_address: self.src_address,
            size: self.size,
            dasb: self.dasb,
            a2a: self.a2a,
            nltr: self.nltr,
            unused: self.unused,
            hdma_table_addr: self.hdma_table_addr,
            hdma_line_counter: self.hdma_line_counter,
            hdma_repeat_flag: self.hdma_repeat_flag,
            hdma_do_transfer: self.hdma_do_transfer,
            hdma_enabled: self.hdma_enabled,
            hdma_terminated: self.hdma_terminated,
            hdma_latched: self.hdma_latched,
            hdma_latched_len: self.hdma_latched_len,
            hdma_indirect: self.hdma_indirect,
            hdma_indirect_addr: self.hdma_indirect_addr,
            configured: self.configured,
            cfg_ctrl: self.cfg_ctrl,
            cfg_dest: self.cfg_dest,
            cfg_src: self.cfg_src,
            cfg_size: self.cfg_size,
        }
    }

    pub fn load_from_save_state(&mut self, st: &DmaChannelSaveState) {
        self.control = st.control;
        self.dest_address = st.dest_address;
        self.src_address = st.src_address;
        self.size = st.size;
        self.dasb = st.dasb;
        self.a2a = st.a2a;
        self.nltr = st.nltr;
        self.unused = st.unused;
        self.hdma_table_addr = st.hdma_table_addr;
        self.hdma_line_counter = st.hdma_line_counter;
        self.hdma_repeat_flag = st.hdma_repeat_flag;
        self.hdma_do_transfer = st.hdma_do_transfer;
        self.hdma_enabled = st.hdma_enabled;
        self.hdma_terminated = st.hdma_terminated;
        self.hdma_latched = st.hdma_latched;
        self.hdma_latched_len = st.hdma_latched_len;
        self.hdma_indirect = st.hdma_indirect;
        self.hdma_indirect_addr = st.hdma_indirect_addr;
        self.configured = st.configured;
        self.cfg_ctrl = st.cfg_ctrl;
        self.cfg_dest = st.cfg_dest;
        self.cfg_src = st.cfg_src;
        self.cfg_size = st.cfg_size;
    }
}

#[derive(Debug)]
pub struct DmaController {
    pub channels: [DmaChannel; 8],
    pub dma_enable: u8,  // DMA有効チャンネル ($420B)
    pub hdma_enable: u8, // HDMA有効チャンネル ($420C)
}

impl DmaController {
    pub fn new() -> Self {
        Self {
            channels: Default::default(),
            dma_enable: 0,
            hdma_enable: 0,
        }
    }

    pub fn to_save_state(&self) -> DmaControllerSaveState {
        let mut channels: [DmaChannelSaveState; 8] = Default::default();
        for (dst, src) in channels.iter_mut().zip(self.channels.iter()) {
            *dst = src.to_save_state();
        }
        DmaControllerSaveState {
            channels,
            dma_enable: self.dma_enable,
            hdma_enable: self.hdma_enable,
        }
    }

    pub fn load_from_save_state(&mut self, st: &DmaControllerSaveState) {
        for (dst, src) in self.channels.iter_mut().zip(st.channels.iter()) {
            dst.load_from_save_state(src);
        }
        self.dma_enable = st.dma_enable;
        self.hdma_enable = st.hdma_enable;
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.dma_enable = 0;
        self.hdma_enable = 0;
    }

    // DMAレジスタ書き込み
    pub fn write(&mut self, addr: u16, value: u8) {
        // Lightweight debug hook: dump early DMA register writes when TRACE_DMA_REG is set.
        if crate::debug_flags::trace_dma_reg() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNT: AtomicU32 = AtomicU32::new(0);
            let n = COUNT.fetch_add(1, Ordering::Relaxed);
            if n < 256 {
                let ch = ((addr.saturating_sub(0x4300)) >> 4) as u8;
                let reg = addr & 0x0F;
                println!(
                    "[DMA-REG] W ${:04X} ch{} reg=${:X} val={:02X}",
                    addr, ch, reg, value
                );
            }
        }
        match addr {
            0x420B => {
                self.dma_enable = value;
                if (debug_flags::dma() || debug_flags::dma_reg()) && !debug_flags::quiet() {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static EN_LOG: AtomicU32 = AtomicU32::new(0);
                    let n = EN_LOG.fetch_add(1, Ordering::Relaxed);
                    if n < 64 {
                        println!("[DMA-EN] $420B MDMAEN=0x{:02X}", value);
                    }
                }
            }
            0x420C => {
                let old = self.hdma_enable;
                self.hdma_enable = value;
                if debug_flags::trace_hdmaen() && value != old {
                    use std::sync::atomic::{AtomicU32, Ordering};
                    static CNT: std::sync::atomic::AtomicU32 = AtomicU32::new(0);
                    let n = CNT.fetch_add(1, Ordering::Relaxed);
                    if n < 32 {
                        eprintln!(
                            "[HDMAEN] #{} old=0x{:02X} new=0x{:02X} ch_bits={:08b}",
                            n + 1,
                            old,
                            value,
                            value
                        );
                    }
                }
                // HDMAEN records which channels participate in HDMA.
                // Actual table initialisation happens once per frame in
                // Bus::on_frame_start (scanline 0).  We must NOT re-init the
                // table pointer here because doing so resets mid-frame HDMA
                // state and corrupts per-scanline data (e.g. Mode 7
                // perspective in Pilotwings).
                //
                // Channels newly disabled are stopped immediately. Bus wraps this
                // write to handle live rising edges without resetting table state.
                let disabled = old & !value;
                for i in 0..8u8 {
                    if disabled & (1 << i) != 0 {
                        self.channels[i as usize].hdma_enabled = false;
                    }
                }
            }
            0x4300..=0x43FF => {
                // チャンネル別レジスタ
                let channel = ((addr - 0x4300) >> 4) as usize;
                let reg = (addr & 0x0F) as u8;

                if channel < 8 {
                    if channel == 1 && debug_flags::dma_reg() {
                        use std::sync::atomic::{AtomicU32, Ordering};
                        static COUNT1: AtomicU32 = AtomicU32::new(0);
                        let n = COUNT1.fetch_add(1, Ordering::Relaxed);
                        if n < 32 {
                            println!(
                                "[DMA1-REG] W ${:04X} reg=${:X} val={:02X}",
                                addr, reg, value
                            );
                        }
                    }
                    match reg {
                        0x00 => {
                            self.channels[channel].control = value;
                            // bit6: HDMA indirect addressing
                            self.channels[channel].hdma_indirect = (value & 0x40) != 0;
                            self.channels[channel].configured = true;
                            self.channels[channel].cfg_ctrl = true;
                            if debug_flags::dma_reg() {
                                println!(
                                    "DMA ch{} control=0x{:02X} (unit={}, addr_mode={})",
                                    channel,
                                    value,
                                    self.channels[channel].get_transfer_unit(),
                                    self.channels[channel].get_address_mode()
                                );
                            }
                        }
                        0x01 => {
                            // B-bus destination ($43x1) — use value as-is.
                            self.channels[channel].dest_address = value;
                            self.channels[channel].configured = true;
                            self.channels[channel].cfg_dest = true;
                            if debug_flags::dma_reg() {
                                use std::sync::atomic::{AtomicU32, Ordering};
                                static DEST_LOG: AtomicU32 = AtomicU32::new(0);
                                let n = DEST_LOG.fetch_add(1, Ordering::Relaxed);
                                if n < 64 {
                                    println!(
                                        "[DMA-DEST-REG] ch{} BBAD=$21{:02X} (reg=${:04X})",
                                        channel, value, addr
                                    );
                                }
                                // Lightweight trace for graphics-related destinations
                                if matches!(value, 0x18 | 0x19 | 0x22 | 0x04) {
                                    static DEST_TRACE: AtomicU32 = AtomicU32::new(0);
                                    let n = DEST_TRACE.fetch_add(1, Ordering::Relaxed);
                                    if n < 32 {
                                        println!(
                                            "[DMA-DEST] ch{} dest=$21{:02X} (graphics path)",
                                            channel, value
                                        );
                                    }
                                }
                            }
                            if (debug_flags::dma_reg() || debug_flags::cgram_dma()) && value == 0x22
                            {
                                println!("DMA ch{} configured for CGRAM ($2122)", channel);
                            }
                            if debug_flags::dma_reg() {
                                println!("DMA ch{} dest=$21{:02X}", channel, value);
                            }
                        }
                        0x02 => {
                            self.channels[channel].src_address =
                                (self.channels[channel].src_address & 0xFFFF00) | value as u32;
                            self.channels[channel].hdma_table_addr =
                                self.channels[channel].src_address;
                            self.channels[channel].configured = true;
                            self.channels[channel].cfg_src = true;
                            if debug_flags::dma_reg() {
                                println!("DMA ch{} src.lo=0x{:02X}", channel, value);
                            }
                        }
                        0x03 => {
                            self.channels[channel].src_address =
                                (self.channels[channel].src_address & 0xFF00FF)
                                    | ((value as u32) << 8);
                            self.channels[channel].hdma_table_addr =
                                self.channels[channel].src_address;
                            self.channels[channel].configured = true;
                            self.channels[channel].cfg_src = true;
                            if debug_flags::dma_reg() {
                                println!("DMA ch{} src.mid=0x{:02X}", channel, value);
                            }
                        }
                        0x04 => {
                            self.channels[channel].src_address =
                                (self.channels[channel].src_address & 0x00FFFF)
                                    | ((value as u32) << 16);
                            self.channels[channel].configured = true;
                            self.channels[channel].cfg_src = true;
                            // If HDMA is using A2A, update only the bank portion for subsequent table reads.
                            let low = self.channels[channel].hdma_table_addr & 0x0000_FFFF;
                            self.channels[channel].hdma_table_addr =
                                (self.channels[channel].src_address & 0x00FF_0000) | low;
                            if debug_flags::dma_reg() {
                                println!("DMA ch{} src.bank=0x{:02X}", channel, value);
                            }
                        }
                        0x05 => {
                            self.channels[channel].size =
                                (self.channels[channel].size & 0xFF00) | value as u16;
                            self.channels[channel].sync_indirect_addr_from_regs();
                            self.channels[channel].configured = true;
                            self.channels[channel].cfg_size = true;
                            if (debug_flags::dma_reg() || debug_flags::cgram_dma())
                                && self.channels[channel].dest_address == 0x22
                            {
                                println!(
                                    "DMA ch{} CGRAM size.lo set -> size={} bytes",
                                    channel, self.channels[channel].size
                                );
                            }
                            if debug_flags::dma_reg() {
                                println!("DMA ch{} size.lo=0x{:02X}", channel, value);
                            }
                        }
                        0x06 => {
                            self.channels[channel].size =
                                (self.channels[channel].size & 0x00FF) | ((value as u16) << 8);
                            self.channels[channel].sync_indirect_addr_from_regs();
                            self.channels[channel].configured = true;
                            self.channels[channel].cfg_size = true;
                            if (debug_flags::dma_reg() || debug_flags::cgram_dma())
                                && self.channels[channel].dest_address == 0x22
                            {
                                println!(
                                    "DMA ch{} CGRAM size.hi set -> size={} bytes",
                                    channel, self.channels[channel].size
                                );
                            }
                            if debug_flags::dma_reg() {
                                static mut DMA_SIZE_LOG_CNT2: u32 = 0;
                                unsafe {
                                    DMA_SIZE_LOG_CNT2 += 1;
                                    if DMA_SIZE_LOG_CNT2 <= 16 {
                                        println!(
                                            "DMA ch{} size.hi=0x{:02X} (size={})",
                                            channel, value, self.channels[channel].size
                                        );
                                    }
                                }
                            }
                        }
                        0x07 => {
                            // DASBn ($43x7): Indirect HDMA bank. RW8.
                            self.channels[channel].dasb = value;
                            self.channels[channel].sync_indirect_addr_from_regs();
                        }
                        0x08 => {
                            // A2AnL ($43x8): HDMA table current address low. RW8.
                            self.channels[channel].a2a =
                                (self.channels[channel].a2a & 0xFF00) | value as u16;
                            // Mirror into internal HDMA table pointer (bank from A1Bn/src_address).
                            let bank = self.channels[channel].src_address & 0xFF0000;
                            self.channels[channel].hdma_table_addr =
                                bank | (self.channels[channel].a2a as u32);
                        }
                        0x09 => {
                            // A2AnH ($43x9): HDMA table current address high. RW8.
                            self.channels[channel].a2a =
                                (self.channels[channel].a2a & 0x00FF) | ((value as u16) << 8);
                            // Mirror into internal HDMA table pointer (bank from A1Bn/src_address).
                            let bank = self.channels[channel].src_address & 0xFF0000;
                            self.channels[channel].hdma_table_addr =
                                bank | (self.channels[channel].a2a as u32);
                        }
                        0x0A => {
                            // NLTRn ($43xA): HDMA reload flag + line counter. RW8.
                            self.channels[channel].write_nltr(value);
                        }
                        0x0B | 0x0F => {
                            // UNUSEDn ($43xB/$43xF): shared RW8 byte with no effect on DMA/HDMA.
                            self.channels[channel].unused = value;
                        }
                        0x0C..=0x0E => {
                            // Unused holes: ignore writes (open bus on read).
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // DMAレジスタ読み込み
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x420B => self.dma_enable,
            0x420C => self.hdma_enable,
            0x2200..=0x23FF => {
                // SA-1 register window is handled by Bus before DMA controller dispatch.
                0xFF
            }
            0x4300..=0x43FF => {
                let channel = ((addr - 0x4300) >> 4) as usize;
                let reg = (addr & 0x0F) as u8;

                if channel < 8 {
                    match reg {
                        0x00 => self.channels[channel].control,
                        0x01 => self.channels[channel].dest_address,
                        0x02 => (self.channels[channel].src_address & 0xFF) as u8,
                        0x03 => ((self.channels[channel].src_address >> 8) & 0xFF) as u8,
                        0x04 => ((self.channels[channel].src_address >> 16) & 0xFF) as u8,
                        0x05 => (self.channels[channel].size & 0xFF) as u8,
                        0x06 => ((self.channels[channel].size >> 8) & 0xFF) as u8,
                        0x07 => self.channels[channel].dasb,
                        0x08 => (self.channels[channel].a2a & 0xFF) as u8,
                        0x09 => ((self.channels[channel].a2a >> 8) & 0xFF) as u8,
                        0x0A => self.channels[channel].nltr,
                        0x0B | 0x0F => self.channels[channel].unused,
                        _ => 0xFF,
                    }
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DmaController;

    fn configure_hdma_channel(dma: &mut DmaController, channel: usize, src_address: u32) {
        let ch = &mut dma.channels[channel];
        ch.configured = true;
        ch.control = 0x00;
        ch.dest_address = 0x18;
        ch.src_address = src_address;
        ch.dasb = (src_address >> 16) as u8;
        ch.hdma_enabled = true;
        ch.hdma_terminated = false;
        ch.hdma_table_addr = src_address.wrapping_add(0x20);
        ch.hdma_line_counter = 0x12;
        ch.hdma_repeat_flag = true;
        ch.hdma_do_transfer = true;
        ch.a2a = 0x3456;
        ch.nltr = 0x9A;
    }

    #[test]
    fn hdmaen_disable_stops_channel_without_reinitialising_state() {
        let mut dma = DmaController::new();
        configure_hdma_channel(&mut dma, 0, 0x12_3400);
        dma.hdma_enable = 0x01;

        dma.write(0x420C, 0x00);

        let ch = &dma.channels[0];
        assert_eq!(dma.hdma_enable, 0x00);
        assert!(
            !ch.hdma_enabled,
            "channel must stop immediately when disabled"
        );
        assert_eq!(ch.hdma_table_addr, 0x12_3420);
        assert_eq!(ch.hdma_line_counter, 0x12);
        assert!(ch.hdma_repeat_flag);
        assert!(ch.hdma_do_transfer);
        assert_eq!(ch.a2a, 0x3456);
        assert_eq!(ch.nltr, 0x9A);
    }

    #[test]
    fn hdmaen_midframe_enable_updates_mask_without_reinitialising_state() {
        let mut dma = DmaController::new();
        configure_hdma_channel(&mut dma, 0, 0x7E_2000);
        dma.hdma_enable = 0x00;
        dma.channels[0].hdma_enabled = false;

        dma.write(0x420C, 0x01);

        let ch = &dma.channels[0];
        assert_eq!(dma.hdma_enable, 0x01);
        assert!(
            !ch.hdma_enabled,
            "mid-frame enable should not immediately restart the channel"
        );
        assert_eq!(
            ch.hdma_table_addr, 0x7E_2020,
            "table pointer must be preserved by the controller write"
        );
        assert_eq!(ch.hdma_line_counter, 0x12);
        assert_eq!(ch.a2a, 0x3456);
        assert_eq!(ch.nltr, 0x9A);
    }

    #[test]
    fn hdma_state_register_writes_update_internal_transfer_state() {
        let mut dma = DmaController::new();

        dma.write(0x4317, 0x12);
        dma.write(0x4315, 0x34);
        dma.write(0x4316, 0x56);
        assert_eq!(dma.channels[1].hdma_indirect_addr, 0x12_5634);

        dma.channels[1].hdma_enabled = false;
        dma.channels[1].hdma_do_transfer = true;
        dma.write(0x431A, 0x81);
        assert_eq!(dma.channels[1].hdma_line_counter, 1);
        assert!(dma.channels[1].hdma_repeat_flag);
        assert!(!dma.channels[1].hdma_do_transfer);

        dma.write(0x431A, 0x80);
        assert_eq!(dma.channels[1].hdma_line_counter, 128);
        assert!(!dma.channels[1].hdma_repeat_flag);
    }
}
