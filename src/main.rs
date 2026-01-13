mod cpu;
mod mmu;
mod ppu;
mod apu;

use ppu::PPU;
use cpu::CPU;
use mmu::MMU;
use std::fs;
use rfd::FileDialog; 
use minifb::{Window, WindowOptions, Key};
use std::time::{Duration, Instant};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};

fn main() {
    // 1. Load the ROM
    // let rom_filename = "roms/yellow1.gb";
    let file_path = FileDialog::new()
        .add_filter("Game Boy ROM", &["gb", "gbc", "bin"])
        .set_directory(".")
        .pick_file();

    // 2. Handle the selection
    let rom_path = match file_path {
        Some(path) => path,
        None => {
            println!("No file selected. Exiting.");
            return;
        }
    };

    let rom_filename = rom_path.to_str().unwrap_or("game.gb");

    // 3. Load the ROM data
    let rom_data = std::fs::read(&rom_path).expect("Failed to read ROM file");

    // 4. Initialize MMU with selected ROM

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

let host = cpal::default_host();
let device = host.default_output_device().expect("No output device found");
let config = device.default_output_config().unwrap();

// We'll use a shared buffer (Ring Buffer) to pass samples from the Emulator to CPAL
let audio_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
let cb_buffer = Arc::clone(&audio_buffer);

let stream = device.build_output_stream(
    &config.into(),
    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let mut buffer = cb_buffer.lock().unwrap();
        for sample in data.iter_mut() {
            // If we have a sample in our buffer, play it; otherwise, silence
            *sample = if buffer.len() > 0 { buffer.remove(0) } else { 0.0 };
        }
    },
    |err| eprintln!("Audio stream error: {}", err),
    None
).unwrap();

stream.play().unwrap();

  const CYCLES_PER_FRAME: u32 = 70224; // Standard DMG-01 cycles per frame
  
while window.is_open() && !window.is_key_down(Key::Escape) {
    let mut cycles_this_frame = 0;
  

    while cycles_this_frame < CYCLES_PER_FRAME {
        cpu.handle_interrupts();
        let cycles = cpu.step() as u8;
        
        cpu.bus.tick(cycles);      // Ticks timers/div
        cpu.bus.apu.tick(cycles);  // NEW: Tick the APU
        ppu.tick(&mut cpu.bus, cycles);
        
        cycles_this_frame += cycles as u32;
    }

    // Inside the main loop, after the "while cycles_this_frame < CYCLES_PER_FRAME" loop:
let mut samples = cpu.bus.apu.get_samples();
if let Ok(mut buffer) = audio_buffer.lock() {
    // Prevent the buffer from growing too large (latency control)
    if buffer.len() < 8192 {
        buffer.append(&mut samples);
    }
}
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
    // Debug output
    let lcd_on = (cpu.bus.lcdc & 0x80) != 0;
    let sprites_on = (cpu.bus.lcdc & 0x02) != 0;
    let bg_on = (cpu.bus.lcdc & 0x01) != 0;
    
    let mut visible_sprites = 0;
    let mut non_zero_tiles = 0;
    
    for i in 0..40 {
        let y = cpu.bus.oam[i * 4];
        let x = cpu.bus.oam[i * 4 + 1];
        let tile = cpu.bus.oam[i * 4 + 2];
        
        if y > 0 && y < 160 && x > 0 && x < 168 {
            visible_sprites += 1;
        }
        if tile != 0 {
            non_zero_tiles += 1;
        }
    }
    
    println!("LCDC: LCD={} BG={} SPR={} | OAM: {} visible, {} non-zero",
             if lcd_on { "ON" } else { "OFF" },
             if bg_on { "ON" } else { "OFF" },
             if sprites_on { "ON" } else { "OFF" },
             visible_sprites,
             non_zero_tiles);
    
    // Only save if RAM is enabled AND has non-zero data
if cpu.bus.save_dirty && cpu.bus.has_save_data() {
        cpu.bus.save_ram();
    }
    
    last_save = Instant::now();
}

    // // Game loop
    // while cpu.bus.ly >= 144 {
    //     cpu.handle_interrupts();
    //     let cycles = cpu.step();
    //     cpu.bus.tick(cycles);
    //     ppu.tick(&mut cpu.bus, cycles);
    // }

    // while cpu.bus.ly < 144 {
    //     cpu.handle_interrupts();
    //     let cycles = cpu.step();
    //     cpu.bus.tick(cycles);
    //     ppu.tick(&mut cpu.bus, cycles);
    // }

    window
        .update_with_buffer(&ppu.frame_buffer, 160, 144)
        .unwrap();

    
}
    
    if cpu.bus.save_dirty {
    cpu.bus.save_ram();
}
    println!("Game exited");
}