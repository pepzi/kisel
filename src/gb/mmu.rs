#![allow(dead_code)]
use std::fs;

pub struct Mmu {
    memory: [u8; 65536],
    rom: Vec<u8>,
    rom_bank: usize,
    buttons: u8,
    dpad: u8,
    pub div_cycles: u32,
    tima_counter: u32,
    line_cycles: u32,
    pub scx_line: [u8; 144],
    pub scy_line: [u8; 144],
}

impl Mmu {
    pub fn new() -> Self {
        Self {
            memory: [0; 65536],
            rom: Vec::new(),
            rom_bank: 1,
            buttons: 0x0F,
            dpad: 0x0F,
            div_cycles: 0,
            tima_counter: 0,
            line_cycles: 0,
            scx_line: [0; 144],
            scy_line: [0; 144],
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

        let pressed_now = (old.0 & !buttons) | (old.1 & !dpad);
        if pressed_now & 0x0F != 0 {
            self.memory[0xFF0F] |= 0x10;
        }

        fn report(name: &str, old_bit: bool, new_bit: bool) {
            if old_bit && !new_bit {
                println!("{name} pressed");
            } else if !old_bit && new_bit {
                println!("{name} released");
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
        match addr {
            0x0000..=0x3FFF => *self.rom.get(addr as usize).unwrap_or(&0xFF),
            0x4000..=0x7FFF => {
                let off = self.rom_bank * 0x4000 + (addr as usize - 0x4000);
                *self.rom.get(off).unwrap_or(&0xFF)
            }
            0xFF00 => {
                let select = self.memory[0xFF00];
                let mut lo = 0x0F;
                if select & 0x20 == 0 {
                    lo &= self.buttons;
                }
                if select & 0x10 == 0 {
                    lo &= self.dpad;
                }
                let result = 0xC0 | (select & 0x30) | lo;
                if self.dpad != 0x0F || self.buttons != 0x0F {
                    println!(
                        "P1rd sel={:02X} -> {:02X} (btn={:02X} dpad={:02X})",
                        select, result, self.buttons, self.dpad
                    );
                }
                result
            }
            _ => self.memory[addr as usize],
        }
    }

    pub fn write_ly(&mut self, v: u8) {
        self.memory[0xFF44] = v;
    }

    pub fn or_if(&mut self, bits: u8) {
        self.memory[0xFF0F] |= bits;
    }

    pub fn ppu_step(&mut self, cycles: u32) {
        if self.memory[0xFF40] & 0x80 == 0 {
            return;
        }

        self.line_cycles += cycles;
        while self.line_cycles >= 456 {
            self.line_cycles -= 456;
            let ly = self.memory[0xFF44];
            if ly < 144 {
                self.scx_line[ly as usize] = self.memory[0xFF43];
                self.scy_line[ly as usize] = self.memory[0xFF42];
            }
            let mut ly = ly.wrapping_add(1);
            if ly >= 154 {
                ly = 0;
            }
            self.memory[0xFF44] = ly;
            if ly == 144 {
                self.memory[0xFF0F] |= 0x01;
            }
        }

        let ly = self.memory[0xFF44];
        let lyc = self.memory[0xFF45];
        let mut stat = self.memory[0xFF41] & 0xF8;
        let mode = if ly >= 144 {
            1u8
        } else if self.line_cycles < 80 {
            2
        } else if self.line_cycles < 80 + 172 {
            3
        } else {
            0
        };
        let old_mode = self.memory[0xFF41] & 0x03;
        stat |= mode;
        if ly == lyc {
            stat |= 0x04;
        }
        self.memory[0xFF41] = stat;

        if ly == lyc && stat & 0x40 != 0 {
            self.memory[0xFF0F] |= 0x02;
        }
        if mode != old_mode {
            if mode == 0 && stat & 0x08 != 0 {
                self.memory[0xFF0F] |= 0x02;
            }
            if mode == 1 && stat & 0x10 != 0 {
                self.memory[0xFF0F] |= 0x02;
            }
            if mode == 2 && stat & 0x20 != 0 {
                self.memory[0xFF0F] |= 0x02;
            }
        }
    }

    pub fn write_byte(&mut self, addr: u16, value: u8) {
        if addr == 0xFF44 {
            return;
        }

        if addr == 0xFF41 {
            let old = self.memory[0xFF41];
            self.memory[0xFF41] = (value & 0x78) | (old & 0x07);
            return;
        }

        if (0x2000..=0x3FFF).contains(&addr) {
            let banks = (self.rom.len() / 0x4000).max(1);
            let mut bank = (value as usize) & 0x1F;
            if bank == 0 {
                bank = 1;
            }
            self.rom_bank = bank % banks;
            return;
        }

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
            let c = self.memory[0xFF01];
            if c.is_ascii_graphic() || c == b'\n' {
                print!("{}", c as char);
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
        }
        if addr == 0xFF46 {
            let src = (value as u16) << 8;
            for i in 0..160u16 {
                let b = self.read_byte(src + i);
                self.memory[0xFE00 + i as usize] = b;
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

    pub fn init_after_boot(&mut self) {
        self.memory[0xFF40] = 0x91; // LCD på, BG på, $8000 tiles, $9800 map
        self.memory[0xFF41] = 0x85;
        self.memory[0xFF42] = 0x00;
        self.memory[0xFF43] = 0x00;
        self.memory[0xFF45] = 0x00;
        self.memory[0xFF47] = 0xFC; // BGP
        self.memory[0xFF48] = 0xFF;
        self.memory[0xFF49] = 0xFF;
        self.memory[0xFF4A] = 0x00;
        self.memory[0xFF4B] = 0x00;
        self.memory[0xFF0F] = 0xE1;
    }

    pub fn load_rom(&mut self, path: &str) {
        match fs::read(path) {
            Ok(bytes) => {
                println!("Successfully loaded ROM: {} ({} bytes)", path, bytes.len());
                self.rom = bytes;
                self.rom_bank = 1;
                let n = self.rom.len().min(0x8000);
                self.memory[..n].copy_from_slice(&self.rom[..n]);
            }
            Err(e) => panic!("Unable to load ROM '{}': {}", path, e),
        }
    }
}
