pub struct MMU {
    pub rom: Vec<u8>,         // The game file
    pub vram: [u8; 0x2000],    // 8KB Video RAM (0x8000 - 0x9FFF)
    pub oam: [u8; 0xA0],       // 160 bytes Object Attribute Memory (0xFE00 - 0xFE9F)
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
    
    // LCD Registers
    pub lcdc: u8, // 0xFF40
    pub stat: u8, // 0xFF41
    pub scy: u8,  // 0xFF42 (Scroll Y)
    pub scx: u8,  // 0xFF43 (Scroll X)
    pub ly: u8,   // 0xFF44 (Current Scanline)
    pub lyc: u8,  // 0xFF45 (LY Compare)
    pub bgp: u8,  // 0xFF47 (Background Palette)
    pub obp0: u8, // 0xFF48
    pub obp1: u8, // 0xFF49
    pub wy: u8,   // 0xFF4A (Window Y)
    pub wx: u8,   // 0xFF4B (Window X)
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
            vram: [0; 0x2000],
            oam: [0; 0xA0],
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
            // LCD Registers (DMG Power Up Values)
            lcdc: 0x91, // LCD Enabled, BG Display Enabled, etc.
            stat: 0x85, // Mode 1 (V-Blank) usually on startup
            scy: 0x00,
            scx: 0x00,
            ly: 0x00,   // Reset scanline to 0
            lyc: 0x00,
            bgp: 0xFC,  // Standard palette (11 11 11 00 in binary)
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0x00,
            wx: 0x00,
        }
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        match addr {

            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize], // Video RAM
        0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],  // Sprite Info

        // I/O Registers
        0xFF40 => self.lcdc,
        0xFF41 => self.stat | 0x80, // Bit 7 is always 1
        0xFF42 => self.scy,
        0xFF43 => self.scx,
        0xFF44 => self.ly,
        0xFF45 => self.lyc,
        0xFF47 => self.bgp,
        0xFF48 => self.obp0,
        0xFF49 => self.obp1,
        0xFF4A => self.wy,
        0xFF4B => self.wx,
        // ------------------------------

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

            // --- ADD THESE PPU MAPPINGS ---
        0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
        0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = val,
        
        0xFF40 => self.lcdc = val,
        0xFF41 => self.stat = (val & 0xF8) | (self.stat & 0x07), // Only bits 3-6 writable
        0xFF42 => self.scy = val,
        0xFF43 => self.scx = val,
        0xFF44 => {}, // LY is Read Only!
        0xFF45 => self.lyc = val,
        0xFF47 => self.bgp = val,
        0xFF48 => self.obp0 = val,
        0xFF49 => self.obp1 = val,
        0xFF4A => self.wy = val,
        0xFF4B => self.wx = val,

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
        0xFF02 => {
            if val == 0x81 {
                
            // This is the "Start Transfer" flag. In a real GB, this triggers the link.
            // We can ignore the actual transfer logic for now.
        
        }
    }
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            _ => {} // Ignore writes to ROM or unmapped areas for now
        }
    }
}