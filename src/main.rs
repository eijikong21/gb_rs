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

use minifb::{Window, WindowOptions, Key, Scale, ScaleMode, MouseMode, MouseButton, KeyRepeat};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use gilrs::{Gilrs, Event, EventType, Button as GamepadButton};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use rfd::FileDialog;

// --- 1. CONFIGURATION STRUCTS ---

#[derive(Clone, Copy, Debug)]
pub struct InputMapping {
    // Controller
    pub up_btn: GamepadButton,
    pub down_btn: GamepadButton,
    pub left_btn: GamepadButton,
    pub right_btn: GamepadButton,
    pub a_btn: GamepadButton,
    pub b_btn: GamepadButton,
    pub start_btn: GamepadButton,
    pub select_btn: GamepadButton,
    
    // Keyboard
    pub up_key: Key,
    pub down_key: Key,
    pub left_key: Key,
    pub right_key: Key,
    pub a_key: Key,
    pub b_key: Key,
    pub start_key: Key,
    pub select_key: Key,
}

impl InputMapping {
    pub fn default() -> Self {
        Self {
            // Default Controller (Xbox/PS Standard)
            up_btn: GamepadButton::DPadUp,
            down_btn: GamepadButton::DPadDown,
            left_btn: GamepadButton::DPadLeft,
            right_btn: GamepadButton::DPadRight,
            a_btn: GamepadButton::East,
            b_btn: GamepadButton::South,
            start_btn: GamepadButton::Start,
            select_btn: GamepadButton::Select,

            // Default Keyboard
            up_key: Key::Up,
            down_key: Key::Down,
            left_key: Key::Left,
            right_key: Key::Right,
            a_key: Key::Z,
            b_key: Key::X,
            start_key: Key::Enter,
            select_key: Key::Space,
        }
    }
}

// --- 2. HELPER STRUCTS & FUNCTIONS ---

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

// Text Helper
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

// --- CONTROLLER & KEYBOARD CONFIG WINDOW ---
fn open_controller_config(mut current_mapping: InputMapping, gilrs: &mut Gilrs) -> InputMapping {
    const W: usize = 500; // Slightly wider for text
    const H: usize = 350;
    
    let mut config_window = Window::new(
        "Bind Controls (Press Key OR Button)",
        W, H,
        WindowOptions { resize: false, ..WindowOptions::default() },
    ).unwrap();

    let mut buffer = vec![0; W * H];
    let mut binding_target: Option<usize> = None;
    let row_height = 30;
    let start_y = 50;

    while config_window.is_open() && !config_window.is_key_down(Key::Escape) {
        for p in buffer.iter_mut() { *p = 0xFF202020; } 

        // 1. POLL INPUTS (Keyboard + Controller)
        let mut pressed_btn = None;
        let mut pressed_key = None;

        // Check Controller
        while let Some(Event { event, .. }) = gilrs.next_event() {
            if let EventType::ButtonPressed(btn, _) = event {
                pressed_btn = Some(btn);
            }
        }
        
        // Check Keyboard (Get first pressed key)
        let keys = config_window.get_keys_pressed(KeyRepeat::No);
        if !keys.is_empty() {
            pressed_key = Some(keys[0]);
        }

        // 2. BINDING LOGIC
        if let Some(idx) = binding_target {
            // We are waiting for input...
            
            if let Some(btn) = pressed_btn {
                // User pressed a CONTROLLER BUTTON
                match idx {
                    0 => current_mapping.up_btn = btn,
                    1 => current_mapping.down_btn = btn,
                    2 => current_mapping.left_btn = btn,
                    3 => current_mapping.right_btn = btn,
                    4 => current_mapping.a_btn = btn,
                    5 => current_mapping.b_btn = btn,
                    6 => current_mapping.start_btn = btn,
                    7 => current_mapping.select_btn = btn,
                    _ => {}
                }
                binding_target = None;
            } else if let Some(key) = pressed_key {
                // User pressed a KEYBOARD KEY
                match idx {
                    0 => current_mapping.up_key = key,
                    1 => current_mapping.down_key = key,
                    2 => current_mapping.left_key = key,
                    3 => current_mapping.right_key = key,
                    4 => current_mapping.a_key = key,
                    5 => current_mapping.b_key = key,
                    6 => current_mapping.start_key = key,
                    7 => current_mapping.select_key = key,
                    _ => {}
                }
                binding_target = None;
            }
        } else {
            // Waiting for mouse click to select row
            if config_window.get_mouse_down(MouseButton::Left) {
                if let Some((_, my)) = config_window.get_mouse_pos(MouseMode::Clamp) {
                    if my >= start_y as f32 {
                        let row = ((my as usize) - start_y) / row_height;
                        if row < 8 { binding_target = Some(row); }
                    }
                }
            }
        }

        // 3. DRAW UI
        draw_text(&mut buffer, W, "CLICK LINE THEN PRESS KEY/BTN", 10, 10, 0xFFFFFF00, 1);
        
        let labels = ["UP", "DOWN", "LEFT", "RIGHT", "A", "B", "START", "SELECT"];
        
        for i in 0..8 {
            let y = start_y + (i * row_height);
            let is_binding = binding_target == Some(i);
            let color = if is_binding { 0xFFFF0000 } else { 0xFFFFFFFF };
            
            // Get current values to display
            let (btn, key) = match i {
                0 => (current_mapping.up_btn, current_mapping.up_key),
                1 => (current_mapping.down_btn, current_mapping.down_key),
                2 => (current_mapping.left_btn, current_mapping.left_key),
                3 => (current_mapping.right_btn, current_mapping.right_key),
                4 => (current_mapping.a_btn, current_mapping.a_key),
                5 => (current_mapping.b_btn, current_mapping.b_key),
                6 => (current_mapping.start_btn, current_mapping.start_key),
                7 => (current_mapping.select_btn, current_mapping.select_key),
                _ => (GamepadButton::Unknown, Key::Unknown),
            };

            let val_str = if is_binding { 
                "Waiting for Input...".to_string() 
            } else { 
                // Show "Btn: X  Key: Z"
                format!("Btn:{:?}  Key:{:?}", btn, key) 
            };
            
            draw_text(&mut buffer, W, labels[i], 20, y, color, 1);
            draw_text(&mut buffer, W, &val_str, 100, y, color, 1);
        }

        config_window.update_with_buffer(&buffer, W, H).unwrap();
    }
    
    current_mapping
}

