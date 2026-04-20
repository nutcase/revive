// SPC7110 — Data compression / decompression chip used in select SNES titles
// (e.g. Momotarou Dentetsu Happy, Far East of Eden Zero, Super Power League 4).
//
// Provides:
//   1. Hardware data decompression (context-based adaptive arithmetic coding)
//   2. Extended ROM banking ($D0-$FF mapped via bank registers)
//   3. Data ROM read port with auto-increment
//   4. Signed 16x16 multiply / 32/16 divide unit
//
// Decompressor ported from ares (neviksti / talarubi implementation).

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
//  Decompressor — ares-compatible arithmetic decoder
// ---------------------------------------------------------------------------

const MPS: u32 = 0;
const LPS: u32 = 1;
const HALF: u16 = 0x55;
const MAX: u16 = 0xFF;

#[derive(Clone, Copy)]
struct ModelState {
    probability: u8,
    next: [u8; 2], // next state after {MPS, LPS}
}

static EVOLUTION: [ModelState; 53] = [
    ModelState {
        probability: 0x5A,
        next: [1, 1],
    },
    ModelState {
        probability: 0x25,
        next: [2, 6],
    },
    ModelState {
        probability: 0x11,
        next: [3, 8],
    },
    ModelState {
        probability: 0x08,
        next: [4, 10],
    },
    ModelState {
        probability: 0x03,
        next: [5, 12],
    },
    ModelState {
        probability: 0x01,
        next: [5, 15],
    },
    ModelState {
        probability: 0x5A,
        next: [7, 7],
    },
    ModelState {
        probability: 0x3F,
        next: [8, 19],
    },
    ModelState {
        probability: 0x2C,
        next: [9, 21],
    },
    ModelState {
        probability: 0x20,
        next: [10, 22],
    },
    ModelState {
        probability: 0x17,
        next: [11, 23],
    },
    ModelState {
        probability: 0x11,
        next: [12, 25],
    },
    ModelState {
        probability: 0x0C,
        next: [13, 26],
    },
    ModelState {
        probability: 0x09,
        next: [14, 28],
    },
    ModelState {
        probability: 0x07,
        next: [15, 29],
    },
    ModelState {
        probability: 0x05,
        next: [16, 31],
    },
    ModelState {
        probability: 0x04,
        next: [17, 32],
    },
    ModelState {
        probability: 0x03,
        next: [18, 34],
    },
    ModelState {
        probability: 0x02,
        next: [5, 35],
    },
    ModelState {
        probability: 0x5A,
        next: [20, 20],
    },
    ModelState {
        probability: 0x48,
        next: [21, 39],
    },
    ModelState {
        probability: 0x3A,
        next: [22, 40],
    },
    ModelState {
        probability: 0x2E,
        next: [23, 42],
    },
    ModelState {
        probability: 0x26,
        next: [24, 44],
    },
    ModelState {
        probability: 0x1F,
        next: [25, 45],
    },
    ModelState {
        probability: 0x19,
        next: [26, 46],
    },
    ModelState {
        probability: 0x15,
        next: [27, 25],
    },
    ModelState {
        probability: 0x11,
        next: [28, 26],
    },
    ModelState {
        probability: 0x0E,
        next: [29, 26],
    },
    ModelState {
        probability: 0x0B,
        next: [30, 27],
    },
    ModelState {
        probability: 0x09,
        next: [31, 28],
    },
    ModelState {
        probability: 0x08,
        next: [32, 29],
    },
    ModelState {
        probability: 0x07,
        next: [33, 30],
    },
    ModelState {
        probability: 0x05,
        next: [34, 31],
    },
    ModelState {
        probability: 0x04,
        next: [35, 33],
    },
    ModelState {
        probability: 0x04,
        next: [36, 33],
    },
    ModelState {
        probability: 0x03,
        next: [37, 34],
    },
    ModelState {
        probability: 0x02,
        next: [38, 35],
    },
    ModelState {
        probability: 0x02,
        next: [5, 36],
    },
    ModelState {
        probability: 0x58,
        next: [40, 39],
    },
    ModelState {
        probability: 0x4D,
        next: [41, 47],
    },
    ModelState {
        probability: 0x43,
        next: [42, 48],
    },
    ModelState {
        probability: 0x3B,
        next: [43, 49],
    },
    ModelState {
        probability: 0x34,
        next: [44, 50],
    },
    ModelState {
        probability: 0x2E,
        next: [45, 51],
    },
    ModelState {
        probability: 0x29,
        next: [46, 44],
    },
    ModelState {
        probability: 0x25,
        next: [24, 45],
    },
    ModelState {
        probability: 0x56,
        next: [48, 47],
    },
    ModelState {
        probability: 0x4F,
        next: [49, 47],
    },
    ModelState {
        probability: 0x47,
        next: [50, 48],
    },
    ModelState {
        probability: 0x41,
        next: [51, 49],
    },
    ModelState {
        probability: 0x3C,
        next: [52, 50],
    },
    ModelState {
        probability: 0x37,
        next: [43, 51],
    },
];

#[derive(Clone, Copy)]
struct DecompContext {
    prediction: u8,
    swap: u8,
}

/// Streaming decompressor matching ares's implementation.
/// Each call to `decode()` produces one `result` word (8 pixels packed).
struct Decompressor {
    context: [[DecompContext; 15]; 5],
    bpp: u32,
    offset: u32,
    bits: u32,
    range: u16,
    input: u16,
    output: u8,
    pixels: u64,
    colormap: u64,
    result: u32,
    // ROM reference data
    data_rom_start: usize,
}

const RTC_REG_COUNT: usize = 16;
const RTC_REG_INDEX_MASK: u8 = 0x0F;
const RTC_CTRL_PAUSE: u8 = 0x01;
const RTC_CTRL_STOP: u8 = 0x02;
const RTC_CTRL_24H: u8 = 0x04;
const RTC_CTRL_RESET_SECONDS: u8 = 0x01;

#[derive(Clone, Copy, PartialEq, Eq)]
enum RtcPhase {
    Disabled,
    Command,
    Index,
    Data,
}

