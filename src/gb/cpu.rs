#![allow(dead_code)]
use std::println;

use crate::gb::mmu::Mmu;

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

    pub ime: bool, // Interrupt Master Enable
    pub last_rom_pc: u16,
    pub halted: bool,
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
            pc: 0x0100,
            sp: 0xFFFE,
            ime: false,
            last_rom_pc: 0x0100,
            halted: false,
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
    fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        self.f = 0;
        if z {
            self.f |= Z_FLAG;
        }
        if n {
            self.f |= N_FLAG;
        }
        if h {
            self.f |= H_FLAG;
        }
        if c {
            self.f |= C_FLAG;
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

    fn set_n(&mut self, v: bool) {
        if v {
            self.f |= N_FLAG;
        } else {
            self.f &= !N_FLAG;
        }
    }

    fn set_h(&mut self, v: bool) {
        if v {
            self.f |= H_FLAG;
        } else {
            self.f &= !H_FLAG;
        }
    }

    fn set_c(&mut self, v: bool) {
        if v {
            self.f |= C_FLAG;
        } else {
            self.f &= !C_FLAG;
        }
    }
    // Run one instruction and return how many clock cycles it took
    pub fn step(&mut self, mmu: &mut Mmu) -> u32 {
        if self.halted {
            let pending = mmu.read_byte(0xFFFF) & mmu.read_byte(0xFF0F) & 0x1F;
            if pending == 0 {
                return 4;
            }
            self.halted = false;
        }

        if self.ime {
            let ie = mmu.read_byte(0xFFFF);
            let iflag = mmu.read_byte(0xFF0F);
            let pending = ie & iflag & 0x1F;
            if pending != 0 {
                self.ime = false;
                let (bit, addr) = if pending & 0x01 != 0 {
                    (0x01u8, 0x0040u16)
                } else if pending & 0x02 != 0 {
                    (0x02, 0x0048)
                } else if pending & 0x04 != 0 {
                    (0x04, 0x0050)
                } else if pending & 0x08 != 0 {
                    (0x08, 0x0058)
                } else {
                    (0x10, 0x0060)
                };

                mmu.write_byte(0xFF0F, iflag & !bit);

                let ret = self.pc;

                /*                 if addr == 0x0040 {
                                   println!(
                                       "IRQ VBlank PC_in={:04X} FF85={:02X}",
                                       ret,
                                       mmu.read_byte(0xFF85)
                                   );
                               }
                */
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, (ret >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, (ret & 0xFF) as u8);
                self.pc = addr;
                return 20;
            }
        }

        if (0x8000..0xC000).contains(&self.pc) {
            println!(
                "[PC I VRAM/CART] PC={:04X} från {:04X} SP={:04X} HL={:04X} FFC0={:02X} FFCD={:02X} FFE1={:02X}",
                self.pc,
                self.last_rom_pc,
                self.sp,
                self.get_hl(),
                mmu.read_byte(0xFFC0),
                mmu.read_byte(0xFFCD),
                mmu.read_byte(0xFFE1)
            );
            self.debug_dump();
            return 0;
        }

        let opcode = mmu.read_byte(self.pc);
        self.pc = self.pc.wrapping_add(1);

        match opcode {
            0x00 => {
                // NOP
                4
            }

            0x26 => {
                // LD H, n
                let n = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.h = n;
                8
            }
            0x2E => {
                // LD L, n
                let n = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.l = n;
                8
            }

            0x96 => {
                // SUB (HL)
                let val = mmu.read_byte(self.get_hl());
                let half_carry = (self.a & 0x0F) < (val & 0x0F);
                let (result, carry) = self.a.overflowing_sub(val);
                self.a = result;
                self.set_flags(self.a == 0, true, half_carry, carry);
                8
            }

            0x06 => {
                // LB B, n
                let n = mmu.read_byte(self.pc);
                self.pc += 1;
                self.b = n;
                8 // uses 8 cycles (4 for opcode fetch, 4 to read n)
            }

            0x16 => {
                // LD D, n (Load 8-bit immediate value into register D)
                let n = mmu.read_byte(self.pc);
                self.pc += 1;
                self.d = n;
                8 // Tar 8 klockcykler
            }

            0x39 => {
                // ADD HL, SP (Add Stack Pointer to HL)
                let hl = self.get_hl();
                let sp = self.sp;

                // Kontrollera Half-Carry (bit 11) och Carry (bit 15)
                let half_carry = ((hl & 0x0FFF) + (sp & 0x0FFF)) > 0x0FFF;
                let (result, carry) = hl.overflowing_add(sp);

                self.set_hl(result);

                // Z-flaggan lämnas orörd (använder self.get_z() eller vad din struktur har för
                // Z)
                self.set_flags(self.get_z(), false, half_carry, carry);
                8
            }

            0x09 => {
                // ADD HL, BC (Add 16-bit register pair DE to HL pair)
                let hl = self.get_hl();
                let bc = self.get_bc();

                // Räkna ut 16-bitars Half-Carry (overflow vid bit 11)
                let half_carry = ((hl & 0x0FFF) + (bc & 0x0FFF)) > 0x0FFF;

                // Utför additionen säkert och fånga Carry (overflow vid bit 15)
                let (result, carry) = hl.overflowing_add(bc);
                self.set_hl(result);

                // Uppdatera flaggorna: Z lämnas orörd, N=false, H och C styrs av beräkningen!
                self.set_n(false);
                self.set_h(half_carry);
                self.set_c(carry);

                8 // Tar 8 klockcykler på den interna 16-bitarsbussen
            }

            0x19 => {
                // ADD HL, DE (Add 16-bit register pair DE to HL pair)
                let hl = self.get_hl();
                let de = self.get_de();

                // Räkna ut 16-bitars Half-Carry (overflow vid bit 11)
                let half_carry = ((hl & 0x0FFF) + (de & 0x0FFF)) > 0x0FFF;

                // Utför additionen säkert och fånga Carry (overflow vid bit 15)
                let (result, carry) = hl.overflowing_add(de);
                self.set_hl(result);

                // Uppdatera flaggorna: Z lämnas orörd, N=false, H och C styrs av beräkningen!
                self.set_n(false);
                self.set_h(half_carry);
                self.set_c(carry);

                8 // Tar 8 klockcykler på den interna 16-bitarsbussen
            }

            0x46 => {
                // LD B, (HL)
                self.b = mmu.read_byte(self.get_hl());
                8
            }
            0x4E => {
                // LD C, (HL)
                self.c = mmu.read_byte(self.get_hl());
                8
            }
            0x50 => {
                // LD D, B (Load register B into D)
                self.d = self.b;
                4
            }
            0x51 => {
                // LD D, C (Load register C into D)
                self.d = self.c;
                4
            }
            0x52 => {
                // LD D, D (Load register D into D - No-op)
                4
            }
            0x53 => {
                // LD D, E (Load register E into D)
                self.d = self.e;
                4
            }
            0x54 => {
                // LD D, H (Load register H into D)
                self.d = self.h;
                4
            }
            0x55 => {
                // LD D, L (Load register L into D)
                self.d = self.l;
                4
            }
            0x56 => {
                // LD D, (HL) (Load byte at memory address HL into D)
                self.d = mmu.read_byte(self.get_hl());
                8
            }
            0x57 => {
                // LD D, A (Load register A into D)
                self.d = self.a;
                4
            }
            0x58 => {
                // LD E, B (Load register B into E)
                self.e = self.b;
                4
            }
            0x59 => {
                // LD E, C (Load register C into E)
                self.e = self.c;
                4
            }
            0x5A => {
                // LD E, D (Load register D into E)
                self.e = self.d;
                4
            }
            0x5B => {
                // LD E, E (Load register E into E - No-op)
                4
            }
            0x5C => {
                // LD E, H (Load register H into E)
                self.e = self.h;
                4
            }
            0x5D => {
                // LD E, L (Load register L into E)
                self.e = self.l;
                4
            }
            0x5E => {
                // LD E, (HL) (Load byte at memory address HL into E)
                self.e = mmu.read_byte(self.get_hl());
                8
            }
            0x5F => {
                // LD E, A (Load register A into E)
                self.e = self.a;
                4
            }
            0x1E => {
                // LD E, n (Load immediate byte into register E)
                let val = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);

                self.e = val;
                8
            }
            0x63 => {
                self.h = self.e;
                4
            }
            0x64 => {
                // LD H, H = NOOP
                4
            }
            0x65 => {
                // LD H, L
                self.h = self.l;
                4
            }

            0x68 => {
                // LD L, B
                self.l = self.b;
                4
            }

            0x6A => {
                // LD L, D
                self.l = self.d;
                4
            }

            0x6D => {
                // LD L, L == noop
                4
            }

            0x61 => {
                // LD H, C
                self.h = self.c;
                4
            }
            0x66 => {
                // LD H, (HL)
                self.h = mmu.read_byte(self.get_hl());
                8
            }
            0x67 => {
                // LD H, A (Load register A into H)
                self.h = self.a;
                4
            }
            0x6E => {
                // LD L, (HL)
                self.l = mmu.read_byte(self.get_hl());
                8
            }
            0x6C => {
                // LD L, H
                self.l = self.h;
                4
            }
            0x6F => {
                // LD L, A (Load register A into L)
                self.l = self.a;
                4
            }
            0x7D => {
                // LD A, L (Load register L into A)
                self.a = self.l;
                4
            }
            0x7E => {
                // LD A, (HL)
                self.a = mmu.read_byte(self.get_hl());
                8
            }

            0x7F => {
                // LD A, A == noop
                4
            }

            0x60 => {
                // LD H, B
                self.h = self.b;
                4
            }

            0x62 => {
                // LD H, D (Load register D into H)
                self.h = self.d;
                4
            }

            0x69 => {
                // LD L, C
                self.l = self.c;
                4
            }

            0x6B => {
                // LD L, E (Load register E into L)
                self.l = self.e;
                4
            }

            0xE9 => {
                let hl = self.get_hl();
                /*                 if hl >= 0x8000 {
                    println!(
                        "JP (HL) HL={:04X} A={:02X} från {:04X} FFC0={:02X} FFCD={:02X} FFE1={:02X}",
                        hl,
                        self.a,
                        self.pc.wrapping_sub(1),
                        mmu.read_byte(0xFFC0),
                        mmu.read_byte(0xFFCD),
                        mmu.read_byte(0xFFE1)
                    );
                } */
                if self.pc.wrapping_sub(1) < 0x8000 {
                    self.last_rom_pc = self.pc.wrapping_sub(1);
                }
                self.pc = hl;
                4
            }

            0x03 => {
                // INC BC
                let val = self.get_bc().wrapping_add(1);
                self.set_bc(val);
                8
            }

            0x13 => {
                // INC DE
                let val = self.get_de().wrapping_add(1);
                self.set_de(val);
                8
            }

            0x23 => {
                // INC HL
                let val = self.get_hl().wrapping_add(1);
                self.set_hl(val);
                8
            }

            0x33 => {
                // INC SP
                self.sp = self.sp.wrapping_add(1);
                8
            }

            0x34 => {
                // INC (HL) (Increment value at memory address HL)
                let addr = self.get_hl();
                let val = mmu.read_byte(addr);
                let result = val.wrapping_add(1);

                mmu.write_byte(addr, result);

                // Flaggor: C-flaggan skickas med orörd via t.ex. self.get_c()
                let half_carry = (val & 0x0F) == 0x0F;
                self.set_flags(result == 0, false, half_carry, self.get_c());

                12
            }

            0x18 => {
                // JR r8 (Unconditional relative jump)
                let offset = mmu.read_byte(self.pc) as i8;
                self.pc += 1;

                // Jump around!
                self.pc = (self.pc as i32).wrapping_add(offset as i32) as u16;
                12
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
            0x11 => {
                // LD DE, d16 (Load 16-bit immediate value into DE pair)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;

                let d16 = (high << 8) | low;
                self.set_de(d16); // Använd din fina 16-bitarsmetod!
                12 // Tar 12 klockcykler
            }
            0x12 => {
                let addr = self.get_de();
                mmu.write_byte(addr, self.a);
                8
            }

            0x02 => {
                let addr = self.get_bc();
                if (0xC000..0xC0A0).contains(&addr) || (0x2B00..0x2C00).contains(&addr) {
                    println!("(BC)={:04X} A={:02X}", addr, self.a);
                }
                mmu.write_byte(addr, self.a);
                8
            }

            0x1A => {
                // LD A, (DE) (Load value from memory address DE into register A)
                let addr = self.get_de();
                let value = mmu.read_byte(addr);
                self.a = value;
                8 // Tar 8 klockcykler
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

            0xA0 => {
                // AND B
                self.a &= self.b;
                self.set_flags(self.a == 0, false, true, false);
                4
            }

            0xA1 => {
                // AND C ( Bitwise AND register C with register A)
                self.a &= self.c;

                self.set_flags(self.a == 0, false, true, false);
                4 // 4 cycles
            }
            0xA2 => {
                // AND D
                self.a &= self.d;
                self.set_flags(self.a == 0, false, true, false);
                4
            }
            0xA3 => {
                // AND E
                self.a &= self.e;
                self.set_flags(self.a == 0, false, true, false);
                4
            }
            0xA4 => {
                // AND H
                self.a &= self.h;
                self.set_flags(self.a == 0, false, true, false);
                4
            }
            0xA5 => {
                // AND L
                self.a &= self.l;
                self.set_flags(self.a == 0, false, true, false);
                4
            }
            0xA7 => {
                // AND A
                self.a &= self.a;
                self.set_flags(self.a == 0, false, true, false);
                4
            }

            0x35 => {
                // DEC (HL) (Decrement value at memory address HL by 1)
                let addr = self.get_hl();
                let current_val = mmu.read_byte(addr);

                // Kolla Half-Carry innan subtraktion
                let half_carry = (current_val & 0x0F) == 0x00;

                let new_val = current_val.wrapping_sub(1);
                mmu.write_byte(addr, new_val);

                // Uppdatera flaggorna: Z styrs av nya värdet, N=true, H, C lämnas orörd
                self.set_z(new_val == 0);
                self.set_n(true);
                self.set_h(half_carry);

                12 // Sätter vi till 12 cykler så matchar det Game Boys standard-timing perfekt!
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
                // LD (HL-), A
                let addr = self.get_hl();
                mmu.write_byte(addr, self.a);
                self.set_hl(addr.wrapping_sub(1));
                8
            }
            0x05 => {
                let half_carry = (self.b & 0x0F) == 0x00;
                self.b = self.b.wrapping_sub(1);
                self.set_flags(self.b == 0, true, half_carry, self.get_c());
                4
            }

            0x07 => {
                // RLCA (Rotate register A left. Old bit 7 to Carry and to bit 0)
                let carry = (self.a & 0x80) != 0; // Hämta bit 7

                // Rotera ett steg till vänster och lägg till gamla bit 7 på bit 0:s plats
                self.a = (self.a << 1) | (if carry { 1 } else { 0 });

                // Z svingas alltid till false på Game Boy för denna instruktion!
                self.set_flags(false, false, false, carry);
                4
            }

            0x0D => {
                // DEC C — yttre loopen
                let old = self.c;
                self.c = old.wrapping_sub(1);
                self.set_flags(self.c == 0, true, (old & 0x0F) == 0, self.get_c());
                4
            }
            0x15 => {
                // DEC D
                let old = self.d;
                self.d = old.wrapping_sub(1);
                self.set_flags(self.d == 0, true, (old & 0x0F) == 0, self.get_c());
                4
            }
            0x1D => {
                // DEC E
                let old = self.e;
                self.e = old.wrapping_sub(1);
                self.set_flags(self.e == 0, true, (old & 0x0F) == 0, self.get_c());
                4
            }
            0x25 => {
                // DEC H
                let old = self.h;
                self.h = old.wrapping_sub(1);
                self.set_flags(self.h == 0, true, (old & 0x0F) == 0, self.get_c());
                4
            }

            0x0B => {
                // DEC BC (Decrement BC register pair)
                let bc = self.get_bc();
                self.set_bc(bc.wrapping_sub(1));
                8
            }
            0x1B => {
                // DEC DE (Decrement DE register pair)
                let de = self.get_de();
                self.set_de(de.wrapping_sub(1));
                8
            }
            0x2B => {
                // DEC HL (Decrement HL register pair - Denna har du redan, men bra för blocket)
                let hl = self.get_hl();
                self.set_hl(hl.wrapping_sub(1));
                8
            }
            0x3B => {
                // DEC SP (Decrement Stack Pointer)
                self.sp = self.sp.wrapping_sub(1);
                8
            }

            0x2D => {
                // DEC L
                let old = self.l;
                self.l = old.wrapping_sub(1);
                self.set_flags(self.l == 0, true, (old & 0x0F) == 0, self.get_c());
                4
            }

            0x3D => {
                // DEC A
                let old = self.a;
                self.a = old.wrapping_sub(1);
                self.set_flags(self.a == 0, true, (old & 0x0F) == 0, self.get_c());
                4
            }
            0x20 => {
                // JR NZ r8 (Jump Relative if Not Zero)
                // Read jump-offset as a signed i8
                let offset = mmu.read_byte(self.pc) as i8;
                self.pc += 1;

                if !self.get_z() {
                    self.pc = (self.pc as i32).wrapping_add(offset as i32) as u16;
                    12 // 12 cycles used
                } else {
                    8 // 8 cycles used when not jumping
                }
            }

            0x28 => {
                // JR Z, r8 (Zump if Zero)
                let offset = mmu.read_byte(self.pc) as i8;
                self.pc += 1;

                if self.get_z() {
                    self.pc = (self.pc as i32).wrapping_add(offset as i32) as u16;
                    12
                } else {
                    8
                }
            }

            0x30 => {
                // JR NC, r8 (Jump if Not Carry)
                let offset = mmu.read_byte(self.pc) as i8;
                self.pc += 1;
                if !self.get_c() {
                    self.pc = (self.pc as i32).wrapping_add(offset as i32) as u16;
                    12
                } else {
                    8
                }
            }

            0x38 => {
                // JR C, r8 (Jump if Carry)
                let offset = mmu.read_byte(self.pc) as i8;
                self.pc += 1;
                if self.get_c() {
                    self.pc = (self.pc as i32).wrapping_add(offset as i32) as u16;
                    12
                } else {
                    8
                }
            }

            0x3E => {
                // LD A, n
                let n = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a = n;
                8
            }
            0xF3 => {
                self.ime = false;
                4
            }

            0xE0 => {
                let n = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                mmu.write_byte(0xFF00 | n as u16, self.a);
                12
            }

            0xF0 => {
                let n = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let addr = 0xFF00 | n as u16;
                self.a = mmu.read_byte(addr);
                12
            }

            0xFE => {
                // CP n (Compare register A with immediate 8-bit value)
                let n = mmu.read_byte(self.pc);
                self.pc += 1;

                // Calculate flags based on (A - n)
                let zero = self.a == n;
                let half_carry = (self.a & 0x0F) < (n & 0x0F);
                let carry = self.a < n;

                self.set_flags(zero, true, half_carry, carry);
                8 // 8 cycles
            }

            0x36 => {
                // LD (HL), n ( Write immediate 8-bit value to memory address HL)
                let n = mmu.read_byte(self.pc);
                self.pc += 1;

                let addr = self.get_hl();
                mmu.write_byte(addr, n);
                12 // 12 cycles
            }

            0xEA => {
                let low = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let high = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let addr = (high << 8) | low;
                if (0xC000..0xC0A0).contains(&addr) {
                    println!("EA [{:04X}]={:02X}", addr, self.a);
                }
                mmu.write_byte(addr, self.a);
                16
            }

            0xFA => {
                // LD A, (nn) (Read from absolute 16-bit address nn into register A)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;

                let nn = (high << 8) | low;
                let value = mmu.read_byte(nn);
                self.a = value;

                16
            }

            0x31 => {
                //LD SP, d16 (load 16-bit immediate value into stack pointer)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;

                let d16 = (high << 8) | low;
                self.sp = d16;
                12 // 12 cycles
            }

            0x2A => {
                // LD A, (HL+) (Read memory at HL into register A, then increment HL)
                let addr = self.get_hl();
                let value = mmu.read_byte(addr);
                self.a = value;

                // Increase HL with 1 (with wrapping)
                let new_hl = addr.wrapping_add(1);
                self.set_hl(new_hl);

                8 // 8 cycles
            }

            0x3A => {
                // LD A, (HL-) (Read memory at HL into register A, then decrement HL)
                let addr = self.get_hl();
                let value = mmu.read_byte(addr);
                self.a = value;

                // Decrease HL with 1 (with wrapping)
                let new_hl = addr.wrapping_sub(1);
                self.set_hl(new_hl);

                8 // 8 cycles
            }

            0x22 => {
                let addr = self.get_hl();
                /*                 if (0xC000..0xC0A0).contains(&addr) || (0x2B00..0x2C00).contains(&addr) {
                    println!("LDI (HL)={:04X} A={:02X}", addr, self.a);
                } */
                mmu.write_byte(addr, self.a);
                self.set_hl(addr.wrapping_add(1));
                8
            }
            0xE2 => {
                // LD ($FF00+C), A (Write register A to memory address 0xFF00 + register C)
                let addr = 0xFF00 | (self.c as u16);
                mmu.write_byte(addr, self.a);
                8 // 8 cycles
            }

            // ==========================================
            // SUB-REGISTERSFAMILJEN (SUB r)
            // ==========================================
            0x90 => {
                // SUB B
                let half_carry = (self.a & 0x0F) < (self.b & 0x0F);
                let (result, carry) = self.a.overflowing_sub(self.b);
                self.a = result;
                self.set_flags(self.a == 0, true, half_carry, carry);
                4
            }
            0x91 => {
                // SUB C
                let half_carry = (self.a & 0x0F) < (self.c & 0x0F);
                let (result, carry) = self.a.overflowing_sub(self.c);
                self.a = result;
                self.set_flags(self.a == 0, true, half_carry, carry);
                4
            }
            0x92 => {
                // SUB D
                let half_carry = (self.a & 0x0F) < (self.d & 0x0F);
                let (result, carry) = self.a.overflowing_sub(self.d);
                self.a = result;
                self.set_flags(self.a == 0, true, half_carry, carry);
                4
            }
            0x93 => {
                // SUB E
                let half_carry = (self.a & 0x0F) < (self.e & 0x0F);
                let (result, carry) = self.a.overflowing_sub(self.e);
                self.a = result;
                self.set_flags(self.a == 0, true, half_carry, carry);
                4
            }
            0x94 => {
                // SUB H
                let half_carry = (self.a & 0x0F) < (self.h & 0x0F);
                let (result, carry) = self.a.overflowing_sub(self.h);
                self.a = result;
                self.set_flags(self.a == 0, true, half_carry, carry);
                4
            }
            0x95 => {
                // SUB L
                let half_carry = (self.a & 0x0F) < (self.l & 0x0F);
                let (result, carry) = self.a.overflowing_sub(self.l);
                self.a = result;
                self.set_flags(self.a == 0, true, half_carry, carry);
                4
            }
            0x97 => {
                // SUB A (Subtract register A from itself - always results in 0)
                self.a = 0;

                // Flaggor: Z=true (eftersom A=0), N=true (det var en subtraktion), H=false,
                // C=false
                self.set_flags(true, true, false, false);
                4
            }

            0xD6 => {
                // SUB A, n (Subtract immediate byte from A)
                let val = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);

                let half_carry = (self.a & 0x0F) < (val & 0x0F);
                let (result, carry) = self.a.overflowing_sub(val);
                self.a = result;

                self.set_flags(self.a == 0, true, half_carry, carry);
                8
            }

            0xDE => {
                // SBC A, n (Subtract immediate byte + Carry from A)
                let val = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let carry_val = if self.get_c() { 1 } else { 0 };

                // Beräkna Half-Carry (bit 4) med hänsyn till carry
                let half_carry = (self.a & 0x0F) < (val & 0x0F) + carry_val;

                // Utför subtraktionen i två steg för att fånga Carry korrekt via
                // overflowing_sub
                let (res1, carry1) = self.a.overflowing_sub(val);
                let (final_res, carry2) = res1.overflowing_sub(carry_val);
                self.a = final_res;

                self.set_flags(self.a == 0, true, half_carry, carry1 || carry2);
                8
            }

            0x04 => {
                // INC B
                let old = self.b;
                self.b = old.wrapping_add(1);
                self.set_flags(self.b == 0, false, (old & 0x0F) == 0x0F, self.get_c());
                4
            }

            0x0C => {
                let old = self.c;
                self.c = old.wrapping_add(1);
                self.set_flags(self.c == 0, false, (old & 0x0F) == 0x0F, self.get_c());
                4
            }

            0x14 => {
                // INC D
                let old = self.d;
                self.d = old.wrapping_add(1);
                self.set_flags(self.d == 0, false, (old & 0x0F) == 0x0F, self.get_c());
                4
            }

            0x1C => {
                // INC E
                let old = self.e;
                self.e = old.wrapping_add(1);
                self.set_flags(self.e == 0, false, (old & 0x0F) == 0x0F, self.get_c());
                4
            }

            0x24 => {
                // INC H
                let old = self.h;
                self.h = old.wrapping_add(1);
                self.set_flags(self.h == 0, false, (old & 0x0F) == 0x0F, self.get_c());
                4
            }
            0x2C => {
                // INC L
                let old = self.l;
                self.l = old.wrapping_add(1);
                self.set_flags(self.l == 0, false, (old & 0x0F) == 0x0F, self.get_c());
                4
            }

            0x3C => {
                // INC A
                let old = self.a;
                self.a = old.wrapping_add(1);
                self.set_flags(self.a == 0, false, (old & 0x0F) == 0x0F, self.get_c());
                4
            }

            0xCD => {
                // CALL nn
                let low = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let high = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let nn = (high << 8) | low;

                /*                 let call_addr = self.pc.wrapping_sub(3);
                               if (0x0170..0x01E0).contains(&call_addr) {
                                   println!("VBlank CALL {:04X} från {:04X}", nn, call_addr);
                               }
                */
                let ret = self.pc;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, (ret >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, ret as u8);
                self.pc = nn;
                24
            }

            0x70 => {
                // LD (HL), B
                mmu.write_byte(self.get_hl(), self.b);
                8
            }
            0x71 => {
                // LD (HL), C
                mmu.write_byte(self.get_hl(), self.c);
                8
            }
            0x72 => {
                // LD (HL), D
                mmu.write_byte(self.get_hl(), self.d);
                8
            }
            0x73 => {
                // LD (HL), E
                mmu.write_byte(self.get_hl(), self.e);
                8
            }
            0x74 => {
                // LD (HL), H
                mmu.write_byte(self.get_hl(), self.h);
                8
            }
            0x75 => {
                // LD (HL), L
                mmu.write_byte(self.get_hl(), self.l);
                8
            }
            0x77 => {
                let addr = self.get_hl();
                if (0xC000..0xC0A0).contains(&addr) || (0x2B00..0x2C00).contains(&addr) {
                    println!("(HL)={:04X} A={:02X}", addr, self.a);
                }
                mmu.write_byte(addr, self.a);
                8
            }

            0x78 => {
                // LD A, B (Copy register B into register A)
                self.a = self.b;
                4 // uses 4 cycles
            }

            0x79 => {
                // LD A, C (Copy register C into register A)
                self.a = self.c;
                4 // uses 4 cycles
            }

            0x7A => {
                // LD A, D (Load register D into A)
                self.a = self.d;
                4
            }

            0x7B => {
                // LD A, E (Load register E into A)
                self.a = self.e;
                4
            }

            0x7C => {
                // LD A, H (Copy register H into register A)
                self.a = self.h;
                4 // 4 cycles
            }

            0xC0 => {
                // RET NZ (Return if Not Zero)
                if !self.get_z() {
                    let low = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    let high = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    self.pc = (high << 8) | low;
                    20
                } else {
                    8
                }
            }

            0xC8 => {
                // RET Z (Return if Zero)
                if self.get_z() {
                    let low = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    let high = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    self.pc = (high << 8) | low;
                    20
                } else {
                    8
                }
            }

            0xD0 => {
                // RET NC (Return if Not Carry)
                if !self.get_c() {
                    let low = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    let high = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    self.pc = (high << 8) | low;
                    20
                } else {
                    8
                }
            }

            0xD8 => {
                // RET C (Return if Carry)
                if self.get_c() {
                    let low = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    let high = mmu.read_byte(self.sp) as u16;
                    self.sp = self.sp.wrapping_add(1);
                    self.pc = (high << 8) | low;
                    20
                } else {
                    8
                }
            }

            0xC9 => {
                // RET (Return from Subroutine)
                // Pop the return address from stack (little endian)
                let low = mmu.read_byte(self.sp) as u16;
                self.sp = self.sp.wrapping_add(1);

                let high = mmu.read_byte(self.sp) as u16;
                self.sp = self.sp.wrapping_add(1);

                let return_addr = (high << 8) | low;

                // Set PC to address to return to
                self.pc = return_addr;
                16 // uses 16 cycles
            }

            0xC2 => {
                // JP NZ, nn (Jump if Not Zero)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let nn = (high << 8) | low;

                if !self.get_z() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }

            0xCA => {
                // JP Z, nn (Jump if Zero)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let nn = (high << 8) | low;

                if self.get_z() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }

            0xD2 => {
                // JP NC, nn (Jump if Not Carry)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let nn = (high << 8) | low;

                if !self.get_c() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }

            0xDA => {
                // JP C, nn (Jump if Carry)
                let low = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let high = mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                let nn = (high << 8) | low;

                if self.get_c() {
                    self.pc = nn;
                    16
                } else {
                    12
                }
            }

            0xF5 => {
                // Push AF
                let a = self.a;
                let f = self.f;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, a);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, f);
                16
            }

            0xC5 => {
                // Push BC
                let b = self.b;
                let c = self.c;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, b);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, c);
                16
            }

            0xC6 => {
                // ADD A, n (Add immediate byte to A)
                let val = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);

                let half_carry = ((self.a & 0x0F) + (val & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(val);
                self.a = result;

                self.set_flags(self.a == 0, false, half_carry, carry);
                8
            }

            0xD5 => {
                // Push DE
                let d = self.d;
                let e = self.e;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, d);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, e);
                16
            }

            0xE5 => {
                // Push HL
                let h = self.h;
                let l = self.l;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, h);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, l);
                16
            }

            0xF6 => {
                // OR A, n (Bitwise OR immediate byte with A)
                let val = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);

                self.a |= val;

                self.set_flags(self.a == 0, false, false, false);
                8
            }

            0xFB => {
                self.ime = true;
                4
            }
            0x2F => {
                // CPL (Complement A - Invert all bits in register A)
                self.a = !self.a;

                // Update flags: N and H are always true. Z and C unaltered
                self.set_n(true);
                self.set_h(true);

                4 // 4 cycles
            }

            0xE6 => {
                // AND n (Bitwise AND register A with immediate 8-bit balue n)
                let n = mmu.read_byte(self.pc);
                self.pc += 1;

                self.a &= n; // Execute AND and save in A

                // Update flags: Z set by result, N=false, H=true (always!), C=false
                self.set_flags(self.a == 0, false, true, false);
                8 // 8 cycles
            }

            0xEE => {
                // XOR A, n
                let n = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                self.a ^= n;
                self.set_flags(self.a == 0, false, false, false);
                8
            }

            0xCB => {
                // CB Prefix - Read next byte and run from secondary table
                let cb_opcode = mmu.read_byte(self.pc);
                self.pc += 1;

                // Let's call our new method (returning it's cycles +4 for the 0xCB search)
                self.step_cb(cb_opcode, mmu) + 4
            }

            0x4F => {
                // LD C, A (Copy register A to register C)
                self.c = self.a;
                4 // 4 cycles
            }

            0x80 => {
                // ADD A, B (Add register B to A)
                let half_carry = ((self.a & 0x0F) + (self.b & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(self.b);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                4
            }
            0x81 => {
                // ADD A, C (Add register C to A)
                let half_carry = ((self.a & 0x0F) + (self.c & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(self.c);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                4
            }
            0x82 => {
                // ADD A, D (Add register D to A)
                let half_carry = ((self.a & 0x0F) + (self.d & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(self.d);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                4
            }
            0x83 => {
                // ADD A, E (Add register E to A)
                let half_carry = ((self.a & 0x0F) + (self.e & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(self.e);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                4
            }
            0x84 => {
                // ADD A, H (Add register H to A)
                let half_carry = ((self.a & 0x0F) + (self.h & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(self.h);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                4
            }
            0x85 => {
                // ADD A, L (Add register L to A)
                let half_carry = ((self.a & 0x0F) + (self.l & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(self.l);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                4
            }
            0x86 => {
                // ADD A, (HL) (Add byte at memory address HL to A)
                let val = mmu.read_byte(self.get_hl());
                let half_carry = ((self.a & 0x0F) + (val & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(val);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                8
            }
            0x87 => {
                // ADD A, A (Add register A to itself)
                let half_carry = ((self.a & 0x0F) + (self.a & 0x0F)) > 0x0F;
                let (result, carry) = self.a.overflowing_add(self.a);
                self.a = result;
                self.set_flags(self.a == 0, false, half_carry, carry);
                4
            }

            0x88 => {
                // ADC A, B (Add register B + Carry to A)
                let a = self.a;
                let b = self.b;
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (b & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(b);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                4
            }
            0x89 => {
                // ADC A, C (Add register C + Carry to A)
                let a = self.a;
                let c = self.c;
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (c & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(c);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                4
            }
            0x8A => {
                // ADC A, D (Add register D + Carry to A)
                let a = self.a;
                let d = self.d;
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (d & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(d);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                4
            }
            0x8B => {
                // ADC A, E (Add register E + Carry to A)
                let a = self.a;
                let e = self.e;
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (e & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(e);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                4
            }
            0x8C => {
                // ADC A, H (Add register H + Carry to A)
                let a = self.a;
                let h = self.h;
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (h & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(h);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                4
            }
            0x8D => {
                // ADC A, L (Add register L + Carry to A)
                let a = self.a;
                let l = self.l;
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (l & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(l);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                4
            }
            0x8E => {
                // ADC A, (HL) (Add byte at memory address HL + Carry to A)
                let a = self.a;
                let val = mmu.read_byte(self.get_hl());
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (val & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(val);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                8
            }
            0x8F => {
                // ADC A, A (Add register A + Carry to itself)
                let a = self.a;
                let carry_val = if self.get_c() { 1 } else { 0 };

                let half_carry = ((a & 0x0F) + (a & 0x0F) + carry_val) > 0x0F;
                let (res1, carry1) = a.overflowing_add(a);
                let (final_res, carry2) = res1.overflowing_add(carry_val);

                self.a = final_res;
                self.set_flags(self.a == 0, false, half_carry, carry1 || carry2);
                4
            }

            0x27 => {
                // DAA
                let mut a = self.a;
                let mut adjust = 0u8;
                let mut c = self.get_c();

                if !self.get_n() {
                    if self.get_c() || a > 0x99 {
                        adjust |= 0x60;
                        c = true;
                    }
                    if self.get_h() || (a & 0x0F) > 0x09 {
                        adjust |= 0x06;
                    }
                    a = a.wrapping_add(adjust);
                } else {
                    if self.get_c() {
                        adjust |= 0x60;
                    }
                    if self.get_h() {
                        adjust |= 0x06;
                    }
                    a = a.wrapping_sub(adjust);
                }

                self.a = a;
                self.set_flags(self.a == 0, self.get_n(), false, c);
                4
            }

            0xC1 => {
                // POP BC
                self.c = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.b = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);
                12
            }

            0xD1 => {
                // POP DE
                self.e = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.d = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);
                12
            }

            0xD9 => {
                let low = mmu.read_byte(self.sp) as u16;
                self.sp = self.sp.wrapping_add(1);
                let high = mmu.read_byte(self.sp) as u16;
                self.sp = self.sp.wrapping_add(1);
                self.pc = (high << 8) | low;
                self.ime = true;
                16
            }

            0xE1 => {
                // POP HL
                self.l = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.h = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);
                12
            }

            0xF1 => {
                // POP AF (Viktigt: Uppdaterar flaggregistret F!)
                let f_value = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);
                self.a = mmu.read_byte(self.sp);
                self.sp = self.sp.wrapping_add(1);

                // Maskera bort de 4 lägsta bitarna i F, de är alltid 0 på riktig hårdvara
                self.f = f_value & 0xF0;
                12
            }

            // ==========================================
            // SBC A, r FAMILJEN (Subtract with Carry) - 4 CYKLER VAR
            // ==========================================
            0x98 => {
                // SBC A, B
                let carry_val = if self.get_c() { 1 } else { 0 };
                let hl_borrow = (self.a & 0x0F) < (self.b & 0x0F) + carry_val;
                let full_sub = (self.a as i32) - (self.b as i32) - (carry_val as i32);
                self.a = self.a.wrapping_sub(self.b).wrapping_sub(carry_val);
                self.set_flags(self.a == 0, true, hl_borrow, full_sub < 0);
                4
            }
            0x99 => {
                // SBC A, C
                let carry_val = if self.get_c() { 1 } else { 0 };
                let hl_borrow = (self.a & 0x0F) < (self.c & 0x0F) + carry_val;
                let full_sub = (self.a as i32) - (self.c as i32) - (carry_val as i32);
                self.a = self.a.wrapping_sub(self.c).wrapping_sub(carry_val);
                self.set_flags(self.a == 0, true, hl_borrow, full_sub < 0);
                4
            }
            0x9A => {
                // SBC A, D
                let carry_val = if self.get_c() { 1 } else { 0 };
                let hl_borrow = (self.a & 0x0F) < (self.d & 0x0F) + carry_val;
                let full_sub = (self.a as i32) - (self.d as i32) - (carry_val as i32);
                self.a = self.a.wrapping_sub(self.d).wrapping_sub(carry_val);
                self.set_flags(self.a == 0, true, hl_borrow, full_sub < 0);
                4
            }
            0x9B => {
                // SBC A, E
                let carry_val = if self.get_c() { 1 } else { 0 };
                let hl_borrow = (self.a & 0x0F) < (self.e & 0x0F) + carry_val;
                let full_sub = (self.a as i32) - (self.e as i32) - (carry_val as i32);
                self.a = self.a.wrapping_sub(self.e).wrapping_sub(carry_val);
                self.set_flags(self.a == 0, true, hl_borrow, full_sub < 0);
                4
            }
            0x9C => {
                // SBC A, H
                let carry_val = if self.get_c() { 1 } else { 0 };
                let hl_borrow = (self.a & 0x0F) < (self.h & 0x0F) + carry_val;
                let full_sub = (self.a as i32) - (self.h as i32) - (carry_val as i32);
                self.a = self.a.wrapping_sub(self.h).wrapping_sub(carry_val);
                self.set_flags(self.a == 0, true, hl_borrow, full_sub < 0);
                4
            }
            0x9D => {
                // SBC A, L
                let carry_val = if self.get_c() { 1 } else { 0 };
                let hl_borrow = (self.a & 0x0F) < (self.l & 0x0F) + carry_val;
                let full_sub = (self.a as i32) - (self.l as i32) - (carry_val as i32);
                self.a = self.a.wrapping_sub(self.l).wrapping_sub(carry_val);
                self.set_flags(self.a == 0, true, hl_borrow, full_sub < 0);
                4
            }
            0x9F => {
                // SBC A, A (Subtract register A and Carry from A)
                let carry_val = if self.get_c() { 1 } else { 0 };

                // Eftersom A - A blir 0, så blir resultatet alltid bara 0 - carry_val
                let result = (0u8).wrapping_sub(carry_val);

                // Ett borrow sker alltid på båda ställen om carry var 1 (satt)
                let borrow = carry_val == 1;

                self.a = result;
                self.set_flags(self.a == 0, true, borrow, borrow);
                4
            }

            // ==========================================
            // LD B, r FAMILJEN (Kopiera register till B) - 4 CYKLER VAR
            // ==========================================
            0x40 => {
                // LD B, B
                //self.b = self.b;
                4
            }
            0x41 => {
                // LD B, C
                self.b = self.c;
                4
            }
            0x42 => {
                // LD B, D
                self.b = self.d;
                4
            }
            0x43 => {
                // LD B, E
                self.b = self.e;
                4
            }
            0x44 => {
                // LD B, H
                self.b = self.h;
                4
            }
            0x45 => {
                // LD B, L
                self.b = self.l;
                4
            }
            0x47 => {
                // LD B, A
                self.b = self.a;
                4
            }

            0xB0 => {
                // OR A, B (Bitwise OR register B with A)
                self.a |= self.b;
                self.set_flags(self.a == 0, false, false, false);
                4
            }
            0xB1 => {
                // OR A, C (Bitwise OR register C with A)
                self.a |= self.c;
                self.set_flags(self.a == 0, false, false, false);
                4
            }
            0xB2 => {
                // OR A, D (Bitwise OR register D with A)
                self.a |= self.d;
                self.set_flags(self.a == 0, false, false, false);
                4
            }
            0xB3 => {
                // OR A, E (Bitwise OR register E with A)
                self.a |= self.e;
                self.set_flags(self.a == 0, false, false, false);
                4
            }
            0xB4 => {
                // OR A, H (Bitwise OR register H with A)
                self.a |= self.h;
                self.set_flags(self.a == 0, false, false, false);
                4
            }
            0xB5 => {
                // OR A, L (Bitwise OR register L with A)
                self.a |= self.l;
                self.set_flags(self.a == 0, false, false, false);
                4
            }
            0xB6 => {
                // OR A, (HL) (Bitwise OR byte at memory address HL with A)
                let val = mmu.read_byte(self.get_hl());
                self.a |= val;
                self.set_flags(self.a == 0, false, false, false);
                8
            }
            0xB7 => {
                // OR A, A (Bitwise OR register A with itself - Sätter bara flaggor)
                self.a |= self.a;
                self.set_flags(self.a == 0, false, false, false);
                4
            }

            0xB8 => {
                /* CP B */
                let n = self.b;
                self.set_flags(self.a == n, true, (self.a & 0x0F) < (n & 0x0F), self.a < n);
                4
            }
            0xB9 => {
                let n = self.c;
                self.set_flags(self.a == n, true, (self.a & 0x0F) < (n & 0x0F), self.a < n);
                4
            }
            0xBA => {
                let n = self.d;
                self.set_flags(self.a == n, true, (self.a & 0x0F) < (n & 0x0F), self.a < n);
                4
            }
            0xBB => {
                let n = self.e;
                self.set_flags(self.a == n, true, (self.a & 0x0F) < (n & 0x0F), self.a < n);
                4
            }
            0xBC => {
                let n = self.h;
                self.set_flags(self.a == n, true, (self.a & 0x0F) < (n & 0x0F), self.a < n);
                4
            }
            0xBD => {
                let n = self.l;
                self.set_flags(self.a == n, true, (self.a & 0x0F) < (n & 0x0F), self.a < n);
                4
            }
            0xBE => {
                // CP (HL)
                let n = mmu.read_byte(self.get_hl());
                let zero = self.a == n;
                let half_carry = (self.a & 0x0F) < (n & 0x0F);
                let carry = self.a < n;
                self.set_flags(zero, true, half_carry, carry);
                8
            }
            0xBF => {
                self.set_flags(true, true, false, false);
                4
            } // CP A → alltid Z

            0xC7 => {
                // RST 00H
                let high = ((self.pc >> 8) & 0xFF) as u8;
                let low = (self.pc & 0xFF) as u8;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, high);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, low);
                self.pc = 0x0000;
                16
            }
            0xCF => {
                // RST 08H
                let high = ((self.pc >> 8) & 0xFF) as u8;
                let low = (self.pc & 0xFF) as u8;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, high);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, low);
                self.pc = 0x0008;
                16
            }
            0xD7 => {
                // RST 10H (Denna har du redan, men bra att ha i blocket)
                let high = ((self.pc >> 8) & 0xFF) as u8;
                let low = (self.pc & 0xFF) as u8;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, high);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, low);
                self.pc = 0x0010;
                16
            }
            0xDF => {
                // RST 18H
                let high = ((self.pc >> 8) & 0xFF) as u8;
                let low = (self.pc & 0xFF) as u8;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, high);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, low);
                self.pc = 0x0018;
                16
            }
            0xE7 => {
                // RST 20H
                let high = ((self.pc >> 8) & 0xFF) as u8;
                let low = (self.pc & 0xFF) as u8;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, high);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, low);
                self.pc = 0x0020;
                16
            }
            0xEF => {
                let return_addr = self.pc;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, (return_addr >> 8) as u8);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, (return_addr & 0xFF) as u8);
                self.pc = 0x0028;
                16
            }
            0xF7 => {
                // RST 30H
                let high = ((self.pc >> 8) & 0xFF) as u8;
                let low = (self.pc & 0xFF) as u8;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, high);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, low);
                self.pc = 0x0030;
                16
            }
            0xFF => {
                // RST 38H (Den som kraschade just nu)
                let high = ((self.pc >> 8) & 0xFF) as u8;
                let low = (self.pc & 0xFF) as u8;
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, high);
                self.sp = self.sp.wrapping_sub(1);
                mmu.write_byte(self.sp, low);
                self.pc = 0x0038;
                16
            }

            0x76 => {
                self.halted = true;
                4
            }

            0xC4 => {
                // CALL NZ, nn
                let low = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let high = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let nn = (high << 8) | low;
                if !self.get_z() {
                    let ret = self.pc;
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, (ret >> 8) as u8);
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, ret as u8);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }

            0xCC => {
                // CALL Z, nn
                let low = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let high = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let nn = (high << 8) | low;
                if self.get_z() {
                    let ret = self.pc;
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, (ret >> 8) as u8);
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, ret as u8);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }

            0xD4 => {
                // CALL NC, nn
                let low = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let high = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let nn = (high << 8) | low;
                if !self.get_c() {
                    let ret = self.pc;
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, (ret >> 8) as u8);
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, ret as u8);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }

            0xDC => {
                // CALL C, nn
                let low = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let high = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let nn = (high << 8) | low;
                if self.get_c() {
                    let ret = self.pc;
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, (ret >> 8) as u8);
                    self.sp = self.sp.wrapping_sub(1);
                    mmu.write_byte(self.sp, ret as u8);
                    self.pc = nn;
                    24
                } else {
                    12
                }
            }

            0xAE => {
                // XOR (HL)
                self.a ^= mmu.read_byte(self.get_hl());
                self.set_flags(self.a == 0, false, false, false);
                8
            }

            0x1F => {
                // RRA
                let old_c = self.get_c();
                let new_c = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | if old_c { 0x80 } else { 0 };
                self.set_flags(false, false, false, new_c);
                4
            }

            0xCE => {
                // ADC A, n
                let n = mmu.read_byte(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let c = if self.get_c() { 1u8 } else { 0 };
                let half_carry = ((self.a & 0x0F) + (n & 0x0F) + c) > 0x0F;
                let sum = self.a as u16 + n as u16 + c as u16;
                self.a = sum as u8;
                self.set_flags(self.a == 0, false, half_carry, sum > 0xFF);
                8
            }

            0x29 => {
                // ADD HL, HL
                let hl = self.get_hl();
                let half_carry = (hl & 0x0FFF) + (hl & 0x0FFF) > 0x0FFF;
                let (result, carry) = hl.overflowing_add(hl);
                self.set_hl(result);
                self.set_n(false);
                self.set_h(half_carry);
                self.set_c(carry);
                8
            }

            0xA8 => {
                // XOR B
                self.a ^= self.b;
                self.set_flags(self.a == 0, false, false, false);
                4
            }

            0xA9 => {
                // XOR C
                self.a ^= self.c;
                self.set_flags(self.a == 0, false, false, false);
                4
            }

            0xAA => {
                // XOR D
                self.a ^= self.d;
                self.set_flags(self.a == 0, false, false, false);
                4
            }

            0xAB => {
                // XOR E
                self.a ^= self.e;
                self.set_flags(self.a == 0, false, false, false);
                4
            }

            0xAC => {
                // XOR H
                self.a ^= self.h;
                self.set_flags(self.a == 0, false, false, false);
                4
            }

            0xAD => {
                // XOR L
                self.a ^= self.l;
                self.set_flags(self.a == 0, false, false, false);
                4
            }

            0x48 => {
                // LD C, B
                self.c = self.b;
                4
            }

            0x49 => {
                // LD C, C = NOOP
                4
            }

            0x4A => {
                // LD C, D
                self.c = self.d;
                4
            }

            0x4B => {
                // LD C, E
                self.c = self.e;
                4
            }

            0x4C => {
                // LD C, H
                self.c = self.h;
                4
            }

            0x4D => {
                // LD C, L
                self.c = self.l;
                4
            }

            0x37 => {
                // SCF (set carry) z unaffected
                self.set_n(false);
                self.set_h(false);
                self.set_c(true);
                4
            }

            0x3F => {
                let old_c = self.get_c();
                self.set_flags(self.get_z(), false, false, !old_c);
                self.f &= 0xF0;
                4
            }

            0x17 => {
                // RLA
                let new_c = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | if self.get_c() { 1 } else { 0 };
                self.set_flags(false, false, false, new_c);
                4
            }

            0x0F => {
                // RRCA
                let new_c = (self.a & 0x01) != 0;
                self.a = self.a.rotate_right(1);
                self.set_flags(false, false, false, new_c);
                4
            }

            0x08 => {
                // LD (nn), SP
                let low = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let high = mmu.read_byte(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                let addr = (high << 8) | low;
                mmu.write_byte(addr, self.sp as u8);
                mmu.write_byte(addr.wrapping_add(1), (self.sp >> 8) as u8);
                20
            }

            0xF9 => {
                self.sp = self.get_hl();
                8
            }

            0x9E => {
                // SBC A, (HL)
                let n = mmu.read_byte(self.get_hl());
                let c = if self.get_c() { 1u8 } else { 0 };
                let half_carry = (self.a & 0x0F) < (n & 0x0F) + c;
                let sum = n as u16 + c as u16;
                let carry = (self.a as u16) < sum;
                self.a = self.a.wrapping_sub(n).wrapping_sub(c);
                self.set_flags(self.a == 0, true, half_carry, carry);
                8
            }

            0xA6 => {
                // AND (HL)
                self.a &= mmu.read_byte(self.get_hl());
                self.set_flags(self.a == 0, false, true, false);
                8
            }

            0xE8 => {
                // ADD SP, r8
                let imm = mmu.read_byte(self.pc) as i8 as i16;
                self.pc = self.pc.wrapping_add(1);
                let sp = self.sp;
                let e = imm as u16; // wrapping two's complement
                let half_carry = ((sp & 0x0F) + (e & 0x0F)) > 0x0F;
                let carry = ((sp & 0xFF) + (e & 0xFF)) > 0xFF;
                self.sp = sp.wrapping_add(e);
                self.set_flags(false, false, half_carry, carry);
                16
            }

            0xF8 => {
                // LD HL, SP+r8
                let imm = mmu.read_byte(self.pc) as i8 as i16;
                self.pc = self.pc.wrapping_add(1);
                let sp = self.sp;
                let e = imm as u16;
                let half_carry = ((sp & 0x0F) + (e & 0x0F)) > 0x0F;
                let carry = ((sp & 0xFF) + (e & 0xFF)) > 0xFF;
                self.set_hl(sp.wrapping_add(e));
                self.set_flags(false, false, half_carry, carry);
                12
            }

            0xF2 => {
                // LD A, (C)
                self.a = mmu.read_byte(0xFF00 | self.c as u16);
                8
            }

            0x10 => {
                // STOP
                self.pc = self.pc.wrapping_add(1);
                4
            }

            // END OF OPCODES
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

    // Secondary instruction table for bit-manipulation (0xCB-prefix)
    fn step_cb(&mut self, opcode: u8, mmu: &mut Mmu) -> u32 {
        fn rlc(val: u8) -> (u8, bool) {
            let c = val & 0x80 != 0;
            (val.rotate_left(1), c)
        }
        fn rrc(val: u8) -> (u8, bool) {
            let c = val & 0x01 != 0;
            (val.rotate_right(1), c)
        }
        fn rl(val: u8, old_c: bool) -> (u8, bool) {
            let c = val & 0x80 != 0;
            ((val << 1) | u8::from(old_c), c)
        }
        fn rr(val: u8, old_c: bool) -> (u8, bool) {
            let c = val & 0x01 != 0;
            ((val >> 1) | if old_c { 0x80 } else { 0 }, c)
        }
        fn sla(val: u8) -> (u8, bool) {
            (val << 1, val & 0x80 != 0)
        }
        fn sra(val: u8) -> (u8, bool) {
            ((val >> 1) | (val & 0x80), val & 0x01 != 0)
        }
        fn srl(val: u8) -> (u8, bool) {
            (val >> 1, val & 0x01 != 0)
        }
        fn swap(val: u8) -> u8 {
            val.rotate_left(4)
        }

        match opcode {
            0x00..=0x3F => {
                let old_c = self.get_c();
                let (result, carry, cycles) = match opcode {
                    0x00 => {
                        let (v, c) = rlc(self.b);
                        self.b = v;
                        (v, c, 8)
                    }
                    0x01 => {
                        let (v, c) = rlc(self.c);
                        self.c = v;
                        (v, c, 8)
                    }
                    0x02 => {
                        let (v, c) = rlc(self.d);
                        self.d = v;
                        (v, c, 8)
                    }
                    0x03 => {
                        let (v, c) = rlc(self.e);
                        self.e = v;
                        (v, c, 8)
                    }
                    0x04 => {
                        let (v, c) = rlc(self.h);
                        self.h = v;
                        (v, c, 8)
                    }
                    0x05 => {
                        let (v, c) = rlc(self.l);
                        self.l = v;
                        (v, c, 8)
                    }
                    0x06 => {
                        let a = self.get_hl();
                        let (v, c) = rlc(mmu.read_byte(a));
                        mmu.write_byte(a, v);
                        (v, c, 16)
                    }
                    0x07 => {
                        let (v, c) = rlc(self.a);
                        self.a = v;
                        (v, c, 8)
                    }

                    0x08 => {
                        let (v, c) = rrc(self.b);
                        self.b = v;
                        (v, c, 8)
                    }
                    0x09 => {
                        let (v, c) = rrc(self.c);
                        self.c = v;
                        (v, c, 8)
                    }
                    0x0A => {
                        let (v, c) = rrc(self.d);
                        self.d = v;
                        (v, c, 8)
                    }
                    0x0B => {
                        let (v, c) = rrc(self.e);
                        self.e = v;
                        (v, c, 8)
                    }
                    0x0C => {
                        let (v, c) = rrc(self.h);
                        self.h = v;
                        (v, c, 8)
                    }
                    0x0D => {
                        let (v, c) = rrc(self.l);
                        self.l = v;
                        (v, c, 8)
                    }
                    0x0E => {
                        let a = self.get_hl();
                        let (v, c) = rrc(mmu.read_byte(a));
                        mmu.write_byte(a, v);
                        (v, c, 16)
                    }
                    0x0F => {
                        let (v, c) = rrc(self.a);
                        self.a = v;
                        (v, c, 8)
                    }

                    0x10 => {
                        let (v, c) = rl(self.b, old_c);
                        self.b = v;
                        (v, c, 8)
                    }
                    0x11 => {
                        let (v, c) = rl(self.c, old_c);
                        self.c = v;
                        (v, c, 8)
                    }
                    0x12 => {
                        let (v, c) = rl(self.d, old_c);
                        self.d = v;
                        (v, c, 8)
                    }
                    0x13 => {
                        let (v, c) = rl(self.e, old_c);
                        self.e = v;
                        (v, c, 8)
                    }
                    0x14 => {
                        let (v, c) = rl(self.h, old_c);
                        self.h = v;
                        (v, c, 8)
                    }
                    0x15 => {
                        let (v, c) = rl(self.l, old_c);
                        self.l = v;
                        (v, c, 8)
                    }
                    0x16 => {
                        let a = self.get_hl();
                        let (v, c) = rl(mmu.read_byte(a), old_c);
                        mmu.write_byte(a, v);
                        (v, c, 16)
                    }
                    0x17 => {
                        let (v, c) = rl(self.a, old_c);
                        self.a = v;
                        (v, c, 8)
                    }

                    0x18 => {
                        let (v, c) = rr(self.b, old_c);
                        self.b = v;
                        (v, c, 8)
                    }
                    0x19 => {
                        let (v, c) = rr(self.c, old_c);
                        self.c = v;
                        (v, c, 8)
                    }
                    0x1A => {
                        let (v, c) = rr(self.d, old_c);
                        self.d = v;
                        (v, c, 8)
                    }
                    0x1B => {
                        let (v, c) = rr(self.e, old_c);
                        self.e = v;
                        (v, c, 8)
                    }
                    0x1C => {
                        let (v, c) = rr(self.h, old_c);
                        self.h = v;
                        (v, c, 8)
                    }
                    0x1D => {
                        let (v, c) = rr(self.l, old_c);
                        self.l = v;
                        (v, c, 8)
                    }
                    0x1E => {
                        let a = self.get_hl();
                        let (v, c) = rr(mmu.read_byte(a), old_c);
                        mmu.write_byte(a, v);
                        (v, c, 16)
                    }
                    0x1F => {
                        let (v, c) = rr(self.a, old_c);
                        self.a = v;
                        (v, c, 8)
                    }

                    0x20 => {
                        let (v, c) = sla(self.b);
                        self.b = v;
                        (v, c, 8)
                    }
                    0x21 => {
                        let (v, c) = sla(self.c);
                        self.c = v;
                        (v, c, 8)
                    }
                    0x22 => {
                        let (v, c) = sla(self.d);
                        self.d = v;
                        (v, c, 8)
                    }
                    0x23 => {
                        let (v, c) = sla(self.e);
                        self.e = v;
                        (v, c, 8)
                    }
                    0x24 => {
                        let (v, c) = sla(self.h);
                        self.h = v;
                        (v, c, 8)
                    }
                    0x25 => {
                        let (v, c) = sla(self.l);
                        self.l = v;
                        (v, c, 8)
                    }
                    0x26 => {
                        let a = self.get_hl();
                        let (v, c) = sla(mmu.read_byte(a));
                        mmu.write_byte(a, v);
                        (v, c, 16)
                    }
                    0x27 => {
                        let (v, c) = sla(self.a);
                        self.a = v;
                        (v, c, 8)
                    }

                    0x28 => {
                        let (v, c) = sra(self.b);
                        self.b = v;
                        (v, c, 8)
                    }
                    0x29 => {
                        let (v, c) = sra(self.c);
                        self.c = v;
                        (v, c, 8)
                    }
                    0x2A => {
                        let (v, c) = sra(self.d);
                        self.d = v;
                        (v, c, 8)
                    }
                    0x2B => {
                        let (v, c) = sra(self.e);
                        self.e = v;
                        (v, c, 8)
                    }
                    0x2C => {
                        let (v, c) = sra(self.h);
                        self.h = v;
                        (v, c, 8)
                    }
                    0x2D => {
                        let (v, c) = sra(self.l);
                        self.l = v;
                        (v, c, 8)
                    }
                    0x2E => {
                        let a = self.get_hl();
                        let (v, c) = sra(mmu.read_byte(a));
                        mmu.write_byte(a, v);
                        (v, c, 16)
                    }
                    0x2F => {
                        let (v, c) = sra(self.a);
                        self.a = v;
                        (v, c, 8)
                    }

                    0x30 => {
                        let v = swap(self.b);
                        self.b = v;
                        (v, false, 8)
                    }
                    0x31 => {
                        let v = swap(self.c);
                        self.c = v;
                        (v, false, 8)
                    }
                    0x32 => {
                        let v = swap(self.d);
                        self.d = v;
                        (v, false, 8)
                    }
                    0x33 => {
                        let v = swap(self.e);
                        self.e = v;
                        (v, false, 8)
                    }
                    0x34 => {
                        let v = swap(self.h);
                        self.h = v;
                        (v, false, 8)
                    }
                    0x35 => {
                        let v = swap(self.l);
                        self.l = v;
                        (v, false, 8)
                    }
                    0x36 => {
                        let a = self.get_hl();
                        let v = swap(mmu.read_byte(a));
                        mmu.write_byte(a, v);
                        (v, false, 16)
                    }
                    0x37 => {
                        let v = swap(self.a);
                        self.a = v;
                        (v, false, 8)
                    }

                    0x38 => {
                        let (v, c) = srl(self.b);
                        self.b = v;
                        (v, c, 8)
                    }
                    0x39 => {
                        let (v, c) = srl(self.c);
                        self.c = v;
                        (v, c, 8)
                    }
                    0x3A => {
                        let (v, c) = srl(self.d);
                        self.d = v;
                        (v, c, 8)
                    }
                    0x3B => {
                        let (v, c) = srl(self.e);
                        self.e = v;
                        (v, c, 8)
                    }
                    0x3C => {
                        let (v, c) = srl(self.h);
                        self.h = v;
                        (v, c, 8)
                    }
                    0x3D => {
                        let (v, c) = srl(self.l);
                        self.l = v;
                        (v, c, 8)
                    }
                    0x3E => {
                        let a = self.get_hl();
                        let (v, c) = srl(mmu.read_byte(a));
                        mmu.write_byte(a, v);
                        (v, c, 16)
                    }
                    0x3F => {
                        let (v, c) = srl(self.a);
                        self.a = v;
                        (v, c, 8)
                    }
                    _ => unreachable!(),
                };
                self.set_flags(result == 0, false, false, carry);
                cycles
            }

            0x40..=0x7F => {
                let bit = 1u8 << ((opcode >> 3) & 7);
                let val = match opcode & 7 {
                    0 => self.b,
                    1 => self.c,
                    2 => self.d,
                    3 => self.e,
                    4 => self.h,
                    5 => self.l,
                    6 => mmu.read_byte(self.get_hl()),
                    7 => self.a,
                    _ => unreachable!(),
                };
                self.set_z((val & bit) == 0);
                self.set_n(false);
                self.set_h(true);
                if opcode & 7 == 6 { 12 } else { 8 }
            }

            0x80..=0xBF => {
                let mask = !(1u8 << ((opcode >> 3) & 7));
                match opcode & 7 {
                    0 => self.b &= mask,
                    1 => self.c &= mask,
                    2 => self.d &= mask,
                    3 => self.e &= mask,
                    4 => self.h &= mask,
                    5 => self.l &= mask,
                    6 => {
                        let a = self.get_hl();
                        mmu.write_byte(a, mmu.read_byte(a) & mask);
                    }
                    7 => self.a &= mask,
                    _ => unreachable!(),
                }
                if opcode & 7 == 6 { 16 } else { 8 }
            }

            0xC0..=0xFF => {
                let bit = 1u8 << ((opcode >> 3) & 7);
                match opcode & 7 {
                    0 => self.b |= bit,
                    1 => self.c |= bit,
                    2 => self.d |= bit,
                    3 => self.e |= bit,
                    4 => self.h |= bit,
                    5 => self.l |= bit,
                    6 => {
                        let a = self.get_hl();
                        mmu.write_byte(a, mmu.read_byte(a) | bit);
                    }
                    7 => self.a |= bit,
                    _ => unreachable!(),
                }
                if opcode & 7 == 6 { 16 } else { 8 }
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
