mod cpu;
mod mmu;
mod ppu;
mod apu;

use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ppu::PPU;
use cpu::CPU;
use mmu::MMU;

use minifb::{Window, WindowOptions, Key, Scale, ScaleMode, MouseMode, MouseButton};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gilrs::{Gilrs, Event};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use rfd::FileDialog;

// --- HELPER STRUCTS ---
struct EmulatorState {
    cpu: CPU,
    ppu: PPU,
    mmu_filename: String,
}

impl EmulatorState {
    fn load_rom(path: &str) -> Self {
        let rom_data = fs::read(path).expect("Failed to read ROM");
        let mmu = MMU::new(rom_data, path);
        let cpu = CPU::new(mmu);
        let ppu = PPU::new();
        println!("Loaded ROM: {}", path);
        Self { cpu, ppu, mmu_filename: path.to_string() }
    }
}

// Text Helper (Scale 1 support)
fn draw_text(buffer: &mut [u32], width: usize, text: &str, x: usize, y: usize, color: u32, scale: usize) {
    for (i, ch) in text.chars().enumerate() {
        if let Some(glyph) = BASIC_FONTS.get(ch) {
            for row in 0..8 {
                for bit in 0..8 {
                    if (glyph[row] & (1 << bit)) != 0 {
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let draw_x = x + (i * 8 * scale) + bit * scale + dx;
                                let draw_y = y + row * scale + dy;
                                if draw_x < width && draw_y < buffer.len() / width {
                                    buffer[draw_y * width + draw_x] = color;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    // --- CONFIGURATION ---
    const MENU_HEIGHT: usize = 2; // Tiny strip
    const GB_WIDTH: usize = 160;
    const GB_HEIGHT: usize = 144;
    
    const TOTAL_WIDTH: usize = GB_WIDTH;
    const TOTAL_HEIGHT: usize = GB_HEIGHT + MENU_HEIGHT;

    const SS_SCALE: usize = 4;
    const SS_WIDTH: usize = TOTAL_WIDTH * SS_SCALE;
    const SS_HEIGHT: usize = TOTAL_HEIGHT * SS_SCALE;

    // --- WINDOW SETUP ---
    let mut window = Window::new(
        "Rust Game Boy",
        160 * 3, 
        (144 + MENU_HEIGHT) * 3,
        WindowOptions {
            resize: true,
            scale: Scale::FitScreen,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    ).unwrap();

    window.limit_update_rate(Some(Duration::from_micros(16600)));

    let mut window_buffer: Vec<u32> = vec![0; SS_WIDTH * SS_HEIGHT];

    // --- INIT ---
    let mut current_emulator: Option<EmulatorState> = None;
    let mut gilrs = Gilrs::new().unwrap(); 
    
    // Audio
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device found");
    let config = device.default_output_config().unwrap();
    let audio_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let cb_buffer = Arc::clone(&audio_buffer);
    let stream = device.build_output_stream(
        &config.into(),
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut buffer = cb_buffer.lock().unwrap();
            for sample in data.iter_mut() {
                *sample = if buffer.len() > 0 { buffer.remove(0) } else { 0.0 };
            }
        },
        |err| eprintln!("Err: {}", err),
        None
    ).unwrap();
    stream.play().unwrap();

    let mut last_save = Instant::now();

    // --- MAIN LOOP ---
    while window.is_open() && !window.is_key_down(Key::Escape) {
        
        let mut rom_to_load: Option<String> = None;
        let mut is_hovering_load = false;

        // --- ROBUST MOUSE LOGIC ---
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Discard) {
            
            // 1. Calculate Screen Geometry
            let (win_w, win_h) = window.get_size();
            
            // Calculate how much the game is scaled
            let scale_x = win_w as f32 / TOTAL_WIDTH as f32;
            let scale_y = win_h as f32 / TOTAL_HEIGHT as f32;
            let scale = scale_x.min(scale_y);

            // Calculate the "Black Bar" size (offset)
            // If scale_mode puts the game in the middle, we need to know where it starts.
            let draw_h = TOTAL_HEIGHT as f32 * scale;
            let offset_y = (win_h as f32 - draw_h) / 2.0;

            // 2. The "Click Line" Calculation
            // We want to detect clicks on the menu bar.
            // The menu bar sits at Game Y = 0 to 2.
            // In WINDOW pixels, that is: offset_y to (offset_y + 2 * scale)
            let menu_bottom_y_screen = offset_y + (MENU_HEIGHT as f32 * 3.00);

            // 3. The "Forgiving" Check
            // We check if the mouse Y is LESS than the bottom of the menu bar.
            // This means ANY click in the top black bar OR the menu bar itself counts.
            // We also limit it to roughly the left side where the text is (0 to 1/3 of screen width).
            if my < menu_bottom_y_screen {
                // Ensure we are somewhat on the left side (where "LOAD ROM" is)
                // Let's say the first 35% of the screen width is the button.
                if mx < (win_w as f32 * 1.00) {
                    is_hovering_load = true;

                    if window.get_mouse_down(MouseButton::Left) {
                         let file = FileDialog::new()
                            .add_filter("Game Boy", &["gb", "gbc", "bin"])
                            .set_directory(".")
                            .pick_file();

                        if let Some(path) = file {
                            rom_to_load = Some(path.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        // --- EMULATOR UPDATE ---
        if let Some(path) = rom_to_load {
            current_emulator = Some(EmulatorState::load_rom(&path));
            if let Some(emu) = &current_emulator {
                window.set_title(&format!("Rust Game Boy - {}", emu.mmu_filename));
            }
        }

        if let Some(emu) = &mut current_emulator {
            let mut cycles = 0;
            while cycles < 70224 {
                let c = emu.cpu.step() as u8;
                emu.cpu.bus.tick(c); emu.cpu.bus.apu.tick(c); emu.ppu.tick(&mut emu.cpu.bus, c);
                cycles += c as u32;
                let i = emu.cpu.handle_interrupts();
                if i > 0 { emu.cpu.bus.tick(i); emu.cpu.bus.apu.tick(i); emu.ppu.tick(&mut emu.cpu.bus, i); cycles += i as u32; }
            }
            let mut s = emu.cpu.bus.apu.get_samples();
            if let Ok(mut b) = audio_buffer.lock() { if b.len() < 8192 { b.append(&mut s); } }
            
            while let Some(_) = gilrs.next_event() {} 
            let mut pad = 0xFF;
            if window.is_key_down(Key::Right) { pad &= !(1 << 0); }
            if window.is_key_down(Key::Left)  { pad &= !(1 << 1); }
            if window.is_key_down(Key::Up)    { pad &= !(1 << 2); }
            if window.is_key_down(Key::Down)  { pad &= !(1 << 3); }
            if window.is_key_down(Key::Z)     { pad &= !(1 << 4); }
            if window.is_key_down(Key::X)     { pad &= !(1 << 5); }
            if window.is_key_down(Key::Space) { pad &= !(1 << 6); }
            if window.is_key_down(Key::Enter) { pad &= !(1 << 7); }
            emu.cpu.bus.joypad_state = pad;
            if pad != 0xFF { emu.cpu.bus.interrupt_flag |= 0x10; }
            if last_save.elapsed() > Duration::from_secs(1) { if emu.cpu.bus.save_dirty { emu.cpu.bus.save_ram(); } last_save = Instant::now(); }
        }

        // --- RENDER ---
        for i in 0..(SS_WIDTH * MENU_HEIGHT * SS_SCALE) {
            window_buffer[i] = 0xFF222222; 
        }

        let text_color = if is_hovering_load { 0xFF55FF55 } else { 0xFFFFFFFF };
        draw_text(&mut window_buffer, SS_WIDTH, "LOAD ROM", 2 * SS_SCALE, 0, text_color, 1);

        if let Some(emu) = &current_emulator {
            for y in 0..144 {
                for x in 0..160 {
                    let pixel = emu.ppu.frame_buffer[y * 160 + x];
                    for dy in 0..SS_SCALE {
                        for dx in 0..SS_SCALE {
                            let dest_x = x * SS_SCALE + dx;
                            let dest_y = (y + MENU_HEIGHT) * SS_SCALE + dy;
                            window_buffer[dest_y * SS_WIDTH + dest_x] = pixel;
                        }
                    }
                }
            }
        } else {
            let start = SS_WIDTH * MENU_HEIGHT * SS_SCALE;
            for i in start..window_buffer.len() { window_buffer[i] = 0xFF000000; }
            draw_text(&mut window_buffer, SS_WIDTH, "NO ROM", 60 * SS_SCALE, 60 * SS_SCALE, 0xFF555555, 4);
        }

        window.update_with_buffer(&window_buffer, SS_WIDTH, SS_HEIGHT).unwrap();
    }
} 