impl RtcPhase {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Command,
            2 => Self::Index,
            3 => Self::Data,
            _ => Self::Disabled,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Command => 1,
            Self::Index => 2,
            Self::Data => 3,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Rtc4513SaveData {
    pub enabled: bool,
    pub phase: u8,
    pub command: u8,
    pub index: u8,
    pub regs: [u8; RTC_REG_COUNT],
    pub base_epoch: i64,
    pub base_host_epoch: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Spc7110SaveData {
    pub dcu_pending: bool,
    pub dcu_mode: u8,
    pub dcu_address: u32,
    pub dcu_offset: u32,
    pub dcu_tile: [u8; 32],
    pub decomp_prediction: [[u8; 15]; 5],
    pub decomp_swap: [[u8; 15]; 5],
    pub decomp_bpp: u32,
    pub decomp_offset: u32,
    pub decomp_bits: u32,
    pub decomp_range: u16,
    pub decomp_input: u16,
    pub decomp_output: u8,
    pub decomp_pixels: u64,
    pub decomp_colormap: u64,
    pub decomp_result: u32,
    pub decomp_data_rom_start: usize,
    pub r4801: u8,
    pub r4802: u8,
    pub r4803: u8,
    pub r4804: u8,
    pub r4805: u8,
    pub r4806: u8,
    pub r4807: u8,
    pub r4809: u8,
    pub r480a: u8,
    pub r480b: u8,
    pub r480c: u8,
    pub data_rom_offset: u32,
    pub data_rom_adjust: u16,
    pub data_rom_increment: u16,
    pub data_rom_mode: u8,
    pub data_port_ready: u8,
    pub alu_multiplicand: u16,
    pub alu_multiplier: u16,
    pub alu_dividend: u32,
    pub alu_divisor: u16,
    pub alu_result: u32,
    pub alu_remainder: u16,
    pub alu_control: u8,
    pub alu_status: u8,
    pub reg_4830: u8,
    pub bank_d0: u8,
    pub bank_e0: u8,
    pub bank_f0: u8,
    pub sram_bank: u8,
    pub data_rom_start: usize,
    pub rtc: Rtc4513SaveData,
    pub debug_4800_count: u64,
}

impl Default for Spc7110SaveData {
    fn default() -> Self {
        Self {
            dcu_pending: false,
            dcu_mode: 0,
            dcu_address: 0,
            dcu_offset: 0,
            dcu_tile: [0; 32],
            decomp_prediction: [[0; 15]; 5],
            decomp_swap: [[0; 15]; 5],
            decomp_bpp: 1,
            decomp_offset: 0,
            decomp_bits: 0,
            decomp_range: 0,
            decomp_input: 0,
            decomp_output: 0,
            decomp_pixels: 0,
            decomp_colormap: 0,
            decomp_result: 0,
            decomp_data_rom_start: 0,
            r4801: 0,
            r4802: 0,
            r4803: 0,
            r4804: 0,
            r4805: 0,
            r4806: 0,
            r4807: 0,
            r4809: 0,
            r480a: 0,
            r480b: 0,
            r480c: 0,
            data_rom_offset: 0,
            data_rom_adjust: 0,
            data_rom_increment: 0,
            data_rom_mode: 0,
            data_port_ready: 0,
            alu_multiplicand: 0,
            alu_multiplier: 0,
            alu_dividend: 0,
            alu_divisor: 0,
            alu_result: 0,
            alu_remainder: 0,
            alu_control: 0,
            alu_status: 0,
            reg_4830: 0,
            bank_d0: 0,
            bank_e0: 0,
            bank_f0: 0,
            sram_bank: 0,
            data_rom_start: 0,
            rtc: Rtc4513SaveData::default(),
            debug_4800_count: 0,
        }
    }
}

struct Rtc4513 {
    enabled: bool,
    phase: RtcPhase,
    command: u8,
    index: u8,
    regs: [u8; RTC_REG_COUNT],
    base_epoch: i64,
    base_host_epoch: i64,
}

impl Rtc4513 {
    fn new() -> Self {
        let now = Self::host_unix_seconds();
        let mut rtc = Self {
            enabled: false,
            phase: RtcPhase::Disabled,
            command: 0,
            index: 0,
            regs: [0; RTC_REG_COUNT],
            base_epoch: now,
            base_host_epoch: now,
        };
        rtc.regs[0x0E] = 0x0F;
        rtc.regs[0x0F] = RTC_CTRL_24H;
        rtc.sync_to_regs(now);
        rtc
    }

    fn host_unix_seconds() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|dur| dur.as_secs() as i64)
            .unwrap_or(0)
    }

    fn is_paused(&self) -> bool {
        (self.regs[0x0D] & RTC_CTRL_PAUSE) != 0
    }

    fn is_stopped(&self) -> bool {
        (self.regs[0x0F] & RTC_CTRL_STOP) != 0
    }

    fn is_24_hour_mode(&self) -> bool {
        (self.regs[0x0F] & RTC_CTRL_24H) != 0
    }

    fn current_epoch(&self, now: i64) -> i64 {
        if self.is_stopped() {
            self.base_epoch
        } else {
            self.base_epoch
                .saturating_add(now.saturating_sub(self.base_host_epoch))
        }
    }

    fn read_enable(&self) -> u8 {
        u8::from(self.enabled)
    }

    fn write_enable(&mut self, value: u8) {
        self.enabled = (value & 0x01) != 0;
        if self.enabled {
            self.phase = RtcPhase::Command;
            self.command = 0;
            self.index = 0;
        } else {
            self.phase = RtcPhase::Disabled;
        }
    }

    fn read_status(&self) -> u8 {
        if self.enabled {
            0x80
        } else {
            0x00
        }
    }

    fn read_data(&mut self) -> u8 {
        if !self.enabled || self.phase != RtcPhase::Data {
            return 0x00;
        }

        let now = Self::host_unix_seconds();
        self.sync_to_regs(now);
        let value = self.regs[self.index as usize];
        self.index = self.index.wrapping_add(1) & RTC_REG_INDEX_MASK;
        value
    }

    fn write_data(&mut self, value: u8) {
        if !self.enabled {
            return;
        }

        match self.phase {
            RtcPhase::Disabled => {}
            RtcPhase::Command => {
                self.command = value;
                self.phase = if matches!(value, 0x03 | 0x0C) {
                    RtcPhase::Index
                } else {
                    RtcPhase::Command
                };
            }
            RtcPhase::Index => {
                self.index = value & RTC_REG_INDEX_MASK;
                self.phase = RtcPhase::Data;
            }
            RtcPhase::Data => {
                let now = Self::host_unix_seconds();
                self.sync_to_regs(now);
                self.write_current_register(value, now);
                self.index = self.index.wrapping_add(1) & RTC_REG_INDEX_MASK;
            }
        }
    }

    fn write_current_register(&mut self, value: u8, now: i64) {
        let idx = self.index as usize;
        match idx {
            0x0D => {
                let old = self.regs[0x0D];
                let new = value & RTC_CTRL_PAUSE;
                if (old & RTC_CTRL_PAUSE) == 0 && new != 0 {
                    self.sync_to_regs(now);
                }
                self.regs[0x0D] = new;
                self.base_epoch =
                    Self::epoch_from_regs_with_format(&self.regs, self.is_24_hour_mode());
                self.base_host_epoch = now;
            }
            0x0E => {
                self.regs[0x0E] = value & 0x0F;
            }
            0x0F => {
                let old_24h = self.is_24_hour_mode();
                self.sync_to_regs(now);
                if (value & RTC_CTRL_RESET_SECONDS) != 0 {
                    self.regs[0x00] = 0;
                    self.regs[0x01] = 0;
                }
                let current_epoch = Self::epoch_from_regs_with_format(&self.regs, old_24h);
                self.regs[0x0F] = value & 0x07;
                self.base_epoch = current_epoch;
                self.base_host_epoch = now;
                if !self.is_paused() {
                    self.sync_to_regs(now);
                }
            }
            0x00..=0x0C => {
                self.regs[idx] = value & Self::register_mask(idx, self.is_24_hour_mode());
                self.base_epoch =
                    Self::epoch_from_regs_with_format(&self.regs, self.is_24_hour_mode());
                self.base_host_epoch = now;
                if !self.is_paused() {
                    self.sync_to_regs(now);
                }
            }
            _ => {}
        }
    }

