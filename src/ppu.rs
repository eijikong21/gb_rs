pub struct PPU {
    pub frame_buffer: [u32; 160 * 144],
    pub mode_clock: u32,
    pub window_line_counter: u8,
}

impl PPU {
    fn render_window(&mut self, mmu: &crate::mmu::MMU) {
        if (mmu.lcdc & 0x20) == 0 { return; }

        let ly = mmu.ly;
        let wy = mmu.wy;
        let wx = mmu.wx;
        
        // Don't render if we haven't reached the window Y position yet
        if ly < wy { return; }
        
        // WX range: 0-166 are valid. WX=7 means window starts at screen X=0
        if wx >= 167 { return; }

        let tile_map_base: u16 = if (mmu.lcdc & 0x40) != 0 { 0x9C00 } else { 0x9800 };
        let window_ly = self.window_line_counter; 
        let tile_row = (window_ly as u16 / 8) * 32;

        // Track if we actually rendered any pixels (important for counter increment)
        let mut rendered_pixels = false;

        for x in 0..160u8 {
            // Window starts at WX - 7
            // So if WX=7, window starts at x=0
            // If WX=0, window would start at x=-7 (off screen)
            if x + 7 < wx { continue; }

            rendered_pixels = true;
            let window_x = (x + 7 - wx) as u8;
            let tile_col = window_x as u16 / 8;
            
            let tile_address = tile_map_base + tile_row + tile_col;
            let tile_id = mmu.read_byte(tile_address);

            let tile_data_address = self.get_tile_data_addr(mmu, tile_id, window_ly % 8);
            let byte1 = mmu.read_byte(tile_data_address);
            let byte2 = mmu.read_byte(tile_data_address + 1);

            let bit_idx = 7 - (window_x % 8);
            let color_id = ((byte2 >> bit_idx) & 0x01) << 1 | ((byte1 >> bit_idx) & 0x01);

            let color = self.get_color(mmu.bgp, color_id);
            self.frame_buffer[ly as usize * 160 + x as usize] = color;
        }

        // CRITICAL: Only increment if we actually rendered window pixels on this scanline
        if rendered_pixels {
            self.window_line_counter += 1;
        }
    }

    fn render_sprites(&mut self, mmu: &crate::mmu::MMU) {
        if (mmu.lcdc & 0x02) == 0 { return; }
        let sprite_height = if (mmu.lcdc & 0x04) != 0 { 16 } else { 8 };

        for i in (0..40).rev() {
            let oam_addr = 0xFE00 + (i * 4);
            
            let y_pos = mmu.read_byte(oam_addr).wrapping_sub(16);
            let x_pos = mmu.read_byte(oam_addr + 1).wrapping_sub(8);
            let mut tile_id = mmu.read_byte(oam_addr + 2);
            let attributes = mmu.read_byte(oam_addr + 3);

            if sprite_height == 16 {
                tile_id &= 0xFE;
            }

            let ly = mmu.ly;

            if ly >= y_pos && ly < y_pos + sprite_height {
                let mut row = ly - y_pos;
                
                if (attributes & 0x40) != 0 {
                    row = (sprite_height - 1) - row;
                }

                let current_tile = if sprite_height == 16 && row >= 8 {
                    tile_id + 1
                } else {
                    tile_id
                };
                
                let tile_row = row % 8;
                let tile_data_addr = 0x8000 + (current_tile as u16 * 16) + (tile_row as u16 * 2);
                let byte1 = mmu.read_byte(tile_data_addr);
                let byte2 = mmu.read_byte(tile_data_addr + 1);
                let behind_bg = (attributes & 0x80) != 0;

                for x in 0..8 {
                    let bit_idx = if (attributes & 0x20) != 0 { x } else { 7 - x };
                    let low_bit = (byte1 >> bit_idx) & 0x01;
                    let high_bit = (byte2 >> bit_idx) & 0x01;
                    let color_id = (high_bit << 1) | low_bit;

                    if color_id != 0 {
                        let screen_x = x_pos.wrapping_add(x as u8);
                        if screen_x < 160 {
                            let pixel_index = ly as usize * 160 + screen_x as usize;
                            
                            if behind_bg {
                                let current_pixel = self.frame_buffer[pixel_index];
                                if current_pixel != 0xFFFFFFFF {
                                    continue;
                                }
                            }
                            
                            let palette = if (attributes & 0x10) != 0 { mmu.obp1 } else { mmu.obp0 };
                            let color = self.get_color(palette, color_id);
                            self.frame_buffer[pixel_index] = color;
                        }
                    }
                }
            }
        }
    }

