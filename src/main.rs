mod cpu;
mod mmu;
use minifb::{Key, Window, WindowOptions};
use std::{env, println};

use cpu::Cpu;
use mmu::Mmu;

const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mode = if args.len() > 1 {
        args[1].as_str()
    } else {
        "noise"
    };

    match mode {
        "cpu" => {
            println!("Starting Game Boy Emulator in CPU-test mode...");
            run_cpu_test();
        }
        "noise" => {
            println!("Running in windowed mode with graphical noise...");
            run_graphic_noise();
        }
        _ => {
            println!("Unknown argument '{}'. Use 'cpu' or 'noise'.", mode);
        }
    }
}

fn run_cpu_test() {
    let mut mmu = Mmu::new();
    let mut cpu = Cpu::new();

    mmu.load_rom("Tetris.gb");

    // i run_cpu_test, en gång efter load_rom
    print!("ROM1FF0:");
    for i in 0..24u16 {
        print!(" {:02X}", mmu.read_byte(0x1FF0 + i));
    }
    println!();

    println!("Starting executing from 0x0100 with graphical output...");

    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];

    let mut window = Window::new(
        "Rust GBEMU - Tetris Mode",
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions {
            scale: minifb::Scale::X4,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| panic!("{}", e));

    window.set_target_fps(60);

    let mut cycle_accumulator: u32 = 0;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mut buttons = 0x0Fu8;
        let mut dpad = 0x0Fu8;
        if window.is_key_down(Key::Enter) {
            buttons &= !0x08;
        }
        if window.is_key_down(Key::Tab) {
            buttons &= !0x04;
        }
        if window.is_key_down(Key::Z) {
            buttons &= !0x02;
        }
        if window.is_key_down(Key::X) {
            buttons &= !0x01;
        }
        if window.is_key_down(Key::S) | window.is_key_down(Key::Down) {
            dpad &= !0x08;
        }
        if window.is_key_down(Key::W) | window.is_key_down(Key::Up) {
            dpad &= !0x04;
        }
        if window.is_key_down(Key::A) | window.is_key_down(Key::Left) {
            dpad &= !0x02;
        }
        if window.is_key_down(Key::D) | window.is_key_down(Key::Right) {
            dpad &= !0x01;
        }
        mmu.set_joypad(buttons, dpad);

        let mut frame_cycles = 0;
        while frame_cycles < 70224 {
            let cycles = cpu.step(&mut mmu);
            mmu.tick_div(cycles);

            if cycles == 0 {
                println!(
                    "\n[STOPP] Emulatorn stängdes av på grund av en oimplementerad instruktion."
                );
                std::process::exit(1);
            }

            frame_cycles += cycles;
            cycle_accumulator += cycles;

            if cycle_accumulator >= 456 {
                cycle_accumulator -= 456;
                let current_ly = mmu.read_byte(0xFF44);
                mmu.write_byte(0xFF44, current_ly.wrapping_add(1) % 154);
            }
        }

        let current_if = mmu.read_byte(0xFF0F);
        mmu.write_byte(0xFF0F, current_if | 0x01);

        let lcdc = mmu.read_byte(0xFF40);
        let lcd_on = (lcdc & 0x80) != 0;
        let colors = [0xFFFFFFFF, 0xFFB5B5B5, 0xFF6B6B6B, 0xFF000000];

        if !lcd_on {
            for pixel in buffer.iter_mut() {
                *pixel = 0xFF8BAC0F;
            }
        } else {
            draw_layer(&mmu, &mut buffer, &colors, false);
            if lcdc & 0x20 != 0 {
                draw_layer(&mmu, &mut buffer, &colors, true);
            }
            draw_sprites(&mmu, &mut buffer, &colors, lcdc);

            if window.is_key_pressed(Key::P, minifb::KeyRepeat::No) {
                let lcdc = mmu.read_byte(0xFF40);
                println!(
                    "LCDC={:02X} LY={} DIV={:02X} PC={:04X} FFC0={:02X} FFCD={:02X} FFE1={:02X}",
                    lcdc,
                    mmu.read_byte(0xFF44),
                    mmu.read_byte(0xFF04),
                    cpu.pc,
                    mmu.read_byte(0xFFC0),
                    mmu.read_byte(0xFFCD),
                    mmu.read_byte(0xFFE1)
                );
                print!("map9800:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0x9800 + i));
                }
                println!();
                print!("tile2F:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0x8000 + 0x2F * 16 + i));
                }
                println!();
                print!("oam:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0xFE00 + i));
                }
                println!();
                print!("@0062:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0x0062 + i));
                }
                println!();
                print!("@00C0:");
                for i in 0..8u16 {
                    print!(" {:02X}", mmu.read_byte(0x00C0 + i));
                }
                println!();
                println!(
                    "FF80={:02X} FF85={:02X} FFCD={:02X} FFE1={:02X}",
                    mmu.read_byte(0xFF80),
                    mmu.read_byte(0xFF85),
                    mmu.read_byte(0xFFCD),
                    mmu.read_byte(0xFFE1)
                );
                println!();
                print!("C080:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0xC080 + i));
                }
                println!();
                print!("00CC:");
                for i in 0..4u16 {
                    print!(" {:02X}", mmu.read_byte(0x00CC + i));
                }
                println!();
                println!("-----------------------");
                println!(
                    "LCDC={:02X} IME={} IE={:02X} IF={:02X}",
                    mmu.read_byte(0xFF40),
                    cpu.ime,
                    mmu.read_byte(0xFFFF),
                    mmu.read_byte(0xFF0F)
                );
                print!("FE00:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0xFE00 + i));
                }
                println!();
                print!("C000:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0xC000 + i));
                }
                println!();
                print!("C200:");
                for i in 0..16u16 {
                    print!(" {:02X}", mmu.read_byte(0xC200 + i));
                }
                println!();
                println!(
                    "FFB6(dma)={:02X} FF85={:02X}",
                    mmu.read_byte(0xFFB6),
                    mmu.read_byte(0xFF85)
                );

                println!("-------------");
                println!(
                    "FFE1(state)={:02X} C213(next)={:02X} FF98(drop)={:02X}",
                    mmu.read_byte(0xFFE1),
                    mmu.read_byte(0xC213),
                    mmu.read_byte(0xFF98)
                );
                print!("C200:");
                for i in 0..32u16 {
                    print!(" {:02X}", mmu.read_byte(0xC200 + i));
                }
                println!();
                println!("----------------");
                println!(
                    "FFAB(paus)={:02X} FFA6={:02X} FFA9(lvl)={:02X} C201={:02X}",
                    mmu.read_byte(0xFFAB),
                    mmu.read_byte(0xFFA6),
                    mmu.read_byte(0xFFA9),
                    mmu.read_byte(0xC201)
                );

                println!("----------------");

                println!(
                    "FF8D={:02X}{:02X} FF8F(count)={:02X} FF98={:02X} FF9A={:02X}",
                    mmu.read_byte(0xFF8D),
                    mmu.read_byte(0xFF8E),
                    mmu.read_byte(0xFF8F),
                    mmu.read_byte(0xFF98),
                    mmu.read_byte(0xFF9A)
                );
            }
        }

        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}

