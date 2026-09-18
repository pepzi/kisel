pub mod cart;
pub mod cpu;
pub mod ppu;

use cart::Cart;
use cpu::{Bus, Cpu, FLAG_B, FLAG_I, FLAG_U};
use minifb::{Key, Window, WindowOptions};
use ppu::Ppu;
use std::path::Path;

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 240;

struct NesBus<'a> {
    ram: [u8; 0x0800],
    cart: &'a mut Cart,
    ppu: Ppu,
    buttons: u8,
    strobe: bool,
    shift: u8,
    dma_stall: u32,
}

impl Bus for NesBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF],
            0x2000..=0x3FFF => self.ppu.read_reg(addr, self.cart),
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
            0x8000..=0xFFFF => self.cart.prg_read(addr),
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF] = value,
            0x2000..=0x3FFF => self.ppu.write_reg(addr, value, self.cart),
            0x4014 => {
                let src = (value as u16) << 8;
                for i in 0..256u16 {
                    let b = self.read(src + i);
                    self.ppu.write_oam(i as u8, b);
                }
                self.ppu.oam_addr = 0;
                self.dma_stall += 513;
            }
            0x4016 => {
                self.strobe = value & 1 != 0;
                if self.strobe {
                    self.shift = self.buttons;
                }
            }
            0x8000..=0xFFFF => self.cart.mmc1_write(addr, value),
            _ => {}
        }
    }
}

fn run_cpu(cpu: &mut Cpu, bus: &mut NesBus, budget: u32) {
    let mut cycles = 0u32;
    while cycles < budget {
        if bus.dma_stall > 0 {
            let n = bus.dma_stall.min(budget - cycles);
            bus.dma_stall -= n;
            cycles += n;
            continue;
        }
        let n = cpu.step(bus);
        if n == 0 {
            std::process::exit(1);
        }
        cycles += n;
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

    if cart.mapper != 0 && cart.mapper != 1 {
        eprintln!("NES: mapper {} is not implemented", cart.mapper);
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
        ppu: Ppu::new(),
        buttons: 0,
        strobe: false,
        shift: 0,
        dma_stall: 0,
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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut buttons = 0u8;
        if window.is_key_down(Key::X) {
            buttons |= 0x01;
        }
        if window.is_key_down(Key::Z) {
            buttons |= 0x02;
        }
        if window.is_key_down(Key::Tab) {
            buttons |= 0x04;
        }
        if window.is_key_down(Key::Enter) {
            buttons |= 0x08;
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

        if window.is_key_pressed(Key::P, minifb::KeyRepeat::No) {
            println!(
                "PC={:04X} A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X} LY={}",
                cpu.pc, cpu.a, cpu.x, cpu.y, cpu.p, cpu.sp, bus.ppu.ly
            );
            println!(
                "PPUCTRL={:02X} PPUMASK={:02X} PPUSTATUS={:02X} t={:04X} v={:04X} x={}",
                bus.ppu.ctrl, bus.ppu.mask, bus.ppu.status, bus.ppu.t, bus.ppu.v, bus.ppu.x
            );
            println!(
                "pad buttons={:02X} strobe={} shift={:02X}",
                bus.buttons, bus.strobe, bus.shift
            );
        }

        for ly in 0..262u16 {
            bus.ppu.ly = ly;

            if ly == 241 {
                bus.ppu.enter_vblank();
                if bus.ppu.nmi_enabled() {
                    let ret = cpu.pc;
                    cpu.push(&mut bus, (ret >> 8) as u8);
                    cpu.push(&mut bus, ret as u8);
                    cpu.push(&mut bus, (cpu.p | FLAG_U) & !FLAG_B);
                    cpu.p |= FLAG_I;
                    let lo = bus.read(0xFFFA);
                    let hi = bus.read(0xFFFB);
                    cpu.pc = u16::from_le_bytes([lo, hi]);
                }
            }

            if ly >= 241 {
                run_cpu(&mut cpu, &mut bus, 113);
                if ly == 261 {
                    bus.ppu.pre_render();
                }
                continue;
            }

            if ly < 240 {
                bus.ppu.begin_visible_line(bus.cart);
                bus.ppu.draw_scanline(bus.cart, &mut buffer);
                bus.ppu.draw_sprites(bus.cart, &mut buffer);
                bus.ppu.end_visible_line();
            }

            run_cpu(&mut cpu, &mut bus, 113);
        }

        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}
