pub struct Memory {
    pub(crate) ram: [u8; 0x800],
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Memory {
    pub fn new() -> Self {
        Memory { ram: [0; 0x800] }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x7FF) as usize],
            _ => 0,
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        if let 0x0000..=0x1FFF = addr {
            self.ram[(addr & 0x7FF) as usize] = data;
        }
    }

    // Save state methods
    pub fn get_ram(&self) -> [u8; 0x800] {
        self.ram
    }

    pub fn set_ram(&mut self, ram: [u8; 0x800]) {
        self.ram = ram;
    }
}
