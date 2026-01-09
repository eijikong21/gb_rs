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
    let rom = fs::read("tests/cpu_instrs.gb").unwrap();
    
    // 2. Initialize Hardware
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