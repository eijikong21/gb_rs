mod cpu;
mod mmu;

use cpu::CPU;
use mmu::MMU;
use std::fs;

fn main() {
    // 1. Load the ROM (Use a Blargg test ROM to start)
    let rom = fs::read("tests/06-ld_r_r.gb").unwrap();
    
    // 2. Initialize Hardware
    let mmu = MMU::new(rom);
    let mut cpu = CPU::new(mmu);

    println!("CPU Initialized at PC: {:#06X}", cpu.registers.pc);

    // 3. The Execution Loop
    loop {
        let _cycles = cpu.step();
        // We will implement cpu.step() next!
        // For now, let's just break so we don't loop forever
    }
}