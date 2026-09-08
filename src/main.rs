mod cpu;
mod mmu;
use minifb::{Key, Window, WindowOptions};
use std::env;
use std::println;
use std::vec;

use cpu::Cpu;
use mmu::Mmu;

const SCREEN_WIDTH: usize = 160;
const SCREEN_HEIGHT: usize = 144;

fn main() {
    // Get arguments from command
    let args: Vec<String> = env::args().collect();

    // Check if user send an argument, otherwise set "noise" as standard
    let mode = if args.len() > 1 {
        args[1].as_str()
    } else {
        "noise"
    };

    // 2. Run selected mode
    match mode {
        "cpu" => {
            println!("Starting Game Boy Emulator in CPU-test mode (dummy ROM)...");
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

    mmu.load_rom("tetris.gb");

    println!("Staring executing from 0x0100 with graphical output...");

    // 1. Create an empty pixel buffer (framebuffer) for Tetris (160x144 pixels)
    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];

    // 2. Open window specifically for CPU-mode (Scale::X4 to match noise-mode)
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

    let mut instruction_count: u32 = 0;

    // 3. Replace loop to keep it running as long as the window is open
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // 4. Run an amount of instructions per frame (for example 7000) to allow the
        //    game to do some work
        for _ in 0..7000 {
            let cycles = cpu.step(&mut mmu);

            if cycles == 0 {
                println!(
                    "\n[STOPP] Emulatorn stängdes av på grund av en oimplementerad instruktion."
                );
                std::process::exit(1); // <--- ÄNDRA HÄR: Döda processen stenhårt direkt!
            }

            instruction_count = instruction_count.wrapping_add(1);

            if instruction_count.is_multiple_of(100) {
                let current_ly = mmu.read_byte(0xFF44);
                // SKärmen har 154 scanlines totalt (0-153)
                mmu.write_byte(0xFF44, (current_ly + 1) % 154);
            }
        }

        // Let's draw the TETRIS-SCREEN!
        // We read directly from the Game Boy VRAM and translate to our screen.

        // Define Game Boys 4 classical gray/green tones (ARGB format)
        let colors = [0xFFFFFFFF, 0xFFB5B5B5, 0xFF6B6B6B, 0x00000000];

        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                // Find which 8x8 block we are currently at in the background map
                let tile_x = x / 8;
                let tile_y = y / 8;

                // The Game Boy background map starts at memory address 0x9800
                let map_addr = (0x9800 + (tile_y * 32) + tile_x) as u16;
                let tile_id = mmu.read_byte(map_addr) as u16;

                // Calculate exactly where in the pixel data block to read (each row is 2 bytes)
                let pixel_x = x % 8;
                let pixel_y = y % 8;

                // Graphics data start address in VRAM (0x8000)
                let tile_data_addr = 0x8000 + (tile_id * 16) + (pixel_y as u16 * 2);

                // Game Boy stores colors in a smart "2bpp" format (2 bits per pixel)
                let byte1 = mmu.read_byte(tile_data_addr);
                let byte2 = mmu.read_byte(tile_data_addr + 1);

                // Fetch the two bits for this specific pixel
                let bit_index = 7 - pixel_x;
                let color_bit1 = (byte1 >> bit_index) & 1;
                let color_bit2 = (byte2 >> bit_index) & 1;
                let color_id = (color_bit2 << 1) | color_bit1;

                // Store correct pixel color in our minifb framebuffer
                buffer[y * SCREEN_WIDTH + x] = colors[color_id as usize];
            }
        }

        // 6. Send our buffer to the window once per frame
        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}

fn run_graphic_noise() {
    // Create empty pixel-buffer (minifb wants 32-bit ARGB pixels)
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

    // Limit frame rate to around 60 Hz
    window.set_target_fps(60);

    // Main loop: Running as long as window is open and ESC isn't pressed
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // 1. This is where the emulation loop will run instructions in the future:
        // cpu.step(&mut mmu);

        // 2. Create white noise to see the window "alive"
        for pixel in buffer.iter_mut() {
            let rand_val = rand_brightness(); // Random grey-scale
            *pixel = (255 << 24) | (rand_val << 16) | (rand_val << 8) | rand_val;
        }

        // 3. Update windows with pixel buffer
        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}

// Simple and fast helper method to create a random value between 0 and 255
// without the rand-crate
fn rand_brightness() -> u32 {
    static mut SEED: u32 = 123456789;
    unsafe {
        SEED = SEED.overflowing_mul(1103515245).0.overflowing_add(12345).0;
        (SEED / 65536) % 256
    }
}
