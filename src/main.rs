mod cpu;
mod mmu;
use minifb::{Key, Window, WindowOptions};
use std::env;
use std::println;

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

    // Create a minimal "dummy" ROM in code to test, using just two NOP instructions
    let dummy_rom = vec![
        0x06, 0x42, // 0x0000: LD B, 0x42
        0x01, 0x05, 0x00, // 0x0002: LD BC, 0x0005 (Points to address 0x0005 in memory)
        0x0a, // 0x0005: LD A, (BC) -> Will read its own address and fetch 0x0A!
    ];
    mmu.load_rom(&dummy_rom);

    println!("Starttillstånd:");
    cpu.debug_dump();

    // Let's run 3 instructions (one for each opcode in our dummy_rom)
    for i in 0..=3 {
        println!("\n--- Steg {} ---", i);
        let cycles = cpu.step(&mut mmu);
        println!("Running instruction took {} cycles.", cycles);
        cpu.debug_dump();
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
