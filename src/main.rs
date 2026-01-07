mod cpu;
mod mmu;

use cpu::CPU;
use mmu::MMU;
use std::fs;

fn main() {
    // 1. Load the ROM (Use a Blargg test ROM to start)
    let rom = fs::read("tests/03-op sp,hl.gb").unwrap();
    
    // 2. Initialize Hardware
    let mmu = MMU::new(rom);
    let mut cpu = CPU::new(mmu);

    println!("CPU Initialized at PC: {:#06X}", cpu.registers.pc);

    // 3. The Execution Loop
    loop {
       // 1. Check if any interrupts need to fire before the next instruction
        cpu.handle_interrupts();

        // 2. Execute one instruction and get the cycles it took
        let cycles = cpu.step();

        // 3. Update the hardware timers based on those cycles
        cpu.bus.tick(cycles); 

        // Optional: Add a safety break if you want to inspect things
        // if cpu.registers.pc == 0xDEAD { break; }
    }
}