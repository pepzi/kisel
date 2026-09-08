#![allow(dead_code)]
use std::{fs, println};

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

    pub fn load_rom(&mut self, path: &str) {
        match fs::read(path) {
            Ok(bytes) => {
                // Tetris is 32 KB in size (0x0000 - 0x7FFFF), which fits in memory nicely
                let size = bytes.len().min(self.memory.len());
                self.memory[..size].copy_from_slice(&bytes[..size]);
                println!("Successfully loaded ROM: {} ({} bytes)", path, bytes.len());
            }
            Err(e) => {
                panic!("Unable to load ROM '{}': {}", path, e);
            }
        }
    }
}
