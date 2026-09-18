use std::fs;
use std::io::{self, ErrorKind};

pub struct Cart {
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub mapper: u16,
    pub prg_banks: u8,
    pub chr_banks: u8,
    pub has_trainer: bool,
    /// 0: one-screen A, 1: one-screen B, 2: vertical, 3: horizontal
    mirror: u8,
    prg_mode: u8,
    prg_bank: usize,
    chr_bank0: u8,
    chr_bank1: u8,
    shift: u8,
    shift_count: u8,
    control: u8,
    prg_ram: [u8; 0x2000],
    prg_ram_disable: bool,
}

impl Cart {
    pub fn load(path: &str) -> io::Result<Self> {
        let data = fs::read(path)?;
        if data.len() < 16 || &data[0..4] != b"NES\x1A" {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "inte en iNES-fil (saknar NES\\x1A)",
            ));
        }

        let prg_banks = data[4];
        let chr_banks = data[5];
        let flags6 = data[6];
        let flags7 = data[7];
        let mapper = (flags6 >> 4) as u16 | ((flags7 & 0xF0) as u16);
        let vertical_mirror = flags6 & 0x01 != 0;
        let has_trainer = flags6 & 0x04 != 0;
        let nes2 = flags7 & 0x0C == 0x08;

        if nes2 {
            eprintln!("NES: NES 2.0-header, behandlas som iNES 1.0 tills vidare");
        }

        let mut off = 16usize;
        if has_trainer {
            off += 512;
        }

        let prg_len = prg_banks as usize * 0x4000;
        let chr_len = chr_banks as usize * 0x2000;
        if data.len() < off + prg_len + chr_len {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "filen är för kort ({} byte, behöver {})",
                    data.len(),
                    off + prg_len + chr_len
                ),
            ));
        }

        let prg = data[off..off + prg_len].to_vec();
        let mut chr = data[off + prg_len..off + prg_len + chr_len].to_vec();
        if chr.is_empty() {
            chr = vec![0; 0x2000];
        }

        Ok(Self {
            prg,
            chr,
            mapper,
            prg_banks,
            chr_banks,
            has_trainer,
            mirror: if vertical_mirror { 2 } else { 3 },
            prg_mode: 3,
            prg_bank: 0,
            chr_bank0: 0,
            chr_bank1: 0,
            shift: 0,
            shift_count: 0,
            control: 0x0C,
            prg_ram: [0; 0x2000],
            prg_ram_disable: false,
        })
    }

    pub fn mirror_name(&self) -> &'static str {
        match self.mirror & 3 {
            0 => "1A",
            1 => "1B",
            2 => "V",
            _ => "H",
        }
    }

    pub fn nt_bank(&self, nt_offset: u16) -> usize {
        let n = (nt_offset / 0x400) & 3;
        match self.mirror & 3 {
            0 => 0,
            1 => 1,
            2 => (n & 1) as usize,
            _ => ((n >> 1) & 1) as usize,
        }
    }

    pub fn chr_read(&self, addr: u16) -> u8 {
        let off = self.chr_map(addr);
        self.chr.get(off).copied().unwrap_or(0)
    }

    pub fn chr_write(&mut self, addr: u16, value: u8) {
        if self.chr_banks != 0 {
            return;
        }
        let off = self.chr_map(addr);
        if let Some(slot) = self.chr.get_mut(off) {
            *slot = value;
        }
    }

    pub fn wram_read(&self, addr: u16) -> u8 {
        if self.mapper != 1 || self.prg_ram_disable {
            return 0;
        }
        self.prg_ram[addr as usize & 0x1FFF]
    }

    pub fn wram_write(&mut self, addr: u16, value: u8) {
        if self.mapper != 1 || self.prg_ram_disable {
            return;
        }
        self.prg_ram[addr as usize & 0x1FFF] = value;
    }

    fn chr_map(&self, addr: u16) -> usize {
        let addr = addr as usize & 0x1FFF;
        let len = self.chr.len().max(1);
        if self.mapper != 1 {
            return addr % len;
        }
        let banks = (len / 0x1000).max(1);
        let chr_4k = self.control & 0x10 != 0;
        let bank = if chr_4k {
            if addr < 0x1000 {
                self.chr_bank0
            } else {
                self.chr_bank1
            }
        } else {
            (self.chr_bank0 & !1) + u8::from(addr >= 0x1000)
        };
        (bank as usize % banks) * 0x1000 + (addr & 0x0FFF)
    }

    pub fn mmc1_write(&mut self, addr: u16, value: u8) {
        if self.mapper != 1 {
            return;
        }
        if value & 0x80 != 0 {
            self.shift = 0;
            self.shift_count = 0;
            self.control |= 0x0C;
            self.prg_mode = 3;
            return;
        }
        self.shift |= (value & 1) << self.shift_count;
        self.shift_count += 1;
        if self.shift_count < 5 {
            return;
        }
        let data = self.shift;
        self.shift = 0;
        self.shift_count = 0;

        match addr {
            0x8000..=0x9FFF => {
                self.control = data;
                self.prg_mode = (data >> 2) & 3;
                self.mirror = data & 3;
            }
            0xA000..=0xBFFF => self.chr_bank0 = data,
            0xC000..=0xDFFF => self.chr_bank1 = data,
            0xE000..=0xFFFF => {
                let banks = (self.prg.len() / 0x4000).max(1);
                self.prg_bank = (data as usize & 0x0F) % banks;
                self.prg_ram_disable = data & 0x10 != 0;
            }
            _ => {}
        }
    }

    pub fn prg_read(&self, addr: u16) -> u8 {
        let banks = (self.prg.len() / 0x4000).max(1);
        let last = banks - 1;
        let slot_low = matches!(addr, 0x8000..=0xBFFF);

        let bank = if self.mapper != 1 {
            if slot_low {
                0
            } else if banks > 1 {
                1
            } else {
                0
            }
        } else {
            match self.prg_mode {
                2 => {
                    if slot_low {
                        0
                    } else {
                        self.prg_bank
                    }
                }
                3 => {
                    if slot_low {
                        self.prg_bank
                    } else {
                        last
                    }
                }
                _ => {
                    let base = self.prg_bank & !1;
                    if slot_low { base } else { base + 1 }
                }
            }
        };

        let off = bank * 0x4000 + (addr as usize & 0x3FFF);
        *self.prg.get(off).unwrap_or(&0xFF)
    }
}
