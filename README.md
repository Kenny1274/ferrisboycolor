# 🦀 FerrisBoy v0.2

**A Game Boy (DMG) and Game Boy Color (CGB) emulator written in Rust, inspired by mGBA.**

---

## What's New in v0.2 — CGB Support

FerrisBoy now auto-detects whether to run in DMG or CGB mode based on the cartridge header. Just load any `.gb` or `.gbc` ROM and it works.

| Feature | DMG | CGB |
|---------|-----|-----|
| CPU (LR35902, all 512 opcodes) | ✅ | ✅ |
| PPU — BG, Window, Sprites | ✅ | ✅ |
| Timer (DIV/TIMA/TMA/TAC) | ✅ | ✅ |
| Joypad | ✅ | ✅ |
| MBC1 / MBC2 / MBC3 / MBC5 | ✅ | ✅ |
| OAM DMA | ✅ | ✅ |
| VRAM Bank switching (VBK) | — | ✅ |
| WRAM Banks 0–7 (SVBK) | — | ✅ |
| CGB BG Palettes (8×4 RGB555) | — | ✅ |
| CGB OBJ Palettes (8×4 RGB555) | — | ✅ |
| BG tile attributes (VRAM bank 1) | — | ✅ |
| HDMA / GDMA transfers | — | ✅ |
| Double-speed mode (KEY1 / STOP) | — | ✅ |
| Serial port | stub | stub |
| APU (audio) | stub | stub |

---

## Architecture

```
ferrisboy/
├── Cargo.toml
└── src/
    ├── main.rs        — SDL2 loop, input, rendering
    ├── gameboy.rs     — Orchestrator, auto DMG/CGB detection
    ├── model.rs       — Model enum (DMG / CGB)
    ├── cpu.rs         — Sharp LR35902, all opcodes, HALT bug, EI delay
    ├── bus.rs         — Memory map, WRAM banking, HDMA, speed switch
    ├── ppu.rs         — Scanline renderer (DMG green + CGB RGB555)
    ├── cartridge.rs   — MBC1/2/3/5, CGB flag detection
    ├── timer.rs       — DIV/TIMA with falling-edge accuracy
    ├── dma.rs         — OAM DMA + HDMA/GDMA (CGB)
    ├── joypad.rs      — 8 buttons with interrupt
    ├── audio.rs       — APU register stub
    └── serial.rs      — Serial stub (debug output)
```

### CGB Timing Model

```
Single speed:  4.194304 MHz  (70224 T-cycles/frame)
Double speed:  8.388608 MHz  (CPU runs 2×, PPU/Timer unchanged)

Frame rate: ~59.73 fps regardless of speed mode
```

### CGB PPU Pipeline

```
Mode 2 → OAM Scan (80 dots)
           ↓ collect up to 10 sprites per scanline
Mode 3 → Drawing (172+ dots)
           ↓ BG from VRAM bank 0 tiles + bank 1 attributes
           ↓ Window same as BG
           ↓ Sprites with per-sprite bank, CGB palette, priority
Mode 0 → H-Blank (remaining dots)
           ↓ HDMA transfers 16 bytes here (if armed)
Mode 1 → V-Blank (10 lines)
```

---

## Controls

| Key | Game Boy |
|-----|----------|
| `Z` | A |
| `X` | B |
| `Enter` | Start |
| `Space` | Select |
| Arrow keys | D-Pad |
| `Escape` | Quit |

---

## Building

```bash
# Install SDL2
sudo apt install libsdl2-dev   # Ubuntu/Debian
brew install sdl2               # macOS

# Build & run
cargo run --release -- path/to/game.gb
cargo run --release -- path/to/game.gbc

# Force DMG mode for a CGB ROM
cargo run --release -- game.gbc --force-dmg

# Skip boot animation
cargo run --release -- game.gb --skip-boot

# Custom scale
cargo run --release -- game.gb --scale 4

# Verbose logging
RUST_LOG=debug cargo run -- game.gb
```

---

## Compatibility

| Game | Mode | Status |
|------|------|--------|
| Tetris | DMG | ✅ |
| Super Mario Land | DMG | ✅ |
| Pokémon Red/Blue | DMG MBC3 | ✅ |
| Pokémon Gold/Silver | CGB MBC3 | ✅ |
| Zelda: Link's Awakening DX | CGB MBC1 | ✅ |
| Pokémon Crystal | CGB MBC3 | ✅ |
| Dragon Warrior Monsters | CGB MBC5 | ✅ |

---

## Roadmap

- [ ] Full APU (4-channel audio)
- [ ] Save RAM to `.sav` files
- [ ] Game Boy Advance in DMG/CGB mode
- [ ] Rewind / save states
- [ ] Built-in debugger (VRAM viewer, CPU step, breakpoints)
- [ ] WebAssembly target

---

## References

- [Pan Docs](https://gbdev.io/pandocs/) — Complete hardware reference
- [mGBA](https://github.com/mgba-emu/mgba) — Architecture inspiration
- [TCAGBD](https://github.com/AntonioND/giibiiadvance/blob/master/docs/TCAGBD.pdf) — CGB hardware details
- [Mooneye test ROMs](https://github.com/Gekkio/mooneye-gb) — Hardware test suite

---

## License

MIT
