// apu.rs
pub struct APU {
    // Channel 1: Square wave with sweep
    pub nr10: u8, // 0xFF10 - Sweep
    pub nr11: u8, // 0xFF11 - Length timer & duty
    pub nr12: u8, // 0xFF12 - Volume & envelope
    pub nr13: u8, // 0xFF13 - Frequency low
    pub nr14: u8, // 0xFF14 - Frequency high & control
    
    // Channel 2: Square wave
    pub nr21: u8, // 0xFF16 - Length timer & duty
    pub nr22: u8, // 0xFF17 - Volume & envelope
    pub nr23: u8, // 0xFF18 - Frequency low
    pub nr24: u8, // 0xFF19 - Frequency high & control
    
    // Channel 3: Wave output
    pub nr30: u8, // 0xFF1A - DAC enable
    pub nr31: u8, // 0xFF1B - Length timer
    pub nr32: u8, // 0xFF1C - Output level
    pub nr33: u8, // 0xFF1D - Frequency low
    pub nr34: u8, // 0xFF1E - Frequency high & control
    pub wave_ram: [u8; 16], // 0xFF30-0xFF3F - Wave pattern RAM
    
    // Channel 4: Noise
    pub nr41: u8, // 0xFF20 - Length timer
    pub nr42: u8, // 0xFF21 - Volume & envelope
    pub nr43: u8, // 0xFF22 - Frequency & randomness
    pub nr44: u8, // 0xFF23 - Control
    
    // Control registers
    pub nr50: u8, // 0xFF24 - Master volume
    pub nr51: u8, // 0xFF25 - Sound panning
    pub nr52: u8, // 0xFF26 - Sound on/off
    
    // Internal state
    ch1_frequency_timer: u16,
    ch1_duty_position: u8,
    ch1_length_counter: u8,
    ch1_volume: u8,
    ch1_envelope_timer: u8,
    ch1_sweep_timer: u8,
    ch1_sweep_shadow: u16,
    ch1_enabled: bool,
    
    ch2_frequency_timer: u16,
    ch2_duty_position: u8,
    ch2_length_counter: u8,
    ch2_volume: u8,
    ch2_envelope_timer: u8,
    ch2_enabled: bool,
    
    ch3_frequency_timer: u16,
    ch3_position: u8,
    ch3_length_counter: u16,
    ch3_enabled: bool,
    
    ch4_lfsr: u16, // Linear feedback shift register
    ch4_frequency_timer: u16,
    ch4_length_counter: u8,
    ch4_volume: u8,
    ch4_envelope_timer: u8,
    ch4_enabled: bool,
    
    frame_sequencer: u8,
    frame_sequencer_timer: u32,
    
    // Audio buffer
    pub sample_buffer: Vec<f32>,
    sample_timer: f32,
   ch1_sweep_neg_mode: bool, // "Taint" flag
    ch1_sweep_enabled: bool,  // Latch flag
    ch1_sweep_period_was_zero: bool, // Track if period was 0 at trigger
}

impl APU {
    pub fn new() -> Self {
        Self {
            nr10: 0, nr11: 0, nr12: 0, nr13: 0, nr14: 0,
            nr21: 0, nr22: 0, nr23: 0, nr24: 0,
            nr30: 0, nr31: 0, nr32: 0, nr33: 0, nr34: 0,
            wave_ram: [0; 16],
            nr41: 0, nr42: 0, nr43: 0, nr44: 0,
            nr50: 0, nr51: 0, nr52: 0xF0, // Power on = 0xF1 (with audio), 0xF0 (without)
            
            ch1_frequency_timer: 0,
            ch1_duty_position: 0,
            ch1_length_counter: 0,
            ch1_volume: 0,
            ch1_envelope_timer: 0,
            ch1_sweep_timer: 0,
            ch1_sweep_shadow: 0,
            ch1_enabled: false,
            
            ch2_frequency_timer: 0,
            ch2_duty_position: 0,
            ch2_length_counter: 0,
            ch2_volume: 0,
            ch2_envelope_timer: 0,
            ch2_enabled: false,
            
            ch3_frequency_timer: 0,
            ch3_position: 0,
            ch3_length_counter: 0,
            ch3_enabled: false,
            
            ch4_lfsr: 0x7FFF,
            ch4_frequency_timer: 0,
            ch4_length_counter: 0,
            ch4_volume: 0,
            ch4_envelope_timer: 0,
            ch4_enabled: false,
            
            frame_sequencer: 0,
            frame_sequencer_timer: 0,
            
            sample_buffer: Vec::with_capacity(4096),
            sample_timer: 0.0,
           ch1_sweep_neg_mode: false, // Starts clean (not tainted by subtraction)
            ch1_sweep_enabled: false,  // Starts disabled
            ch1_sweep_period_was_zero: false,
        }
    }
    
