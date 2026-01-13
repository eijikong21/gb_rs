use std::fs;
use crate::apu;
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

    pub joypad_state: u8, // We'll store: 7:Start, 6:Select, 5:B, 4:A, 3:Down, 2:Up, 1:Left, 0:Right
    pub joyp_sel: u8,     // Stores what the game wrote to 0xFF00 (bits 4 and 5)


    pub rom_bank: u16,   // Currently selected ROM bank (1-127)
    pub ram_enabled: bool,
    pub mode: u8,           // 0 = ROM banking mode, 1 = RAM banking mode
    pub ram_bank: u8,
    pub eram: [u8; 0x8000], // 32KB of External RAM (4 banks of 8KB)

    pub rtc_registers: [u8; 5], // 08:Sec, 09:Min, 0A:Hour, 0B:DayL, 0C:DayH
    pub rtc_sel: u8,            // Currently selected RTC register
    pub mbc_type: u8, // Read from ROM index 0x0147
    pub save_filename: String,
    pub save_dirty: bool,
    pub apu: apu::APU,
}
impl MMU {
       pub fn has_save_data(&self) -> bool {
        self.eram.iter().any(|&byte| byte != 0)
    }
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
    pub fn new(rom: Vec<u8>, rom_filename:&str) -> Self {
    let mbc_type = rom[0x0147];
     let title_bytes = &rom[0x0134..0x0144];
        let title: String = title_bytes
            .iter()
            .take_while(|&&b| b != 0 && b.is_ascii_graphic())
            .map(|&b| b as char)
            .collect();
     use std::path::Path;
        let rom_path = Path::new(rom_filename);
        let save_filename = if let Some(stem) = rom_path.file_stem() {
            format!("{}.sav", stem.to_string_lossy())
        } else {
            "game.sav".to_string()
        };
                println!("=== ROM INFO ===");
        println!("Game Title: '{}'", title);
        println!("MBC Type: {:#04X}", mbc_type);
        println!("ROM Size: {} bytes ({} banks)", rom.len(), rom.len() / 0x4000);
        println!("Save File: {}", save_filename);
        println!("================\n");
      let mut mmu=  Self {
            rom,
            mbc_type,
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
            joypad_state: 0xFF, // All buttons released (1 = released)
            joyp_sel: 0x30,     // Default to neither group selected

            rom_bank: 1,        // The swappable bank starts at 1
            ram_enabled: false, // RAM is disabled by default for safety
            ram_bank: 0,
            mode: 0, // Start in ROM Banking Mode (Mode 0)
            eram: [0; 0x8000],
            save_filename,
            save_dirty: false,
            // --- Added for MBC3 (Pokemon) ---
            rtc_registers: [0; 5],  // The five clock registers
            rtc_sel: 0,             // Register selection for 0xA000 range
            apu: apu::APU::new(),
        };
                mmu.load_save();
        mmu
    }
   pub fn load_save(&mut self) {
        if let Ok(data) = fs::read(&self.save_filename) {
            let len = data.len().min(0x8000);
            self.eram[..len].copy_from_slice(&data[..len]);
            println!("✓ Loaded save file '{}': {} bytes", self.save_filename, len);
        } else {
            println!("✗ No save file found ('{}'), starting fresh", self.save_filename);
        }
    }

pub fn save_ram(&mut self) {
    if let Err(e) = fs::write(&self.save_filename, &self.eram[..]) {
        eprintln!("Failed to save '{}': {}", self.save_filename, e);
    } else {
        println!("✓ Save file '{}' written", self.save_filename);
        self.save_dirty = false;  // Clear dirty flag after successful save
    }
}
    pub fn read_byte(&self, addr: u16) -> u8 {
    match addr {
        0xFF10..=0xFF3F => self.apu.read_register(addr),
        // ROM Bank 0 (Fixed)
        0x0000..=0x3FFF => self.rom[addr as usize],
        
        // ROM Bank 1-N (Switchable)
        0x4000..=0x7FFF => {
            let actual_bank = match self.mbc_type {
                0x01..=0x03 => {
                    if self.mode == 0 { self.rom_bank as usize } 
                    else { (self.rom_bank & 0x1F) as usize }
                }
                0x0F..=0x13 => (self.rom_bank & 0x7F) as usize,
                0x19..=0x1E => (self.rom_bank & 0x1FF) as usize,
                _ => self.rom_bank as usize,
            };
            let offset = actual_bank * 0x4000;
            let rom_addr = offset + (addr - 0x4000) as usize;
            if rom_addr < self.rom.len() { self.rom[rom_addr] } else { 0xFF }
        }
        
        // VRAM
        0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
        
        // External RAM
        0xA000..=0xBFFF => {
            if !self.ram_enabled { return 0xFF; }
            match self.mbc_type {
                0x01..=0x03 => {
                    let bank = if self.mode == 1 { self.ram_bank as usize } else { 0 };
                    self.eram[(bank * 0x2000) + (addr - 0xA000) as usize]
                }
                0x0F..=0x13 => {
                    if self.rtc_sel <= 0x03 {
                        self.eram[(self.rtc_sel as usize * 0x2000) + (addr - 0xA000) as usize]
                    } else if self.rtc_sel >= 0x08 && self.rtc_sel <= 0x0C {
                        self.rtc_registers[(self.rtc_sel - 0x08) as usize]
                    } else { 0xFF }
                }
                0x19..=0x1E => {
                    let offset = (self.ram_bank as usize) * 0x2000;
                    self.eram[offset + (addr - 0xA000) as usize]
                }
                _ => 0xFF,
            }
        }
        
        // WRAM
        0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
        
        // Echo RAM (mirrors C000-DDFF)
        0xE000..=0xFDFF => {
            let mirrored_addr = addr - 0x2000;
            self.wram[(mirrored_addr - 0xC000) as usize]
        }
        
        // OAM
        0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
        
        // I/O Registers
        0xFF00 => {
            let mut res = 0xC0 | (self.joyp_sel & 0x30);
            let mut low_nibble = 0x0F;
            if (self.joyp_sel & 0x10) == 0 {
                low_nibble &= self.joypad_state & 0x0F;
            }
            if (self.joyp_sel & 0x20) == 0 {
                low_nibble &= (self.joypad_state >> 4) & 0x0F;
            }
            res | low_nibble
        }
        0xFF04 => self.div,
        0xFF05 => self.tima,
        0xFF06 => self.tma,
        0xFF07 => self.tac | 0xF8,
        0xFF0F => self.interrupt_flag | 0xE0,
        0xFF40 => self.lcdc,
        0xFF41 => self.stat | 0x80,
        0xFF42 => self.scy,
        0xFF43 => self.scx,
        0xFF44 => self.ly,
        0xFF45 => self.lyc,
        0xFF47 => self.bgp,
        0xFF48 => self.obp0,
        0xFF49 => self.obp1,
        0xFF4A => self.wy,
        0xFF4B => self.wx,
        0xFFFF => self.interrupt_enable,
        
        // HRAM
        0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
        
        // Everything else
        _ => 0xFF,
    }
}


