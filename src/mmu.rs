#![allow(dead_code)]
use std::fs;

pub struct Mmu {
    memory: [u8; 65536],
    /// bit3=Start bit2=Select bit1=B bit0=A  (0 = nedtryckt)
    buttons: u8,
    /// bit3=Down bit2=Up bit1=Left bit0=Right
    dpad: u8,
    pub div_cycles: u32,
    tima_counter: u32,
}

impl Mmu {
    pub fn new() -> Self {
        Self {
            memory: [0; 65536],
            buttons: 0x0F,
            dpad: 0x0F,
            div_cycles: 0,
            tima_counter: 0,
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
            return 0xC0 | (select & 0x30) | lo;
        }
        self.memory[addr as usize]
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        if addr < 0x8000 {
            return;
        }

        if addr == 0xFF04 {
            self.memory[0xFF04] = 0;
            self.div_cycles = 0;
            return;
        }

        if addr == 0xFF00 {
            self.memory[0xFF00] = value & 0x30;
            return;
        }

        if addr == 0xFF02 && value & 0x80 != 0 {
            let c = self.memory[0xFF01] as char;
            print!("{}", c);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }

        if addr == 0xFF46 {
            let src = (value as u16) << 8;
            for i in 0..160u16 {
                self.memory[0xFE00 + i as usize] = self.memory[(src + i) as usize];
            }
            self.memory[0xFF46] = value;
            return;
        }

        self.memory[addr as usize] = value;
    }

    pub fn tick(&mut self, cycles: u32) {
        self.div_cycles += cycles;
        while self.div_cycles >= 256 {
            self.div_cycles -= 256;
            self.memory[0xFF04] = self.memory[0xFF04].wrapping_add(1);
        }

        let tac = self.memory[0xFF07];
        if tac & 0x04 == 0 {
            return;
        }
        let period = match tac & 0x03 {
            0 => 1024u32,
            1 => 16,
            2 => 64,
            _ => 256,
        };
        self.tima_counter += cycles;
        while self.tima_counter >= period {
            self.tima_counter -= period;
            let tima = self.memory[0xFF05].wrapping_add(1);
            if tima == 0 {
                self.memory[0xFF05] = self.memory[0xFF06];
                self.memory[0xFF0F] |= 0x04;
            } else {
                self.memory[0xFF05] = tima;
            }
        }
    }

    pub fn load_rom(&mut self, path: &str) {
        match fs::read(path) {
            Ok(bytes) => {
                let size = bytes.len().min(self.memory.len());
                self.memory[..size].copy_from_slice(&bytes[..size]);
                println!("Successfully loaded ROM: {} ({} bytes)", path, bytes.len());
            }
            Err(e) => panic!("Unable to load ROM '{}': {}", path, e),
        }
    }
}
