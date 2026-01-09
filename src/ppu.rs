pub struct PPU {
    pub frame_buffer: [u32; 160 * 144],
    pub mode_clock: u32,
}

impl PPU {
    fn render_background(&mut self, mmu: &crate::mmu::MMU) {
        let ly = mmu.ly;
        let scy = mmu.scy;
        let scx = mmu.scx;

        // 1. Determine which Tile Map to use (Bit 3 of LCDC)
        // 0x9800 or 0x9C00
        let tile_map_base: u16 = if (mmu.lcdc & 0x08) != 0 { 0x9C00 } else { 0x9800 };

        // 2. Find the vertical position inside the tile map (0-255)
        let y_pos = scy.wrapping_add(ly);
        let tile_row = (y_pos as u16 / 8) * 32; // Which of the 32 rows of tiles?

        for x in 0..160u8 {
            let x_pos = x.wrapping_add(scx);
            let tile_col = x_pos as u16 / 8;
            
            // Get the Tile ID from the Tile Map
            let tile_address = tile_map_base + tile_row + tile_col;
            let tile_id = mmu.read_byte(tile_address);

            // 3. Find the Tile Data address based on Tile ID
            // This depends on LCDC Bit 4 (Addressing Mode)
            let tile_data_address = self.get_tile_data_addr(mmu, tile_id, y_pos % 8);

            // 4. Fetch the two bytes for the current row
            let byte1 = mmu.read_byte(tile_data_address);
            let byte2 = mmu.read_byte(tile_data_address + 1);

            // 5. Extract the specific pixel color (0-3)
            let bit_idx = 7 - (x_pos % 8);
            let low_bit = (byte1 >> bit_idx) & 0x01;
            let high_bit = (byte2 >> bit_idx) & 0x01;
            let color_id = (high_bit << 1) | low_bit;

            // 6. Map the color ID through the Palette (BGP) and write to buffer
            let color = self.get_color(mmu.bgp, color_id);
            self.frame_buffer[ly as usize * 160 + x as usize] = color;
        }
    }

    fn get_tile_data_addr(&self, mmu: &crate::mmu::MMU, tile_id: u8, row: u8) -> u16 {
        let is_signed = (mmu.lcdc & 0x10) == 0;
        if !is_signed {
            // Mode 1: 0x8000 - 0x8FFF (Unsigned)
            0x8000 + (tile_id as u16 * 16) + (row as u16 * 2)
        } else {
            // Mode 0: 0x8800 - 0x97FF (Signed ID, 0x9000 is base)
            let offset = (tile_id as i8 as i16) + 128;
            0x8800 + (offset as u16 * 16) + (row as u16 * 2)
        }
    }
    fn get_color(&self, palette: u8, color_id: u8) -> u32 {
        let hi = (color_id << 1) + 1;
        let lo = color_id << 1;
        let actual_color = ((palette >> hi) & 0x01) << 1 | ((palette >> lo) & 0x01);

        match actual_color {
            0 => 0xFFFFFFFF, // White
            1 => 0xFFAAAAAA, // Light Gray
            2 => 0xFF555555, // Dark Gray
            _ => 0xFF000000, // Black
        }
    }
    pub fn new() -> Self {
        Self {
            frame_buffer: [0xFFFFFFFF; 160 * 144],
            mode_clock: 0,
        }
    }

    pub fn tick(&mut self, mmu: &mut crate::mmu::MMU, cycles: u8) {
        // If LCD is disabled (Bit 7 of LCDC), reset PPU state
        if (mmu.lcdc & 0x80) == 0 {
            mmu.ly = 0;
            self.mode_clock = 0;
            mmu.stat &= 0xFC; // Clear mode bits
            return;
        }
               
        self.mode_clock += cycles as u32;    

        let current_mode = mmu.stat & 0x03;

        // One scanline is 456 cycles
        if self.mode_clock >= 456 {
            self.mode_clock -= 456;
            mmu.ly = (mmu.ly + 1) % 154;

            // Check for LYC coincidence
            if mmu.ly == mmu.lyc {
                mmu.stat |= 0x04; // Set Coincidence Flag
                if (mmu.stat & 0x40) != 0 { mmu.interrupt_flag |= 0x02; } // STAT Interrupt
            } else {
                mmu.stat &= !0x04;
            }

            if mmu.ly >= 144 {
                // Entering V-Blank
                if current_mode != 1 {
                    self.set_mode(mmu, 1);
                    mmu.interrupt_flag |= 0x01; // Request V-Blank Interrupt
                }
            }
        }

        // Mode Switching Logic for visible lines (0-143)
        if mmu.ly < 144 {
            if self.mode_clock <= 80 {
                if current_mode != 2 { self.set_mode(mmu, 2); }
            } else if self.mode_clock <= 80 + 172 {
                if current_mode != 3 { self.set_mode(mmu, 3);
                     self.render_background(mmu); // Draw the line once per scanline}
            } else {
                if current_mode != 0 { self.set_mode(mmu, 0); }
            }
        }
    }
}

    fn set_mode(&self, mmu: &mut crate::mmu::MMU, mode: u8) {
        mmu.stat = (mmu.stat & 0xFC) | mode;

        // Handle STAT Interrupts (Selection bits 3, 4, 5)
        let interrupt_requested = match mode {
            0 => (mmu.stat & 0x08) != 0, // H-Blank
            1 => (mmu.stat & 0x10) != 0, // V-Blank
            2 => (mmu.stat & 0x20) != 0, // OAM
            _ => false,
        };

        if interrupt_requested {
            mmu.interrupt_flag |= 0x02; // Request STAT Interrupt
        }
    }
}