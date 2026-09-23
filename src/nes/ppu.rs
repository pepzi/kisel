use crate::nes::cart::Cart;

/// Approximate 2C02 palette as 0xAARRGGBB (minifb).
const NES_PALETTE: [u32; 64] = [
    0xFF545454, 0xFF001E74, 0xFF081090, 0xFF300088, 0xFF440064, 0xFF5C0030, 0xFF540400, 0xFF3C1800,
    0xFF202A00, 0xFF083A00, 0xFF004000, 0xFF003C00, 0xFF00323C, 0xFF000000, 0xFF000000, 0xFF000000,
    0xFF989698, 0xFF084CC4, 0xFF3032EC, 0xFF5C1EE4, 0xFF8814B0, 0xFFA01464, 0xFF982220, 0xFF783C00,
    0xFF545A00, 0xFF287200, 0xFF087C00, 0xFF007628, 0xFF006678, 0xFF000000, 0xFF000000, 0xFF000000,
    0xFFECEEEC, 0xFF4C9AEC, 0xFF787CEC, 0xFFB062EC, 0xFFE454EC, 0xFFEC58B4, 0xFFEC6A64, 0xFFD48820,
    0xFFA0AA00, 0xFF74C400, 0xFF4CD020, 0xFF38CC6C, 0xFF38B4CC, 0xFF3C3C3C, 0xFF000000, 0xFF000000,
    0xFFECEEEC, 0xFFA8CCEC, 0xFFBCBCEC, 0xFFD4B2EC, 0xFFECAEEC, 0xFFECAED4, 0xFFECB4B0, 0xFFE4C490,
    0xFFCCD278, 0xFFB4DE78, 0xFFA8E290, 0xFF98E2B4, 0xFFA0D6E0, 0xFFA0A2A0, 0xFF000000, 0xFF000000,
];

