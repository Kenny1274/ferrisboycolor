/// FerrisBoy Memory Bus — DMG + CGB
///
/// CGB additions:
///  - WRAM banks 0–7 (0xD000–0xDFFF switchable via SVBK 0xFF70)
///  - KEY1 double-speed CPU switch (0xFF4D)
///  - HDMA registers (0xFF51–0xFF55)
///  - VBK VRAM bank (0xFF4F) — handled in PPU
///  - CGB palette registers (0xFF68–0xFF6B) — handled in PPU

use crate::cartridge::Cartridge;
use crate::dma::{OamDma, Hdma, HdmaMode};
use crate::joypad::Joypad;
use crate::model::Model;
use crate::ppu::{Ppu, PpuMode};
use crate::timer::Timer;
use crate::audio::Apu;
use crate::serial::Serial;

pub mod irq {
    pub const VBLANK: u8 = 1 << 0;
    pub const STAT:   u8 = 1 << 1;
    pub const TIMER:  u8 = 1 << 2;
    pub const SERIAL: u8 = 1 << 3;
    pub const JOYPAD: u8 = 1 << 4;
}

pub struct Bus {
    pub cart:   Cartridge,
    pub ppu:    Ppu,
    pub timer:  Timer,
    pub joypad: Joypad,
    pub apu:    Apu,
    pub serial: Serial,
    pub oam_dma: OamDma,
    pub hdma:    Hdma,

    /// 8 WRAM banks of 4KiB (bank 0 always mapped at 0xC000, bank 1–7 at 0xD000)
    wram: Box<[[u8; 0x1000]; 8]>,
    wram_bank: usize,   // active bank for 0xD000–0xDFFF (default 1; CGB only)

    hram: [u8; 0x7F],

    pub interrupt_flag:   u8,
    pub interrupt_enable: u8,

    boot_rom: Option<Vec<u8>>,
    boot_rom_enabled: bool,

    pub model: Model,

    // CGB double-speed
    pub double_speed: bool,
    speed_switch_armed: bool, // KEY1 bit 0 set
}

impl Bus {
    pub fn new(cart: Cartridge, boot_rom: Option<Vec<u8>>, skip_boot: bool, model: Model) -> Self {
        let boot_rom_enabled = boot_rom.is_some() && !skip_boot;
        Bus {
            ppu: Ppu::new(model),
            cart, timer: Timer::new(), joypad: Joypad::new(),
            apu: Apu::new(), serial: Serial::new(),
            oam_dma: OamDma::new(), hdma: Hdma::new(),
            wram: Box::new([[0; 0x1000]; 8]),
            wram_bank: 1,
            hram: [0; 0x7F],
            interrupt_flag: 0xE1, interrupt_enable: 0x00,
            boot_rom, boot_rom_enabled,
            model,
            double_speed: false,
            speed_switch_armed: false,
        }
    }

