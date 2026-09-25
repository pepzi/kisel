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
| NES | Super Mario Bros. is playable without sound (mappers 0–3) |
| Commodore 64 | Planned |

The Game Boy CPU passes [Blargg `cpu_instrs`](https://github.com/retrio/gb-test-roms). The PPU is scanline-based with per-line scroll (enough for Mario Land’s status bar). There is no sound.

One window runs either core. File → Load ROM switches system from the extension (`.gb` or `.nes`). `.gbc` is not Game Boy Color — the core is DMG-only.

## Requirements

- Rust (edition 2024)
- A legal ROM you own
- [minifb](https://crates.io/crates/minifb) for the window (pulled in by Cargo)
- [rfd](https://crates.io/crates/rfd) for the native file dialog (Windows)

## Install

```bash
cargo install kisel
kisel game.gb
```

## Build and run

```bash
git clone https://github.com/pepzi/kisel
cd kisel
cargo run --release
cargo run --release -- game.gb
cargo run --release -- game.nes
cargo run --release -- noise
```

No argument opens an empty window. A path starts that ROM. Type is picked from the extension. Escape quits.

### Controls

| Key | Action |
|-----|--------|
| X | A |
| Z | B |
| Enter | Start |
| Tab | Select |
| WASD or arrows | D-pad |
| F5 | Save state |
| F6 | Load state |
| R (hold) | Rewind (NES) |
| P | Debug dump |
| Esc | Quit |

### Menus (Windows)

| Menu | Item |
|------|------|
| File | Load ROM… (Ctrl+O) |
| File | Exit (Ctrl+Q) |
| State | Save state (F5) |
| State | Load state (F6) |

After Load ROM the new window keeps the previous top-left corner. States live in RAM and disappear when you quit.

## Layout

```text
src/main.rs      window loop, ROM picker
src/gui.rs       minifb window, menus, pad
src/gb/          SM83 CPU, MMU/MBC1, PPU tick, renderer
src/nes/         6502, PPU, mappers 0–3
src/c64/         (forthcoming)
```

Each system is its own module. They do not share a CPU type.

## Honesty box

- Sound is missing on every system.
- Game Boy: MBC1 is the only mapper, and only the ROM-bank part.
- Game Boy Color is not implemented.
- NES: no APU, no MMC3, PPU is not cycle-accurate.
- A handful of commercial games boot; many do not.
- Native menus are wired for Windows. Linux/macOS are untested.
- This is a learning project, not a replacement for SameBoy, BGB, or Mesen.

## License

MIT © pepzi