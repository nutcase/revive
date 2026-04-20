use std::time::SystemTime;

const RAM_BANK_SIZE: usize = 8 * 1024;

#[derive(Debug, Clone)]
pub struct Mbc3State {
    rom_bank: u8,
    ram_or_rtc_select: u8,
    ram_enabled: bool,
    has_rtc: bool,
    latch_armed: bool,
    latched_valid: bool,
    rtc: Mbc3Rtc,
    latched_rtc: Mbc3Rtc,
}

#[derive(Debug, Clone)]
struct Mbc3Rtc {
    seconds: u8,
    minutes: u8,
    hours: u8,
    day_counter: u16,
    halt: bool,
    carry: bool,
    last_update: SystemTime,
}

impl Mbc3State {
    pub fn new(has_rtc: bool) -> Self {
        let rtc = Mbc3Rtc::new();
        Self {
            rom_bank: 1,
            ram_or_rtc_select: 0,
            ram_enabled: false,
            has_rtc,
            latch_armed: false,
            latched_valid: false,
            rtc: rtc.clone(),
            latched_rtc: rtc,
        }
    }

    pub fn write_rom_control(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                let mut bank = value & 0x7F;
                if bank == 0 {
                    bank = 1;
                }
                self.rom_bank = bank;
            }
            0x4000..=0x5FFF => {
                self.ram_or_rtc_select = value;
            }
            0x6000..=0x7FFF => {
                let bit0 = value & 0x01;
                if bit0 == 0 {
                    self.latch_armed = true;
                } else if self.latch_armed {
                    self.latch_clock();
                    self.latch_armed = false;
                }
            }
            _ => {}
        }
    }

    pub fn current_rom_bank(&self, bank_count: usize) -> usize {
        (self.rom_bank as usize) % bank_count.max(1)
    }

    pub fn read_ram(&self, ram: &[u8], addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }

        let offset = (usize::from(addr).wrapping_sub(0xA000)) % RAM_BANK_SIZE;
        match self.ram_or_rtc_select {
            0x00..=0x03 => {
                if ram.is_empty() {
                    return 0xFF;
                }
                let bank = self.ram_or_rtc_select as usize;
                let index = bank.saturating_mul(RAM_BANK_SIZE).saturating_add(offset) % ram.len();
                ram[index]
            }
            0x08..=0x0C => self.read_rtc_reg(self.ram_or_rtc_select),
            _ => 0xFF,
        }
    }

    pub fn write_ram(&mut self, ram: &mut [u8], addr: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        let offset = (usize::from(addr).wrapping_sub(0xA000)) % RAM_BANK_SIZE;
        match self.ram_or_rtc_select {
            0x00..=0x03 => {
                if ram.is_empty() {
                    return;
                }
                let bank = self.ram_or_rtc_select as usize;
                let index = bank.saturating_mul(RAM_BANK_SIZE).saturating_add(offset) % ram.len();
                ram[index] = value;
            }
            0x08..=0x0C => self.write_rtc_reg(self.ram_or_rtc_select, value),
            _ => {}
        }
    }

    fn latch_clock(&mut self) {
        if !self.has_rtc {
            return;
        }
        self.rtc.tick_to_now();
        self.latched_rtc = self.rtc.clone();
        self.latched_valid = true;
    }

    fn read_rtc_reg(&self, select: u8) -> u8 {
        if !self.has_rtc {
            return 0xFF;
        }
        if self.latched_valid {
            self.latched_rtc.read_reg(select)
        } else {
            self.rtc.read_reg(select)
        }
    }

    fn write_rtc_reg(&mut self, select: u8, value: u8) {
        if !self.has_rtc {
            return;
        }
        self.rtc.tick_to_now();
        self.rtc.write_reg(select, value);
        if self.latched_valid {
            self.latched_rtc.write_reg(select, value);
        }
    }
}

impl Mbc3Rtc {
    fn new() -> Self {
        Self {
            seconds: 0,
            minutes: 0,
            hours: 0,
            day_counter: 0,
            halt: false,
            carry: false,
            last_update: SystemTime::now(),
        }
    }

    fn tick_to_now(&mut self) {
        if self.halt {
            self.last_update = SystemTime::now();
            return;
        }
        let now = SystemTime::now();
        let Ok(elapsed) = now.duration_since(self.last_update) else {
            self.last_update = now;
            return;
        };
        let secs = elapsed.as_secs();
        if secs > 0 {
            self.advance_seconds(secs);
            self.last_update = now;
        }
    }

    fn advance_seconds(&mut self, seconds: u64) {
        let mut total = u64::from(self.seconds) + seconds;
        self.seconds = (total % 60) as u8;
        total /= 60;

        total += u64::from(self.minutes);
        self.minutes = (total % 60) as u8;
        total /= 60;

        total += u64::from(self.hours);
        self.hours = (total % 24) as u8;
        total /= 24;

        if total > 0 {
            let days = total as u16;
            let next = self.day_counter.wrapping_add(days);
            if next > 511 {
                self.carry = true;
            }
            self.day_counter = next & 0x01FF;
        }
    }

    fn read_reg(&self, select: u8) -> u8 {
        match select {
            0x08 => self.seconds,
            0x09 => self.minutes,
            0x0A => self.hours,
            0x0B => (self.day_counter & 0xFF) as u8,
            0x0C => {
                ((self.day_counter >> 8) as u8 & 0x01)
                    | (u8::from(self.halt) << 6)
                    | (u8::from(self.carry) << 7)
            }
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, select: u8, value: u8) {
        match select {
            0x08 => self.seconds = value % 60,
            0x09 => self.minutes = value % 60,
            0x0A => self.hours = value % 24,
            0x0B => {
                self.day_counter = (self.day_counter & 0x0100) | u16::from(value);
            }
            0x0C => {
                self.day_counter = (self.day_counter & 0x00FF) | (u16::from(value & 0x01) << 8);
                self.halt = (value & 0x40) != 0;
                self.carry = (value & 0x80) != 0;
                self.last_update = SystemTime::now();
            }
            _ => {}
        }
    }
}
