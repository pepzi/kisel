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
    ppustatus: u8,
}

impl Bus for NesBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF],
            0x2000..=0x3FFF => {
                if addr & 7 == 2 {
                    let v = self.ppustatus;
                    self.ppustatus &= !0xC0; // nollställ både VBlank och sprite 0
                    v
                } else {
                    0
                }
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
            0x2000..=0x3FFF if addr & 7 == 0 => self.ppuctrl = value,
            _ => {}
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
        ppustatus: 0,
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
                "PC={:04X} A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X} PPUCTRL={:02X} PPUSTATUS={:02X} I={}",
                cpu.pc,
                cpu.a,
                cpu.x,
                cpu.y,
                cpu.p,
                cpu.sp,
                bus.ppuctrl,
                bus.ppustatus,
                cpu.p & cpu::FLAG_I != 0
            );
            print!("@PC:");
            for i in 0..8u16 {
                print!(" {:02X}", bus.read(cpu.pc.wrapping_add(i)));
            }
            println!();
        }

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

        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}