#[derive(Clone)]
pub struct Ppu {
    pub ctrl: u8,
    pub mask: u8,
    pub status: u8,
    pub vram: [u8; 0x800],
    pub palette: [u8; 0x20],
    pub oam: [u8; 256],
    pub oam_addr: u8,
    pub t: u16,
    pub v: u16,
    pub x: u8,
    pub w: bool,
    pub ly: u16,
    read_buffer: u8,
    bg_cid: [u8; 256],
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            ctrl: 0,
            mask: 0,
            status: 0,
            vram: [0; 0x800],
            palette: [0; 0x20],
            oam: [0xFF; 256],
            oam_addr: 0,
            t: 0,
            v: 0,
            x: 0,
            w: false,
            ly: 0,
            read_buffer: 0,
            bg_cid: [0; 256],
        }
    }

    pub fn read_reg(&mut self, reg: u16, cart: &Cart) -> u8 {
        match reg & 7 {
            2 => {
                let v = self.status;
                self.status &= !0x80;
                self.w = false;
                v
            }
            4 => self.oam[self.oam_addr as usize],
            7 => {
                let addr = self.v;
                self.v = self
                    .v
                    .wrapping_add(if self.ctrl & 0x04 != 0 { 32 } else { 1 })
                    & 0x7FFF;
                let raw = self.mem_read(addr, cart);
                if addr & 0x3FFF >= 0x3F00 {
                    self.read_buffer = self.mem_read(addr.wrapping_sub(0x1000), cart);
                    raw
                } else {
                    let old = self.read_buffer;
                    self.read_buffer = raw;
                    old
                }
            }
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, reg: u16, value: u8, cart: &mut Cart) {
        match reg & 7 {
            0 => {
                self.ctrl = value;
                self.t = (self.t & !0x0C00) | ((value as u16 & 0x03) << 10);
            }
            1 => self.mask = value,
            3 => self.oam_addr = value,
            4 => {
                self.oam[self.oam_addr as usize] = value;
                self.oam_addr = self.oam_addr.wrapping_add(1);
            }
            5 => {
                if !self.w {
                    self.x = value & 0x07;
                    self.t = (self.t & !0x001F) | ((value as u16) >> 3);
                } else {
                    self.t = (self.t & !0x73E0)
                        | ((value as u16 & 0x07) << 12)
                        | ((value as u16 & 0xF8) << 2);
                }
                self.w = !self.w;
            }
            6 => {
                if !self.w {
                    self.t = (self.t & 0x00FF) | ((value as u16 & 0x3F) << 8);
                } else {
                    self.t = (self.t & 0xFF00) | value as u16;
                    self.v = self.t;
                }
                self.w = !self.w;
            }
            7 => {
                self.mem_write(self.v, value, cart);
                self.v = self
                    .v
                    .wrapping_add(if self.ctrl & 0x04 != 0 { 32 } else { 1 })
                    & 0x7FFF;
            }
            _ => {}
        }
    }

    pub fn write_oam(&mut self, i: u8, value: u8) {
        self.oam[i as usize] = value;
    }

    pub fn enter_vblank(&mut self) {
        self.status |= 0x80;
    }

    pub fn nmi_enabled(&self) -> bool {
        self.ctrl & 0x80 != 0
    }

    fn rendering(&self) -> bool {
        self.mask & 0x18 != 0
    }

    pub fn pre_render(&mut self) {
        self.status &= !0xC0;
        if self.rendering() {
            self.v = self.t;
        }
    }

    pub fn begin_visible_line(&mut self, cart: &Cart) {
        if !self.rendering() {
            return;
        }
        self.v = copy_x(self.v, self.t);
        self.eval_sprite0(cart);
    }

    pub fn end_visible_line(&mut self) {
        if self.rendering() {
            self.v = inc_y(self.v);
        }
    }

    fn eval_sprite0(&mut self, cart: &Cart) {
        if self.status & 0x40 != 0 || self.mask & 0x18 != 0x18 {
            return;
        }
        let sy = self.oam[0] as i32 + 1;
        let tall = self.ctrl & 0x20 != 0;
        let h = if tall { 16 } else { 8 };
        let ly = self.ly as i32;
        if ly < sy || ly >= sy + h {
            return;
        }
        let sx = self.oam[3] as i32;
        let clip = self.mask & 0x06 != 0x06;
        for col in 0..8i32 {
            let px = sx + col;
            if !(0..255).contains(&px) {
                continue;
            }
            if clip && px < 8 {
                continue;
            }
            if self.sprite0_opaque(cart, col, ly - sy) && self.bg_opaque(cart, px as u16) {
                self.status |= 0x40;
                return;
            }
        }
    }

    fn sprite0_opaque(&self, cart: &Cart, col: i32, row: i32) -> bool {
        let tile = self.oam[1];
        let attr = self.oam[2];
        let tall = self.ctrl & 0x20 != 0;
        let h = if tall { 16 } else { 8 };
        let ry = if attr & 0x80 != 0 { h - 1 - row } else { row };
        let bit = if attr & 0x40 != 0 { col } else { 7 - col };
        let (tid, base) = if tall {
            let t = tile & 0xFE;
            let extra = u8::from(ry >= 8);
            ((t + extra) as u16, if tile & 1 != 0 { 0x1000 } else { 0 })
        } else {
            let pat8 = if self.ctrl & 0x08 != 0 { 0x1000u16 } else { 0 };
            (tile as u16, pat8)
        };
        let fy = (ry as u16) % 8;
        let p0 = self.mem_read(base + tid * 16 + fy, cart);
        let p1 = self.mem_read(base + tid * 16 + fy + 8, cart);
        let cid = (((p1 >> bit) & 1) << 1) | ((p0 >> bit) & 1);
        cid != 0
    }

    fn bg_opaque(&self, cart: &Cart, x: u16) -> bool {
        let mut v = self.v;
        let pixel = self.x as u16 + x;
        for _ in 0..pixel / 8 {
            v = inc_x(v);
        }
        let col = pixel % 8;
        let fine_y = (v >> 12) & 7;
        let pat = if self.ctrl & 0x10 != 0 { 0x1000u16 } else { 0 };
        let tile = self.mem_read(0x2000 | (v & 0x0FFF), cart) as u16;
        let p0 = self.mem_read(pat + tile * 16 + fine_y, cart);
        let p1 = self.mem_read(pat + tile * 16 + fine_y + 8, cart);
        let bit = 7 - col;
        let cid = (((p1 >> bit) & 1) << 1) | ((p0 >> bit) & 1);
        cid != 0
    }

    pub fn draw_scanline(&mut self, cart: &Cart, buffer: &mut [u32]) {
        let ly = self.ly;
        if ly >= 240 {
            return;
        }
        let row = ly as usize * 256;
        buffer[row..row + 256].fill(self.rgb(0));
        self.bg_cid = [0; 256];
        if self.mask & 0x08 == 0 {
            return;
        }
        let pat = if self.ctrl & 0x10 != 0 { 0x1000u16 } else { 0 };
        let mut v = self.v;
        let fine_x = self.x as u16;
        let fine_y = (v >> 12) & 7;
        let mut out = 0u16;

        for tile_n in 0..33u16 {
            let addr = 0x2000 | (v & 0x0FFF);
            let tile = self.mem_read(addr, cart) as u16;
            let attr_addr = 0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07);
            let attr = self.mem_read(attr_addr, cart);
            let shift = ((v >> 4) & 4) | (v & 2);
            let pal = (attr >> shift) & 3;
            let p0 = self.mem_read(pat + tile * 16 + fine_y, cart);
            let p1 = self.mem_read(pat + tile * 16 + fine_y + 8, cart);
            for col in 0..8u16 {
                if tile_n == 0 && col < fine_x {
                    continue;
                }
                if out >= 256 {
                    break;
                }
                let bit = 7 - col;
                let cid = (((p1 >> bit) & 1) << 1) | ((p0 >> bit) & 1);
                if self.mask & 0x02 != 0 || out >= 8 {
                    let idx = if cid == 0 { 0 } else { (pal << 2) | cid };
                    buffer[row + out as usize] = self.rgb(idx);
                    self.bg_cid[out as usize] = cid;
                }
                out += 1;
            }
            v = inc_x(v);
            if out >= 256 {
                break;
            }
        }
    }

    pub fn draw_sprites(&self, cart: &Cart, buffer: &mut [u32]) {
        if self.mask & 0x10 == 0 {
            return;
        }
        let ly = self.ly as i32;
        if !(0..240).contains(&ly) {
            return;
        }
        let tall = self.ctrl & 0x20 != 0;
        let pat8 = if self.ctrl & 0x08 != 0 { 0x1000u16 } else { 0 };
        let clip = self.mask & 0x04 == 0;

        for i in (0..64).rev() {
            let o = i * 4;
            let sy = self.oam[o] as i32 + 1;
            let tile = self.oam[o + 1];
            let attr = self.oam[o + 2];
            let pal = 4 + (attr & 0x03);
            let sx = self.oam[o + 3] as i32;
            let h = if tall { 16 } else { 8 };
            let row = ly - sy;
            if row < 0 || row >= h {
                continue;
            }
            let behind = attr & 0x20 != 0;
            let ry = if attr & 0x80 != 0 { h - 1 - row } else { row };
            let (tid, base) = if tall {
                let t = tile & 0xFE;
                let extra = u8::from(ry >= 8);
                ((t + extra) as u16, if tile & 1 != 0 { 0x1000 } else { 0 })
            } else {
                (tile as u16, pat8)
            };
            let p0 = self.mem_read(base + tid * 16 + (ry as u16 % 8), cart);
            let p1 = self.mem_read(base + tid * 16 + (ry as u16 % 8) + 8, cart);
            for col in 0..8i32 {
                let bit = if attr & 0x40 != 0 { col } else { 7 - col };
                let cid = (((p1 >> bit) & 1) << 1) | ((p0 >> bit) & 1);
                if cid == 0 {
                    continue;
                }
                let px = sx + col;
                if !(0..256).contains(&px) || (clip && px < 8) {
                    continue;
                }
                if behind && self.bg_cid[px as usize] != 0 {
                    continue;
                }
                buffer[ly as usize * 256 + px as usize] = self.rgb((pal << 2) | cid);
            }
        }
    }

    fn map(&self, mut addr: u16, cart: &Cart) -> (bool, usize) {
        addr &= 0x3FFF;
        if addr >= 0x3F00 {
            let mut p = (addr as usize - 0x3F00) & 0x1F;
            if p & 0x13 == 0x10 {
                p &= !0x10;
            }
            return (true, p);
        }
        let nt = (addr.saturating_sub(0x2000)) & 0x0FFF;
        let table = cart.nt_bank(nt);
        (false, table * 0x400 + (nt as usize & 0x3FF))
    }

    fn mem_read(&self, addr: u16, cart: &Cart) -> u8 {
        let addr = addr & 0x3FFF;
        if addr < 0x2000 {
            cart.chr_read(addr)
        } else {
            let (pal, i) = self.map(addr, cart);
            if pal { self.palette[i] } else { self.vram[i] }
        }
    }

    fn mem_write(&mut self, addr: u16, value: u8, cart: &mut Cart) {
        let addr = addr & 0x3FFF;
        if addr < 0x2000 {
            cart.chr_write(addr, value);
            return;
        }
        let (pal, i) = self.map(addr, cart);
        if pal {
            self.palette[i] = value & 0x3F;
        } else {
            self.vram[i] = value;
        }
    }

    fn rgb(&self, pal_index: u8) -> u32 {
        let mut hue = self.palette[pal_index as usize & 0x1F] & 0x3F;
        if self.mask & 0x01 != 0 {
            hue &= 0x30;
        }
        NES_PALETTE[hue as usize]
    }
}

fn copy_x(v: u16, t: u16) -> u16 {
    (v & !0x041F) | (t & 0x041F)
}

fn inc_y(mut v: u16) -> u16 {
    if v & 0x7000 != 0x7000 {
        return v + 0x1000;
    }
    v &= !0x7000;
    let mut y = (v & 0x03E0) >> 5;
    if y == 29 {
        y = 0;
        v ^= 0x0800;
    } else if y == 31 {
        y = 0;
    } else {
        y += 1;
    }
    (v & !0x03E0) | (y << 5)
}

fn inc_x(mut v: u16) -> u16 {
    if v & 0x001F == 0x001F {
        v &= !0x001F;
        v ^= 0x0400;
    } else {
        v += 1;
    }
    v
}