fn tile_addr(_mmu: &Mmu, lcdc: u8, tile_id: u8, row: u8) -> u16 {
    let base = if lcdc & 0x10 != 0 {
        0x8000u16 + tile_id as u16 * 16
    } else {
        (0x9000i32 + tile_id as i8 as i32 * 16) as u16
    };
    base + row as u16 * 2
}

fn draw_layer(mmu: &Mmu, buffer: &mut [u32], colors: &[u32; 4], window: bool) {
    let lcdc = mmu.read_byte(0xFF40);
    let (ox, oy, map_base) = if window {
        let wx = mmu.read_byte(0xFF4B) as i32 - 7;
        let wy = mmu.read_byte(0xFF4A) as i32;
        let map = if lcdc & 0x40 != 0 { 0x9C00 } else { 0x9800 };
        (wx, wy, map)
    } else {
        let map = if lcdc & 0x08 != 0 { 0x9C00 } else { 0x9800 };
        (
            -(mmu.read_byte(0xFF43) as i32),
            -(mmu.read_byte(0xFF42) as i32),
            map,
        )
    };

    for y in 0..SCREEN_HEIGHT as i32 {
        for x in 0..SCREEN_WIDTH as i32 {
            let lx = if window { x - ox } else { x - ox };
            let ly = if window { y - oy } else { y - oy };
            if window && (lx < 0 || ly < 0) {
                continue;
            }
            let px = ((lx as u32) % 256) as u16;
            let py = ((ly as u32) % 256) as u16;
            let tile_id = mmu.read_byte(map_base + (py / 8) * 32 + (px / 8));
            let addr = tile_addr(mmu, lcdc, tile_id, (py % 8) as u8);
            let b1 = mmu.read_byte(addr);
            let b2 = mmu.read_byte(addr + 1);
            let bit = 7 - (px % 8);
            let cid = (((b2 >> bit) & 1) << 1) | ((b1 >> bit) & 1);
            buffer[y as usize * SCREEN_WIDTH + x as usize] = colors[cid as usize];
        }
    }
}

fn draw_sprites(mmu: &Mmu, buffer: &mut [u32], colors: &[u32; 4], lcdc: u8) {
    if lcdc & 0x02 == 0 {
        return;
    }
    let tall = lcdc & 0x04 != 0;
    for i in 0..40u16 {
        let o = 0xFE00 + i * 4;
        let sy = mmu.read_byte(o) as i32 - 16;
        let sx = mmu.read_byte(o + 1) as i32 - 8;
        let tile = mmu.read_byte(o + 2);
        let attr = mmu.read_byte(o + 3);
        let h = if tall { 16 } else { 8 };
        for row in 0..h {
            let ry = if attr & 0x40 != 0 { h - 1 - row } else { row };
            let tid = if tall {
                (tile & 0xFE) + if ry >= 8 { 1 } else { 0 }
            } else {
                tile
            };
            let addr = 0x8000 + tid as u16 * 16 + (ry as u16 % 8) * 2;
            let b1 = mmu.read_byte(addr);
            let b2 = mmu.read_byte(addr + 1);
            for col in 0..8 {
                let bit = if attr & 0x20 != 0 { col } else { 7 - col };
                let cid = (((b2 >> bit) & 1) << 1) | ((b1 >> bit) & 1);
                if cid == 0 {
                    continue;
                }
                let px = sx + col;
                let py = sy + row;
                if (0..160).contains(&px) && (0..144).contains(&py) {
                    buffer[py as usize * SCREEN_WIDTH + px as usize] = colors[cid as usize];
                }
            }
        }
    }
}

fn run_graphic_noise() {
    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];

    let mut window = Window::new(
        "Rust Game Boy Emulator",
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions {
            scale: minifb::Scale::X4,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| panic!("{}", e));

    window.set_target_fps(60);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        for pixel in buffer.iter_mut() {
            let rand_val = rand_brightness();
            *pixel = (255 << 24) | (rand_val << 16) | (rand_val << 8) | rand_val;
        }
        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}

fn rand_brightness() -> u32 {
    static mut SEED: u32 = 123456789;
    unsafe {
        SEED = SEED.overflowing_mul(1103515245).0.overflowing_add(12345).0;
        (SEED / 65536) % 256
    }
}
