pub mod cart;
pub mod cpu;

use cart::Cart;
use cpu::{Bus, Cpu};
use minifb::{Key, Window, WindowOptions};
use std::path::Path;

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 240;
const CYCLES_PER_FRAME: u32 = 29828;

struct NesBus<'a> {
    ram: [u8; 0x0800],
    cart: &'a mut Cart,
    ppuctrl: u8,
    ppumask: u8,
    ppustatus: u8,
    vram: [u8; 0x800],
    palette: [u8; 0x20],
    v: u16,
    w: bool,
    oam: [u8; 256],
    oam_addr: u8,
    buttons: u8,
    strobe: bool,
    shift: u8,
    scroll_x: u8,
    scroll_y: u8,
    render_x: u8,
    render_y: u8,
    hud_x: u8,
    hud_y: u8,
    cam_x: u8,
    cam_y: u8,
}

impl NesBus<'_> {
    fn ppu_map(&self, mut addr: u16) -> (bool, usize) {
        addr &= 0x3FFF;
        if addr >= 0x3F00 {
            let mut p = (addr as usize - 0x3F00) & 0x1F;
            if p & 0x13 == 0x10 {
                p &= !0x10;
            }
            return (true, p);
        }
        let nt = (addr.saturating_sub(0x2000)) & 0x0FFF;
        let table = if self.cart.vertical_mirror {
            (nt / 0x400) & 1
        } else {
            (nt / 0x800) & 1
        };
        let off = table as usize * 0x400 + (nt as usize & 0x3FF);
        (false, off)
    }

    fn ppu_read(&self, addr: u16) -> u8 {
        let addr = addr & 0x3FFF;
        if addr < 0x2000 {
            let chr = &self.cart.chr;
            if chr.is_empty() {
                0
            } else {
                chr[addr as usize % chr.len()]
            }
        } else {
            let (pal, i) = self.ppu_map(addr);
            if pal { self.palette[i] } else { self.vram[i] }
        }
    }

    fn ppu_write(&mut self, addr: u16, value: u8) {
        let addr = addr & 0x3FFF;
        if addr < 0x2000 {
            let len = self.cart.chr.len();
            if len != 0 {
                self.cart.chr[addr as usize % len] = value;
            }
            return;
        }
        let (pal, i) = self.ppu_map(addr);
        if pal {
            self.palette[i] = value;
        } else {
            self.vram[i] = value;
        }
    }
}

impl Bus for NesBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF],
            0x2000..=0x3FFF => match addr & 7 {
                2 => {
                    let v = self.ppustatus;
                    self.ppustatus &= !0xC0;
                    self.w = false;
                    v
                }
                7 => {
                    let v = self.ppu_read(self.v);
                    self.v = self
                        .v
                        .wrapping_add(if self.ppuctrl & 0x04 != 0 { 32 } else { 1 });
                    v
                }
                _ => 0,
            },
            0x4016 => {
                let bit = if self.strobe {
                    self.buttons & 1
                } else {
                    let b = self.shift & 1;
                    self.shift >>= 1;
                    b
                };
                bit | 0x40
            }
            0x8000..=0xFFFF => {
                let off = (addr as usize - 0x8000) % self.cart.prg.len();
                self.cart.prg[off]
            }
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF] = value,
            0x2000..=0x3FFF => match addr & 7 {
                0 => self.ppuctrl = value,
                1 => self.ppumask = value,
                3 => self.oam_addr = value,
                4 => {
                    self.oam[self.oam_addr as usize] = value;
                    self.oam_addr = self.oam_addr.wrapping_add(1);
                }
                5 => {
                    if !self.w {
                        self.scroll_x = value;
                    } else {
                        self.scroll_y = value;
                        if self.scroll_x == 0 && self.scroll_y == 0 {
                            self.hud_x = 0;
                            self.hud_y = 0;
                        } else {
                            self.cam_x = self.scroll_x;
                            self.cam_y = self.scroll_y;
                        }
                    }
                    self.w = !self.w;
                }
                6 => {
                    if !self.w {
                        self.v = (self.v & 0x00FF) | ((value as u16 & 0x3F) << 8);
                    } else {
                        self.v = (self.v & 0xFF00) | value as u16;
                    }
                    self.w = !self.w;
                }
                7 => {
                    self.ppu_write(self.v, value);
                    self.v = self
                        .v
                        .wrapping_add(if self.ppuctrl & 0x04 != 0 { 32 } else { 1 });
                }
                _ => {}
            },
            0x4014 => {
                let src = (value as u16) << 8;
                for i in 0..256u16 {
                    self.oam[i as usize] = self.read(src + i);
                }
                self.oam_addr = 0;
            }
            0x4016 => {
                self.strobe = value & 1 != 0;
                if self.strobe {
                    self.shift = self.buttons;
                }
            }
            _ => {}
        }
    }
}

