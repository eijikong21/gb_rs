pub struct MMU {
    pub rom: Vec<u8>,         // The game file
    pub wram: [u8; 0x2000],    // Work RAM (8KB)
    pub hram: [u8; 0x7F],      // High RAM (127 bytes)
    pub interrupt_flag: u8,   // IF (0xFF0F)
    pub interrupt_enable: u8, // IE (0xFFFF)
    pub div: u8,           // 0xFF04
    pub tima: u8,          // 0xFF05
    pub tma: u8,           // 0xFF06
    pub tac: u8,           // 0xFF07
    pub div_counter: u16,  // Internal counter to track DIV timing
    pub tima_counter: u32, // Internal counter to track TIMA timing
    
}

impl MMU {
    pub fn tick(&mut self, cycles: u8) {
        // 1. DIV logic: Increments at 16384Hz (every 256 cycles)
        self.div_counter = self.div_counter.wrapping_add(cycles as u16);
        self.div = (self.div_counter >> 8) as u8;

        // 2. TIMA logic: Only runs if TAC bit 2 is set
        if (self.tac & 0x04) != 0 {
            self.tima_counter += cycles as u32;

            let threshold = match self.tac & 0x03 {
                0x00 => 1024, // 4096 Hz
                0x01 => 16,   // 262144 Hz
                0x02 => 64,   // 65536 Hz
                0x03 => 256,  // 16384 Hz
                _ => 1024,
            };

            while self.tima_counter >= threshold {
                self.tima_counter -= threshold;

                if self.tima == 0xFF {
                    self.tima = self.tma; // Reset to Modulo
                    self.interrupt_flag |= 0x04; // Request Timer Interrupt (Bit 2)
                } else {
                    self.tima += 1;
                }
            }
        }
    }
    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            rom,
            wram: [0; 0x2000],
            hram: [0; 0x7F],
            interrupt_flag: 0xE1,   // Default: Top bits 1, V-Blank bit often 1 at start
            interrupt_enable: 0x00, // Disabled by default
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            div_counter: 0,
            tima_counter: 0,
        }
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => self.div,
        0xFF05 => self.tima,
        0xFF06 => self.tma,
        0xFF07 => self.tac | 0xF8, // Top 5 bits are unused and usually read as 1
            0xFF0F => self.interrupt_flag | 0xE0,
        0xFFFF => self.interrupt_enable,
            0x0000..=0x7FFF => self.rom[addr as usize],
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            _ => 0xFF, // Return "empty" bus value
        }
    }

    pub fn write_byte(&mut self, addr: u16, val: u8) {
        match addr {
             0xFF0F => self.interrupt_flag = val | 0xE0, // Top 3 bits always read 1
        0xFFFF => self.interrupt_enable = val,
            0xFF01 => {
            // When a byte is written here, it's intended for the link cable.
            // For now, we just print it to our console so we can read Blargg's messages!
            print!("{}", val as char);
        }
        0xFF04 => {self.div = 0;
                self.div_counter =0;
            }, // Any write to DIV resets it to 0
    0xFF05 => self.tima = val,
    0xFF06 => self.tma = val,
    0xFF07 => self.tac = val & 0x07,
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