    pub fn tick(&mut self, cycles: u8) {
        if (self.nr52 & 0x80) == 0 {
            return; // APU is disabled
        }
        
        // Frame sequencer runs at 512 Hz
        self.frame_sequencer_timer += cycles as u32;
        while self.frame_sequencer_timer >= 8192 {
            self.frame_sequencer_timer -= 8192;
            self.clock_frame_sequencer();
        }
        
        // Clock all channels
        for _ in 0..cycles {
            self.clock_channel1();
            self.clock_channel2();
            self.clock_channel3();
            self.clock_channel4();
            
            // Generate sample at ~48kHz (every ~87 cycles)
            self.sample_timer += 1.0;
            if self.sample_timer >= 87.0 {
                self.sample_timer -= 87.0;
                self.generate_sample();
            }
        }
    }
    
    fn clock_frame_sequencer(&mut self) {
        // Frame sequencer steps:
        // Step 0: Length
        // Step 1: Nothing
        // Step 2: Length & Sweep
        // Step 3: Nothing
        // Step 4: Length
        // Step 5: Nothing
        // Step 6: Length & Sweep
        // Step 7: Envelope
        
        match self.frame_sequencer {
            0 | 2 | 4 | 6 => self.clock_length(),
            7 => self.clock_envelope(),
            _ => {}
        }
        
        if self.frame_sequencer == 2 || self.frame_sequencer == 6 {
            self.clock_sweep();
        }
        
        self.frame_sequencer = (self.frame_sequencer + 1) % 8;
    }
    
    
   fn clock_envelope(&mut self) {
    // Channel 1
    let period = self.nr12 & 0x07;
    if period != 0 {  // ADD THIS CHECK
        if self.ch1_envelope_timer > 0 {
            self.ch1_envelope_timer -= 1;
        }
        if self.ch1_envelope_timer == 0 {
            self.ch1_envelope_timer = period;
            
            if (self.nr12 & 0x08) != 0 && self.ch1_volume < 15 {
                self.ch1_volume += 1;
            } else if (self.nr12 & 0x08) == 0 && self.ch1_volume > 0 {
                self.ch1_volume -= 1;
            }
        }
    }
    
    // Channel 2
    let period = self.nr22 & 0x07;
    if period != 0 {  // ADD THIS CHECK
        if self.ch2_envelope_timer > 0 {
            self.ch2_envelope_timer -= 1;
        }
        if self.ch2_envelope_timer == 0 {
            self.ch2_envelope_timer = period;
            
            if (self.nr22 & 0x08) != 0 && self.ch2_volume < 15 {
                self.ch2_volume += 1;
            } else if (self.nr22 & 0x08) == 0 && self.ch2_volume > 0 {
                self.ch2_volume -= 1;
            }
        }
    }
    
    // Channel 4
    let period = self.nr42 & 0x07;
    if period != 0 {  // ADD THIS CHECK
        if self.ch4_envelope_timer > 0 {
            self.ch4_envelope_timer -= 1;
        }
        if self.ch4_envelope_timer == 0 {
            self.ch4_envelope_timer = period;
            
            if (self.nr42 & 0x08) != 0 && self.ch4_volume < 15 {
                self.ch4_volume += 1;
            } else if (self.nr42 & 0x08) == 0 && self.ch4_volume > 0 {
                self.ch4_volume -= 1;
            }
        }
    }
}
    