fn draw_bg(bus: &NesBus, buffer: &mut [u32]) {
    let nt0 = 0x2000 | ((bus.ppuctrl as u16 & 0x03) * 0x400);
    let pat = if bus.ppuctrl & 0x10 != 0 {
        0x1000u16
    } else {
        0
    };
    let colors = [0xFF000000, 0xFF555555, 0xFFAAAAAA, 0xFFFFFFFF];
    const SPLIT: u16 = 32;

    for py in 0..240u16 {
        let sx = if py < SPLIT {
            bus.hud_x as u16
        } else {
            bus.cam_x as u16
        };
        let sy = if py < SPLIT {
            bus.hud_y as u16
        } else {
            bus.cam_y as u16
        };

        for px in 0..256u16 {
            let wx = px.wrapping_add(sx);
            let wy = py.wrapping_add(sy);
            let ntx = (wx >> 8) & 1;
            let nty = (wy >> 8) & 1;
            let nt = nt0 ^ (ntx * 0x400) ^ (nty * 0x800);
            let tile = bus.ppu_read(nt + ((wy >> 3) & 31) * 32 + ((wx >> 3) & 31)) as u16;
            let row = wy & 7;
            let col = wx & 7;
            let p0 = bus.ppu_read(pat + tile * 16 + row);
            let p1 = bus.ppu_read(pat + tile * 16 + row + 8);
            let bit = 7 - col;
            let cid = (((p1 >> bit) & 1) << 1) | ((p0 >> bit) & 1);
            buffer[py as usize * 256 + px as usize] = colors[cid as usize];
        }
    }
}
fn draw_sprites(bus: &NesBus, buffer: &mut [u32]) {
    if bus.ppumask & 0x10 == 0 {
        return;
    }
    let tall = bus.ppuctrl & 0x20 != 0;
    let pat8 = if bus.ppuctrl & 0x08 != 0 {
        0x1000u16
    } else {
        0
    };
    let colors = [0xFF000000, 0xFF555555, 0xFFAAAAAA, 0xFFFFFFFF];

    for i in (0..64).rev() {
        let o = i * 4;
        let sy = bus.oam[o] as i32 + 1;
        let tile = bus.oam[o + 1];
        let attr = bus.oam[o + 2];
        let sx = bus.oam[o + 3] as i32;
        let h = if tall { 16 } else { 8 };

        for row in 0..h {
            let ry = if attr & 0x80 != 0 { h - 1 - row } else { row };
            let (tid, base) = if tall {
                let t = tile & 0xFE;
                let extra = u8::from(ry >= 8);
                ((t + extra) as u16, if tile & 1 != 0 { 0x1000 } else { 0 })
            } else {
                (tile as u16, pat8)
            };
            let p0 = bus.ppu_read(base + tid * 16 + (ry as u16 % 8));
            let p1 = bus.ppu_read(base + tid * 16 + (ry as u16 % 8) + 8);
            for col in 0..8i32 {
                let bit = if attr & 0x40 != 0 { col } else { 7 - col };
                let cid = (((p1 >> bit) & 1) << 1) | ((p0 >> bit) & 1);
                if cid == 0 {
                    continue;
                }
                let px = sx + col;
                let py = sy + row;
                if (0..256).contains(&px) && (0..240).contains(&py) {
                    buffer[py as usize * 256 + px as usize] = colors[cid as usize];
                }
            }
        }
    }
}

