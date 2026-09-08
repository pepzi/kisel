#![allow(dead_code)]
use std::println;

use crate::mmu::Mmu;

const Z_FLAG: u8 = 0b1000_0000; // Bit 7
const N_FLAG: u8 = 0b0100_0000; // Bit 6
const H_FLAG: u8 = 0b0010_0000; // Bit 5
const C_FLAG: u8 = 0b0001_0000; // Bit 4

pub struct Cpu {
    // 8-bit registers
    pub a: u8,
    pub f: u8, // AF (F is the flag register)
    pub b: u8,
    pub c: u8, // BC
    pub d: u8,
    pub e: u8, // DE
    pub h: u8,
    pub l: u8, // HL

    // 16-bit pointers
    pub pc: u16, // Program Counter
    pub sp: u16, // Stack Pointer
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            pc: 0x0100, // Startar på 0x0100 för att hoppa över boot-rom
            sp: 0xFFFE, // Sätter Stack Pointer till toppen av High RAM
        }
    }

    pub fn get_bc(&self) -> u16 {
        ((self.b as u16) << 8) | (self.c as u16)
    }

    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }

    pub fn get_de(&self) -> u16 {
        ((self.d as u16) << 8) | (self.e as u16)
    }

    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }

    // HL-pair (the Game Boys most important register pair for memory pointers)
    pub fn get_hl(&self) -> u16 {
        (self.h as u16) << 8 | (self.l as u16)
    }

    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }

    // AF-pair (seldome used for direct calculations, but needed for stack
    // operations)
    pub fn get_af(&self) -> u16 {
        (self.a as u16) << 8 | (self.f as u16)
    }

    pub fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        // NOTICE: The lowest four bits of the F register are ALWAYS zero!
        self.f = (value & 0xF0) as u8;
    }

    // Getters for easilly checking if a flag is set (returns true/false)
    pub fn get_z(&self) -> bool {
        (self.f & Z_FLAG) != 0
    }
    pub fn get_n(&self) -> bool {
        (self.f & N_FLAG) != 0
    }
    pub fn get_h(&self) -> bool {
        (self.f & H_FLAG) != 0
    }
    pub fn get_c(&self) -> bool {
        (self.f & C_FLAG) != 0
    }

    // Universal method to set or clear flag using bools
    pub fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        self.f = 0; // Zero out everything
        if z {
            self.f |= Z_FLAG
        }
        if n {
            self.f |= N_FLAG
        }
        if h {
            self.f |= H_FLAG
        }
        if c {
            self.f |= C_FLAG
        }
    }

    // Useful helper functions to change a single flag, leaving them all untouched
    pub fn set_z(&mut self, value: bool) {
        if value {
            self.f |= Z_FLAG;
        } else {
            self.f &= !Z_FLAG;
        }
    }

    pub fn set_n(&mut self, value: bool) {
        if value {
            self.f |= N_FLAG;
        } else {
            self.f &= !N_FLAG;
        }
    }

    pub fn set_h(&mut self, value: bool) {
        if value {
            self.f |= H_FLAG;
        } else {
            self.f &= !H_FLAG;
        }
    }

    pub fn set_c(&mut self, value: bool) {
        if value {
            self.f |= C_FLAG;
        } else {
            self.f &= !C_FLAG;
        }
    }

    // Run one instruction and return how many clock cycles it took
    pub fn step(&mut self, mmu: &mut Mmu) -> u32 {
        let opcode = mmu.read_byte(self.pc);
        self.pc += 1;

        match opcode {
            0x00 => {
                // NOP
                4
            }

            0x06 => {
                // LB B, n
                let n = mmu.read_byte(self.pc);
                self.pc += 1;
                self.b = n;
                8 // uses 8 cycles (4 for opcode fetch, 4 to read n)
            }

            0x01 => {
                // LD BC, d16
                // Game Boy is little endian, so the least significant byte comes first
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;

                let d16 = (high << 8) | low;
                self.set_bc(d16);
                12 // uses 12 cycles
            }

            0x0A => {
                // LD A, (BC)
                let addr = self.get_bc(); // Fetch next address from BC pair
                let value = mmu.read_byte(addr); // Read from memory at that address
                self.a = value; // store in A
                8 // Uses 8 clock cycles
            }

            0xC3 => {
                // JP nn (Absolute jump to 16-bit immediate address)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;

                let nn = (high << 8) | low;
                self.pc = nn; // Change PC to new address!
                16 // uses 16 cloc cycles
            }

            0xAF => {
                // XOR a (Exclusive OR register A with itself. Quick way to clear A)
                self.a ^= self.a; // Blir alltid 0

                self.set_flags(true, false, false, false);
                4 // 4 cycles
            }

            0x21 => {
                // LD HL, d16 (Load 16-bit immediate value into HL pair)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;

                let d16 = (high << 8) | low;
                self.set_hl(d16);
                12 // 12 cycles
            }

            0x0E => {
                // LD C, n (load 8-bit immediate value into register C)
                let n = mmu.read_byte(self.pc);
                self.pc += 1;
                self.c = n;
                8 // 8 cycles
            }

            0x32 => {
                // LD (HL-), A (Write register A to memory address HL, then decremnt HL)
                let addr = self.get_hl();
                mmu.write_byte(addr, self.a);

                // Decrease HL by 1
                let new_hl = addr.wrapping_sub(1);
                self.set_hl(new_hl);

                8 // 8 cycles
            }

            _ => {
                println!(
                    "\n[KRASCH] Unknown or unimplemented opcode: 0x{:02X} at PC: 0x{:04X}",
                    opcode,
                    self.pc - 1
                );
                self.debug_dump(); // Dumpa registertillståndet direkt vid kraschen
                0 // Returnera 0 för att tala om för main-loopen att stanna!
            }
        }
    }

    // Print status for each register
    pub fn debug_dump(&self) {
        println!("--- CPU REGISTERS ---");
        println!(
            "A: 0x{:02X} | F: 0x{:02X} (Z:{} N:{} H:{} C:{})",
            self.a,
            self.f,
            self.get_z() as u8,
            self.get_n() as u8,
            self.get_h() as u8,
            self.get_c() as u8
        );
        println!(
            "B: 0x{:02X} | C: 0x{:02X} -> BC: 0x{:04X}",
            self.b,
            self.c,
            self.get_bc()
        );
        println!(
            "D: 0x{:02X} | E: 0x{:02X} -> DE: 0x{:04X}",
            self.d,
            self.e,
            self.get_de()
        );
        println!(
            "H: 0x{:02X} | L: 0x{:02X} -> HL: 0x{:04X}",
            self.h,
            self.l,
            self.get_hl()
        );
        println!("PC: 0x{:04X} | SP: 0x{:04X}", self.pc, self.sp);
        println!("---------------------");
    }
}
