use crate::mmu::MMU;

pub struct Registers {
    pub a: u8, pub f: u8,
    pub b: u8, pub c: u8,
    pub d: u8, pub e: u8,
    pub h: u8, pub l: u8,
    pub pc: u16,
    pub sp: u16,
}

pub struct CPU {
    pub registers: Registers,
    pub bus: MMU,
    pub ime: bool, // Interrupt Master Enable
    pub halted: bool, // 2. Add this too (you'll need it for the HALT instruction soon)
    pub interrupt_enable_delay: bool, // Shadow flag for EI delay
}

impl CPU {
    fn sra_8bit(&mut self, val: u8) -> u8 {
    let carry = (val & 0x01) != 0;       // Check bit 0
    let res = (val >> 1) | (val & 0x80); // Shift right, preserve bit 7

    self.registers.f = 0;
    if res == 0 { self.registers.f |= 0x80; } // Z flag
    if carry { self.registers.f |= 0x10; }    // C flag
    // N and H are already 0

    res
}
    fn sla_8bit(&mut self, val: u8) -> u8 {
    let carry = (val & 0x80) != 0; // Check bit 7
    let res = val << 1;            // Shift left (bit 0 becomes 0)

    self.registers.f = 0;
    if res == 0 { self.registers.f |= 0x80; } // Z
    if carry { self.registers.f |= 0x10; }    // C
    // N and H are already 0

    res
}
    fn sbc_8bit(&mut self, val: u8) {
    let a = self.registers.a;
    let c_in = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
    
    // Calculate the result
    let res = a.wrapping_sub(val).wrapping_sub(c_in);

    // Update Flags: N=1
    self.registers.f = 0x40; 
    if res == 0 { self.registers.f |= 0x80; } // Z
    
    // Half-Carry: borrow from bit 4
    if (a as i32 & 0xF) - (val as i32 & 0xF) - (c_in as i32) < 0 {
        self.registers.f |= 0x20;
    }
    
    // Carry: borrow from bit 8 (if total value is negative)
    if (a as i32) - (val as i32) - (c_in as i32) < 0 {
        self.registers.f |= 0x10;
    }

    self.registers.a = res;
}
    fn rst(&mut self, address: u16) -> u8 {
    let pc = self.registers.pc;
    self.push_u16(pc);
    self.registers.pc = address;
    16 // RST always takes 16 cycles
}
    pub fn handle_interrupts(&mut self) {
    // 1. Get the pending interrupts
    let fired = self.bus.interrupt_flag & self.bus.interrupt_enable & 0x1F;

    // 2. WAKE UP logic: If any interrupt is pending, clear the halted flag
    // This happens regardless of whether IME is true or false!
    if fired != 0 {
        self.halted = false; 
    } else {
        return; // No interrupts pending, nothing to do
    }

    // 3. SERVICE logic: Only jump to the vector if IME is actually enabled
    if !self.ime { 
        return; 
    }

    // 4. If we got here, we are actually performing the jump
    self.ime = false; // Disable interrupts globally
    
    for i in 0..5 {
        if (fired & (1 << i)) != 0 {
            // Clear the interrupt bit in IF
            self.bus.interrupt_flag &= !(1 << i);
            
            // Push current PC to stack
            let pc = self.registers.pc;
            self.push_u16(pc);
            
            // Jump to the interrupt vector
            self.registers.pc = match i {
                0 => 0x0040, // V-Blank
                1 => 0x0048, // LCD STAT
                2 => 0x0050, // Timer
                3 => 0x0058, // Serial
                4 => 0x0060, // Joypad
                _ => unreachable!(),
            };
            break;
        }
    }
}
fn daa(&mut self) {
    let mut a = self.registers.a as u16;
    let n_flag = (self.registers.f & 0x40) != 0;
    let h_flag = (self.registers.f & 0x20) != 0;
    let c_flag = (self.registers.f & 0x10) != 0;

    if !n_flag {
        // After Addition
        if c_flag || a > 0x99 {
            a = a.wrapping_add(0x60);
            self.registers.f |= 0x10; // Set Carry
        }
        if h_flag || (a & 0x0F) > 0x09 {
            a = a.wrapping_add(0x06);
        }
    } else {
        // After Subtraction
        if c_flag {
            a = a.wrapping_sub(0x60);
        }
        if h_flag {
            a = a.wrapping_sub(0x06);
        }
    }

    // Update Flags: Z - 0 -
    self.registers.f &= 0x50; // Keep N and C, clear Z and H
    if (a as u8) == 0 {
        self.registers.f |= 0x80; // Set Z
    }
    
    self.registers.a = a as u8;
}
    fn add_hl(&mut self, value: u16) {
    let hl = self.get_hl();
    let res = hl.wrapping_add(value);
    
    // N (Subtract) is always cleared to 0
    self.registers.f &= 0x80; // Keep Z (Bit 7), clear N, H, C
    
    // H: Half-Carry from bit 11 to bit 12
    if (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF {
        self.registers.f |= 0x20;
    }
    
    // C: Carry from bit 15 to 16
    if (hl as u32) + (value as u32) > 0xFFFF {
        self.registers.f |= 0x10;
    }
    
    self.set_hl(res);
}
    fn adc_a(&mut self, value: u8) {
    let a = self.registers.a;
    let c = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
    let res = a.wrapping_add(value).wrapping_add(c);
    
    self.registers.f = 0; // Clear all flags
    
    // Z: Zero Flag
    if res == 0 { self.registers.f |= 0x80; }
    
    // H: Half-Carry (Carry from bit 3 to bit 4)
    if (a & 0x0F) + (value & 0x0F) + c > 0x0F {
        self.registers.f |= 0x20;
    }
    
    // C: Carry (Carry from bit 7 to bit 8)
    if (a as u16) + (value as u16) + (c as u16) > 0xFF {
        self.registers.f |= 0x10;
    }
    
    self.registers.a = res;
}
    fn execute_cb(&mut self, cb_opcode: u8) -> u8 {
    let bit = (cb_opcode >> 3) & 0x07; // Which bit (0-7)
    let reg_idx = cb_opcode & 0x07;    // Which register index
    let val = self.get_reg_by_index(reg_idx);

    match cb_opcode {
        // Inside execute_cb match
0x28 => { self.registers.b = self.sra_8bit(self.registers.b); }
0x29 => { self.registers.c = self.sra_8bit(self.registers.c); }
0x2A => { self.registers.d = self.sra_8bit(self.registers.d); }
0x2B => { self.registers.e = self.sra_8bit(self.registers.e); }
0x2C => { self.registers.h = self.sra_8bit(self.registers.h); }
0x2D => { self.registers.l = self.sra_8bit(self.registers.l); }
0x2E => {
    let addr = self.get_hl();
    let val = self.bus.read_byte(addr);
    let res = self.sra_8bit(val);
    self.bus.write_byte(addr, res);
}
0x2F => { self.registers.a = self.sra_8bit(self.registers.a); }
       0x20 => { self.registers.b = self.sla_8bit(self.registers.b); }
    0x21 => { self.registers.c = self.sla_8bit(self.registers.c); }
    0x22 => { self.registers.d = self.sla_8bit(self.registers.d); }
    0x23 => { self.registers.e = self.sla_8bit(self.registers.e); }
    0x24 => { self.registers.h = self.sla_8bit(self.registers.h); }
    0x25 => { self.registers.l = self.sla_8bit(self.registers.l); }
    0x26 => {
        let addr = self.get_hl();
        let val = self.bus.read_byte(addr);
        let res = self.sla_8bit(val);
        self.bus.write_byte(addr, res);
    }
    0x27 => { self.registers.a = self.sla_8bit(self.registers.a); }
        0x00..=0x07 => {
        let carry = (val & 0x80) >> 7;
        let res = (val << 1) | carry;
        self.registers.f = if carry == 1 { 0x10 } else { 0 };
        if res == 0 { self.registers.f |= 0x80; }
        self.set_reg_by_index(reg_idx, res);
    },

    // 0x08 - 0x0F: RRC r (Rotate Right)
    0x08..=0x0F => {
        let carry = val & 0x01;
        let res = (val >> 1) | (carry << 7);
        self.registers.f = if carry == 1 { 0x10 } else { 0 };
        if res == 0 { self.registers.f |= 0x80; }
        self.set_reg_by_index(reg_idx, res);
    },

    // 0x10 - 0x17: RL r (Rotate Left through Carry)
    0x10..=0x17 => {
        let old_carry = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
        let new_carry = (val & 0x80) >> 7;
        let res = (val << 1) | old_carry;
        self.registers.f = if new_carry == 1 { 0x10 } else { 0 };
        if res == 0 { self.registers.f |= 0x80; }
        self.set_reg_by_index(reg_idx, res);
    },

    // 0x18 - 0x1F: RR r (Rotate Right through Carry)
    0x18..=0x1F => {
        let old_carry = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
        let new_carry = val & 0x01;
        let res = (val >> 1) | (old_carry << 7);
        self.registers.f = if new_carry == 1 { 0x10 } else { 0 };
        if res == 0 { self.registers.f |= 0x80; }
        self.set_reg_by_index(reg_idx, res);
    },

    // 0x38 - 0x3F: SRL r (Shift Right Logical - The one you hit!)
    0x38..=0x3F => {
        let carry = val & 0x01;
        let res = val >> 1; // High bit always becomes 0
        self.registers.f = if carry == 1 { 0x10 } else { 0 };
        if res == 0 { self.registers.f |= 0x80; }
        self.set_reg_by_index(reg_idx, res);
    },
        // 0x40..=0x7F: BIT n, r (Test bit n in register r)
        0x40..=0x7F => {
            let is_set = (val & (1 << bit)) != 0;
            self.registers.f &= 0x10; // Keep Carry, clear others
            self.registers.f |= 0x20; // H flag is ALWAYS set for BIT
            if !is_set { self.registers.f |= 0x80; } // Set Z if bit is 0
        },

        // 0x80..=0xBF: RES n, r (Reset bit n)
        0x80..=0xBF => {
            let res = val & !(1 << bit);
            self.set_reg_by_index(reg_idx, res);
        },

        // 0xC0..=0xFF: SET n, r (Set bit n)
        0xC0..=0xFF => {
            let res = val | (1 << bit);
            self.set_reg_by_index(reg_idx, res);
        },

        // 0x10..=0x17: RL r (Rotate Left through Carry)
        0x10..=0x17 => {
            let old_carry = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
            let new_carry = (val & 0x80) >> 7;
            let res = (val << 1) | old_carry;
            
            self.registers.f = 0;
            if res == 0 { self.registers.f |= 0x80; }
            if new_carry == 1 { self.registers.f |= 0x10; }
            self.set_reg_by_index(reg_idx, res);
        },
        0x30..=0x37 => {
        let low_nibble = val & 0x0F;
        let high_nibble = (val & 0xF0) >> 4;
        let res = (low_nibble << 4) | high_nibble;
        
        // Flags: Z 0 0 0
        self.registers.f = if res == 0 { 0x80 } else { 0 };
        self.set_reg_by_index(reg_idx, res);
    },

        _ => {
            println!("CB Opcode not yet implemented: {:#04X}", cb_opcode);
            panic!("CB CRASH");
        }
    }

    // Timing: (HL) takes more cycles
    if reg_idx == 6 {
        if (0x40..=0x7F).contains(&cb_opcode) { 12 } else { 16 }
    } else { 8 }
}
    fn sbc_a(&mut self, value: u8) {
    let a = self.registers.a;
    let c = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
    let res = a.wrapping_sub(value).wrapping_sub(c);
    
    self.registers.f = 0x40; // Set N
    
    if res == 0 { self.registers.f |= 0x80; } // Z
    
    // H: Set if (a & 0xf) - (value & 0xf) - c < 0
    if (a as i32 & 0x0F) - (value as i32 & 0x0F) - (c as i32) < 0 {
        self.registers.f |= 0x20;
    }
    
    // C: Set if a - value - c < 0
    if (a as i32) - (value as i32) - (c as i32) < 0 {
        self.registers.f |= 0x10;
    }
    
    self.registers.a = res;
}
    fn sub_a(&mut self, value: u8) {
    let a = self.registers.a;
    let res = a.wrapping_sub(value);
    
    self.registers.f = 0x40; // Set N flag to 1, clear others
    
    if res == 0 { self.registers.f |= 0x80; } // Z
    
    // H: Set if there is a borrow from bit 4
    if (a & 0x0F) < (value & 0x0F) { self.registers.f |= 0x20; }
    
    // C: Set if there is a borrow from bit 8 (a < value)
    if a < value { self.registers.f |= 0x10; }
    
    self.registers.a = res;
}
    fn or_a(&mut self, value: u8) {
    self.registers.a |= value;
    self.registers.f = if self.registers.a == 0 { 0x80 } else { 0 };
}

fn xor_a(&mut self, value: u8) {
    self.registers.a ^= value;
    self.registers.f = if self.registers.a == 0 { 0x80 } else { 0 };
}

fn and_a(&mut self, value: u8) {
    self.registers.a &= value;
    // AND is special: it sets the Half-Carry (H) flag to 1
    self.registers.f = if self.registers.a == 0 { 0x80 | 0x20 } else { 0x20 };
}
    fn compare(&mut self, value: u8) {
    let a = self.registers.a;
    let res = a.wrapping_sub(value);
    
    self.registers.f = 0x40; // Set N (Subtract flag) to 1
    if res == 0 { self.registers.f |= 0x80; } // Set Z if equal
    
    // H: Borrow from bit 4 (result of low nibble < value low nibble)
    if (a & 0x0F) < (value & 0x0F) { self.registers.f |= 0x20; }
    
    // C: Set if a < value (Full borrow)
    if a < value { self.registers.f |= 0x10; }
}
    fn add_a(&mut self, value: u8) {
    let a = self.registers.a;
    let res = a.wrapping_add(value);
    
    self.registers.f = 0; // Clear all flags
    if res == 0 { self.registers.f |= 0x80; } // Z
    // H: Carry from bit 3 to bit 4
    if (a & 0x0F) + (value & 0x0F) > 0x0F { self.registers.f |= 0x20; }
    // C: Carry from bit 7 to bit 8
    if (a as u16) + (value as u16) > 0xFF { self.registers.f |= 0x10; }
    
    self.registers.a = res;
}
    pub fn new(bus: MMU) -> Self {
        Self {
            registers: Registers {
                a: 0x01, f: 0xB0,
                b: 0x00, c: 0x13,
                d: 0x00, e: 0xD8,
                h: 0x01, l: 0x4D,
                pc: 0x100,
                sp: 0xFFFE,
            },
            bus,
            ime: false,
            interrupt_enable_delay: false,
            halted: false, // Usually starts disabled
        }
    }

    fn dec_8bit(&mut self, val: u8) -> u8 {
    let res = val.wrapping_sub(1);
    
    // Flags: Z 1 H -
    // We keep the Carry (C) flag as it is unaffected by 8-bit DEC.
    // We clear Z, N, and H to prep for new values.
    self.registers.f &= 0x10; 
    
    // Set N (Subtract) flag to 1 because this is a subtraction operation.
    self.registers.f |= 0x40; 
    
    // Z: Set if the result is 0
    if res == 0 { 
        self.registers.f |= 0x80; 
    }
    
    // H: Set if there is a borrow from bit 4.
    // In a decrement, this only happens if the lower nibble was 0x00.
    if (val & 0x0F) == 0x00 { 
        self.registers.f |= 0x20; 
    }
    
    res
}
    fn push_u16(&mut self, value: u16) {
    let hi = (value >> 8) as u8;
    let lo = (value & 0xFF) as u8;
    
    self.registers.sp = self.registers.sp.wrapping_sub(1);
    self.bus.write_byte(self.registers.sp, hi);
    
    self.registers.sp = self.registers.sp.wrapping_sub(1);
    self.bus.write_byte(self.registers.sp, lo);
}

fn pop_u16(&mut self) -> u16 {
    let lo = self.bus.read_byte(self.registers.sp) as u16;
    self.registers.sp = self.registers.sp.wrapping_add(1);
    
    let hi = self.bus.read_byte(self.registers.sp) as u16;
    self.registers.sp = self.registers.sp.wrapping_add(1);
    
    (hi << 8) | lo
}
    fn get_reg_by_index(&mut self, index: u8) -> u8 {
    match index {
        0 => self.registers.b,
        1 => self.registers.c,
        2 => self.registers.d,
        3 => self.registers.e,
        4 => self.registers.h,
        5 => self.registers.l,
        6 => self.bus.read_byte(self.get_hl()), // Memory access at address HL
        7 => self.registers.a,
        _ => unreachable!(),
    }
}

fn set_reg_by_index(&mut self, index: u8, val: u8) {
    match index {
        0 => self.registers.b = val,
        1 => self.registers.c = val,
        2 => self.registers.d = val,
        3 => self.registers.e = val,
        4 => self.registers.h = val,
        5 => self.registers.l = val,
        6 => self.bus.write_byte(self.get_hl(), val), // Write to memory at address HL
        7 => self.registers.a = val,
        _ => unreachable!(),
    }
}
    // --- 16-bit Register Helpers ---
fn get_hl(&self) -> u16 {
    ((self.registers.h as u16) << 8) | (self.registers.l as u16)
}

fn set_hl(&mut self, value: u16) {
    self.registers.h = (value >> 8) as u8;
    self.registers.l = (value & 0xFF) as u8;
}

fn get_bc(&self) -> u16 {
    ((self.registers.b as u16) << 8) | (self.registers.c as u16)
}

fn set_bc(&mut self, value: u16) {
    self.registers.b = (value >> 8) as u8;
    self.registers.c = (value & 0xFF) as u8;
}

fn get_de(&self) -> u16 {
    ((self.registers.d as u16) << 8) | (self.registers.e as u16)
}

fn set_de(&mut self, value: u16) {
    self.registers.d = (value >> 8) as u8;
    self.registers.e = (value & 0xFF) as u8;
}
    fn inc_8bit(&mut self, val: u8) -> u8 {
    let res = val.wrapping_add(1);
    self.registers.f &= 0x10; // Keep Carry flag, clear others
    if res == 0 { self.registers.f |= 0x80; } // Set Zero
    if (val & 0x0F) == 0x0F { self.registers.f |= 0x20; } // Set Half-Carry
    res
}
    pub fn step(&mut self) -> u8 {
        
        if self.halted {
        // While halted, we just return 4 cycles (the smallest unit of time)
        // so the MMU timer can continue to tick.
        return 4; 
    }
        
        if self.interrupt_enable_delay {
        self.ime = true;
        self.interrupt_enable_delay = false;
    }

    let opcode = self.fetch_byte();
  
        
        // We'll return cycles (u8) to sync with the PPU/Timer later
       let cycles= match opcode {
        
        
        0x9A => { self.sbc_8bit(self.registers.d); 4 }
0x9B => { self.sbc_8bit(self.registers.e); 4 }
0x9C => { self.sbc_8bit(self.registers.h); 4 }
0x9D => { self.sbc_8bit(self.registers.l); 4 }
0x9E => { 
    let val = self.bus.read_byte(self.get_hl()); 
    self.sbc_8bit(val); 
    8 
}
0x9F => { self.sbc_8bit(self.registers.a); 4 }
        0x99 => {
    let val = self.registers.c;
    self.sbc_8bit(val);
    4
},
        0x98 => {
    let b_val = self.registers.b;
    self.sbc_8bit(b_val);
    4
},
        // Helper logic (you can do this inline or as a function)
// Note: self.registers.pc is already pointing at the NEXT instruction 
// because fetch_byte() incremented it. This is the correct value to push.

0xC7 => self.rst(0x0000), // RST 00H
0xCF => self.rst(0x0008), // RST 08H
0xD7 => self.rst(0x0010), // RST 10H
0xDF => self.rst(0x0018), // RST 18H
0xE7 => self.rst(0x0020), // RST 20H
0xEF => self.rst(0x0028), // RST 28H
0xF7 => self.rst(0x0030), // RST 30H
0xFF => self.rst(0x0038), // RST 38H
        0xD9 => {
    // 1. Pop the return address from the stack
    let low = self.bus.read_byte(self.registers.sp) as u16;
    self.registers.sp = self.registers.sp.wrapping_add(1);
    let high = self.bus.read_byte(self.registers.sp) as u16;
    self.registers.sp = self.registers.sp.wrapping_add(1);
    
    self.registers.pc = (high << 8) | low;

    // 2. Enable interrupts immediately
    self.ime = true;

    16
},
        0xCE => {
    let n = self.fetch_byte();
    let a = self.registers.a;
    let c_in = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
    
    let res_full = a as u16 + n as u16 + c_in as u16;
    let res = res_full as u8;

    self.registers.f = 0;
    if res == 0 { self.registers.f |= 0x80; }
    // H: Carry from bit 3 to 4
    if (a & 0xF) + (n & 0xF) + c_in > 0xF { self.registers.f |= 0x20; }
    // C: Carry from bit 7 to 8
    if res_full > 0xFF { self.registers.f |= 0x10; }

    self.registers.a = res;
    8
},
        0xDE => {
    let n = self.fetch_byte();
    let a = self.registers.a;
    let c_in = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
    
    // Perform subtraction with the incoming carry
    let res = a.wrapping_sub(n).wrapping_sub(c_in);

    // Update Flags
    self.registers.f = 0x40; // Set N flag
    if res == 0 { self.registers.f |= 0x80; } // Z
    
    // Half-Carry: borrow from bit 4
    if (a as i32 & 0xF) - (n as i32 & 0xF) - (c_in as i32) < 0 {
        self.registers.f |= 0x20;
    }
    
    // Carry: borrow from bit 8
    if (a as i32) - (n as i32) - (c_in as i32) < 0 {
        self.registers.f |= 0x10;
    }

    self.registers.a = res;
    8
},
        0x36 => {
    // 1. Fetch the 8-bit value from the ROM
    let val = self.fetch_byte();
    
    // 2. Get the address from HL
    let addr = self.get_hl();
    
    // 3. Write the value to the bus
    self.bus.write_byte(addr, val);
    
    12
},
        0xF8 => {
    let offset = self.fetch_byte() as i8;
    let sp = self.registers.sp;

    let h_flag = (sp & 0xF) + (offset as u16 & 0xF) > 0xF;
    let c_flag = (sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF;

    let res = sp.wrapping_add(offset as i16 as u16);
    self.set_hl(res);

    self.registers.f = 0;
    if h_flag { self.registers.f |= 0x20; }
    if c_flag { self.registers.f |= 0x10; }

    12 // LD HL, SP+r8 takes 12 cycles
}
        0xE8 => {
    let offset = self.fetch_byte() as i8; // Signed 8-bit value
    let sp = self.registers.sp;
    
    // Half-carry: check carry from bit 3
    let h_flag = (sp & 0xF) + (offset as u16 & 0xF) > 0xF;
    
    // Carry: check carry from bit 7
    let c_flag = (sp & 0xFF) + (offset as u16 & 0xFF) > 0xFF;

    // Actual 16-bit addition (must wrap)
    self.registers.sp = sp.wrapping_add(offset as i16 as u16);

    // Update Flags: Z=0, N=0, H, C
    self.registers.f = 0;
    if h_flag { self.registers.f |= 0x20; }
    if c_flag { self.registers.f |= 0x10; }

    16
}
        0x0B => {
    let val = self.get_bc().wrapping_sub(1);
    self.set_bc(val);
    8
},
0x1B => {
    let val = self.get_de().wrapping_sub(1);
    self.set_de(val);
    8
},
0x2B => {
    let val = self.get_hl().wrapping_sub(1);
    self.set_hl(val);
    8
},
        0x3B => {
    self.registers.sp = self.registers.sp.wrapping_sub(1);
    8
},
        0xF9 => {
    // Assuming you have a helper get_hl() that combines H and L
    self.registers.sp = self.get_hl();
    8
},
        0x08 => {
    // 1. Fetch the 16-bit address (nn) from the next two bytes
    let low_addr = self.fetch_byte() as u16;
    let high_addr = self.fetch_byte() as u16;
    let addr = (high_addr << 8) | low_addr;

    // 2. Get the current Stack Pointer value
    let sp_val = self.registers.sp;

    // 3. Store the Low Byte of SP into addr, and High Byte into addr + 1
    self.bus.write_byte(addr, (sp_val & 0xFF) as u8);
    self.bus.write_byte(addr.wrapping_add(1), (sp_val >> 8) as u8);

    20 // This instruction takes 20 cycles
}
        0x76 => {
            self.halted = true;
            4 // It takes 4 cycles to enter the halt state
        },
            // 0xF3: DI (Disable Interrupts)
0xF3 => {
    self.ime = false;
    self.interrupt_enable_delay = false; // Cancel any pending EI delay
    4
},
            // 0xF8: LD HL, SP+e8
0xF8 => {
    let offset = self.fetch_byte() as i8 as i16 as u16;
    let sp = self.registers.sp;
    
    // Calculate flags based on the lower 8 bits (u8)
    let low_sp = (sp & 0xFF) as u8;
    let low_offset = (offset & 0xFF) as u8;
    
    self.registers.f = 0; // Z and N are always 0
    
    // H: Carry from bit 3 to 4
    if (low_sp & 0x0F) + (low_offset & 0x0F) > 0x0F {
        self.registers.f |= 0x20;
    }
    
    // C: Carry from bit 7 to 8
    if (low_sp as u16) + (low_offset as u16) > 0xFF {
        self.registers.f |= 0x10;
    }
    
    let res = sp.wrapping_add(offset);
    self.set_hl(res);
    12
},
            // 0x2F: CPL (Complement A - flip all bits)
0x2F => {
    self.registers.a = !self.registers.a;
    self.registers.f |= 0x60; // Set N and H flags
    4
},

// 0x37: SCF (Set Carry Flag)
0x37 => {
    self.registers.f &= 0x80; // Keep Z, clear N and H
    self.registers.f |= 0x10; // Set Carry
    4
},

// 0x3F: CCF (Complement Carry Flag)
0x3F => {
    let carry = (self.registers.f & 0x10) != 0;
    self.registers.f &= 0x80; // Keep Z, clear N and H
    if !carry { self.registers.f |= 0x10; } // Flip Carry
    4
},
            // 0x27: DAA (Decimal Adjust Accumulator)
0x27 => {
    self.daa();
    4
},
            // 0xC2: JP NZ, nn (Jump if Not Zero)
0xC2 => {
    let addr = self.fetch_u16();
    if (self.registers.f & 0x80) == 0 { // Check if Z flag is 0
        self.registers.pc = addr;
        16 // Takes 16 cycles if jump is taken
    } else {
        12 // Takes 12 cycles if jump is ignored
    }
},

// 0xCA: JP Z, nn (Jump if Zero)
0xCA => {
    let addr = self.fetch_u16();
    if (self.registers.f & 0x80) != 0 { // Check if Z flag is 1
        self.registers.pc = addr;
        16
    } else {
        12
    }
},

// 0xD2: JP NC, nn (Jump if No Carry)
0xD2 => {
    let addr = self.fetch_u16();
    if (self.registers.f & 0x10) == 0 { // Check if C flag is 0
        self.registers.pc = addr;
        16
    } else {
        12
    }
},

// 0xDA: JP C, nn (Jump if Carry)
0xDA => {
    let addr = self.fetch_u16();
    if (self.registers.f & 0x10) != 0 { // Check if C flag is 1
        self.registers.pc = addr;
        16
    } else {
        12
    }
},
            // 0xE2: LD (C), A (Store A into address 0xFF00 + C)
0xE2 => {
    let addr = 0xFF00 | (self.registers.c as u16);
    self.bus.write_byte(addr, self.registers.a);
    8
},

// 0xF2: LD A, (C) (Load from 0xFF00 + C into A)
0xF2 => {
    let addr = 0xFF00 | (self.registers.c as u16);
    self.registers.a = self.bus.read_byte(addr);
    8
},
            // 0xE9: JP (HL) (Jump to the address currently in HL)
0xE9 => {
    self.registers.pc = self.get_hl();
    4 // This is a very fast jump, taking only 4 cycles
},
            // 0x09: ADD HL, BC
0x09 => { let val = self.get_bc(); self.add_hl(val); 8 },

// 0x19: ADD HL, DE
0x19 => { let val = self.get_de(); self.add_hl(val); 8 },

// 0x29: ADD HL, HL (The one you just hit!)
0x29 => { let val = self.get_hl(); self.add_hl(val); 8 },

// 0x39: ADD HL, SP
0x39 => { let val = self.registers.sp; self.add_hl(val); 8 },
            // 0xCE: ADC A, d8 (The one you just hit!)
0xCE => {
    let val = self.fetch_byte();
    self.adc_a(val);
    8
},

// 0x88..=0x8F: ADC A, r (Add register + carry to A)
0x88..=0x8F => {
    let idx = opcode & 0x07;
    let val = self.get_reg_by_index(idx);
    self.adc_a(val);
    if idx == 6 { 8 } else { 4 }
},
            // 0x07: RLCA (Rotate Left Accumulator)
0x07 => {
    let a = self.registers.a;
    let carry = (a & 0x80) >> 7;
    self.registers.a = (a << 1) | carry;
    // Flags: 0 0 0 C
    self.registers.f = if carry == 1 { 0x10 } else { 0 };
    4
},

// 0x0F: RRCA (Rotate Right Accumulator)
0x0F => {
    let a = self.registers.a;
    let carry = a & 0x01;
    self.registers.a = (a >> 1) | (carry << 7);
    // Flags: 0 0 0 C
    self.registers.f = if carry == 1 { 0x10 } else { 0 };
    4
},

// 0x17: RLA (Rotate Left Accumulator through Carry)
0x17 => {
    let a = self.registers.a;
    let old_carry = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
    let new_carry = (a & 0x80) >> 7;
    self.registers.a = (a << 1) | old_carry;
    // Flags: 0 0 0 C
    self.registers.f = if new_carry == 1 { 0x10 } else { 0 };
    4
},

// 0x1F: RRA (The one you just hit!)
0x1F => {
    let a = self.registers.a;
    let old_carry = if (self.registers.f & 0x10) != 0 { 1 } else { 0 };
    let new_carry = a & 0x01;
    self.registers.a = (a >> 1) | (old_carry << 7);
    // Flags: 0 0 0 C
    self.registers.f = if new_carry == 1 { 0x10 } else { 0 };
    4
},
            0xCB => {
        let cb_opcode = self.fetch_byte();
        self.execute_cb(cb_opcode) // Returns the cycles taken
    },
            // 0xD6: SUB d8 (Subtract immediate 8-bit from A)
0xD6 => {
    let val = self.fetch_byte();
    self.sub_a(val);
    8
},

// 0x90..=0x97: SUB r (Subtract register r from A)
0x90..=0x97 => {
    let idx = opcode & 0x07;
    let val = self.get_reg_by_index(idx);
    self.sub_a(val);
    if idx == 6 { 8 } else { 4 }
},
            // 0xC4: CALL NZ, nn (Call if Not Zero)
0xC4 => {
    let dest = self.fetch_u16();
    if (self.registers.f & 0x80) == 0 {
        let ret = self.registers.pc;
        self.push_u16(ret);
        self.registers.pc = dest;
        24
    } else {
        12
    }
},

// 0xCC: CALL Z, nn (Call if Zero)
0xCC => {
    let dest = self.fetch_u16();
    if (self.registers.f & 0x80) != 0 {
        let ret = self.registers.pc;
        self.push_u16(ret);
        self.registers.pc = dest;
        24
    } else {
        12
    }
},

// 0xD4: CALL NC, nn (Call if No Carry)
0xD4 => {
    let dest = self.fetch_u16();
    if (self.registers.f & 0x10) == 0 {
        let ret = self.registers.pc;
        self.push_u16(ret);
        self.registers.pc = dest;
        24
    } else {
        12
    }
},

// 0xDC: CALL C, nn (Call if Carry)
0xDC => {
    let dest = self.fetch_u16();
    if (self.registers.f & 0x10) != 0 {
        let ret = self.registers.pc;
        self.push_u16(ret);
        self.registers.pc = dest;
        24
    } else {
        12
    }
},
// 0xC0: RET NZ (Return if Not Zero)
0xC0 => {
    if (self.registers.f & 0x80) == 0 {
        self.registers.pc = self.pop_u16();
        20
    } else {
        8
    }
},

// 0xC8: RET Z (Return if Zero)
0xC8 => {
    if (self.registers.f & 0x80) != 0 {
        self.registers.pc = self.pop_u16();
        20
    } else {
        8
    }
},

// 0xD0: RET NC (Return if No Carry)
0xD0 => {
    if (self.registers.f & 0x10) == 0 {
        self.registers.pc = self.pop_u16();
        20
    } else {
        8
    }
},

// 0xD8: RET C (Return if Carry)
0xD8 => {
    if (self.registers.f & 0x10) != 0 {
        self.registers.pc = self.pop_u16();
        20
    } else {
        8
    }
},
            // 0xE6: AND d8 (Bitwise AND A with immediate byte)
0xE6 => {
    let val = self.fetch_byte();
    self.and_a(val);
    8
},

// 0xEE: XOR d8 (Bitwise XOR A with immediate byte)
0xEE => {
    let val = self.fetch_byte();
    self.xor_a(val);
    8
},

// 0xF6: OR d8 (Bitwise OR A with immediate byte)
0xF6 => {
    let val = self.fetch_byte();
    self.or_a(val);
    8
},
            // 0xB0..=0xB7: OR r
0xB0..=0xB7 => {
    let val = self.get_reg_by_index(opcode & 0x07);
    self.or_a(val);
    if (opcode & 0x07) == 6 { 8 } else { 4 }
},

// 0xA0..=0xA7: AND r
0xA0..=0xA7 => {
    let val = self.get_reg_by_index(opcode & 0x07);
    self.and_a(val);
    if (opcode & 0x07) == 6 { 8 } else { 4 }
},

// 0xA8..=0xAF: XOR r
0xA8..=0xAF => {
    let val = self.get_reg_by_index(opcode & 0x07);
    self.xor_a(val);
    if (opcode & 0x07) == 6 { 8 } else { 4 }
},
            // 0xC1: POP BC
0xC1 => {
    let val = self.pop_u16();
    self.set_bc(val);
    12
},

// 0xD1: POP DE
0xD1 => {
    let val = self.pop_u16();
    self.set_de(val);
    12
},

// 0xE1: POP HL
0xE1 => {
    let val = self.pop_u16();
    self.set_hl(val);
    12
},

// 0xF1: POP AF
0xF1 => {
    let val = self.pop_u16();
    self.registers.a = (val >> 8) as u8;
    self.registers.f = (val & 0xF0) as u8; // Force lower 4 bits to 0
    12
},
            // 0xC5: PUSH BC
0xC5 => {
    let val = self.get_bc();
    self.push_u16(val);
    16
},

// 0xD5: PUSH DE
0xD5 => {
    let val = self.get_de();
    self.push_u16(val);
    16
},

// 0xE5: PUSH HL (The one you just hit!)
0xE5 => {
    let val = self.get_hl();
    self.push_u16(val);
    16
},

// 0xF5: PUSH AF
0xF5 => {
    let val = ((self.registers.a as u16) << 8) | (self.registers.f as u16);
    self.push_u16(val);
    16
},
            // 0xFE: CP d8 (Compare A with immediate 8-bit value)
0xFE => {
    let val = self.fetch_byte();
    self.compare(val);
    8
},

// 0xB8..=0xBF: CP r (Compare A with register r)
0xB8..=0xBF => {
    let idx = opcode & 0x07;
    let val = self.get_reg_by_index(idx);
    self.compare(val);
    if idx == 6 { 8 } else { 4 }
},
            // 0x18: JR n (Unconditional Relative Jump)
0x18 => {
    let offset = self.fetch_byte() as i8; // Fetch the signed 8-bit offset
    // Cast to i16 to preserve the sign, then to u16 to add to PC
    self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
    12 // This instruction always takes 12 cycles
},
            // 0xC6: ADD A, d8
    0xC6 => {
        let val = self.fetch_byte();
        self.add_a(val);
        8
    },

    // 0x80..0x87: ADD A, r
    0x80..=0x87 => {
        let idx = opcode & 0x07;
        let val = self.get_reg_by_index(idx);
        self.add_a(val);
        if idx == 6 { 8 } else { 4 }
    },
            // 0xE0: LDH (n), A (Store A into 0xFF00 + n)
    0xE0 => {
        let n = self.fetch_byte() as u16;
        let addr = 0xFF00 | n;
        self.bus.write_byte(addr, self.registers.a);
        12
    },

    // 0xF0: LDH A, (n) (Load A from 0xFF00 + n)
    0xF0 => {
        let n = self.fetch_byte() as u16;
        let addr = 0xFF00 | n;
        self.registers.a = self.bus.read_byte(addr);
        12
    },
    
    // 0xE2: LD (C), A (Store A into 0xFF00 + Register C)
    0xE2 => {
        let addr = 0xFF00 | (self.registers.c as u16);
        self.bus.write_byte(addr, self.registers.a);
        8
    },

    // 0xF2: LD A, (C) (Load A from 0xFF00 + Register C)
    0xF2 => {
        let addr = 0xFF00 | (self.registers.c as u16);
        self.registers.a = self.bus.read_byte(addr);
        8
    },
            // 0x03: INC BC
    0x03 => {
        let val = self.get_bc().wrapping_add(1);
        self.set_bc(val);
        8
    },

    // 0x13: INC DE
    0x13 => {
        let val = self.get_de().wrapping_add(1);
        self.set_de(val);
        8
    },

    // 0x23: INC HL
    0x23 => {
        let val = self.get_hl().wrapping_add(1);
        self.set_hl(val);
        8
    },

    // 0x33: INC SP
    0x33 => {
        self.registers.sp = self.registers.sp.wrapping_add(1);
        8
    },
            // 0xEA: LD (nn), A (Load A into absolute 16-bit address)
    0xEA => {
        let addr = self.fetch_u16();
        self.bus.write_byte(addr, self.registers.a);
        16 // Takes 16 cycles
    },

    // 0xFA: LD A, (nn) (Load A from absolute 16-bit address)
    0xFA => {
        let addr = self.fetch_u16();
        self.registers.a = self.bus.read_byte(addr);
        16
    },
            // 0xF3: DI (Disable Interrupts)
    0xF3 => {
        self.ime = false;
        4
    },

    // 0xFB: EI (Enable Interrupts)
    0xFB => {
        // Note: On a real Game Boy, EI takes effect AFTER the next instruction.
        // For now, setting it immediately is usually fine for Blargg tests.
self.interrupt_enable_delay = true;
    4
    },
            0x00 => 4, // NOP

    // LD SP, d16 (Load immediate 16-bit value into Stack Pointer)
    0x31 => {
        self.registers.sp = self.fetch_u16();
        12 // Takes 12 cycles
    },

    // JP nn (Jump to 16-bit address)
    0xC3 => {
        self.registers.pc = self.fetch_u16();
        16
    },

    // XOR A (XOR Register A with itself - effectively sets A to 0 and updates flags)
    0xAF => {
        self.registers.a = 0;
        self.registers.f = 0x80; // Set Zero flag, clear others
        4
    },
    0x21 => {
        let val = self.fetch_u16();
        self.set_hl(val);
        12
    },   
    0x20 => {
        let offset = self.fetch_byte() as i8; // This is a signed jump!
        if (self.registers.f & 0x80) == 0 {
            self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
            12 // Takes more cycles if it jumps
        } else {
            8
        }
    },
    // 0x40 - 0x7F: LD r1, r2 (excluding 0x76 which is HALT)
0x40..=0x75 | 0x77..=0x7F => {
    let dest_idx = (opcode >> 3) & 0x07; // Bits 3, 4, 5 define destination
    let src_idx = opcode & 0x07;        // Bits 0, 1, 2 define source
    
    let val = self.get_reg_by_index(src_idx);
    self.set_reg_by_index(dest_idx, val);
    
    // Most LD r,r take 4 cycles, but if it involves (HL), it takes 8
    if dest_idx == 6 || src_idx == 6 { 8 } else { 4 }
},
0x01 => {
        let val = self.fetch_u16();
        self.set_bc(val);
        12
    },

    // 0x11: LD DE, d16 (The one you just hit!)
    0x11 => {
        let val = self.fetch_u16();
        self.set_de(val);
        12
    },

    // 0x06: LD B, d8 (8-bit immediate load)
    0x06 => {
        self.registers.b = self.fetch_byte();
        8
    },

    // 0x0E: LD C, d8
    0x0E => {
        self.registers.c = self.fetch_byte();
        8
    },

    // 0x16: LD D, d8
    0x16 => {
        self.registers.d = self.fetch_byte();
        8
    },

    // 0x1E: LD E, d8
    0x1E => {
        self.registers.e = self.fetch_byte();
        8
    },

    // 0x26: LD H, d8
    0x26 => {
        self.registers.h = self.fetch_byte();
        8
    },

    // 0x2E: LD L, d8
    0x2E => {
        self.registers.l = self.fetch_byte();
        8
    },

    // 0x3E: LD A, d8
    0x3E => {
        self.registers.a = self.fetch_byte();
        8
    },
    0x22 => {
        let addr = self.get_hl();
        self.bus.write_byte(addr, self.registers.a);
        self.set_hl(addr.wrapping_add(1));
        8
    },

    // 0x2A: LD A, (HL+) (Read (HL) into A, then Increment HL)
    0x2A => {
        let addr = self.get_hl();
        self.registers.a = self.bus.read_byte(addr);
        self.set_hl(addr.wrapping_add(1));
        8
    },

    // 0x32: LD (HL-), A (Write A to (HL), then Decrement HL)
    0x32 => {
        let addr = self.get_hl();
        self.bus.write_byte(addr, self.registers.a);
        self.set_hl(addr.wrapping_sub(1));
        8
    },

    // 0x3A: LD A, (HL-) (Read (HL) into A, then Decrement HL)
    0x3A => {
        let addr = self.get_hl();
        self.registers.a = self.bus.read_byte(addr);
        self.set_hl(addr.wrapping_sub(1));
        8
    },
    0x02 => {
        let addr = self.get_bc();
        self.bus.write_byte(addr, self.registers.a);
        8
    },

    // 0x12: LD (DE), A (Store A into memory address pointed to by DE)
    0x12 => {
        let addr = self.get_de();
        self.bus.write_byte(addr, self.registers.a);
        8
    },

    // 0x0A: LD A, (BC) (Load A from memory address pointed to by BC)
    0x0A => {
        let addr = self.get_bc();
        self.registers.a = self.bus.read_byte(addr);
        8
    },

    // 0x1A: LD A, (DE) (Load A from memory address pointed to by DE)
    0x1A => {
        let addr = self.get_de();
        self.registers.a = self.bus.read_byte(addr);
        8
    },
    // 0xCD: CALL nn (Call function at 16-bit address)
    0xCD => {
        let dest = self.fetch_u16();
        // Push the address of the NEXT instruction (the current PC) onto the stack
        let return_addr = self.registers.pc;
        self.push_u16(return_addr);
        // Jump to the destination
        self.registers.pc = dest;
        24 // This is a heavy instruction, takes 24 cycles
    },

    // 0xC9: RET (Return from function)
    0xC9 => {
        self.registers.pc = self.pop_u16();
        16
    },
    0x04 => { self.registers.b = self.inc_8bit(self.registers.b); 4 },
    0x0C => { self.registers.c = self.inc_8bit(self.registers.c); 4 },
    0x14 => { self.registers.d = self.inc_8bit(self.registers.d); 4 },
    0x1C => { self.registers.e = self.inc_8bit(self.registers.e); 4 }, // The one you hit!
    0x24 => { self.registers.h = self.inc_8bit(self.registers.h); 4 },
    0x2C => { self.registers.l = self.inc_8bit(self.registers.l); 4 },
    0x3C => { self.registers.a = self.inc_8bit(self.registers.a); 4 },
    0x34 => { 
        let val = self.bus.read_byte(self.get_hl());
        let res = self.inc_8bit(val);
        self.bus.write_byte(self.get_hl(), res);
        12 
    },
    0x05 => { self.registers.b = self.dec_8bit(self.registers.b); 4 },
    0x0D => { self.registers.c = self.dec_8bit(self.registers.c); 4 }, // The one you hit!
    0x15 => { self.registers.d = self.dec_8bit(self.registers.d); 4 },
    0x1D => { self.registers.e = self.dec_8bit(self.registers.e); 4 },
    0x25 => { self.registers.h = self.dec_8bit(self.registers.h); 4 },
    0x2D => { self.registers.l = self.dec_8bit(self.registers.l); 4 },
    0x3D => { self.registers.a = self.dec_8bit(self.registers.a); 4 },
    0x35 => { 
        let val = self.bus.read_byte(self.get_hl());
        let res = self.dec_8bit(val);
        self.bus.write_byte(self.get_hl(), res);
        12 
    },
    // 0x20: JR NZ, r8 (Jump Relative if Not Zero)
    0x20 => {
        let offset = self.fetch_byte() as i8;
        if (self.registers.f & 0x80) == 0 { // Check if Z flag is 0
            self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
            12 // Takes 12 cycles if jump is taken
        } else {
            8  // Takes 8 cycles if jump is ignored
        }
    },

    // 0x28: JR Z, r8 (Jump Relative if Zero)
    0x28 => {
        let offset = self.fetch_byte() as i8;
        if (self.registers.f & 0x80) != 0 { // Check if Z flag is 1
            self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
            12
        } else {
            8
        }
    },

    // 0x30: JR NC, r8 (Jump Relative if No Carry)
    0x30 => {
        let offset = self.fetch_byte() as i8;
        if (self.registers.f & 0x10) == 0 { // Check if C flag is 0
            self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
            12
        } else {
            8
        }
    },

    // 0x38: JR C, r8 (Jump Relative if Carry)
    0x38 => {
        let offset = self.fetch_byte() as i8;
        if (self.registers.f & 0x10) != 0 { // Check if C flag is 1
            self.registers.pc = self.registers.pc.wrapping_add(offset as i16 as u16);
            12
        } else {
            8
        }
    },
            _ => {
                println!("Unknown Opcode: {:#04X} at PC: {:#06X}", opcode, self.registers.pc.wrapping_sub(1));
                panic!("CPU CRASHED");
            }
        };
        cycles
    }

    fn fetch_byte(&mut self) -> u8 {
        let byte = self.bus.read_byte(self.registers.pc);
        self.registers.pc = self.registers.pc.wrapping_add(1);
        byte
    }

    fn fetch_u16(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }
        
}