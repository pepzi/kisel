pub mod cpu;
pub mod mmu;

use cpu::Cpu;
use minifb::{Key, Window, WindowOptions};
use mmu::Mmu;

const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;

pub fn run(rom_path: &str) {
    let mut mmu = Mmu::new();
    let mut cpu = Cpu::new();

    mmu.load_rom(rom_path);
    mmu.init_after_boot();
    println!("GB: {rom_path}");

    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut window = Window::new(
        "emul8 — Game Boy",
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
            if cycles == 0 {
                println!("\n[STOPP] oimplementerad GB-opcode.");
                std::process::exit(1);
            }
            mmu.tick(cycles);
            mmu.ppu_step(cycles);
            frame_cycles += cycles;
        }

        let lcdc = mmu.read_byte(0xFF40);
        let colors = [0xFFFFFFFF, 0xFFB5B5B5, 0xFF6B6B6B, 0xFF000000];

        if lcdc & 0x80 == 0 {
            for pixel in buffer.iter_mut() {
                *pixel = 0xFF8BAC0F;
            }
        } else {
            draw_layer(&mmu, &mut buffer, &colors, false);
            if lcdc & 0x20 != 0 {
                draw_layer(&mmu, &mut buffer, &colors, true);
            }
            draw_sprites(&mmu, &mut buffer, &colors, lcdc);
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
        (0x9000i32 + (tile_id as i8 as i32) * 16) as u16
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
            let (lx, ly) = if window {
                (x - ox, y - oy)
            } else {
                (
                    x + mmu.scx_line[y as usize] as i32,
                    y + mmu.scy_line[y as usize] as i32,
                )
            };
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

pub fn run_graphic_noise() {
    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut window = Window::new(
        "emul8 — noise",
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