    // ─── Read ───────────────────────────────────────────────────────────────

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF if self.boot_rom_enabled => {
                self.boot_rom.as_ref().map(|b| b[addr as usize]).unwrap_or(0xFF)
            }
            0x0200..=0x08FF if self.boot_rom_enabled && self.model.is_cgb() => {
                // CGB boot ROM occupies 0x0000–0x00FF and 0x0200–0x08FF
                self.boot_rom.as_ref().map(|b| {
                    if (addr as usize) < b.len() { b[addr as usize] } else { 0xFF }
                }).unwrap_or(0xFF)
            }
            0x0000..=0x7FFF => self.cart.read(addr),
            0x8000..=0x9FFF => self.ppu.vram[self.ppu.vram_bank][(addr - 0x8000) as usize],
            0xA000..=0xBFFF => self.cart.read(addr),
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize],
            0xD000..=0xDFFF => self.wram[self.wram_bank][(addr - 0xD000) as usize],
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize],
            0xF000..=0xFDFF => self.wram[self.wram_bank][(addr - 0xF000) as usize],
            0xFE00..=0xFE9F => self.ppu.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0xFF,

            0xFF00          => self.joypad.read(),
            0xFF01..=0xFF02 => self.serial.read(addr),
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F          => self.interrupt_flag | 0xE0,

            0xFF10..=0xFF3F => self.apu.read(addr),

            0xFF40..=0xFF4B => self.ppu.read(addr),
            0xFF4C          => 0xFF,
            0xFF4D          => { // KEY1 — speed switch
                let armed = self.speed_switch_armed as u8;
                let speed = if self.double_speed { 0x80 } else { 0 };
                speed | armed | 0x7E
            }
            0xFF4F          => self.ppu.read(addr), // VBK
            0xFF50          => 0xFF,
            0xFF51..=0xFF55 => self.hdma.read_status(), // simplified
            0xFF68..=0xFF6B => self.ppu.read(addr),
            0xFF70          => self.wram_bank as u8 | 0xF8,

            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF          => self.interrupt_enable,
            _               => 0xFF,
        }
    }

    // ─── Write ──────────────────────────────────────────────────────────────

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.cart.write(addr, val),
            0x8000..=0x9FFF => self.ppu.vram[self.ppu.vram_bank][(addr - 0x8000) as usize] = val,
            0xA000..=0xBFFF => self.cart.write(addr, val),
            0xC000..=0xCFFF => self.wram[0][(addr - 0xC000) as usize] = val,
            0xD000..=0xDFFF => self.wram[self.wram_bank][(addr - 0xD000) as usize] = val,
            0xE000..=0xEFFF => self.wram[0][(addr - 0xE000) as usize] = val,
            0xF000..=0xFDFF => self.wram[self.wram_bank][(addr - 0xF000) as usize] = val,
            0xFE00..=0xFE9F => self.ppu.oam[(addr - 0xFE00) as usize] = val,
            0xFEA0..=0xFEFF => {}

            0xFF00          => self.joypad.write(val),
            0xFF01..=0xFF02 => self.serial.write(addr, val),
            0xFF04..=0xFF07 => self.timer.write(addr, val),
            0xFF0F          => self.interrupt_flag = val | 0xE0,

            0xFF10..=0xFF3F => self.apu.write(addr, val),

            0xFF46          => self.start_oam_dma(val),
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B => self.ppu.write(addr, val),

            0xFF4D => { // KEY1 — prepare speed switch
                if self.model.is_cgb() { self.speed_switch_armed = val & 0x01 != 0; }
            }
            0xFF4F          => self.ppu.write(addr, val), // VBK

            0xFF50 => { if val & 0x01 != 0 { self.boot_rom_enabled = false; } }

            0xFF51..=0xFF55 => {
                if self.model.is_cgb() {
                    self.hdma.write(addr, val);
                    // GDMA executes immediately when armed via FF55 with bit7=0
                    if addr == 0xFF55 { self.hdma_gdma(); }
                }
            }
            0xFF68..=0xFF6B => self.ppu.write(addr, val),

            0xFF70 => { // SVBK — WRAM bank
                if self.model.is_cgb() {
                    self.wram_bank = (val & 0x07) as usize;
                    if self.wram_bank == 0 { self.wram_bank = 1; }
                }
            }

            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF          => self.interrupt_enable = val,
            _               => {}
        }
    }

    // ─── OAM DMA ────────────────────────────────────────────────────────────

    fn start_oam_dma(&mut self, val: u8) {
        let src = (val as u16) << 8;
        for i in 0..160u16 {
            self.ppu.oam[i as usize] = self.read(src + i);
        }
        self.oam_dma.start(val);
    }

    // ─── HDMA (CGB) ─────────────────────────────────────────────────────────

    /// Execute one 16-byte HDMA chunk (called on H-Blank for HDMA mode)
    pub fn hdma_hblank_step(&mut self) {
        if !self.hdma.active || self.hdma.mode != HdmaMode::HDMA { return; }
        if let Some((src, dst, len)) = self.hdma.transfer_chunk() {
            for i in 0..len {
                let byte = self.read(src + i);
                self.write(dst + i, byte);
            }
        }
    }

    /// Execute full GDMA immediately (called right after write to FF55)
    pub fn hdma_gdma(&mut self) {
        if !self.hdma.active || self.hdma.mode != HdmaMode::GDMA { return; }
        while self.hdma.active {
            if let Some((src, dst, len)) = self.hdma.transfer_chunk() {
                for i in 0..len {
                    let byte = self.read(src + i);
                    self.write(dst + i, byte);
                }
            }
        }
    }

    // ─── Speed Switch ────────────────────────────────────────────────────────

    pub fn perform_speed_switch(&mut self) {
        if self.model.is_cgb() && self.speed_switch_armed {
            self.double_speed = !self.double_speed;
            self.speed_switch_armed = false;
            // Reset DIV on speed switch
            self.timer.div_counter = 0;
        }
    }

    // ─── 16-bit helpers ─────────────────────────────────────────────────────

    pub fn read16(&self, addr: u16) -> u16 {
        (self.read(addr) as u16) | ((self.read(addr.wrapping_add(1)) as u16) << 8)
    }

    pub fn write16(&mut self, addr: u16, val: u16) {
        self.write(addr, val as u8);
        self.write(addr.wrapping_add(1), (val >> 8) as u8);
    }

    // ─── Interrupts ─────────────────────────────────────────────────────────

    pub fn collect_interrupts(&mut self) {
        if self.ppu.vblank_irq  { self.interrupt_flag |= irq::VBLANK; self.ppu.vblank_irq  = false; }
        if self.ppu.stat_irq    { self.interrupt_flag |= irq::STAT;   self.ppu.stat_irq    = false; }
        if self.timer.interrupt_request  { self.interrupt_flag |= irq::TIMER;  self.timer.interrupt_request  = false; }
        if self.serial.interrupt_request { self.interrupt_flag |= irq::SERIAL; self.serial.interrupt_request = false; }
        if self.joypad.interrupt_request { self.interrupt_flag |= irq::JOYPAD; self.joypad.interrupt_request = false; }
    }

    pub fn pending_interrupt(&self) -> Option<u8> {
        let p = self.interrupt_flag & self.interrupt_enable & 0x1F;
        if p == 0 { return None; }
        for b in 0..5 { if p & (1 << b) != 0 { return Some(b); } }
        None
    }

    pub fn clear_interrupt(&mut self, bit: u8) { self.interrupt_flag &= !(1 << bit); }

    // ─── Master tick ────────────────────────────────────────────────────────

    pub fn tick(&mut self, t_cycles: u8) {
        self.timer.step(t_cycles);
        self.ppu.step(t_cycles);
        self.apu.step(t_cycles);

        // HDMA H-Blank trigger
        if self.ppu.hblank_flag && self.model.is_cgb() {
            self.hdma_hblank_step();
        }

        self.collect_interrupts();
    }
}
