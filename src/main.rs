mod cpu;
mod mmu;
mod ppu;
use ppu::PPU;
use cpu::CPU;
use mmu::MMU;
use std::fs;

use minifb::{Window, WindowOptions, Key};
use std::time::{Duration, Instant};
fn main() {
    
    // 1. Load the ROM (Use a Blargg test ROM to start)
    let rom = fs::read("roms/yellow.gbc").unwrap();
    
    // 2. Initialize Hardware
    use std::fs::File;
use std::io::Write;

fn save_game(eram: &[u8; 0x8000], filename: &str) {
    let mut file = File::create(filename).unwrap();
    file.write_all(eram).unwrap();
    println!("Game Saved to {}", filename);
}
    let mmu = MMU::new(rom);
    let mut cpu = CPU::new(mmu);
let mut ppu = PPU::new();
    println!("CPU Initialized at PC: {:#06X}", cpu.registers.pc);
let mut window = Window::new(
        "Rust Game Boy",
        160,
        144,
        WindowOptions {
            scale: minifb::Scale::X4, // 4x scale so you can see it
            ..WindowOptions::default()
        },
    ).unwrap_or_else(|e| {
        panic!("{}", e);
    });

    // Limit to ~60 FPS
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    // ... Initialize CPU, MMU, PPU ...

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut pad = 0xFF;

// 2. Map keys (Bits: 7:Start, 6:Sel, 5:B, 4:A, 3:Down, 2:Up, 1:Left, 0:Right)
if window.is_key_down(Key::Right)  { pad &= !(1 << 0); }
if window.is_key_down(Key::Left)   { pad &= !(1 << 1); }
if window.is_key_down(Key::Up)     { pad &= !(1 << 2); }
if window.is_key_down(Key::Down)   { pad &= !(1 << 3); }
if window.is_key_down(Key::Z)      { pad &= !(1 << 4); } // A
if window.is_key_down(Key::X)      { pad &= !(1 << 5); } // B
if window.is_key_down(Key::Space)  { pad &= !(1 << 6); } // Select
if window.is_key_down(Key::Enter)  { pad &= !(1 << 7); } // Start

cpu.bus.joypad_state = pad;

// 3. Trigger Joypad Interrupt if any button was pressed
// Real Game Boys trigger an interrupt (bit 4 of IF) when a button goes from 1 to 0
if pad != 0xFF {
    cpu.bus.interrupt_flag |= 0x10;
}
    // Wait for LY to be in the visible range (ensuring we start fresh)
    while cpu.bus.ly >= 144 {
        cpu.handle_interrupts();
        let cycles = cpu.step();
        cpu.bus.tick(cycles);
        ppu.tick(&mut cpu.bus, cycles);
    }

    // Now run until we hit V-Blank
    while cpu.bus.ly < 144 {
        cpu.handle_interrupts();
        let cycles = cpu.step();
        cpu.bus.tick(cycles);
        ppu.tick(&mut cpu.bus, cycles);
    }

    // Update the window with the PPU's frame buffer
    window
        .update_with_buffer(&ppu.frame_buffer, 160, 144)
        .unwrap();
}
}