    fn sync_to_regs(&mut self, now: i64) {
        if self.is_paused() {
            return;
        }
        let time_regs = Self::time_regs_from_epoch(self.current_epoch(now), self.is_24_hour_mode());
        self.regs[..0x0D].copy_from_slice(&time_regs[..0x0D]);
    }

    fn register_mask(index: usize, is_24_hour_mode: bool) -> u8 {
        match index {
            0x00 | 0x02 | 0x04 | 0x06 | 0x08 | 0x0A | 0x0B => 0x0F,
            0x01 | 0x03 | 0x07 => 0x07,
            0x05 if is_24_hour_mode => 0x03,
            0x05 => 0x05,
            0x09 => 0x01,
            0x0C => 0x07,
            0x0D => 0x01,
            0x0E => 0x0F,
            0x0F => 0x07,
            _ => 0x0F,
        }
    }

    fn time_regs_from_epoch(epoch: i64, is_24_hour_mode: bool) -> [u8; RTC_REG_COUNT] {
        let mut regs = [0u8; RTC_REG_COUNT];
        let days = epoch.div_euclid(86_400);
        let secs_of_day = epoch.rem_euclid(86_400);
        let hour = (secs_of_day / 3_600) as u8;
        let minute = ((secs_of_day / 60) % 60) as u8;
        let second = (secs_of_day % 60) as u8;
        let (year, month, day) = Self::civil_from_days(days);
        let year_two_digits = (year.rem_euclid(100)) as u8;
        let weekday = (days + 4).rem_euclid(7) as u8;

        regs[0x00] = second % 10;
        regs[0x01] = second / 10;
        regs[0x02] = minute % 10;
        regs[0x03] = minute / 10;

        if is_24_hour_mode {
            regs[0x04] = hour % 10;
            regs[0x05] = hour / 10;
        } else {
            let mut hour12 = hour % 12;
            if hour12 == 0 {
                hour12 = 12;
            }
            regs[0x04] = hour12 % 10;
            regs[0x05] = (hour12 / 10) & 0x01;
            if hour >= 12 {
                regs[0x05] |= 0x04;
            }
        }

        regs[0x06] = day % 10;
        regs[0x07] = day / 10;
        regs[0x08] = month % 10;
        regs[0x09] = month / 10;
        regs[0x0A] = year_two_digits % 10;
        regs[0x0B] = year_two_digits / 10;
        regs[0x0C] = weekday;
        regs
    }

    fn epoch_from_regs_with_format(regs: &[u8; RTC_REG_COUNT], is_24_hour_mode: bool) -> i64 {
        let second = ((regs[0x01] & 0x07) as u32) * 10 + (regs[0x00] & 0x0F) as u32;
        let minute = ((regs[0x03] & 0x07) as u32) * 10 + (regs[0x02] & 0x0F) as u32;
        let day = ((regs[0x07] & 0x07) as u32) * 10 + (regs[0x06] & 0x0F) as u32;
        let month = ((regs[0x09] & 0x01) as u32) * 10 + (regs[0x08] & 0x0F) as u32;
        let year_two_digits = ((regs[0x0B] & 0x0F) as u32) * 10 + (regs[0x0A] & 0x0F) as u32;

        let hour = if is_24_hour_mode {
            ((regs[0x05] & 0x03) as u32) * 10 + (regs[0x04] & 0x0F) as u32
        } else {
            let mut hour12 = ((regs[0x05] & 0x01) as u32) * 10 + (regs[0x04] & 0x0F) as u32;
            if hour12 == 0 {
                hour12 = 12;
            }
            hour12 %= 12;
            if (regs[0x05] & 0x04) != 0 {
                hour12 + 12
            } else {
                hour12
            }
        };

        let year = Self::expand_two_digit_year(year_two_digits as u8);
        let clamped_month = month.clamp(1, 12) as u8;
        let clamped_day = day.clamp(1, 31) as u8;
        let days = Self::days_from_civil(year, clamped_month, clamped_day);
        days.saturating_mul(86_400)
            .saturating_add((hour.min(23) * 3_600) as i64)
            .saturating_add((minute.min(59) * 60) as i64)
            .saturating_add(second.min(59) as i64)
    }

    fn expand_two_digit_year(year: u8) -> i32 {
        if year >= 70 {
            1900 + year as i32
        } else {
            2000 + year as i32
        }
    }

    fn civil_from_days(days: i64) -> (i32, u8, u8) {
        let z = days + 719_468;
        let era = if z >= 0 {
            z / 146_097
        } else {
            (z - 146_096) / 146_097
        };
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe as i32 + era as i32 * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = mp + if mp < 10 { 3 } else { -9 };
        let year = y + i32::from(month <= 2);
        (year, month as u8, day as u8)
    }

    fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
        let y = year - i32::from(month <= 2);
        let era = if y >= 0 { y / 400 } else { (y - 399) / 400 };
        let yoe = y - era * 400;
        let mp = month as i32 + if month > 2 { -3 } else { 9 };
        let doy = (153 * mp + 2) / 5 + day as i32 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era as i64 * 146_097 + doe as i64 - 719_468
    }

    fn save_data(&self) -> Rtc4513SaveData {
        Rtc4513SaveData {
            enabled: self.enabled,
            phase: self.phase.as_u8(),
            command: self.command,
            index: self.index,
            regs: self.regs,
            base_epoch: self.base_epoch,
            base_host_epoch: self.base_host_epoch,
        }
    }

    fn load_data(&mut self, state: &Rtc4513SaveData) {
        self.enabled = state.enabled;
        self.phase = RtcPhase::from_u8(state.phase);
        self.command = state.command;
        self.index = state.index & RTC_REG_INDEX_MASK;
        self.regs = state.regs;
        self.base_epoch = state.base_epoch;
        self.base_host_epoch = state.base_host_epoch;
    }
}

impl Decompressor {
    fn new(data_rom_start: usize) -> Self {
        Self {
            context: [[DecompContext {
                prediction: 0,
                swap: 0,
            }; 15]; 5],
            bpp: 1,
            offset: 0,
            bits: 8,
            range: MAX + 1,
            input: 0,
            output: 0,
            pixels: 0,
            colormap: 0xFEDCBA9876543210u64,
            result: 0,
            data_rom_start,
        }
    }