    pub fn write_byte(&mut self, addr: u16, val: u8) {
    match addr {
        0xFF10..=0xFF3F => self.apu.write_register(addr, val),
        // MBC Register: RAM Enable
        0x0000..=0x1FFF => {
            match self.mbc_type {
                0x01..=0x03 | 0x0F..=0x13 | 0x19..=0x1E => {
                    self.ram_enabled = (val & 0x0F) == 0x0A;
                }
                _ => {}
            }
        }
        
        // MBC Register: ROM Bank Number
        0x2000..=0x3FFF => {
            match self.mbc_type {
                0x01..=0x03 => {
                    let mut bank = (val & 0x1F) as u16;
                    if bank == 0 { bank = 1; }
                    self.rom_bank = (self.rom_bank & 0x60) | bank;
                }
                0x0F..=0x13 => {
                    let mut bank = (val & 0x7F) as u16;
                    if bank == 0 { bank = 1; }
                    self.rom_bank = bank;
                }
                0x19..=0x1E => {
                    if addr < 0x3000 {
                        self.rom_bank = (self.rom_bank & 0x100) | (val as u16);
                    } else {
                        self.rom_bank = (self.rom_bank & 0xFF) | (((val & 0x01) as u16) << 8);
                    }
                }
                _ => {}
            }
        }
        
        // MBC Register: RAM Bank / Upper ROM Bank
        0x4000..=0x5FFF => {
            match self.mbc_type {
                0x01..=0x03 => {
                    let upper_bits = ((val & 0x03) as u16) << 5;
                    if self.mode == 1 {
                        self.ram_bank = val & 0x03;
                    } else {
                        self.rom_bank = (self.rom_bank & 0x1F) | upper_bits;
                    }
                }
                0x0F..=0x13 => self.rtc_sel = val,
                0x19..=0x1E => {
                    self.ram_bank = val & 0x0F;
                }
                _ => {}
            }
        }
        
        // MBC Register: Banking Mode
        0x6000..=0x7FFF => {
            if self.mbc_type <= 0x03 {
                self.mode = val & 0x01;
            }
        }
        
        // VRAM
        0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
        
        // External RAM
        0xA000..=0xBFFF => {
            if self.ram_enabled {
                match self.mbc_type {
                    0x01..=0x03 => {
                        let bank = if self.mode == 1 { self.ram_bank as usize } else { 0 };
                        self.eram[(bank * 0x2000) + (addr - 0xA000) as usize] = val;
                        self.save_dirty = true;  // ADD THIS
                    }
                    0x0F..=0x13 => {
                        if self.rtc_sel <= 0x03 {
                            self.eram[(self.rtc_sel as usize * 0x2000) + (addr - 0xA000) as usize] = val;
                            self.save_dirty = true;  // ADD THIS
                        } else if self.rtc_sel >= 0x08 && self.rtc_sel <= 0x0C {
                            self.rtc_registers[(self.rtc_sel - 0x08) as usize] = val;
                            self.save_dirty = true;  // ADD THIS
                        }
                    }
                    0x19..=0x1E => {
                        let offset = (self.ram_bank as usize) * 0x2000;
                        self.eram[offset + (addr - 0xA000) as usize] = val;
                        self.save_dirty = true;  // ADD THIS
                    }
                    _ => {}
                }
            }
        }
        
        // WRAM
        0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
        
        // Echo RAM
        0xE000..=0xFDFF => {
            let mirrored_addr = addr - 0x2000;
            self.wram[(mirrored_addr - 0xC000) as usize] = val;
        }
        
        // OAM
        0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = val,
        
        // I/O Registers
        0xFF00 => self.joyp_sel = val & 0x30,
        0xFF01 => print!("{}", val as char),
        0xFF02 => {},
        0xFF04 => {
            self.div = 0;
            self.div_counter = 0;
        }
        0xFF05 => self.tima = val,
        0xFF06 => self.tma = val,
        0xFF07 => self.tac = val & 0x07,
        0xFF0F => self.interrupt_flag = val | 0xE0,
        0xFF40 => self.lcdc = val,
        0xFF41 => self.stat = (val & 0xF8) | (self.stat & 0x07),
        0xFF42 => self.scy = val,
        0xFF43 => self.scx = val,
        0xFF44 => {}, // LY is read-only
        0xFF45 => self.lyc = val,
        0xFF46 => {
            // DMA Transfer
            let source_base = (val as u16) << 8;
            for i in 0..0xA0 {
                let byte = self.read_byte(source_base + i);
                self.oam[i as usize] = byte;
            }
        }
        0xFF47 => self.bgp = val,
        0xFF48 => self.obp0 = val,
        0xFF49 => self.obp1 = val,
        0xFF4A => self.wy = val,
        0xFF4B => self.wx = val,
        0xFFFF => self.interrupt_enable = val,
        
        // HRAM
        0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
        
        // Ignore writes to ROM
        _ => {}
    }
}
}