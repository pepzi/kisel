#![allow(dead_code)]
use std::{fs, println};

pub struct Mmu {
    memory: [u8; 65536], // 64KB ram
    /// bit3=Start bit2=Select bit1=B bit0=A  (0 = nedtryckt)
    buttons: u8,
    /// bit3=Down bit2=Up bit1=Left bit0=Right
    dpad: u8,
    pub div_cycles: u32,
}

impl Mmu {
    pub fn new() -> Self {
        Self {
            memory: [0; 65536],
            buttons: 0x0F,
            dpad: 0x0F,
            div_cycles: 0,
        }
    }

    pub fn set_joypad(&mut self, buttons: u8, dpad: u8) {
        let buttons = buttons & 0x0F;
        let dpad = dpad & 0x0F;

        let old = (self.buttons, self.dpad);
        self.buttons = buttons;
        self.dpad = dpad;
        if (buttons, dpad) == old {
            return;
        }

        fn report(name: &str, old_bit: bool, new_bit: bool) {
            // bit 0 = nedtryckt
            if old_bit && !new_bit {
                println!("{name} nedtryckt");
            } else if !old_bit && new_bit {
                println!("{name} släppt");
            }
        }

        report("Start", old.0 & 0x08 != 0, buttons & 0x08 != 0);
        report("Select", old.0 & 0x04 != 0, buttons & 0x04 != 0);
        report("B", old.0 & 0x02 != 0, buttons & 0x02 != 0);
        report("A", old.0 & 0x01 != 0, buttons & 0x01 != 0);
        report("Down", old.1 & 0x08 != 0, dpad & 0x08 != 0);
        report("Up", old.1 & 0x04 != 0, dpad & 0x04 != 0);
        report("Left", old.1 & 0x02 != 0, dpad & 0x02 != 0);
        report("Right", old.1 & 0x01 != 0, dpad & 0x01 != 0);
    }

    // Reads one byte from a specific 16 bit address
    pub fn read_byte(&self, addr: u16) -> u8 {
        if addr == 0xFF00 {
            let select = self.memory[0xFF00];
            let mut lo = 0x0F;
            if select & 0x20 == 0 {
                lo &= self.buttons;
            }
            if select & 0x10 == 0 {
                lo &= self.dpad;
            }
            let result = 0xC0 | (select & 0x30) | lo;

            return result;
        }
        self.memory[addr as usize]
    }

    // Writes a byte to a specific 16 bit address
    pub fn write_byte(&mut self, addr: u16, value: u8) {
        if addr == 0xFF04 {
            self.memory[0xFF04] = 0; // skrivning nollställer DIV
            self.div_cycles = 0;
            return;
        }

        /*         if addr == 0xFF8D || addr == 0xFF8E {
            println!("{:04X}={:02X}", addr, value);
        }

        if addr == 0xFF8F {
            println!("FF8F={:02X}", value);
        } */

        /*         if addr == 0xFFE1 {
                   println!("FFE1={:02X}", value);
               }
               if addr == 0xFFAB {
                   println!("paus={:02X}", value);
               }
        */
        /*         if (0xC000..0xC0A0).contains(&addr) && value != 0 {
                   println!("OAM-buf [{:04X}]={:02X}", addr, value);
               }
        */
        /*         if addr == 0xFF85 {
                   println!("skriv FF85={:02X}", value);
               }
        */
        // i write_byte, efter ROM-skyddet
        if addr == 0xFF02 && value & 0x80 != 0 {
            let c = self.memory[0xFF01] as char;
            print!("{}", c);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        self.memory[addr as usize] = value;

        if addr == 0xFF46 {
            // OAM DMA: kopiera 160 byte från value*0x100 till 0xFE00
            let src = (value as u16) << 8;
            for i in 0..160u16 {
                self.memory[0xFE00 + i as usize] = self.memory[(src + i) as usize];
            }
            self.memory[0xFF46] = value;
            return;
        }

        if addr == 0xFF00 {
            self.memory[0xFF00] = value & 0x30;
            return;
        }

        if addr < 0x8000 {
            return;
        }

        self.memory[addr as usize] = value;
    }

    pub fn tick_div(&mut self, cycles: u32) {
        self.div_cycles += cycles;
        while self.div_cycles >= 256 {
            self.div_cycles -= 256;
            self.memory[0xFF04] = self.memory[0xFF04].wrapping_add(1);
        }
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