    fn read_byte(&mut self, rom: &[u8]) -> u8 {
        let idx = self.data_rom_start + self.offset as usize;
        self.offset += 1;
        if idx < rom.len() {
            rom[idx]
        } else {
            0
        }
    }

    /// Inverse Morton code transform: unpack big-endian packed pixels.
    fn deinterleave(data: u64, bits: u32) -> u32 {
        let mut d = data & ((1u64 << bits) - 1);
        d = 0x5555555555555555u64 & (d << bits | d >> 1);
        d = 0x3333333333333333u64 & (d | d >> 1);
        d = 0x0F0F0F0F0F0F0F0Fu64 & (d | d >> 2);
        d = 0x00FF00FF00FF00FFu64 & (d | d >> 4);
        d = 0x0000FFFF0000FFFFu64 & (d | d >> 8);
        (d | d >> 16) as u32
    }

    /// Extract a nibble and move it to position 0 (MRU list).
    fn move_to_front(list: u64, nibble: u32) -> u64 {
        let mut n = 0u64;
        let mut mask = !15u64;
        while n < 64 {
            if (list >> n) & 15 == nibble as u64 {
                return (list & mask) + ((list << 4) & !mask) + nibble as u64;
            }
            n += 4;
            mask <<= 4;
        }
        list
    }

    fn initialize(&mut self, mode: u32, origin: u32, rom: &[u8]) {
        for root in self.context.iter_mut() {
            for node in root.iter_mut() {
                *node = DecompContext {
                    prediction: 0,
                    swap: 0,
                };
            }
        }
        self.bpp = 1 << mode;
        self.offset = origin;
        self.bits = 8;
        self.range = MAX + 1;
        let hi = self.read_byte(rom);
        let lo = self.read_byte(rom);
        self.input = (hi as u16) << 8 | lo as u16;
        self.output = 0;
        self.pixels = 0;
        self.colormap = 0xFEDCBA9876543210u64;
    }

    fn decode(&mut self, rom: &[u8]) {
        for pixel in 0..8u32 {
            let mut map = self.colormap;
            let mut diff: u32 = 0;

            if self.bpp > 1 {
                let pa = if self.bpp == 2 {
                    (self.pixels >> 2) & 3
                } else {
                    self.pixels & 15
                } as u32;
                let pb = if self.bpp == 2 {
                    (self.pixels >> 14) & 3
                } else {
                    (self.pixels >> 28) & 15
                } as u32;
                let pc = if self.bpp == 2 {
                    (self.pixels >> 16) & 3
                } else {
                    (self.pixels >> 32) & 15
                } as u32;

                if pa != pb || pb != pc {
                    let mtch = pa ^ pb ^ pc;
                    diff = 4;
                    if (mtch ^ pc) == 0 {
                        diff = 3;
                    }
                    if (mtch ^ pb) == 0 {
                        diff = 2;
                    }
                    if (mtch ^ pa) == 0 {
                        diff = 1;
                    }
                }

                self.colormap = Self::move_to_front(self.colormap, pa);

                map = Self::move_to_front(map, pc);
                map = Self::move_to_front(map, pb);
                map = Self::move_to_front(map, pa);
            }

            for plane in 0..self.bpp {
                let bit = if self.bpp > 1 {
                    1u32 << plane
                } else {
                    1u32 << (pixel & 3)
                };
                let history = (bit - 1) & self.output as u32;
                let mut set: u32 = 0;

                if self.bpp == 1 {
                    set = if pixel >= 4 { 1 } else { 0 };
                }
                if self.bpp == 2 {
                    set = diff;
                }
                if plane >= 2 && history <= 1 {
                    set = diff;
                }

                let ctx_idx = (bit + history - 1) as usize;
                let prediction = self.context[set as usize][ctx_idx].prediction;
                let swap = self.context[set as usize][ctx_idx].swap;
                let model = &EVOLUTION[prediction as usize];
                let lps_offset = self.range.wrapping_sub(model.probability as u16);
                let symbol = if self.input >= (lps_offset << 8) {
                    LPS
                } else {
                    MPS
                };
                let next_pred = model.next[symbol as usize];
                let model_prob = model.probability;

                self.output = (self.output << 1) | ((symbol ^ swap as u32) as u8 & 1);

                if symbol == MPS {
                    self.range = lps_offset;
                } else {
                    self.range -= lps_offset;
                    self.input = self.input.wrapping_sub(lps_offset << 8);
                }

                while self.range <= MAX / 2 {
                    self.context[set as usize][ctx_idx].prediction = next_pred;

                    self.range <<= 1;
                    self.input <<= 1;

                    self.bits -= 1;
                    if self.bits == 0 {
                        self.bits = 8;
                        let idx = self.data_rom_start + self.offset as usize;
                        let byte = if idx < rom.len() { rom[idx] } else { 0 };
                        self.offset += 1;
                        self.input = self.input.wrapping_add(byte as u16);
                    }
                }

                if symbol == LPS && model_prob > HALF as u8 {
                    self.context[set as usize][ctx_idx].swap = swap ^ 1;
                }
            }

            let index = self.output as u32 & ((1 << self.bpp) - 1);
            let index = if self.bpp == 1 {
                index ^ ((self.pixels >> 15) & 1) as u32
            } else {
                index
            };

            self.pixels = (self.pixels << self.bpp as u64) | ((map >> (4 * index)) & 15);
        }

        if self.bpp == 1 {
            self.result = self.pixels as u32;
        } else if self.bpp == 2 {
            self.result = Self::deinterleave(self.pixels, 16);
        } else {
            self.result = Self::deinterleave(Self::deinterleave(self.pixels, 32) as u64, 32);
        }
    }
}

// ---------------------------------------------------------------------------
//  SPC7110 main chip
// ---------------------------------------------------------------------------

pub struct Spc7110 {
    // --- DCU (Decompression Control Unit) ---
    dcu_pending: bool,  // dcuPending: transfer pending after loadAddress
    dcu_mode: u8,       // from directory table (0=1bpp, 1=2bpp, 2=4bpp, 3=invalid)
    dcu_address: u32,   // compressed data address from directory table
    dcu_offset: u32,    // current byte position within tile buffer
    dcu_tile: [u8; 32], // tile output buffer (max 32 bytes for 4bpp)
    decompressor: Decompressor,

    // DCU I/O registers
    r4801: u8,
    r4802: u8,
    r4803: u8, // table address
    r4804: u8, // table index
    r4805: u8,
    r4806: u8, // buffer offset / trigger
    r4807: u8, // seek stride
    r4809: u8,
    r480a: u8, // length counter
    r480b: u8, // control
    r480c: u8, // status

    // --- Data ROM port ---
    data_rom_offset: u32,    // $4811-$4813 (data pointer)
    data_rom_adjust: u16,    // $4814-$4815 (adjust value, modified by reads)
    data_rom_increment: u16, // $4816-$4817
    data_rom_mode: u8,       // $4818
    data_port_ready: u8,     // bitmask: bit0=$4811, bit1=$4812, bit2=$4813 written

