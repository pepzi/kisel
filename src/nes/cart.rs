use std::fs;
use std::io::{self, ErrorKind};

pub struct Cart {
    pub prg: Vec<u8>,
    pub chr: Vec<u8>,
    pub mapper: u16,
    pub prg_banks: u8,
    pub chr_banks: u8,
    pub vertical_mirror: bool,
    pub has_trainer: bool,
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
            vertical_mirror,
            has_trainer,
        })
    }
}
