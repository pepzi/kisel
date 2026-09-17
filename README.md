# kisel

**kisel** (Swedish for *silicon*), pronounced roughly **chee-sel**.

A small multi-system emulator written in Rust. One binary, several chips in the same socket.

Author: [pepzi](https://github.com/pepzi/)

## What it is

`kisel` is a personal emulator for 8-bit machines. It started as a Game Boy core built to run *Tetris* and grew from there. The long-term idea is the same program launching a `.gb`, a `.nes` or a `.d64` without pretending to be RetroArch.

Current targets:

| System | Status |
|--------|--------|
| Game Boy (DMG) | Playable for a few ROMs (Tetris, Super Mario Land, Alleyway with caveats) |
| NES | Not wired up yet |
| Commodore 64 | Planned |

The Game Boy CPU passes [Blargg `cpu_instrs`](https://github.com/retrio/gb-test-roms). The PPU is scanline-based with per-line scroll (enough for Mario Land’s status bar). There is no sound.

## Requirements

- Rust (edition 2024)
- A legal ROM you own
- [minifb](https://crates.io/crates/minifb) for the window (pulled in by Cargo)

## Install

```bash
cargo install kisel
kisel game.gb
```

## Build and run

```bash
git clone https://github.com/pepzi/kisel
cd kisel
cargo run --release -- game.gb
cargo run --release -- noise
```

First argument is the ROM path. Type is picked from the extension (`.gb` / `.gbc`). Escape quits.

### Game Boy keys

| Key | Button |
|-----|--------|
| X | A |
| Z | B |
| Enter | Start |
| Tab | Select |
| WASD or arrows | D-pad |

## Layout

```text
src/main.rs      dispatch on file extension
src/gb/          LR35902 CPU, MMU/MBC1, PPU tick, renderer
src/nes/         (forthcoming)
src/c64/         (forthcoming)
```

Each system is its own module. They do not share a CPU type.

## Honesty box

- Sound is missing.
- MBC1 is the only mapper, and only the ROM-bank part.
- A handful of commercial GB games boot; many do not.
- This is a learning project, not a replacement for SameBoy or BGB.

## License

MIT © pepzi