    // --- ALU (Math unit) ---
    alu_multiplicand: u16, // $4820-$4821
    alu_multiplier: u16,   // $4822-$4823
    alu_dividend: u32,     // $4820-$4827 (shared low bytes)
    alu_divisor: u16,      // $4826-$4827
    alu_result: u32,       // $4828-$482B
    alu_remainder: u16,    // $482C-$482D
    alu_control: u8,       // $482E
    alu_status: u8,        // $482F

    // --- Bank mapping ---
    pub reg_4830: u8,
    pub bank_d0: u8,
    pub bank_e0: u8,
    pub bank_f0: u8,
    sram_bank: u8,

    // --- ROM layout ---
    data_rom_start: usize,

    // --- RTC-4513 (Far East of Eden Zero) ---
    rtc: Rtc4513,

    // --- Debug ---
    debug_4800_count: u64,
}

impl Spc7110 {
    pub fn new(rom_size: usize) -> Self {
        let data_rom_start = 0x10_0000.min(rom_size);
        Self {
            dcu_pending: false,
            dcu_mode: 0,
            dcu_address: 0,
            dcu_offset: 0,
            dcu_tile: [0; 32],
            decompressor: Decompressor::new(data_rom_start),

            r4801: 0,
            r4802: 0,
            r4803: 0,
            r4804: 0,
            r4805: 0,
            r4806: 0,
            r4807: 0,
            r4809: 0,
            r480a: 0,
            r480b: 0,
            r480c: 0x80, // initially ready

            data_rom_offset: 0,
            data_rom_adjust: 0,
            data_rom_increment: 0,
            data_rom_mode: 0,
            data_port_ready: 0,

            alu_multiplicand: 0,
            alu_multiplier: 0,
            alu_dividend: 0,
            alu_divisor: 0,
            alu_result: 0,
            alu_remainder: 0,
            alu_control: 0,
            alu_status: 0,

            reg_4830: 0,
            bank_d0: 0,
            bank_e0: 1,
            bank_f0: 2,
            sram_bank: 0,

            data_rom_start,
            rtc: Rtc4513::new(),
            debug_4800_count: 0,
        }
    }

    pub fn sram_write_enabled(&self) -> bool {
        (self.reg_4830 & 0x80) != 0
    }

    pub fn save_data(&self) -> Spc7110SaveData {
        let mut decomp_prediction = [[0u8; 15]; 5];
        let mut decomp_swap = [[0u8; 15]; 5];
        for set in 0..5 {
            for node in 0..15 {
                decomp_prediction[set][node] = self.decompressor.context[set][node].prediction;
                decomp_swap[set][node] = self.decompressor.context[set][node].swap;
            }
        }

        Spc7110SaveData {
            dcu_pending: self.dcu_pending,
            dcu_mode: self.dcu_mode,
            dcu_address: self.dcu_address,
            dcu_offset: self.dcu_offset,
            dcu_tile: self.dcu_tile,
            decomp_prediction,
            decomp_swap,
            decomp_bpp: self.decompressor.bpp,
            decomp_offset: self.decompressor.offset,
            decomp_bits: self.decompressor.bits,
            decomp_range: self.decompressor.range,
            decomp_input: self.decompressor.input,
            decomp_output: self.decompressor.output,
            decomp_pixels: self.decompressor.pixels,
            decomp_colormap: self.decompressor.colormap,
            decomp_result: self.decompressor.result,
            decomp_data_rom_start: self.decompressor.data_rom_start,
            r4801: self.r4801,
            r4802: self.r4802,
            r4803: self.r4803,
            r4804: self.r4804,
            r4805: self.r4805,
            r4806: self.r4806,
            r4807: self.r4807,
            r4809: self.r4809,
            r480a: self.r480a,
            r480b: self.r480b,
            r480c: self.r480c,
            data_rom_offset: self.data_rom_offset,
            data_rom_adjust: self.data_rom_adjust,
            data_rom_increment: self.data_rom_increment,
            data_rom_mode: self.data_rom_mode,
            data_port_ready: self.data_port_ready,
            alu_multiplicand: self.alu_multiplicand,
            alu_multiplier: self.alu_multiplier,
            alu_dividend: self.alu_dividend,
            alu_divisor: self.alu_divisor,
            alu_result: self.alu_result,
            alu_remainder: self.alu_remainder,
            alu_control: self.alu_control,
            alu_status: self.alu_status,
            reg_4830: self.reg_4830,
            bank_d0: self.bank_d0,
            bank_e0: self.bank_e0,
            bank_f0: self.bank_f0,
            sram_bank: self.sram_bank,
            data_rom_start: self.data_rom_start,
            rtc: self.rtc.save_data(),
            debug_4800_count: self.debug_4800_count,
        }
    }

    pub fn load_data(&mut self, state: &Spc7110SaveData) {
        self.dcu_pending = state.dcu_pending;
        self.dcu_mode = state.dcu_mode;
        self.dcu_address = state.dcu_address;
        self.dcu_offset = state.dcu_offset;
        self.dcu_tile = state.dcu_tile;
        for set in 0..5 {
            for node in 0..15 {
                self.decompressor.context[set][node].prediction =
                    state.decomp_prediction[set][node];
                self.decompressor.context[set][node].swap = state.decomp_swap[set][node];
            }
        }
        self.decompressor.bpp = state.decomp_bpp;
        self.decompressor.offset = state.decomp_offset;
        self.decompressor.bits = state.decomp_bits;
        self.decompressor.range = state.decomp_range;
        self.decompressor.input = state.decomp_input;
        self.decompressor.output = state.decomp_output;
        self.decompressor.pixels = state.decomp_pixels;
        self.decompressor.colormap = state.decomp_colormap;
        self.decompressor.result = state.decomp_result;
        self.decompressor.data_rom_start = state.decomp_data_rom_start;
        self.r4801 = state.r4801;
        self.r4802 = state.r4802;
        self.r4803 = state.r4803;
        self.r4804 = state.r4804;
        self.r4805 = state.r4805;
        self.r4806 = state.r4806;
        self.r4807 = state.r4807;
        self.r4809 = state.r4809;
        self.r480a = state.r480a;
        self.r480b = state.r480b;
        self.r480c = state.r480c;
        self.data_rom_offset = state.data_rom_offset;
        self.data_rom_adjust = state.data_rom_adjust;
        self.data_rom_increment = state.data_rom_increment;
        self.data_rom_mode = state.data_rom_mode;
        self.data_port_ready = state.data_port_ready;
        self.alu_multiplicand = state.alu_multiplicand;
        self.alu_multiplier = state.alu_multiplier;
        self.alu_dividend = state.alu_dividend;
        self.alu_divisor = state.alu_divisor;
        self.alu_result = state.alu_result;
        self.alu_remainder = state.alu_remainder;
        self.alu_control = state.alu_control;
        self.alu_status = state.alu_status;
        self.reg_4830 = state.reg_4830;
        self.bank_d0 = state.bank_d0;
        self.bank_e0 = state.bank_e0;
        self.bank_f0 = state.bank_f0;
        self.sram_bank = state.sram_bank;
        self.data_rom_start = state.data_rom_start;
        self.rtc.load_data(&state.rtc);
        self.debug_4800_count = state.debug_4800_count;
    }

