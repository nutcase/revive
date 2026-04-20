use super::MapperSpec;

const CPU_IRQ_RAM_MAPPERS: &[u16] = &[18, 21, 23, 24, 25, 26, 68, 240, 241, 245];
const TINY_PRG_RAM_MAPPERS: &[u16] = &[80, 207];
const STANDARD_PRG_RAM_MAPPERS: &[u16] = &[1, 4, 9, 10, 15, 16, 32, 69, 73, 74, 118, 119, 192, 194];

impl MapperSpec {
    pub(in crate::cartridge::load) fn prg_ram_init(self) -> Option<(usize, u8)> {
        let explicit_size = self.explicit_prg_ram_size();
        let size = if explicit_size > 0 {
            explicit_size
        } else if self.mapper == 16 && self.has_battery {
            return Some((256, 0xFF));
        } else if self.mapper == 159 && self.has_battery {
            return Some((128, 0xFF));
        } else if self.mapper == 99 {
            0x0800
        } else if self.mapper == 5 {
            0x20000
        } else if self.mapper == 19 {
            0x2080
        } else if self.mapper == 210 && self.has_battery {
            0x0800
        } else if CPU_IRQ_RAM_MAPPERS.contains(&self.mapper)
            || (self.mapper == 227 && self.has_battery)
        {
            8192
        } else if TINY_PRG_RAM_MAPPERS.contains(&self.mapper) {
            128
        } else if self.mapper == 82 {
            0x1400
        } else if self.mapper == 246 {
            2048
        } else if self.mapper == 225 {
            4
        } else if self.mapper == 103 {
            0x2000
        } else if self.mapper == 153 {
            0x8000
        } else if STANDARD_PRG_RAM_MAPPERS.contains(&self.mapper) || self.mapper34_nina001 {
            8192
        } else {
            return None;
        };
        Some((size, 0x00))
    }

    pub(in crate::cartridge::load) fn initial_prg_bank(self) -> u8 {
        if self.mapper == 208 {
            3
        } else {
            0
        }
    }
}
