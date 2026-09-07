#![allow(dead_code)]
pub struct Mmu {
    memory: [u8; 65536], // 64KB ram
}

impl Mmu {
    pub fn new() -> Self {
        Self { memory: [0; 65536] }
    }

    // Reads one byte from a specific 16 bit address
    pub fn read_byte(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    // Writes a byte to a specific 16 bit address
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        self.memory[addr as usize] = value;
    }

    // Helper method to load a ROM into memory (starting att address 0x0000)
    pub fn load_rom(&mut self, rom: &[u8]) {
        let size = rom.len().min(self.memory.len());
        self.memory[..size].copy_from_slice(&rom[..size]);
    }
}