    pub fn read_register(&mut self, addr: u16, rom: &[u8]) -> u8 {
        let value = self.read_register_inner(addr, rom);
        if std::env::var_os("TRACE_SPC7110").is_some() {
            if addr == 0x4800 {
                self.debug_4800_count += 1;
            } else if addr == 0x4810 || addr == 0x481A {
                // data port reads are too frequent to log individually
            } else {
                println!("[SPC7110] R ${:04X} -> {:02X}", addr, value);
            }
        }
        value
    }

    pub fn debug_drain_4800_count(&mut self) -> u64 {
        let c = self.debug_4800_count;
        self.debug_4800_count = 0;
        c
    }

    fn read_register_inner(&mut self, addr: u16, rom: &[u8]) -> u8 {
        match addr {
            0x4800 => self.dcu_read(rom),
            0x4801 => self.r4801,
            0x4802 => self.r4802,
            0x4803 => self.r4803,
            0x4804 => self.r4804,
            0x4805 => self.r4805,
            0x4806 => self.r4806,
            0x4807 => self.r4807,
            0x4808 => 0x00,
            0x4809 => self.r4809,
            0x480A => self.r480a,
            0x480B => self.r480b,
            0x480C => self.r480c,

            0x4810 => self.data_port_read_4810(rom),
            0x4811 => (self.data_rom_offset & 0xFF) as u8,
            0x4812 => ((self.data_rom_offset >> 8) & 0xFF) as u8,
            0x4813 => ((self.data_rom_offset >> 16) & 0xFF) as u8,
            0x4814 => (self.data_rom_adjust & 0xFF) as u8,
            0x4815 => ((self.data_rom_adjust >> 8) & 0xFF) as u8,
            0x4816 => (self.data_rom_increment & 0xFF) as u8,
            0x4817 => ((self.data_rom_increment >> 8) & 0xFF) as u8,
            0x4818 => self.data_rom_mode,
            0x481A => self.data_port_read_481a(rom),

            0x4820 => (self.alu_multiplicand & 0xFF) as u8,
            0x4821 => ((self.alu_multiplicand >> 8) & 0xFF) as u8,
            0x4822 => (self.alu_multiplier & 0xFF) as u8,
            0x4823 => ((self.alu_multiplier >> 8) & 0xFF) as u8,
            0x4824 => ((self.alu_dividend >> 16) & 0xFF) as u8,
            0x4825 => ((self.alu_dividend >> 24) & 0xFF) as u8,
            0x4826 => (self.alu_divisor & 0xFF) as u8,
            0x4827 => ((self.alu_divisor >> 8) & 0xFF) as u8,
            0x4828 => (self.alu_result & 0xFF) as u8,
            0x4829 => ((self.alu_result >> 8) & 0xFF) as u8,
            0x482A => ((self.alu_result >> 16) & 0xFF) as u8,
            0x482B => ((self.alu_result >> 24) & 0xFF) as u8,
            0x482C => (self.alu_remainder & 0xFF) as u8,
            0x482D => (self.alu_remainder >> 8) as u8,
            0x482E => self.alu_control,
            0x482F => self.alu_status,

            0x4830 => self.reg_4830,
            0x4831 => self.bank_d0,
            0x4832 => self.bank_e0,
            0x4833 => self.bank_f0,
            0x4834 => self.sram_bank,
            0x4840 => self.rtc.read_enable(),
            0x4841 => self.rtc.read_data(),
            0x4842 => self.rtc.read_status(),

            _ => 0x00,
        }
    }

    pub fn write_register(&mut self, addr: u16, value: u8, rom: &[u8]) {
        if std::env::var_os("TRACE_SPC7110").is_some() {
            println!("[SPC7110] W ${:04X} <- {:02X}", addr, value);
        }
        match addr {
            0x4801 => self.r4801 = value,
            0x4802 => self.r4802 = value,
            0x4803 => self.r4803 = value,
            0x4804 => {
                self.r4804 = value;
                self.dcu_load_address(rom);
            }
            0x4805 => self.r4805 = value,
            0x4806 => {
                self.r4806 = value;
                self.dcu_begin_transfer(rom);
            }
            0x4807 => self.r4807 = value,
            0x4808 => {}
            0x4809 => self.r4809 = value,
            0x480A => self.r480a = value,
            0x480B => self.r480b = value & 0x03,
            0x480C..=0x480F => {}

            0x4811 => {
                self.data_port_ready |= 0x01;
                self.data_rom_offset = (self.data_rom_offset & 0xFF_FF00) | value as u32;
            }
            0x4812 => {
                self.data_port_ready |= 0x02;
                self.data_rom_offset = (self.data_rom_offset & 0xFF_00FF) | (value as u32) << 8;
            }
            0x4813 => {
                self.data_port_ready |= 0x04;
                self.data_rom_offset = (self.data_rom_offset & 0x00_FFFF) | (value as u32) << 16;
            }
            0x4814 => {
                self.data_rom_adjust = (self.data_rom_adjust & 0xFF00) | value as u16;
            }
            0x4815 => {
                self.data_rom_adjust = (self.data_rom_adjust & 0x00FF) | (value as u16) << 8;
            }
            0x4816 => {
                self.data_rom_increment = (self.data_rom_increment & 0xFF00) | value as u16;
            }
            0x4817 => {
                self.data_rom_increment = (self.data_rom_increment & 0x00FF) | (value as u16) << 8;
            }
            0x4818 => {
                self.data_rom_mode = value & 0x7F;
                self.data_port_ready = 0; // reset ready flag
            }

            0x4820 => {
                self.alu_multiplicand = (self.alu_multiplicand & 0xFF00) | value as u16;
                self.alu_dividend = (self.alu_dividend & 0xFFFF_FF00) | value as u32;
            }
            0x4821 => {
                self.alu_multiplicand = (self.alu_multiplicand & 0x00FF) | (value as u16) << 8;
                self.alu_dividend = (self.alu_dividend & 0xFFFF_00FF) | (value as u32) << 8;
            }
            0x4822 => {
                self.alu_multiplier = (self.alu_multiplier & 0xFF00) | value as u16;
            }
            0x4823 => {
                self.alu_multiplier = (self.alu_multiplier & 0x00FF) | (value as u16) << 8;
            }
            0x4824 => {
                self.alu_dividend = (self.alu_dividend & 0xFF00_FFFF) | (value as u32) << 16;
            }
            0x4825 => {
                self.alu_dividend = (self.alu_dividend & 0x00FF_FFFF) | (value as u32) << 24;
                self.alu_status |= 0x81;
                self.execute_multiply();
            }
            0x4826 => {
                self.alu_divisor = (self.alu_divisor & 0xFF00) | value as u16;
            }
            0x4827 => {
                self.alu_divisor = (self.alu_divisor & 0x00FF) | (value as u16) << 8;
                self.alu_status |= 0x80;
                self.execute_divide();
            }
            0x482E => self.alu_control = value & 0x01,

            0x4830 => self.reg_4830 = value & 0x87,
            0x4831 => self.bank_d0 = value & 0x07,
            0x4832 => self.bank_e0 = value & 0x07,
            0x4833 => self.bank_f0 = value & 0x07,
            0x4834 => self.sram_bank = value & 0x07,
            0x4840 => self.rtc.write_enable(value),
            0x4841 => self.rtc.write_data(value),
            0x4842 => {}

            _ => {}
        }
    }