    fn clock_sweep(&mut self) {
    if !self.ch1_sweep_enabled {
        return;
    }
    
    if self.ch1_sweep_timer > 0 {
        self.ch1_sweep_timer -= 1;
        
        if self.ch1_sweep_timer == 0 {
            let period = (self.nr10 >> 4) & 0x07;
            self.ch1_sweep_timer = if period == 0 { 8 } else { period };
            
            let is_subtraction = (self.nr10 & 0x08) != 0;
            
            // Set taint if sweep is actively processing AND in negate mode
            // "Actively processing" means period > 0
            if period > 0 && is_subtraction {
                self.ch1_sweep_neg_mode = true;
            }
            
            // Safety check
            if self.ch1_sweep_neg_mode && !is_subtraction {
                self.ch1_enabled = false;
                self.nr52 &= !0x01;
                return;
            }

            // Only process if period is non-zero
            if period != 0 {
                let shift = self.nr10 & 0x07;
                let new_freq = self.calculate_sweep_frequency();
                
                if new_freq > 2047 {
                    self.ch1_enabled = false;
                    self.nr52 &= !0x01;
                } else if shift > 0 {
                    self.ch1_sweep_shadow = new_freq;
                    self.nr13 = (new_freq & 0xFF) as u8;
                    self.nr14 = (self.nr14 & 0xF8) | ((new_freq >> 8) as u8 & 0x07);
                    
                    if self.calculate_sweep_frequency() > 2047 {
                        self.ch1_enabled = false;
                        self.nr52 &= !0x01;
                    }
                }
            }
        }
    }
}
    
   fn calculate_sweep_frequency(&self) -> u16 {
    let shift = self.nr10 & 0x07;
    // Right shift the current shadow frequency
    let delta = self.ch1_sweep_shadow >> shift;
    
    // Check Bit 3: 0 = Addition, 1 = Subtraction
    if (self.nr10 & 0x08) != 0 {
        self.ch1_sweep_shadow.wrapping_sub(delta)
    } else {
        self.ch1_sweep_shadow.wrapping_add(delta)
    }
}
    
    fn clock_channel1(&mut self) {
        if !self.ch1_enabled { return; }
        
        if self.ch1_frequency_timer > 0 {
            self.ch1_frequency_timer -= 1;
        } else {
            let frequency = ((self.nr14 as u16 & 0x07) << 8) | self.nr13 as u16;
            self.ch1_frequency_timer = (2048 - frequency) * 4;
            self.ch1_duty_position = (self.ch1_duty_position + 1) % 8;
        }
    }
    
    fn clock_channel2(&mut self) {
        if !self.ch2_enabled { return; }
        
        if self.ch2_frequency_timer > 0 {
            self.ch2_frequency_timer -= 1;
        } else {
            let frequency = ((self.nr24 as u16 & 0x07) << 8) | self.nr23 as u16;
            self.ch2_frequency_timer = (2048 - frequency) * 4;
            self.ch2_duty_position = (self.ch2_duty_position + 1) % 8;
        }
    }
    
    fn clock_channel3(&mut self) {
        if !self.ch3_enabled { return; }
        if (self.nr30 & 0x80) == 0 { return; }
        
        if self.ch3_frequency_timer > 0 {
            self.ch3_frequency_timer -= 1;
        } else {
            let frequency = ((self.nr34 as u16 & 0x07) << 8) | self.nr33 as u16;
            self.ch3_frequency_timer = (2048 - frequency) * 2;
            self.ch3_position = (self.ch3_position + 1) % 32;
        }
    }
    
    fn clock_channel4(&mut self) {
        if !self.ch4_enabled { return; }
        
        if self.ch4_frequency_timer > 0 {
            self.ch4_frequency_timer -= 1;
        } else {
            let divisor = match self.nr43 & 0x07 {
                0 => 8,
                n => (n as u16) * 16,
            };
            let shift = (self.nr43 >> 4) & 0x0F;
            self.ch4_frequency_timer = divisor << shift;
            
            let bit = (self.ch4_lfsr & 0x01) ^ ((self.ch4_lfsr >> 1) & 0x01);
            self.ch4_lfsr >>= 1;
            self.ch4_lfsr |= bit << 14;
            
            if (self.nr43 & 0x08) != 0 {
                self.ch4_lfsr &= !(1 << 6);
                self.ch4_lfsr |= bit << 6;
            }
        }
    }
    
