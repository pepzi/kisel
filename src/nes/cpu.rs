pub const FLAG_C: u8 = 0x01;
pub const FLAG_Z: u8 = 0x02;
pub const FLAG_I: u8 = 0x04;
pub const FLAG_D: u8 = 0x08;
pub const FLAG_B: u8 = 0x10;
pub const FLAG_U: u8 = 0x20;
pub const FLAG_V: u8 = 0x40;
pub const FLAG_N: u8 = 0x80;

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
}

pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
    pub sp: u8,
    pub pc: u16,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            p: FLAG_U | FLAG_I,
            sp: 0xFD,
            pc: 0,
        }
    }

    pub fn reset(&mut self, bus: &mut impl Bus) {
        let lo = bus.read(0xFFFC);
        let hi = bus.read(0xFFFD);
        self.pc = u16::from_le_bytes([lo, hi]);
        self.sp = 0xFD;
        self.p |= FLAG_I | FLAG_U;
    }

    /// nestest automation: ignore reset vector.
    pub fn reset_at(&mut self, pc: u16) {
        self.pc = pc;
        self.sp = 0xFD;
        self.p = FLAG_U | FLAG_I;
        self.a = 0;
        self.x = 0;
        self.y = 0;
    }

    fn fetch(&mut self, bus: &mut impl Bus) -> u8 {
        let b = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        b
    }

    fn fetch_u16(&mut self, bus: &mut impl Bus) -> u16 {
        let lo = self.fetch(bus);
        let hi = self.fetch(bus);
        u16::from_le_bytes([lo, hi])
    }

    fn set_zn(&mut self, v: u8) {
        self.p &= !(FLAG_Z | FLAG_N);
        if v == 0 {
            self.p |= FLAG_Z;
        }
        if v & 0x80 != 0 {
            self.p |= FLAG_N;
        }
    }

    fn zp_y(&mut self, bus: &mut impl Bus) -> u16 {
        let base = self.fetch(bus);
        base.wrapping_add(self.y) as u16
    }

    fn abs_y(&mut self, bus: &mut impl Bus) -> (u16, bool) {
        let base = self.fetch_u16(bus);
        let addr = base.wrapping_add(self.y as u16);
        let crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, crossed)
    }

    fn zp_x(&mut self, bus: &mut impl Bus) -> u16 {
        let base = self.fetch(bus);
        base.wrapping_add(self.x) as u16
    }

    fn abs_x(&mut self, bus: &mut impl Bus) -> (u16, bool) {
        let base = self.fetch_u16(bus);
        let addr = base.wrapping_add(self.x as u16);
        let crossed = (base & 0xFF00) != (addr & 0xFF00);
        (addr, crossed)
    }

    fn branch(&mut self, bus: &mut impl Bus, taken: bool) -> u32 {
        let off = self.fetch(bus) as i8 as u16;
        if !taken {
            return 2;
        }
        let old = self.pc;
        self.pc = self.pc.wrapping_add(off);
        if (old & 0xFF00) != (self.pc & 0xFF00) {
            4
        } else {
            3
        }
    }

    fn compare(&mut self, reg: u8, value: u8) {
        let result = reg.wrapping_sub(value);
        self.p &= !(FLAG_C | FLAG_Z | FLAG_N);
        if reg >= value {
            self.p |= FLAG_C;
        }
        if result == 0 {
            self.p |= FLAG_Z;
        }
        if result & 0x80 != 0 {
            self.p |= FLAG_N;
        }
    }

    fn ind_x(&mut self, bus: &mut impl Bus) -> u16 {
        let base = self.fetch(bus).wrapping_add(self.x);
        let lo = bus.read(base as u16);
        let hi = bus.read(base.wrapping_add(1) as u16);
        u16::from_le_bytes([lo, hi])
    }

    fn ind_y(&mut self, bus: &mut impl Bus) -> (u16, bool) {
        let zp = self.fetch(bus);
        let lo = bus.read(zp as u16);
        let hi = bus.read(zp.wrapping_add(1) as u16);
        let ptr = u16::from_le_bytes([lo, hi]);
        let addr = ptr.wrapping_add(self.y as u16);
        let crossed = (ptr & 0xFF00) != (addr & 0xFF00);
        (addr, crossed)
    }

    fn push(&mut self, bus: &mut impl Bus, value: u8) {
        bus.write(0x0100 | self.sp as u16, value);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pop(&mut self, bus: &mut impl Bus) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        bus.read(0x0100 | self.sp as u16)
    }

    /// One instruction. 0 = unimplemented (stop the emulator).
    pub fn step(&mut self, bus: &mut impl Bus) -> u32 {
        let op = self.fetch(bus);
        match op {
            0x10 => self.branch(bus, self.p & FLAG_N == 0), // BPL
            0x30 => self.branch(bus, self.p & FLAG_N != 0), // BMI
            0x50 => self.branch(bus, self.p & FLAG_V == 0), // BVC
            0x70 => self.branch(bus, self.p & FLAG_V != 0), // BVS
            0x90 => self.branch(bus, self.p & FLAG_C == 0), // BCC
            0xB0 => self.branch(bus, self.p & FLAG_C != 0), // BCS
            0xD0 => self.branch(bus, self.p & FLAG_Z == 0), // BNE
            0xF0 => self.branch(bus, self.p & FLAG_Z != 0), // BEQ

            0x20 => {
                // JSR abs
                let dest = self.fetch_u16(bus);
                let ret = self.pc.wrapping_sub(1); // last byte of JSR, not next opcode
                self.push(bus, (ret >> 8) as u8);
                self.push(bus, ret as u8);
                self.pc = dest;
                6
            }

            0x60 => {
                // RTS
                let lo = self.pop(bus);
                let hi = self.pop(bus);
                self.pc = u16::from_le_bytes([lo, hi]).wrapping_add(1);
                6
            }

            // CMP
            0xC9 => {
                let v = self.fetch(bus);
                self.compare(self.a, v);
                2
            }
            0xC5 => {
                let a = self.fetch(bus) as u16;
                let v = bus.read(a);
                self.compare(self.a, v);
                3
            }
            0xD5 => {
                let a = self.zp_x(bus);
                let v = bus.read(a);
                self.compare(self.a, v);
                4
            }
            0xCD => {
                let a = self.fetch_u16(bus);
                let v = bus.read(a);
                self.compare(self.a, v);
                4
            }
            0xDD => {
                let (a, c) = self.abs_x(bus);
                let v = bus.read(a);
                self.compare(self.a, v);
                if c { 5 } else { 4 }
            }
            0xD9 => {
                let (a, c) = self.abs_y(bus);
                let v = bus.read(a);
                self.compare(self.a, v);
                if c { 5 } else { 4 }
            }
            0xC1 => {
                let a = self.ind_x(bus);
                let v = bus.read(a);
                self.compare(self.a, v);
                6
            }
            0xD1 => {
                let (a, c) = self.ind_y(bus);
                let v = bus.read(a);
                self.compare(self.a, v);
                if c { 6 } else { 5 }
            }

            // CPX
            0xE0 => {
                let v = self.fetch(bus);
                self.compare(self.x, v);
                2
            }
            0xE4 => {
                let a = self.fetch(bus) as u16;
                let v = bus.read(a);
                self.compare(self.x, v);
                3
            }
            0xEC => {
                let a = self.fetch_u16(bus);
                let v = bus.read(a);
                self.compare(self.x, v);
                4
            }

            // CPY
            0xC0 => {
                let v = self.fetch(bus);
                self.compare(self.y, v);
                2
            }
            0xC4 => {
                let a = self.fetch(bus) as u16;
                let v = bus.read(a);
                self.compare(self.y, v);
                3
            }
            0xCC => {
                let a = self.fetch_u16(bus);
                let v = bus.read(a);
                self.compare(self.y, v);
                4
            }

            0x78 => {
                // SEI
                self.p |= FLAG_I;
                2
            }
            0xD8 => {
                // CLD
                self.p &= !FLAG_D;
                2
            }

            0xA0 => {
                // LDY #imm
                self.y = self.fetch(bus);
                self.set_zn(self.y);
                2
            }
            0xA4 => {
                // LDY zp
                let addr = self.fetch(bus) as u16;
                self.y = bus.read(addr);
                self.set_zn(self.y);
                3
            }
            0xB4 => {
                // LDY zp,X
                let addr = self.zp_x(bus);
                self.y = bus.read(addr);
                self.set_zn(self.y);
                4
            }
            0xAC => {
                // LDY abs
                let addr = self.fetch_u16(bus);
                self.y = bus.read(addr);
                self.set_zn(self.y);
                4
            }
            0xBC => {
                // LDX abs, X
                let (addr, crossed) = self.abs_x(bus);
                self.y = bus.read(addr);
                self.set_zn(self.y);
                if crossed { 5 } else { 4 }
            }
            // -----------
            0xA2 => {
                // LDX #imm
                self.x = self.fetch(bus);
                self.set_zn(self.x);
                2
            }
            0xA6 => {
                // LDX zp
                let addr = self.fetch(bus) as u16;
                self.x = bus.read(addr);
                self.set_zn(self.x);
                3
            }
            0xB6 => {
                // LDX zp,Y
                let addr = self.zp_y(bus);
                self.x = bus.read(addr);
                self.set_zn(self.x);
                4
            }
            0xAE => {
                // LDX abs
                let addr = self.fetch_u16(bus);
                self.x = bus.read(addr);
                self.set_zn(self.x);
                4
            }
            0xBE => {
                // LDX abs, Y
                let (addr, crossed) = self.abs_y(bus);
                self.x = bus.read(addr);
                self.set_zn(self.x);
                if crossed { 5 } else { 4 }
            }

            0xA9 => {
                self.a = self.fetch(bus);
                self.set_zn(self.a);
                2
            } // LDA #
            0xA5 => {
                let a = self.fetch(bus) as u16;
                self.a = bus.read(a);
                self.set_zn(self.a);
                3
            } // LDA zp
            0xB5 => {
                let a = self.zp_x(bus);
                self.a = bus.read(a);
                self.set_zn(self.a);
                4
            } // LDA zp,X
            0xAD => {
                let a = self.fetch_u16(bus);
                self.a = bus.read(a);
                self.set_zn(self.a);
                4
            } // LDA abs
            0xBD => {
                let (a, c) = self.abs_x(bus);
                self.a = bus.read(a);
                self.set_zn(self.a);
                if c { 5 } else { 4 }
            } // LDA abs,X
            0xB9 => {
                let (a, c) = self.abs_y(bus);
                self.a = bus.read(a);
                self.set_zn(self.a);
                if c { 5 } else { 4 }
            } // LDA abs,Y

            0xAA => {
                // TAX
                self.x = self.a;
                self.set_zn(self.x);
                2
            }

            0x8A => {
                // TXA
                self.a = self.x;
                self.set_zn(self.a);
                2
            }

            0xA8 => {
                // TAY
                self.y = self.a;
                self.set_zn(self.y);
                2
            }

            0x98 => {
                // TYA
                self.a = self.y;
                self.set_zn(self.a);
                2
            }

            0xCA => {
                // DEX
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x);
                2
            }

            0x88 => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y);
                2
            } // DEY
            0xE8 => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x);
                2
            } // INX
            0xC8 => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y);
                2
            } // INY

            0x9A => {
                // TXS
                self.sp = self.x;
                self.set_zn(self.sp);
                2
            }
            0x8D => {
                // STA abs
                let addr = self.fetch_u16(bus);
                bus.write(addr, self.a);
                4
            }
            0x4C => {
                // JMP abs
                self.pc = self.fetch_u16(bus);
                3
            }
            0xEA => {
                // NOP
                2
            }
            _ => {
                println!(
                    "[NES KRASCH] opcode 0x{op:02X} at PC 0x{:04X}",
                    self.pc.wrapping_sub(1)
                );
                self.debug_dump();
                0
            }
        }
    }

    pub fn debug_dump(&self) {
        println!(
            "A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X} PC={:04X}",
            self.a, self.x, self.y, self.p, self.sp, self.pc
        );
    }
}