    // ------------------------------------------------------------------
    //  DCU — on-demand decompression (ares-compatible)
    // ------------------------------------------------------------------

    /// dcuLoadAddress: triggered by $4804 write.
    fn dcu_load_address(&mut self, rom: &[u8]) {
        let table: u32 = self.r4801 as u32 | (self.r4802 as u32) << 8 | (self.r4803 as u32) << 16;
        let index: u32 = (self.r4804 as u32) << 2;
        let address = table.wrapping_add(index);

        self.dcu_mode = self.read_data_byte(address, rom);
        self.dcu_address = (self.read_data_byte(address + 1, rom) as u32) << 16
            | (self.read_data_byte(address + 2, rom) as u32) << 8
            | self.read_data_byte(address + 3, rom) as u32;

        if std::env::var_os("TRACE_SPC7110").is_some() {
            println!(
                "[SPC7110] dcuLoadAddress: table={:06X} idx={} mode={} addr={:06X}",
                table, self.r4804, self.dcu_mode, self.dcu_address
            );
        }
    }

    /// dcuBeginTransfer: triggered by $4806 write.
    fn dcu_begin_transfer(&mut self, rom: &[u8]) {
        if self.dcu_mode == 3 {
            return; // invalid mode
        }

        self.decompressor
            .initialize(self.dcu_mode as u32, self.dcu_address, rom);
        self.decompressor.decode(rom);

        // Seek forward if hardware skip enabled ($480B bit 1)
        let seek: u16 = if (self.r480b & 2) != 0 {
            self.r4805 as u16 | (self.r4806 as u16) << 8
        } else {
            0
        };
        for _ in 0..seek {
            self.decompressor.decode(rom);
        }

        self.r480c |= 0x80; // ready
        self.dcu_offset = 0;

        if std::env::var_os("TRACE_SPC7110").is_some() {
            println!(
                "[SPC7110] dcuBeginTransfer: mode={} addr={:06X} seek={} bpp={}",
                self.dcu_mode, self.dcu_address, seek, self.decompressor.bpp
            );
        }
    }

    /// dcuRead: returns one byte of decompressed tile data on each $4800 read.
    /// Fills the tile buffer on-demand when dcu_offset wraps to 0.
    fn dcu_read(&mut self, rom: &[u8]) -> u8 {
        if (self.r480c & 0x80) == 0 {
            return 0x00;
        }

        if self.dcu_offset == 0 {
            // Fill tile buffer: 8 rows of pixel data
            for row in 0..8u32 {
                let result = self.decompressor.result;
                match self.decompressor.bpp {
                    1 => {
                        self.dcu_tile[row as usize] = result as u8;
                    }
                    2 => {
                        self.dcu_tile[row as usize * 2] = result as u8;
                        self.dcu_tile[row as usize * 2 + 1] = (result >> 8) as u8;
                    }
                    4 => {
                        self.dcu_tile[row as usize * 2] = result as u8;
                        self.dcu_tile[row as usize * 2 + 1] = (result >> 8) as u8;
                        self.dcu_tile[row as usize * 2 + 16] = (result >> 16) as u8;
                        self.dcu_tile[row as usize * 2 + 17] = (result >> 24) as u8;
                    }
                    _ => {}
                }

                let seek: u8 = if (self.r480b & 1) != 0 { self.r4807 } else { 1 };
                for _ in 0..seek {
                    self.decompressor.decode(rom);
                }
            }
        }

        let data = self.dcu_tile[self.dcu_offset as usize];
        self.dcu_offset += 1;
        self.dcu_offset &= 8 * self.decompressor.bpp - 1;
        data
    }

    // ------------------------------------------------------------------
    //  Data ROM port
    // ------------------------------------------------------------------

    fn read_data_byte(&self, offset: u32, rom: &[u8]) -> u8 {
        let idx = self.data_rom_start + offset as usize;
        if idx < rom.len() {
            rom[idx]
        } else {
            0xFF
        }
    }

    // ---- bsnes-compatible data port read ($4810) ----
    // Mode register $4818 bits:
    //   bit 0: use data_increment (1) or fixed +1 (0) in pointer mode
    //   bit 1: adjust mode (1) vs pointer mode (0)
    //   bit 2: sign-extend increment as i16
    //   bit 3: sign-extend adjust as i16
    //   bit 4: increment target — pointer (0) or adjust (1)
    fn data_port_read_4810(&mut self, rom: &[u8]) -> u8 {
        if self.data_port_ready != 0x07 {
            return 0x00;
        }

        let addr = self.data_rom_offset;
        let adjust = if (self.data_rom_mode & 0x08) != 0 {
            self.data_rom_adjust as i16 as u32
        } else {
            self.data_rom_adjust as u32
        };

        let read_addr;
        if (self.data_rom_mode & 0x02) != 0 {
            // Adjust mode: read at pointer + adjust, then adjust++
            read_addr = addr.wrapping_add(adjust);
            self.data_rom_adjust = self.data_rom_adjust.wrapping_add(1);
        } else {
            // Pointer mode: read at pointer, then advance
            read_addr = addr;

            let increment = if (self.data_rom_mode & 0x01) != 0 {
                self.data_rom_increment as u32
            } else {
                1u32
            };
            let increment = if (self.data_rom_mode & 0x04) != 0 {
                increment as u16 as i16 as u32
            } else {
                increment
            };

            if (self.data_rom_mode & 0x10) == 0 {
                self.data_rom_offset = addr.wrapping_add(increment);
            } else {
                self.data_rom_adjust = (self.data_rom_adjust as u32).wrapping_add(increment) as u16;
            }
        }

        let rom_addr = self.data_rom_start + (read_addr & 0xFF_FFFF) as usize;
        if rom_addr < rom.len() {
            rom[rom_addr]
        } else {
            0xFF
        }
    }

