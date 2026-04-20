use crate::bus::{Bus, PAGE_SIZE};

pub(super) const HUCARD_HEADER_SIZE: usize = 512;
pub(super) const HUCARD_MAGIC_LO: u8 = 0xAA;
pub(super) const HUCARD_MAGIC_HI: u8 = 0xBB;
pub(super) const HUCARD_TYPE_PCE: u8 = 0x02;
pub(super) const RESET_VECTOR_PRIMARY: u16 = 0xFFFE;
pub(super) const RESET_VECTOR_LEGACY: u16 = 0xFFFC;
pub(super) const NUM_HUCARD_WINDOW_BANKS: usize = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct HucardHeader {
    rom_pages: u16,
    pub(super) flags: u8,
}

impl HucardHeader {
    pub(super) fn parse(image: &[u8]) -> Option<Self> {
        if image.len() < HUCARD_HEADER_SIZE {
            return None;
        }
        let header = &image[..HUCARD_HEADER_SIZE];
        if header[8] != HUCARD_MAGIC_LO || header[9] != HUCARD_MAGIC_HI {
            return None;
        }
        if header[10] != HUCARD_TYPE_PCE {
            return None;
        }
        let rom_pages = u16::from_le_bytes([header[0], header[1]]);
        if rom_pages == 0 {
            return None;
        }
        let flags = header[2];
        Some(Self { rom_pages, flags })
    }

    pub(super) fn backup_ram_bytes(&self) -> usize {
        match (self.flags >> 2) & 0x03 {
            0 => 0,
            1 => 16 * 1024,
            2 => 64 * 1024,
            _ => 256 * 1024,
        }
    }

    pub(super) fn recommends_mode0(&self) -> bool {
        self.flags & 0x80 != 0
    }

    pub(super) fn uses_reset_vector(&self) -> bool {
        self.flags & 0x02 != 0
    }

    pub(super) fn recommended_layout(
        &self,
        pages: usize,
    ) -> Option<[usize; NUM_HUCARD_WINDOW_BANKS]> {
        if pages == 0 {
            return None;
        }
        let mut layout = [0; NUM_HUCARD_WINDOW_BANKS];
        if self.recommends_mode0() {
            for (slot, bank) in layout.iter_mut().enumerate() {
                *bank = slot % pages;
            }
        } else {
            let start = pages.saturating_sub(NUM_HUCARD_WINDOW_BANKS);
            for (slot, bank) in layout.iter_mut().enumerate() {
                *bank = (start + slot) % pages;
            }
        }
        Some(layout)
    }

    pub(super) fn rom_size_bytes(&self) -> usize {
        self.rom_pages as usize * PAGE_SIZE
    }
}

pub(crate) struct ParsedHuCard {
    pub(super) rom: Vec<u8>,
    pub(super) header: Option<HucardHeader>,
}

impl ParsedHuCard {
    pub(super) fn from_bytes(image: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(header) = HucardHeader::parse(image) {
            let mut rom = image[HUCARD_HEADER_SIZE..].to_vec();
            let expected = header.rom_size_bytes();
            if expected == 0 {
                return Err("HuCard header reports empty ROM".into());
            }
            if rom.len() < expected {
                rom.resize(expected, 0xFF);
            } else if rom.len() > expected {
                rom.truncate(expected);
            }
            if rom.is_empty() {
                return Err("HuCard payload is empty".into());
            }
            Ok(Self {
                rom,
                header: Some(header),
            })
        } else {
            if image.is_empty() {
                return Err("HuCard image is empty".into());
            }
            let mut rom = image.to_vec();
            let remainder = rom.len() % PAGE_SIZE;
            if remainder != 0 {
                rom.resize(rom.len() + (PAGE_SIZE - remainder), 0xFF);
            }
            if rom.is_empty() {
                return Err("HuCard payload is empty".into());
            }
            Ok(Self { rom, header: None })
        }
    }
}

pub(super) fn is_valid_reset_vector(vector: u16) -> bool {
    (0x8000..=0xFFFD).contains(&vector) && vector != 0xFFFF
}

pub(super) fn read_reset_vector(bus: &mut Bus) -> u16 {
    let primary = bus.read_u16(RESET_VECTOR_PRIMARY);
    if primary != 0x0000 && primary != 0xFFFF {
        return primary;
    }

    let fallback = bus.read_u16(RESET_VECTOR_LEGACY);
    if fallback != 0x0000 && fallback != 0xFFFF {
        fallback
    } else {
        primary
    }
}

use super::Emulator;

impl Emulator {
    pub(crate) fn apply_header_layout(
        &mut self,
        layout: &[usize; NUM_HUCARD_WINDOW_BANKS],
        header: &HucardHeader,
    ) -> bool {
        for (slot, bank) in layout.iter().enumerate() {
            self.bus.map_bank_to_rom(4 + slot, *bank);
        }
        let vector = read_reset_vector(&mut self.bus);
        if header.uses_reset_vector() {
            is_valid_reset_vector(vector)
        } else if header.recommends_mode0() {
            vector >= 0x8000 && vector != 0xFFFF
        } else {
            vector != 0 && vector != 0xFFFF
        }
    }

    pub(crate) fn map_boot_window(&mut self, pages: usize) {
        if pages == 0 {
            return;
        }

        let mut reset_bank = None;
        for bank in 0..pages {
            self.bus.map_bank_to_rom(7, bank);
            let vector = read_reset_vector(&mut self.bus);
            if is_valid_reset_vector(vector) {
                reset_bank = Some(bank);
                break;
            }
        }

        let reset_bank = reset_bank.unwrap_or_else(|| pages.saturating_sub(1));
        let base = (reset_bank + pages + 1 - NUM_HUCARD_WINDOW_BANKS) % pages;
        for slot in 0..NUM_HUCARD_WINDOW_BANKS {
            let rom_bank = (base + slot) % pages;
            let mpr_slot = 4 + slot;
            self.bus.map_bank_to_rom(mpr_slot, rom_bank);
        }
    }

    pub(crate) fn seed_cpu_stack(&mut self) {
        let reset_pc = read_reset_vector(&mut self.bus);
        if self.bus.read(reset_pc) != 0x40 {
            return;
        }

        let mut entry = self.bus.read_u16(0xFFF8);
        if !is_valid_reset_vector(entry) || self.bus.read(entry) == 0x00 {
            entry = reset_pc.wrapping_add(1);
        }
        let (pcl, pch) = (entry as u8, (entry >> 8) as u8);

        let status = crate::cpu::FLAG_INTERRUPT_DISABLE;

        let vdc_status = self.bus.read_io(0x00);
        self.bus.write(0x0000, vdc_status);
        self.bus.write(0x0028, vdc_status);

        self.bus.write(0x01FA, status);
        self.bus.write(0x01FB, pcl);
        self.bus.write(0x01FC, pch);
        self.cpu.sp = 0xF9;
    }
}
