mod cpu;
mod mmu;

use std::println;

use cpu::Cpu;
use mmu::Mmu;

fn main() {
    println!("Starting the Game Boy emulator...");

    let mut mmu = Mmu::new();
    let mut cpu = Cpu::new();

    // Create a minimal "dummy" ROM in code to test, using just two NOP instructions
    let dummy_rom = vec![0x00, 0x00];
    mmu.load_rom(&dummy_rom);

    // Simple emulation loop
    for _ in 0..2 {
        let cycles = cpu.step(&mut mmu);
        println!(
            "Running instruction took {} cycles. PC is now at: 0x{:04X}",
            cycles, cpu.pc
        );
    }
}