    // ---- bsnes-compatible data port read ($481A) ----
    // Always reads at pointer + adjust; increment logic same as pointer mode.
    fn data_port_read_481a(&mut self, rom: &[u8]) -> u8 {
        if self.data_port_ready != 0x07 {
            return 0x00;
        }

        let addr = self.data_rom_offset;
        let adjust = if (self.data_rom_mode & 0x08) != 0 {
            self.data_rom_adjust as i16 as u32
        } else {
            self.data_rom_adjust as u32
        };
        let read_addr = addr.wrapping_add(adjust);

        if (self.data_rom_mode & 0x02) == 0 {
            // Pointer mode increment
            let increment = if (self.data_rom_mode & 0x01) != 0 {
                self.data_rom_increment as u32
            } else {
                1u32
            };
            let increment = if (self.data_rom_mode & 0x04) != 0 {
                increment as u16 as i16 as u32
            } else {
                increment
            };

            if (self.data_rom_mode & 0x10) == 0 {
                self.data_rom_offset = addr.wrapping_add(increment);
            } else {
                self.data_rom_adjust = (self.data_rom_adjust as u32).wrapping_add(increment) as u16;
            }
        }

        let rom_addr = self.data_rom_start + (read_addr & 0xFF_FFFF) as usize;
        if rom_addr < rom.len() {
            rom[rom_addr]
        } else {
            0xFF
        }
    }

    // ------------------------------------------------------------------
    //  ALU
    // ------------------------------------------------------------------

    fn execute_multiply(&mut self) {
        if (self.alu_control & 0x01) != 0 {
            let a = self.alu_multiplicand as i16 as i32;
            let b = self.alu_multiplier as i16 as i32;
            self.alu_result = (a * b) as u32;
        } else {
            self.alu_result = self.alu_multiplicand as u32 * self.alu_multiplier as u32;
        }
        self.alu_remainder = 0;
        self.alu_status &= !0x81;
    }

    fn execute_divide(&mut self) {
        if self.alu_divisor == 0 {
            self.alu_result = 0;
            self.alu_remainder = self.alu_dividend as u16;
        } else if (self.alu_control & 0x01) != 0 {
            let a = self.alu_dividend as i32;
            let b = self.alu_divisor as i16 as i32;
            self.alu_result = (a / b) as u32;
            self.alu_remainder = (a % b) as u16;
        } else {
            let a = self.alu_dividend;
            let b = self.alu_divisor as u32;
            self.alu_result = a / b;
            self.alu_remainder = (a % b) as u16;
        }
        self.alu_status &= !0x80;
    }

    // ------------------------------------------------------------------
    //  Extended bank mapping ($C0-$FF)
    // ------------------------------------------------------------------

    pub fn read_bank_c0_ff(&self, bank: u8, offset: u16, rom: &[u8], rom_size: usize) -> u8 {
        match bank {
            0xC0..=0xCF => {
                let rom_addr = ((bank as usize) & 0x0F) * 0x10000 + offset as usize;
                if rom_size == 0 {
                    0xFF
                } else {
                    rom[rom_addr % rom_size]
                }
            }
            0xD0..=0xDF => {
                let pack = self.bank_d0 as usize;
                let local_bank = (bank as usize) & 0x0F;
                let rom_addr =
                    self.data_rom_start + pack * 0x10_0000 + local_bank * 0x10000 + offset as usize;
                if rom_addr < rom.len() {
                    rom[rom_addr]
                } else {
                    0xFF
                }
            }
            0xE0..=0xEF => {
                let pack = self.bank_e0 as usize;
                let local_bank = (bank as usize) & 0x0F;
                let rom_addr =
                    self.data_rom_start + pack * 0x10_0000 + local_bank * 0x10000 + offset as usize;
                if rom_addr < rom.len() {
                    rom[rom_addr]
                } else {
                    0xFF
                }
            }
            0xF0..=0xFF => {
                let pack = self.bank_f0 as usize;
                let local_bank = (bank as usize) & 0x0F;
                let rom_addr =
                    self.data_rom_start + pack * 0x10_0000 + local_bank * 0x10000 + offset as usize;
                if rom_addr < rom.len() {
                    rom[rom_addr]
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
    use super::{Rtc4513, Spc7110, RTC_CTRL_24H, RTC_CTRL_STOP};

    #[test]
    fn rtc_sequential_write_and_read_round_trips_stopped_time() {
        let mut spc = Spc7110::new(0x20_0000);
        let rom = [];

        spc.write_register(0x4840, 0x01, &rom);
        spc.write_register(0x4841, 0x03, &rom);
        spc.write_register(0x4841, 0x0F, &rom);
        spc.write_register(0x4841, RTC_CTRL_24H | RTC_CTRL_STOP, &rom);
        spc.write_register(0x4841, 0x05, &rom); // reg00 seconds low
        spc.write_register(0x4841, 0x04, &rom); // reg01 seconds high
        spc.write_register(0x4841, 0x03, &rom); // reg02 minutes low
        spc.write_register(0x4841, 0x02, &rom); // reg03 minutes high
        spc.write_register(0x4841, 0x01, &rom); // reg04 hours low
        spc.write_register(0x4841, 0x01, &rom); // reg05 hours high
        spc.write_register(0x4841, 0x07, &rom); // reg06 day low
        spc.write_register(0x4841, 0x1, &rom); // reg07 day high
        spc.write_register(0x4841, 0x8, &rom); // reg08 month low
        spc.write_register(0x4841, 0x0, &rom); // reg09 month high
        spc.write_register(0x4841, 0x6, &rom); // reg0A year low
        spc.write_register(0x4841, 0x2, &rom); // reg0B year high
        spc.write_register(0x4841, 0x2, &rom); // reg0C weekday

        spc.write_register(0x4840, 0x00, &rom);
        spc.write_register(0x4840, 0x01, &rom);
        spc.write_register(0x4841, 0x03, &rom);
        spc.write_register(0x4841, 0x00, &rom);

        let expected = [5, 4, 3, 2, 1, 1, 7, 1, 8, 0, 6, 2, 1];
        let mut actual = [0u8; 13];
        for value in &mut actual {
            *value = spc.read_register(0x4841, &rom);
        }

        assert_eq!(actual, expected);
        assert_eq!(spc.read_register(0x4842, &rom), 0x80);
    }

    #[test]
    fn rtc_epoch_conversion_round_trips_24_hour_time() {
        let mut regs = [0u8; 16];
        regs[0x00] = 9;
        regs[0x01] = 5;
        regs[0x02] = 8;
        regs[0x03] = 5;
        regs[0x04] = 3;
        regs[0x05] = 2;
        regs[0x06] = 1;
        regs[0x07] = 1;
        regs[0x08] = 2;
        regs[0x09] = 0;
        regs[0x0A] = 6;
        regs[0x0B] = 2;

        let epoch = Rtc4513::epoch_from_regs_with_format(&regs, true);
        let round_trip = Rtc4513::time_regs_from_epoch(epoch, true);

        assert_eq!(&round_trip[..0x0C], &regs[..0x0C]);
    }
}
