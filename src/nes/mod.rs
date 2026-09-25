pub mod cart;
pub mod cpu;
pub mod ppu;

use crate::gui::{Pad, Ui};
use cart::Cart;
use cpu::{Bus, Cpu, FLAG_B, FLAG_I, FLAG_U};
use ppu::Ppu;
use std::collections::VecDeque;
use std::path::Path;

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 240;

struct RewindManager {
    snapshots: VecDeque<NesState>,
    max_snapshots: usize,
    frame_counter: usize,
    interval: usize,
}

impl RewindManager {
    fn new(seconds_to_record: usize, interval: usize) -> Self {
        let max_snapshots = (seconds_to_record * 60) / interval.max(1);
        Self {
            snapshots: VecDeque::with_capacity(max_snapshots),
            max_snapshots,
            frame_counter: 0,
            interval,
        }
    }

    fn record(&mut self, state: NesState) {
        self.frame_counter += 1;
        if self.frame_counter.is_multiple_of(self.interval) {
            if self.snapshots.len() >= self.max_snapshots {
                self.snapshots.pop_front();
            }
            self.snapshots.push_back(state);
        }
    }

    fn pop_prev(&mut self) -> Option<NesState> {
        self.snapshots.pop_back()
    }

    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.frame_counter = 0;
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
    pub cart_state: cart::CartState,
}

struct NesBus {
    ram: [u8; 0x0800],
    cart: Cart,
    ppu: Ppu,
    buttons: u8,
    strobe: bool,
    shift: u8,
    dma_stall: u32,
}

impl NesBus {
    pub fn capture_state(&self, cpu: &Cpu) -> NesState {
        NesState {
            cpu_pc: cpu.pc,
            cpu_a: cpu.a,
            cpu_x: cpu.x,
            cpu_y: cpu.y,
            cpu_s: cpu.sp,
            cpu_status: cpu.p,
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

impl Bus for NesBus {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr as usize) & 0x07FF],
            0x2000..=0x3FFF => self.ppu.read_reg(addr, &self.cart),
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
            0x2000..=0x3FFF => self.ppu.write_reg(addr, value, &mut self.cart),
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

fn pad_to_nes(p: Pad) -> u8 {
    u8::from(p.a)
        | (u8::from(p.b) << 1)
        | (u8::from(p.select) << 2)
        | (u8::from(p.start) << 3)
        | (u8::from(p.up) << 4)
        | (u8::from(p.down) << 5)
        | (u8::from(p.left) << 6)
        | (u8::from(p.right) << 7)
}

pub struct Emu {
    cpu: Cpu,
    bus: NesBus,
    buffer: Vec<u32>,
    quick_save: Option<NesState>,
    rewind: RewindManager,
}

impl Emu {
    pub fn load(rom_path: &str) -> Self {
        let cart = match Cart::load(rom_path) {
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
            cart,
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

        Self {
            cpu,
            bus,
            buffer: vec![0; WIDTH * HEIGHT],
            quick_save: None,
            rewind: RewindManager::new(300, 1),
        }
    }

    pub fn pixels(&self) -> &[u32] {
        &self.buffer
    }

    pub fn frame(&mut self, pad: Pad, ui: Ui) {
        self.bus.buttons = pad_to_nes(pad);

        if ui.save {
            self.quick_save = Some(self.bus.capture_state(&self.cpu));
        }
        if ui.load {
            if let Some(ref state) = self.quick_save {
                self.bus.load_state(&mut self.cpu, state);
                self.rewind.clear();
            }
        }
        if ui.debug {
            println!(
                "PC={:04X} A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X} LY={}",
                self.cpu.pc,
                self.cpu.a,
                self.cpu.x,
                self.cpu.y,
                self.cpu.p,
                self.cpu.sp,
                self.bus.ppu.ly
            );
            println!(
                "PPUCTRL={:02X} PPUMASK={:02X} PPUSTATUS={:02X} t={:04X} v={:04X} x={}",
                self.bus.ppu.ctrl,
                self.bus.ppu.mask,
                self.bus.ppu.status,
                self.bus.ppu.t,
                self.bus.ppu.v,
                self.bus.ppu.x
            );
            println!(
                "pad buttons={:02X} strobe={} shift={:02X}",
                self.bus.buttons, self.bus.strobe, self.bus.shift
            );
        }

        if ui.rewind {
            if let Some(prev) = self.rewind.pop_prev() {
                self.bus.load_state(&mut self.cpu, &prev);
                self.emulate_one();
            }
        } else {
            self.emulate_one();
            self.rewind.record(self.bus.capture_state(&self.cpu));
        }
    }

    fn emulate_one(&mut self) {
        for ly in 0..262u16 {
            self.bus.ppu.ly = ly;

            if ly == 241 {
                self.bus.ppu.enter_vblank();
                if self.bus.ppu.nmi_enabled() {
                    let ret = self.cpu.pc;
                    self.cpu.push(&mut self.bus, (ret >> 8) as u8);
                    self.cpu.push(&mut self.bus, ret as u8);
                    self.cpu
                        .push(&mut self.bus, (self.cpu.p | FLAG_U) & !FLAG_B);
                    self.cpu.p |= FLAG_I;
                    let lo = self.bus.read(0xFFFA);
                    let hi = self.bus.read(0xFFFB);
                    self.cpu.pc = u16::from_le_bytes([lo, hi]);
                }
            }

            if ly >= 241 {
                run_cpu(&mut self.cpu, &mut self.bus, 113);
                if ly == 261 {
                    self.bus.ppu.pre_render();
                }
                continue;
            }

            if ly < 240 {
                self.bus.ppu.begin_visible_line(&self.bus.cart);
                self.bus.ppu.draw_scanline(&self.bus.cart, &mut self.buffer);
                self.bus.ppu.draw_sprites(&self.bus.cart, &mut self.buffer);
                self.bus.ppu.end_visible_line();
            }

            run_cpu(&mut self.cpu, &mut self.bus, 113);
        }
    }
}