    fn render_background(&mut self, mmu: &crate::mmu::MMU) {
        let ly = mmu.ly;
        let scy = mmu.scy;
        let scx = mmu.scx;

        let tile_map_base: u16 = if (mmu.lcdc & 0x08) != 0 { 0x9C00 } else { 0x9800 };
        let y_pos = scy.wrapping_add(ly);
        let tile_row = (y_pos as u16 / 8) * 32;

        for x in 0..160u8 {
            let x_pos = x.wrapping_add(scx);
            let tile_col = x_pos as u16 / 8;
            
            let tile_address = tile_map_base + tile_row + tile_col;
            let tile_id = mmu.read_byte(tile_address);

            let tile_data_address = self.get_tile_data_addr(mmu, tile_id, y_pos % 8);

            let byte1 = mmu.read_byte(tile_data_address);
            let byte2 = mmu.read_byte(tile_data_address + 1);

            let bit_idx = 7 - (x_pos % 8);
            let low_bit = (byte1 >> bit_idx) & 0x01;
            let high_bit = (byte2 >> bit_idx) & 0x01;
            let color_id = (high_bit << 1) | low_bit;

            let color = self.get_color(mmu.bgp, color_id);
            self.frame_buffer[ly as usize * 160 + x as usize] = color;
        }
    }

    fn get_tile_data_addr(&self, mmu: &crate::mmu::MMU, tile_id: u8, row: u8) -> u16 {
        let is_signed = (mmu.lcdc & 0x10) == 0;
        if !is_signed {
            0x8000 + (tile_id as u16 * 16) + (row as u16 * 2)
        } else {
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
            window_line_counter: 0,
        }
    }

    pub fn tick(&mut self, mmu: &mut crate::mmu::MMU, cycles: u8) {
        if (mmu.lcdc & 0x80) == 0 {
            mmu.ly = 0;
            self.mode_clock = 0;
            mmu.stat &= 0xFC;
            return;
        }
               
        self.mode_clock += cycles as u32;    
        let current_mode = mmu.stat & 0x03;

        if self.mode_clock >= 456 {
            self.mode_clock -= 456;
            mmu.ly = (mmu.ly + 1) % 154;

            if mmu.ly == 0 {
                self.window_line_counter = 0;
            }
            
            if mmu.ly == mmu.lyc {
                mmu.stat |= 0x04;
                if (mmu.stat & 0x40) != 0 { mmu.interrupt_flag |= 0x02; }
            } else {
                mmu.stat &= !0x04;
            }

            if mmu.ly >= 144 {
                if current_mode != 1 {
                    self.set_mode(mmu, 1);
                    mmu.interrupt_flag |= 0x01;
                }
            }
        }

        if mmu.ly < 144 {
            if self.mode_clock <= 80 {
                if current_mode != 2 { 
                    self.set_mode(mmu, 2); 
                }
            } else if self.mode_clock <= 80 + 172 {
                if current_mode != 3 { 
                    self.set_mode(mmu, 3);
                    self.render_background(mmu);
                    self.render_window(mmu);
                    self.render_sprites(mmu);
                }
            } else {
                if current_mode != 0 { 
                    self.set_mode(mmu, 0); 
                }
            }
        }
    }

    fn set_mode(&self, mmu: &mut crate::mmu::MMU, mode: u8) {
        mmu.stat = (mmu.stat & 0xFC) | mode;

        let interrupt_requested = match mode {
            0 => (mmu.stat & 0x08) != 0, // H-Blank
            1 => (mmu.stat & 0x10) != 0, // V-Blank
            2 => (mmu.stat & 0x20) != 0, // OAM
            _ => false,
        };

        if interrupt_requested {
            mmu.interrupt_flag |= 0x02;
        }
    }
}