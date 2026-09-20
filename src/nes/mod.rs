pub mod cart;
pub mod cpu;
pub mod ppu;

use cart::{Cart, CartState};
use cpu::{Bus, Cpu, FLAG_B, FLAG_I, FLAG_U};
use minifb::{Key, Window, WindowOptions};
use ppu::Ppu;
use std::path::Path;

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 240;

use std::collections::VecDeque;

// Denna struktur hanterar kön av states i minnet
struct RewindManager {
    snapshots: VecDeque<NesState>,
    max_snapshots: usize,
    frame_counter: usize,
    interval: usize,
}

impl RewindManager {
    fn new(seconds_to_record: usize, interval: usize) -> Self {
        let max_snapshots = (seconds_to_record * 60) / interval;
        Self {
            snapshots: VecDeque::with_capacity(max_snapshots),
            max_snapshots,
            frame_counter: 0,
            interval,
        }
    }

    fn record(&mut self, state: NesState) {
        self.frame_counter += 1;
        if self.frame_counter % self.interval == 0 {
            if self.snapshots.len() >= self.max_snapshots {
                self.snapshots.pop_front(); // Släng det äldsta tillståndet
            }
            self.snapshots.push_back(state);
        }
    }

    fn pop_prev(&mut self) -> Option<NesState> {
        self.snapshots.pop_back()
    }
}

#[derive(Clone)]
pub struct NesState {
    pub cpu_pc: u16,
    pub cpu_a: u8,
    pub cpu_x: u8,
    pub cpu_y: u8,
    pub cpu_s: u8,
    pub cpu_status: u8,

    pub ram: [u8; 0x0800],
    pub buttons: u8,
    pub strobe: bool,
    pub shift: u8,
    pub dma_stall: u32,
    pub ppu_state: Ppu,
    pub cart_state: CartState,
}

struct NesBus<'a> {
    ram: [u8; 0x0800],
    cart: &'a mut Cart,
    ppu: Ppu,
    buttons: u8,
    strobe: bool,
    shift: u8,
    dma_stall: u32,
}

impl<'a> NesBus<'a> {
    pub fn capture_state(&self, cpu: &Cpu) -> NesState {
        NesState {
            cpu_pc: cpu.pc,
            cpu_a: cpu.a,
            cpu_x: cpu.x,
            cpu_y: cpu.y,
            cpu_s: cpu.sp,
            cpu_status: cpu.p, // eller vad ditt statusregister heter

            ram: self.ram,
            buttons: self.buttons,
            strobe: self.strobe,
            shift: self.shift,
            dma_stall: self.dma_stall,
            ppu_state: self.ppu.clone(),
            cart_state: self.cart.capture_state(),
        }
    }

    pub fn load_state(&mut self, cpu: &mut Cpu, state: &NesState) {
        cpu.pc = state.cpu_pc;
        cpu.a = state.cpu_a;
        cpu.x = state.cpu_x;
        cpu.y = state.cpu_y;
        cpu.sp = state.cpu_s;
        cpu.p = state.cpu_status;

        self.ram = state.ram;
        self.buttons = state.buttons;
        self.strobe = state.strobe;
        self.shift = state.shift;
        self.dma_stall = state.dma_stall;
        self.ppu = state.ppu_state.clone();
        self.cart.load_state(&state.cart_state);
    }
}

impl Bus for NesBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF],
            0x2000..=0x3FFF => self.ppu.read_reg(addr, self.cart),
            0x6000..=0x7FFF => self.cart.wram_read(addr),
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
            0x6000..=0x7FFF => self.cart.wram_write(addr, value),
            0x8000..=0xFFFF => self.cart.prg_write(addr, value),
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
        cart.mirror_name(),
        cart.has_trainer
    );

    if !matches!(cart.mapper, 0..=3) {
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
    let mut quick_save_state: Option<NesState> = None;
    let mut rewind_manager = RewindManager::new(300, 1); // One frame per second for 5 minutes stored

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

        if window.is_key_down(minifb::Key::F5) {
            // VIKTIGT: Om din CPU (pc, a, x, y, status) lever utanför din NesBus,
            // se till att du har uppdaterat capture_state så att den även tar emot och
            // sparar din cpu! Exempel: quick_save_state =
            // Some(nes_bus.capture_state(&cpu));

            quick_save_state = Some(bus.capture_state(&cpu));
            println!("Save state sparad i minnet!");
        }

        if window.is_key_down(minifb::Key::F6) {
            if let Some(ref state) = quick_save_state {
                // VIKTIGT: Om du sparar CPU-register i din NesState, se till att skicka med din
                // cpu här med! Exempel: nes_bus.load_state(&mut cpu, state);

                bus.load_state(&mut cpu, state);
                println!("Save state laddad framgångsrikt!");
            } else {
                println!("Det finns ingen sparfil att ladda ännu! Tryck på F5 först.");
            }
        }

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

        if window.is_key_down(minifb::Key::R) {
            if let Some(prev_state) = rewind_manager.pop_prev() {
                bus.load_state(&mut cpu, &prev_state);
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
        } else {
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
            rewind_manager.record(bus.capture_state(&cpu));
        }

        window
            .update_with_buffer(&buffer, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    }
}