pub fn run(rom_path: &str) {
    let mut cart = match Cart::load(rom_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("NES: could not read {rom_path}: {e}");
            std::process::exit(1);
        }
    };

    println!("NES: {rom_path}");
    println!(
        "  mapper={} PRG={}×16K CHR={}×8K mirror={} trainer={}",
        cart.mapper,
        cart.prg_banks,
        cart.chr_banks,
        if cart.vertical_mirror { "V" } else { "H" },
        cart.has_trainer
    );

    if cart.mapper != 0 {
        eprintln!(
            "NES: mapper {} is not implemented (NROM/0 only).",
            cart.mapper
        );
        std::process::exit(1);
    }

    let nestest = Path::new(rom_path)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_lowercase().contains("nestest"));

    let mut cpu = Cpu::new();
    let mut bus = NesBus {
        ram: [0; 0x0800],
        cart: &mut cart,
        ppuctrl: 0,
        ppumask: 0,
        ppustatus: 0,
        vram: [0; 0x800],
        palette: [0; 0x20],
        v: 0,
        w: false,
        oam: [0; 256],
        oam_addr: 0,
        buttons: 0,
        strobe: false,
        shift: 0,
        scroll_x: 0,
        scroll_y: 0,
        render_x: 0,
        render_y: 0,
        cam_x: 0,
        cam_y: 0,
        hud_x: 0,
        hud_y: 0,
    };

    if nestest {
        cpu.reset_at(0xC000);
        println!("NES: nestest automation, PC=$C000");
    } else {
        cpu.reset(&mut bus);
        println!("NES: reset PC=${:04X}", cpu.pc);
    }

    let mut buffer = vec![0u32; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut window = Window::new(
        "kisel — NES",
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions {
            scale: minifb::Scale::X2,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    window.set_target_fps(60);

    for (i, pixel) in buffer.iter_mut().enumerate() {
        let y = i / SCREEN_WIDTH;
        let shade = 0x20 + (y as u32 * 0xC0 / SCREEN_HEIGHT as u32);
        *pixel = 0xFF000000 | (shade << 16) | (shade << 8) | shade;
    }

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if window.is_key_pressed(Key::P, minifb::KeyRepeat::No) {
            println!(
                "PC={:04X} A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X}",
                cpu.pc, cpu.a, cpu.x, cpu.y, cpu.p, cpu.sp
            );
            println!(
                "PPUCTRL={:02X} PPUMASK={:02X} PPUSTATUS={:02X} v={:04X}",
                bus.ppuctrl, bus.ppumask, bus.ppustatus, bus.v
            );
            println!(
                "pad buttons={:02X} strobe={} shift={:02X}",
                bus.buttons, bus.strobe, bus.shift
            );
            print!("oam0:");
            for i in 0..16 {
                print!(" {:02X}", bus.oam[i]);
            }
            println!();
        }

        let mut buttons = 0u8;
        if window.is_key_down(Key::X) {
            buttons |= 0x01; // A
        }
        if window.is_key_down(Key::Z) {
            buttons |= 0x02; // B
        }
        if window.is_key_down(Key::Tab) {
            buttons |= 0x04; // Select
        }
        if window.is_key_down(Key::Enter) {
            buttons |= 0x08; // Start
        }
        if window.is_key_down(Key::W) | window.is_key_down(Key::Up) {
            buttons |= 0x10;
        }
        if window.is_key_down(Key::S) | window.is_key_down(Key::Down) {
            buttons |= 0x20;
        }
        if window.is_key_down(Key::A) | window.is_key_down(Key::Left) {
            buttons |= 0x40;
        }
        if window.is_key_down(Key::D) | window.is_key_down(Key::Right) {
            buttons |= 0x80;
        }
        bus.buttons = buttons;

        let mut cycles = 0u32;
        while cycles < CYCLES_PER_FRAME {
            let n = cpu.step(&mut bus);
            if n == 0 {
                std::process::exit(1);
            }
            cycles += n;
        }

        bus.ppustatus |= 0xC0; // VBlank + sprite 0

        if bus.ppuctrl & 0x80 != 0 {
            let ret = cpu.pc;
            cpu.push(&mut bus, (ret >> 8) as u8);
            cpu.push(&mut bus, ret as u8);
            cpu.push(&mut bus, (cpu.p | cpu::FLAG_U) & !cpu::FLAG_B);
            cpu.p |= cpu::FLAG_I;
            let lo = bus.read(0xFFFA);
            let hi = bus.read(0xFFFB);
            cpu.pc = u16::from_le_bytes([lo, hi]);
        }
        draw_bg(&bus, &mut buffer);
        draw_sprites(&bus, &mut buffer);
        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}