    fn generate_sample(&mut self) {
        let mut left = 0.0;
        let mut right = 0.0;
        
        // Mix channel 1
        let ch1_output = if self.ch1_enabled {
            let duty_pattern = match self.nr11 >> 6 {
                0 => [0, 0, 0, 0, 0, 0, 0, 1], // 12.5%
                1 => [1, 0, 0, 0, 0, 0, 0, 1], // 25%
                2 => [1, 0, 0, 0, 0, 1, 1, 1], // 50%
                _ => [0, 1, 1, 1, 1, 1, 1, 0], // 75%
            };
            
            if duty_pattern[self.ch1_duty_position as usize] == 1 {
                (self.ch1_volume as f32) / 15.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        // Mix channel 2
        let ch2_output = if self.ch2_enabled {
            let duty_pattern = match self.nr21 >> 6 {
                0 => [0, 0, 0, 0, 0, 0, 0, 1],
                1 => [1, 0, 0, 0, 0, 0, 0, 1],
                2 => [1, 0, 0, 0, 0, 1, 1, 1],
                _ => [0, 1, 1, 1, 1, 1, 1, 0],
            };
            
            if duty_pattern[self.ch2_duty_position as usize] == 1 {
                (self.ch2_volume as f32) / 15.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        // Mix channel 3
        let ch3_output = if self.ch3_enabled && (self.nr30 & 0x80) != 0 {
            let sample_byte = self.wave_ram[self.ch3_position as usize / 2];
            let sample = if self.ch3_position % 2 == 0 {
                (sample_byte >> 4) & 0x0F
            } else {
                sample_byte & 0x0F
            };
            
            let volume_shift = match (self.nr32 >> 5) & 0x03 {
                0 => 4, // Mute
                1 => 0, // 100%
                2 => 1, // 50%
                _ => 2, // 25%
            };
            
            ((sample >> volume_shift) as f32) / 15.0
        } else {
            0.0
        };
        
        // Mix channel 4
        let ch4_output = if self.ch4_enabled {
            if (self.ch4_lfsr & 0x01) == 0 {
                (self.ch4_volume as f32) / 15.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        // Apply panning (NR51)
        if (self.nr51 & 0x01) != 0 { right += ch1_output; }
        if (self.nr51 & 0x10) != 0 { left += ch1_output; }
        if (self.nr51 & 0x02) != 0 { right += ch2_output; }
        if (self.nr51 & 0x20) != 0 { left += ch2_output; }
        if (self.nr51 & 0x04) != 0 { right += ch3_output; }
        if (self.nr51 & 0x40) != 0 { left += ch3_output; }
        if (self.nr51 & 0x08) != 0 { right += ch4_output; }
        if (self.nr51 & 0x80) != 0 { left += ch4_output; }
        
        // Apply master volume (NR50)
        let left_vol = ((self.nr50 >> 4) & 0x07) as f32 / 7.0;
        let right_vol = (self.nr50 & 0x07) as f32 / 7.0;
        
        left *= left_vol * 0.25;
        right *= right_vol * 0.25;
        
        // Interleaved stereo
        self.sample_buffer.push(left);
        self.sample_buffer.push(right);
    }
    
   pub fn write_register(&mut self, addr: u16, val: u8) {
        if (self.nr52 & 0x80) == 0 && addr != 0xFF26 {
            return; 
        }
        
        match addr {
            // ... (previous registers 0xFF10 - 0xFF13) ...
           // Inside write_register match statement:
0xFF10 => {
    let old_negate = (self.nr10 & 0x08) != 0;
    let new_negate = (val & 0x08) != 0;
    
    self.nr10 = val;
    
    // Don't set taint when writing to NR10
    // Only actual calculations in trigger_channel1 and clock_sweep should set it
    
    // CRITICAL: If we were in negate mode (and it was used),
    // switching to addition mode IMMEDIATELY disables the channel
    if self.ch1_sweep_neg_mode && old_negate && !new_negate {
        self.ch1_enabled = false;
        self.nr52 &= !0x01;
    }
    
    // Writing to NR10 can enable sweep ONLY if period was >0 at trigger
    let new_shift = val & 0x07;
    
    if !self.ch1_sweep_period_was_zero && new_shift > 0 {
        self.ch1_sweep_enabled = true;
    }
}
            0xFF11 => { self.nr11 = val; self.ch1_length_counter = 64 - (val & 0x3F); }
            0xFF12 => {
                self.nr12 = val;
                if (val & 0xF8) == 0 { self.ch1_enabled = false; self.nr52 &= !0x01; }
            }
            0xFF13 => self.nr13 = val,

            // CHANNEL 1 CONTROL
        // Inside write_register for 0xFF14 (and others)
0xFF14 => {
    let old_enable = (self.nr14 & 0x40) != 0;
    self.nr14 = val;
    let new_enable = (val & 0x40) != 0;
    let is_trigger = (val & 0x80) != 0;

    // Only clock if NOT triggering (trigger handles it separately now)
    // AND we are transitioning from Disable -> Enable
    // AND it's an odd frame
    if !is_trigger && !old_enable && new_enable && (self.frame_sequencer & 1) == 1 {
        if self.ch1_length_counter > 0 {
            self.ch1_length_counter -= 1;
            if self.ch1_length_counter == 0 {
                self.ch1_enabled = false;
                self.nr52 &= !0x01;
            }
        }
    }
    
    if is_trigger { self.trigger_channel1(); }
}
            // ... (0xFF16 - 0xFF18) ...
            0xFF16 => { self.nr21 = val; self.ch2_length_counter = 64 - (val & 0x3F); }
            0xFF17 => {
                self.nr22 = val;
                if (val & 0xF8) == 0 { self.ch2_enabled = false; self.nr52 &= !0x02; }
            }
            0xFF18 => self.nr23 = val,

            // CHANNEL 2 CONTROL
      0xFF19 => {
    let old_enable = (self.nr24 & 0x40) != 0;
    self.nr24 = val;
    let new_enable = (val & 0x40) != 0;
    let is_trigger = (val & 0x80) != 0;

    if !is_trigger && !old_enable && new_enable && (self.frame_sequencer & 1) == 1 {
        if self.ch2_length_counter > 0 {
            self.ch2_length_counter -= 1;
            if self.ch2_length_counter == 0 {
                self.ch2_enabled = false;
                self.nr52 &= !0x02;
            }
        }
    }
    
    if is_trigger { self.trigger_channel2(); }
}
            
            // ... (0xFF1A - 0xFF1D) ...
            0xFF1A => {
                self.nr30 = val;
                if (val & 0x80) == 0 { self.ch3_enabled = false; self.nr52 &= !0x04; }
            }
            0xFF1B => { self.nr31 = val; self.ch3_length_counter = 256 - val as u16; }
            0xFF1C => self.nr32 = val,
            0xFF1D => self.nr33 = val,

            // CHANNEL 3 CONTROL
       0xFF1E => {
    let old_enable = (self.nr34 & 0x40) != 0;
    self.nr34 = val;
    let new_enable = (val & 0x40) != 0;
    let is_trigger = (val & 0x80) != 0;

    if !is_trigger && !old_enable && new_enable && (self.frame_sequencer & 1) == 1 {
        if self.ch3_length_counter > 0 {
            self.ch3_length_counter -= 1;
            if self.ch3_length_counter == 0 {
                self.ch3_enabled = false;
                self.nr52 &= !0x04;
            }
        }
    }
    
    if is_trigger { self.trigger_channel3(); }
}
            // ... (0xFF20 - 0xFF22) ...
            0xFF20 => { self.nr41 = val; self.ch4_length_counter = 64 - (val & 0x3F); }
            0xFF21 => {
                self.nr42 = val;
                if (val & 0xF8) == 0 { self.ch4_enabled = false; self.nr52 &= !0x08; }
            }
            0xFF22 => self.nr43 = val,

            // CHANNEL 4 CONTROL
        0xFF23 => {
    let old_enable = (self.nr44 & 0x40) != 0;
    self.nr44 = val;
    let new_enable = (val & 0x40) != 0;
    let is_trigger = (val & 0x80) != 0;

    if !is_trigger && !old_enable && new_enable && (self.frame_sequencer & 1) == 1 {
        if self.ch4_length_counter > 0 {
            self.ch4_length_counter -= 1;
            if self.ch4_length_counter == 0 {
                self.ch4_enabled = false;
                self.nr52 &= !0x08;
            }
        }
    }
    
    if is_trigger { self.trigger_channel4(); }
}
            
            // ... (Rest of registers) ...
            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,
           // In apu.rs -> write_register

// In apu.rs -> impl APU -> write_register

0xFF26 => {
    let is_on = (val & 0x80) != 0;
    let was_on = (self.nr52 & 0x80) != 0;

    // Only bit 7 is writable to storage (bits 0-6 are read-only status)
    self.nr52 = (self.nr52 & 0x7F) | (val & 0x80);

    if is_on {
        if !was_on {
            // Powering ON
            // Reset Frame Sequencer. This is critical for test synchronization.
            self.frame_sequencer = 0;
            self.frame_sequencer_timer = 0;
            
            // IMPORTANT: Do NOT reset chX_length_counter variables here for DMG compliance.
            // CGB would reset them here.
        }
    } else {
        if was_on {
            // Powering OFF
            // 1. Clear Global Registers
            self.nr50 = 0;
            self.nr51 = 0;

            // 2. Clear Channel Registers, BUT PRESERVE Length (NRx1)
            // Channel 1
            self.nr10 = 0;
            // self.nr11 = 0; // PRESERVED
            self.nr12 = 0; self.nr13 = 0; self.nr14 = 0;
            
            // Channel 2
            // self.nr21 = 0; // PRESERVED
            self.nr22 = 0; self.nr23 = 0; self.nr24 = 0;
            
            // Channel 3
            self.nr30 = 0;
            // self.nr31 = 0; // PRESERVED
            self.nr32 = 0; self.nr33 = 0; self.nr34 = 0;
            // Channel 3 Wave RAM is also preserved on DMG!
            
            // Channel 4
            // self.nr41 = 0; // PRESERVED
            self.nr42 = 0; self.nr43 = 0; self.nr44 = 0;

            // 3. Disable Channels Internal Flags
            self.ch1_enabled = false;
            self.ch2_enabled = false;
            self.ch3_enabled = false;
            self.ch4_enabled = false;
            
            // 4. CRITICAL: Do NOT clear chX_length_counter variables
        }
    }
}
           0xFF30..=0xFF3F => {
    // If channel 3 is playing, write to the current position instead
    if self.ch3_enabled && (self.nr30 & 0x80) != 0 {
        self.wave_ram[self.ch3_position as usize / 2] = val;
    } else {
        self.wave_ram[(addr - 0xFF30) as usize] = val;
    }
}
            _ => {}
        }
    }
    
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => self.nr10 | 0x80,
            0xFF11 => self.nr11 | 0x3F,
            0xFF12 => self.nr12,
            0xFF13 => 0xFF,
            0xFF14 => self.nr14 | 0xBF,
            
            0xFF16 => self.nr21 | 0x3F,
            0xFF17 => self.nr22,
            0xFF18 => 0xFF,
            0xFF19 => self.nr24 | 0xBF,
            
            0xFF1A => self.nr30 | 0x7F,
            0xFF1B => 0xFF,
            0xFF1C => self.nr32 | 0x9F,
            0xFF1D => 0xFF,
            0xFF1E => self.nr34 | 0xBF,
            
            0xFF20 => 0xFF,
            0xFF21 => self.nr42,
            0xFF22 => self.nr43,
            0xFF23 => self.nr44 | 0xBF,
            
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            // FIX: OR with 0x70 to ensure unused bits 4, 5, 6 read as 1
           // In apu.rs -> impl APU -> read_register

0xFF26 => {
    // 1. Start with the Power Bit (Bit 7) and bits 4-6 (always 1)
    let mut val = (self.nr52 & 0x80) | 0x70;
    
    // 2. Dynamically calculate status bits based on actual enable flags
    if self.ch1_enabled { val |= 0x01; }
    if self.ch2_enabled { val |= 0x02; }
    if self.ch3_enabled { val |= 0x04; }
    if self.ch4_enabled { val |= 0x08; }
    
    val
}
            
            0xFF30..=0xFF3F => {
            // If channel 3 is playing, return the sample being read
            if self.ch3_enabled && (self.nr30 & 0x80) != 0 {
                self.wave_ram[self.ch3_position as usize / 2]
            } else {
                self.wave_ram[(addr - 0xFF30) as usize]
            }
        }
            
            _ => 0xFF,
        }
    }
    
    // Replace your trigger functions with these corrected versions:

// The key insight: Extra length clocking happens when:
// 1. Triggering on an EVEN frame step (0, 2, 4, 6) with length enabled
// 2. Enabling length (0→1 transition) on an ODD frame step (1, 3, 5, 7)

// But CRITICALLY: The extra clock should happen BEFORE setting the status bit!

fn trigger_channel1(&mut self) {
    // ... [Length Logic] ...
    if self.ch1_length_counter == 0 {
        self.ch1_length_counter = 64;
        if (self.frame_sequencer & 1) == 1 && (self.nr14 & 0x40) != 0 {
            self.ch1_length_counter = 63;
        }
    }

    // ... [Frequency Logic] ...
    let frequency = ((self.nr14 as u16 & 0x07) << 8) | self.nr13 as u16;
    self.ch1_frequency_timer = (2048 - frequency) * 4;
    self.ch1_volume = self.nr12 >> 4;
    let env_period = self.nr12 & 0x07;
    self.ch1_envelope_timer = if env_period == 0 { 8 } else { env_period };

    // ... [Sweep Init] ...
    self.ch1_sweep_shadow = frequency;
    let sweep_period = (self.nr10 >> 4) & 0x07;
    let sweep_shift = self.nr10 & 0x07;
    self.ch1_sweep_timer = if sweep_period == 0 { 8 } else { sweep_period };

 // 1. CLEAR NEGATE MODE FLAG
// Don't set it at trigger - only set it when a subtraction actually happens
self.ch1_sweep_neg_mode = false;

// 2. TRACK IF PERIOD WAS ZERO AT TRIGGER
self.ch1_sweep_period_was_zero = sweep_period == 0;

// 3. SET ENABLE LATCH
// Sweep is enabled if Period OR Shift are non-zero.
// If BOTH are zero, sweep is disabled.
self.ch1_sweep_enabled = sweep_period != 0 || sweep_shift != 0;

// 4. IMMEDIATE OVERFLOW CHECK
// Only happens if Shift > 0
let mut overflow = false;
if sweep_shift > 0 {
    let new_freq = self.calculate_sweep_frequency();
    if new_freq > 2047 {
        overflow = true;
    } else {
        // Mark as tainted if this initial calculation used subtraction
        let is_subtraction = (self.nr10 & 0x08) != 0;
        if is_subtraction {
            self.ch1_sweep_neg_mode = true;
        }
    }
}

    // ... [Enable Channel] ...
    if (self.nr12 & 0xF8) != 0 {
        if overflow {
            self.ch1_enabled = false;
            self.nr52 &= !0x01;
        } else {
            self.ch1_enabled = true;
            self.nr52 |= 0x01;
        }
    } else {
        self.ch1_enabled = false;
    }
}

fn trigger_channel2(&mut self) {
    // 1. RELOAD LENGTH
    if self.ch2_length_counter == 0 {
        self.ch2_length_counter = 64;
        if (self.frame_sequencer & 1) == 1 && (self.nr24 & 0x40) != 0 {
            self.ch2_length_counter = 63;
        }
    }

    // 2. RELOAD FREQUENCY & TIMERS
    let frequency = ((self.nr24 as u16 & 0x07) << 8) | self.nr23 as u16;
    self.ch2_frequency_timer = (2048 - frequency) * 4;
    
    self.ch2_volume = self.nr22 >> 4;
    let period = self.nr22 & 0x07;
    self.ch2_envelope_timer = if period == 0 { 8 } else { period };

    // 3. ENABLE CHANNEL (Only if DAC is ON)
    if (self.nr22 & 0xF8) != 0 {
        self.ch2_enabled = true;
        self.nr52 |= 0x02;
    } else {
        self.ch2_enabled = false;
    }
}

fn trigger_channel3(&mut self) {
    // 1. RELOAD LENGTH (Note: Channel 3 is 256 ticks long)
    if self.ch3_length_counter == 0 {
        self.ch3_length_counter = 256;
        if (self.frame_sequencer & 1) == 1 && (self.nr34 & 0x40) != 0 {
            self.ch3_length_counter = 255;
        }
    }

    // 2. RELOAD FREQUENCY & TIMERS
    let frequency = ((self.nr34 as u16 & 0x07) << 8) | self.nr33 as u16;
    self.ch3_frequency_timer = (2048 - frequency) * 2;
    self.ch3_position = 0; // Wave channel resets position to 0

    // 3. ENABLE CHANNEL (Only if DAC is ON)
    // Channel 3 DAC is controlled by Bit 7 of NR30
    if (self.nr30 & 0x80) != 0 {
        self.ch3_enabled = true;
        self.nr52 |= 0x04;
    } else {
        self.ch3_enabled = false;
    }
}

fn trigger_channel4(&mut self) {
    // 1. RELOAD LENGTH
    if self.ch4_length_counter == 0 {
        self.ch4_length_counter = 64;
        if (self.frame_sequencer & 1) == 1 && (self.nr44 & 0x40) != 0 {
            self.ch4_length_counter = 63;
        }
    }

    // 2. RELOAD FREQUENCY & TIMERS
    self.ch4_lfsr = 0x7FFF; // Reset LFSR
    self.ch4_volume = self.nr42 >> 4;
    let period = self.nr42 & 0x07;
    self.ch4_envelope_timer = if period == 0 { 8 } else { period };
    
    // Recalculate frequency timer based on polynomial
    let divisor = match self.nr43 & 0x07 {
        0 => 8,
        n => (n as u16) * 16,
    };
    let shift = (self.nr43 >> 4) & 0x0F;
    self.ch4_frequency_timer = divisor << shift;

    // 3. ENABLE CHANNEL (Only if DAC is ON)
    if (self.nr42 & 0xF8) != 0 {
        self.ch4_enabled = true;
        self.nr52 |= 0x08;
    } else {
        self.ch4_enabled = false;
    }
}

// Keep clock_length as is - it's correct:
fn clock_length(&mut self) {
    // Channel 1
    // Only check if NRx4 Bit 6 (Length Enable) is set
    if (self.nr14 & 0x40) != 0 && self.ch1_length_counter > 0 {
        self.ch1_length_counter -= 1;
        
        // If the counter reaches 0, THEN we disable the channel
        if self.ch1_length_counter == 0 {
            self.ch1_enabled = false;
            self.nr52 &= !0x01; // Clear Channel 1 status bit
        }
    }
    
    // Channel 2
    if (self.nr24 & 0x40) != 0 && self.ch2_length_counter > 0 {
        self.ch2_length_counter -= 1;
        
        if self.ch2_length_counter == 0 {
            self.ch2_enabled = false;
            self.nr52 &= !0x02; // Clear Channel 2 status bit
        }
    }
    
    // Channel 3
    if (self.nr34 & 0x40) != 0 && self.ch3_length_counter > 0 {
        self.ch3_length_counter -= 1;
        
        if self.ch3_length_counter == 0 {
            self.ch3_enabled = false;
            self.nr52 &= !0x04; // Clear Channel 3 status bit
        }
    }
    
    // Channel 4
    if (self.nr44 & 0x40) != 0 && self.ch4_length_counter > 0 {
        self.ch4_length_counter -= 1;
        
        if self.ch4_length_counter == 0 {
            self.ch4_enabled = false;
            self.nr52 &= !0x08; // Clear Channel 4 status bit
        }
    }
}

// NRx4 write handlers - keep the ODD frame check for non-trigger:
// These stay the same as before with the (frame_sequencer & 1) == 1 check
    
    pub fn get_samples(&mut self) -> Vec<f32> {
        let samples = self.sample_buffer.clone();
        self.sample_buffer.clear();
        samples
    }
}