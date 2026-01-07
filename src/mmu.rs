pub struct MMU {
    pub rom: Vec<u8>,         // The game file
    pub wram: [u8; 0x2000],    // Work RAM (8KB)
    pub hram: [u8; 0x7F],      // High RAM (127 bytes)
}

impl MMU {
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            wram: [0; 0x2000],
            hram: [0; 0x7F],
        }
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.rom[addr as usize],
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            _ => 0xFF, // Return "empty" bus value
        }
    }

    pub fn write_byte(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF01 => {
            // When a byte is written here, it's intended for the link cable.
            // For now, we just print it to our console so we can read Blargg's messages!
            print!("{}", val as char);
        }
        0xFF02 if val == 0x81 => {
            // This is the "Start Transfer" flag. In a real GB, this triggers the link.
            // We can ignore the actual transfer logic for now.
        }
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            _ => {} // Ignore writes to ROM or unmapped areas for now
        }
    }
}