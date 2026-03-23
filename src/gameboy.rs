/// FerrisBoy — Top-level Game Boy / Game Boy Color orchestrator
///
/// Auto-detects DMG vs CGB from the cartridge header (0x0143).
/// CGB runs at 8.388608 MHz in double-speed mode (via STOP + KEY1),
/// but the frame rate stays at ~59.73 fps (70224 base T-cycles/frame).

use crate::bus::Bus;
use crate::cartridge::{Cartridge, CgbFlag};
use crate::cpu::Cpu;
use crate::joypad::Button;
use crate::model::Model;

/// T-cycles per frame at single speed (4.194304 MHz / 59.7275 fps)
const CYCLES_PER_FRAME: u32 = 70224;

pub struct GameBoy {
    pub cpu: Cpu,
    pub bus: Bus,
    pub model: Model,
}

impl GameBoy {
    pub fn new(rom: Vec<u8>, boot_rom: Option<Vec<u8>>, skip_boot: bool, force_dmg: bool) -> Self {
        let cart = Cartridge::new(rom);

        // Determine model: if cart has CGB flag and user hasn't forced DMG, use CGB
        let model = if !force_dmg && cart.cgb_flag != CgbFlag::DMGOnly {
            Model::CGB
        } else {
            Model::DMG
        };

        log::info!("Running in {} mode", if model.is_cgb() { "CGB" } else { "DMG" });

        let bus = Bus::new(cart, boot_rom, skip_boot, model);
        let mut cpu = Cpu::new(skip_boot);

        // If skipping boot ROM, set CGB-appropriate initial register state
        if skip_boot && model.is_cgb() {
            cpu.cgb_init();
        }

        GameBoy { cpu, bus, model }
    }

    /// Run exactly one video frame
    pub fn run_frame(&mut self) {
        let mut cycles = 0u32;
        while cycles < CYCLES_PER_FRAME {
            let tc = self.step();
            cycles += tc as u32;
        }
    }

    fn step(&mut self) -> u8 {
        self.cpu.handle_interrupts(&mut self.bus);
        self.cpu.step(&mut self.bus);

        let mut t = self.cpu.t_cycles;

        // In double-speed mode, CPU ticks at 2× but PPU/Timer still at 1×
        // We halve the T-cycles passed to peripherals
        let peripheral_t = if self.bus.double_speed { t / 2 } else { t };

        self.bus.tick(peripheral_t);

        // Return canonical T-cycles for frame counting (always single-speed units)
        if self.bus.double_speed { t / 2 } else { t }
    }

    // ─── Public API ─────────────────────────────────────────────────────────

    pub fn framebuffer(&self) -> &[u8] {
        self.bus.ppu.framebuffer.as_ref()
    }

    pub fn cart_title(&self) -> String {
        self.bus.cart.title()
    }

    pub fn mbc_type(&self) -> String {
        self.bus.cart.mbc.to_string()
    }

    pub fn model_name(&self) -> &'static str {
        match self.model {
            Model::CGB => "Game Boy Color",
            Model::DMG => "Game Boy",
        }
    }

    pub fn key_down(&mut self, btn: Button) { self.bus.joypad.press(btn); }
    pub fn key_up  (&mut self, btn: Button) { self.bus.joypad.release(btn); }
}