fn main() {
    // --- CONFIGURATION ---
    const MENU_HEIGHT: usize = 4; 
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
    let mut mapping = InputMapping::default(); 

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
        let mut is_hovering_config = false;
        let mut open_config_requested = false;

        // --- ROBUST MOUSE LOGIC ---
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Discard) {
            
            // 1. Geometry
            let (win_w, win_h) = window.get_size();
            let scale_x = win_w as f32 / TOTAL_WIDTH as f32;
            let scale_y = win_h as f32 / TOTAL_HEIGHT as f32;
            let scale = scale_x.min(scale_y); 

            let draw_h = TOTAL_HEIGHT as f32 * scale;
            let offset_y = (win_h as f32 - draw_h) / 2.0;

            // 2. Define Strips using manual 3.00 scaling (as requested)
            let strip1_top = offset_y;
            let strip1_bot = offset_y + (2.0 * 3.00); 

            let strip2_top = strip1_bot;
            let strip2_bot = offset_y + (4.0 * 3.00); 

            // 3. Logic
            if mx < (win_w as f32 * 1.00) {
                // Check Row 1 (Load)
                if my >= strip1_top && my < strip1_bot {
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
                // Check Row 2 (Config)
                else if my >= strip2_top && my < strip2_bot {
                    is_hovering_config = true;
                    if window.get_mouse_down(MouseButton::Left) {
                        open_config_requested = true;
                    }
                }
            }
        }

        // --- HANDLE CONFIG OPEN ---
        if open_config_requested {
            mapping = open_controller_config(mapping, &mut gilrs);
            // Must clear inputs to prevent stuck keys after closing window
            while let Some(_) = gilrs.next_event() {} 
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
            
            // Drain Gilrs events
            while let Some(_) = gilrs.next_event() {} 
            
            let mut pad = 0xFF;
            
            // KEYBOARD (Dynamic Mapping)
            if window.is_key_down(mapping.right_key) { pad &= !(1 << 0); }
            if window.is_key_down(mapping.left_key)  { pad &= !(1 << 1); }
            if window.is_key_down(mapping.up_key)    { pad &= !(1 << 2); }
            if window.is_key_down(mapping.down_key)  { pad &= !(1 << 3); }
            if window.is_key_down(mapping.a_key)     { pad &= !(1 << 4); }
            if window.is_key_down(mapping.b_key)     { pad &= !(1 << 5); }
            if window.is_key_down(mapping.select_key){ pad &= !(1 << 6); }
            if window.is_key_down(mapping.start_key) { pad &= !(1 << 7); }
            
            // CONTROLLER (Mapped)
            for (_id, gamepad) in gilrs.gamepads() {
                if gamepad.is_pressed(mapping.right_btn) { pad &= !(1 << 0); }
                if gamepad.is_pressed(mapping.left_btn)  { pad &= !(1 << 1); }
                if gamepad.is_pressed(mapping.up_btn)    { pad &= !(1 << 2); }
                if gamepad.is_pressed(mapping.down_btn)  { pad &= !(1 << 3); }
                if gamepad.is_pressed(mapping.a_btn)     { pad &= !(1 << 4); }
                if gamepad.is_pressed(mapping.b_btn)     { pad &= !(1 << 5); }
                if gamepad.is_pressed(mapping.select_btn){ pad &= !(1 << 6); }
                if gamepad.is_pressed(mapping.start_btn) { pad &= !(1 << 7); }
            }

            emu.cpu.bus.joypad_state = pad;
            if pad != 0xFF { emu.cpu.bus.interrupt_flag |= 0x10; }
            if last_save.elapsed() > Duration::from_secs(1) { if emu.cpu.bus.save_dirty { emu.cpu.bus.save_ram(); } last_save = Instant::now(); }
        }

        // --- RENDER ---
        
        // Backgrounds
        for i in 0..(SS_WIDTH * 2 * SS_SCALE) { window_buffer[i] = 0xFF222222; }
        let start_row2 = SS_WIDTH * 2 * SS_SCALE;
        for i in start_row2..(SS_WIDTH * 4 * SS_SCALE) { window_buffer[i] = 0xFF111111; }

        // Text
        let col1 = if is_hovering_load { 0xFF55FF55 } else { 0xFFFFFFFF };
        let col2 = if is_hovering_config { 0xFF55FF55 } else { 0xFFAAAAAA };
        
        draw_text(&mut window_buffer, SS_WIDTH, "LOAD ROM", 2 * SS_SCALE, 0, col1, 1);
        draw_text(&mut window_buffer, SS_WIDTH, "INPUT", 2 * SS_SCALE, 2 * SS_SCALE, col2, 1);

        // Game
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