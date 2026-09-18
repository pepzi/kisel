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
    frame: u32,
}

impl Bus for NesBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF],
            0x2000..=0x3FFF => {
                let reg = 0x2000 + (addr & 0x0007);
                if reg == 0x2002 {
                    // bit 7 = VBlank. Växla varje läsning så init-loopar släpper.
                    self.frame = self.frame.wrapping_add(1);
                    if self.frame & 1 != 0 { 0x80 } else { 0x00 }
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
        if addr <= 0x1FFF {
            self.ram[(addr as usize) & 0x07FF] = value;
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
        frame: 0,
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
        let mut cycles = 0u32;
        while cycles < CYCLES_PER_FRAME {
            let n = cpu.step(&mut bus);
            if n == 0 {
                std::process::exit(1);
            }
            cycles += n;
        }

        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}
