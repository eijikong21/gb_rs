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
    // 1. Load the ROM
    let rom_filename = "roms/yellow1.gb";
let rom = fs::read(rom_filename).unwrap();
let mmu = MMU::new(rom, rom_filename);
    let mut cpu = CPU::new(mmu);
    let mut ppu = PPU::new();
    println!("CPU Initialized at PC: {:#06X}", cpu.registers.pc);
    
    let mut window = Window::new(
        "Rust Game Boy",
        160,
        144,
        WindowOptions {
            scale: minifb::Scale::X4,
            ..WindowOptions::default()
        },
    ).unwrap_or_else(|e| {
        panic!("{}", e);
    });

    // Limit to ~60 FPS
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

   let mut h_history: Vec<Key> = Vec::new(); 
let mut v_history: Vec<Key> = Vec::new();
let mut last_save = Instant::now();
let mut prev_h_state = (false, false); // (Right, Left)
let mut prev_v_state = (false, false); // (Up, Down)

while window.is_open() && !window.is_key_down(Key::Escape) {
    let mut pad = 0xFF;
    
    // 1. Update Horizontal History (detect NEW presses)
    let right_down = window.is_key_down(Key::Right);
    let left_down = window.is_key_down(Key::Left);
    
    // If Right was just pressed (wasn't down before, is down now)
    if right_down && !prev_h_state.0 {
        h_history.retain(|&k| k != Key::Right);
        h_history.insert(0, Key::Right);
    }
    // If Left was just pressed
    if left_down && !prev_h_state.1 {
        h_history.retain(|&k| k != Key::Left);
        h_history.insert(0, Key::Left);
    }
    // Remove released keys
    if !right_down { h_history.retain(|&k| k != Key::Right); }
    if !left_down { h_history.retain(|&k| k != Key::Left); }
    
    prev_h_state = (right_down, left_down);

    // 2. Update Vertical History (detect NEW presses)
    let up_down = window.is_key_down(Key::Up);
    let down_down = window.is_key_down(Key::Down);
    
    if up_down && !prev_v_state.0 {
        v_history.retain(|&k| k != Key::Up);
        v_history.insert(0, Key::Up);
    }
    if down_down && !prev_v_state.1 {
        v_history.retain(|&k| k != Key::Down);
        v_history.insert(0, Key::Down);
    }
    if !up_down { v_history.retain(|&k| k != Key::Up); }
    if !down_down { v_history.retain(|&k| k != Key::Down); }
    
    prev_v_state = (up_down, down_down);

    // 3. Apply ONLY the newest directional key
    if let Some(&last_h) = h_history.first() {
        match last_h {
            Key::Right => pad &= !(1 << 0),
            Key::Left  => pad &= !(1 << 1),
            _ => {}
        }
    }

    if let Some(&last_v) = v_history.first() {
        match last_v {
            Key::Up   => pad &= !(1 << 2),
            Key::Down => pad &= !(1 << 3),
            _ => {}
        }
    }

    // 4. Button keys
    if window.is_key_down(Key::Z)      { pad &= !(1 << 4); }
    if window.is_key_down(Key::X)      { pad &= !(1 << 5); }
    if window.is_key_down(Key::Space)  { pad &= !(1 << 6); }
    if window.is_key_down(Key::Enter)  { pad &= !(1 << 7); }

    cpu.bus.joypad_state = pad;

    if pad != 0xFF {
        cpu.bus.interrupt_flag |= 0x10;
    }

    // Save RAM periodically
    if last_save.elapsed() > Duration::from_secs(5) {
        if cpu.bus.ram_enabled {
            cpu.bus.save_ram();
        }
        last_save = Instant::now();
    }

    // Game loop
    while cpu.bus.ly >= 144 {
        cpu.handle_interrupts();
        let cycles = cpu.step();
        cpu.bus.tick(cycles);
        ppu.tick(&mut cpu.bus, cycles);
    }

    while cpu.bus.ly < 144 {
        cpu.handle_interrupts();
        let cycles = cpu.step();
        cpu.bus.tick(cycles);
        ppu.tick(&mut cpu.bus, cycles);
    }

    window
        .update_with_buffer(&ppu.frame_buffer, 160, 144)
        .unwrap();
}
    
    // Save on exit
    cpu.bus.save_ram();
    println!("Game exited");
}