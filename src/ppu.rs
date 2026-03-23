/// FerrisBoy PPU — DMG + CGB Pixel Processing Unit
///
/// CGB additions over DMG:
///  - 2 VRAM banks (VBK register 0xFF4F)
///  - 8 BG palettes × 4 colors × RGB555  (BCPS/BCPD 0xFF68/0xFF69)
///  - 8 OBJ palettes × 4 colors × RGB555 (OCPS/OCPD 0xFF6A/0xFF6B)
///  - Extended OAM attributes (bank, h/v flip, priority, palette)
///  - Window & BG tile attributes from VRAM bank 1

use crate::model::Model;

pub const SCREEN_W: usize = 160;
pub const SCREEN_H: usize = 144;
pub const FB_SIZE:  usize = SCREEN_W * SCREEN_H * 3;

const OAM_SCAN_DOTS:    u32 = 80;
const DRAWING_MIN_DOTS: u32 = 172;
const HBLANK_MAX_END:   u32 = 456;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PpuMode { HBlank = 0, VBlank = 1, OamScan = 2, Drawing = 3 }

#[derive(Clone, Copy, Default)]
struct Sprite {
    y: u8, x: u8, tile: u8, flags: u8,
}
impl Sprite {
    fn dmg_palette(&self) -> bool  { self.flags & 0x10 != 0 }
    fn vram_bank(&self)   -> usize { ((self.flags >> 3) & 1) as usize }
    fn cgb_palette(&self) -> usize { (self.flags & 0x07) as usize }
    fn x_flip(&self)      -> bool  { self.flags & 0x20 != 0 }
    fn y_flip(&self)      -> bool  { self.flags & 0x40 != 0 }
    fn priority(&self)    -> bool  { self.flags & 0x80 != 0 }
}

pub struct Ppu {
    pub model: Model,

    /// Two VRAM banks (bank 1 only used on CGB)
    pub vram: [[u8; 0x2000]; 2],
    pub vram_bank: usize,
    pub oam: [u8; 0xA0],

    // LCD control registers
    pub lcdc: u8,
    pub stat: u8,
    pub scy:  u8,
    pub scx:  u8,
    pub ly:   u8,
    pub lyc:  u8,
    pub bgp:  u8,   // DMG BG palette
    pub obp0: u8,   // DMG OBJ palette 0
    pub obp1: u8,   // DMG OBJ palette 1
    pub wy:   u8,
    pub wx:   u8,

    // CGB palette memory (8 palettes × 4 colors × 2 bytes = 64 bytes each)
    bg_palette_ram:  [u8; 64],
    obj_palette_ram: [u8; 64],
    bcps: u8,  // BG palette index (0xFF68)
    ocps: u8,  // OBJ palette index (0xFF6A)

    // PPU state machine
    mode: PpuMode,
    dot: u32,
    window_line: u8,
    window_triggered: bool,

    pub framebuffer: Box<[u8; FB_SIZE]>,
    scanline_bg_color: [u8; SCREEN_W],   // color index (0-3) for BG/WIN pixel
    scanline_bg_prio:  [bool; SCREEN_W], // CGB BG-to-OBJ priority bit

    pub vblank_irq: bool,
    pub stat_irq: bool,
    stat_line: bool,

    sprite_buffer: Vec<Sprite>,

    /// Set when H-Blank starts (used by HDMA)
    pub hblank_flag: bool,
}

impl Ppu {
    pub fn new(model: Model) -> Self {
        Ppu {
            model,
            vram: [[0; 0x2000]; 2],
            vram_bank: 0,
            oam: [0; 0xA0],
            lcdc: 0x91, stat: 0x85,
            scy: 0, scx: 0, ly: 0, lyc: 0,
            bgp: 0xFC, obp0: 0xFF, obp1: 0xFF,
            wy: 0, wx: 0,
            bg_palette_ram:  [0xFF; 64],
            obj_palette_ram: [0xFF; 64],
            bcps: 0, ocps: 0,
            mode: PpuMode::OamScan,
            dot: 0,
            window_line: 0,
            window_triggered: false,
            framebuffer: Box::new([0xFF; FB_SIZE]),
            scanline_bg_color: [0; SCREEN_W],
            scanline_bg_prio:  [false; SCREEN_W],
            vblank_irq: false,
            stat_irq: false,
            stat_line: false,
            sprite_buffer: Vec::with_capacity(10),
            hblank_flag: false,
        }
    }

    pub fn lcd_enabled(&self) -> bool { self.lcdc & 0x80 != 0 }
    pub fn mode(&self) -> PpuMode { self.mode }

    // ─── Step ───────────────────────────────────────────────────────────────

    pub fn step(&mut self, t_cycles: u8) {
        if !self.lcd_enabled() { return; }
        self.hblank_flag = false;
        for _ in 0..t_cycles { self.tick(); }
    }

    fn tick(&mut self) {
        self.dot += 1;
        match self.mode {
            PpuMode::OamScan => {
                if self.dot == 1 { self.oam_scan(); }
                if self.dot >= OAM_SCAN_DOTS { self.set_mode(PpuMode::Drawing); }
            }
            PpuMode::Drawing => {
                if self.dot >= OAM_SCAN_DOTS + DRAWING_MIN_DOTS {
                    self.render_scanline();
                    self.set_mode(PpuMode::HBlank);
                    self.hblank_flag = true;
                }
            }
            PpuMode::HBlank => {
                if self.dot >= HBLANK_MAX_END {
                    self.dot = 0;
                    self.ly += 1;
                    self.check_lyc();
                    if self.ly >= 144 {
                        self.set_mode(PpuMode::VBlank);
                        self.vblank_irq = true;
                        self.window_line = 0;
                        self.window_triggered = false;
                    } else {
                        self.set_mode(PpuMode::OamScan);
                    }
                }
            }
            PpuMode::VBlank => {
                if self.dot >= HBLANK_MAX_END {
                    self.dot = 0;
                    self.ly += 1;
                    self.check_lyc();
                    if self.ly > 153 {
                        self.ly = 0;
                        self.check_lyc();
                        self.set_mode(PpuMode::OamScan);
                    }
                }
            }
        }
    }

    fn set_mode(&mut self, mode: PpuMode) {
        self.mode = mode;
        self.stat = (self.stat & 0xFC) | mode as u8;
        self.update_stat_irq();
    }

    fn check_lyc(&mut self) {
        if self.ly == self.lyc { self.stat |= 0x04; } else { self.stat &= !0x04; }
        self.update_stat_irq();
    }

    fn update_stat_irq(&mut self) {
        let new = self.stat_condition();
        if new && !self.stat_line { self.stat_irq = true; }
        self.stat_line = new;
    }

    fn stat_condition(&self) -> bool {
        (self.stat & 0x40 != 0 && self.stat & 0x04 != 0) ||
        (self.stat & 0x20 != 0 && self.mode == PpuMode::OamScan) ||
        (self.stat & 0x10 != 0 && self.mode == PpuMode::VBlank)  ||
        (self.stat & 0x08 != 0 && self.mode == PpuMode::HBlank)
    }

    // ─── OAM Scan ───────────────────────────────────────────────────────────

    fn oam_scan(&mut self) {
        self.sprite_buffer.clear();
        let h: u8 = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        for i in 0..40usize {
            let b = i * 4;
            let sy = self.oam[b]; let sx = self.oam[b+1];
            let tile = self.oam[b+2]; let flags = self.oam[b+3];
            let ly16 = self.ly as u16 + 16;
            if sx > 0 && ly16 >= sy as u16 && ly16 < sy as u16 + h as u16 {
                self.sprite_buffer.push(Sprite { y: sy, x: sx, tile, flags });
                if self.sprite_buffer.len() == 10 { break; }
            }
        }
        // DMG: sort by X for priority. CGB: OAM order takes priority (stable sort by position)
        if self.model.is_dmg() {
            self.sprite_buffer.sort_by_key(|s| s.x);
        }
    }

    // ─── Scanline Render ────────────────────────────────────────────────────

    fn render_scanline(&mut self) {
        let ly = self.ly as usize;
        if ly >= SCREEN_H { return; }
        self.scanline_bg_color = [0; SCREEN_W];
        self.scanline_bg_prio  = [false; SCREEN_W];

        let bg_enabled = self.lcdc & 0x01 != 0;
        let win_enabled = self.lcdc & 0x20 != 0;
        // On CGB, BG/WIN master enable = 0 means BG/WIN always drawn as color 0 (not white)
        if bg_enabled || self.model.is_cgb() { self.render_bg(ly); }
        if win_enabled                        { self.render_window(ly); }
        if self.lcdc & 0x02 != 0             { self.render_sprites(ly); }
    }

    fn render_bg(&mut self, ly: usize) {
        let map_base: u16 = if self.lcdc & 0x08 != 0 { 0x1C00 } else { 0x1800 };
        let signed = self.lcdc & 0x10 == 0;
        let y = (ly as u8).wrapping_add(self.scy);
        for x in 0..SCREEN_W {
            let px = (x as u8).wrapping_add(self.scx);
            let (color_idx, rgb) = self.get_bg_pixel(px, y, map_base, signed, x);
            self.scanline_bg_color[x] = color_idx;
            self.set_pixel(x, ly, rgb);
        }
    }

    fn render_window(&mut self, ly: usize) {
        if self.wy > ly as u8 { return; }
        let wx = self.wx.saturating_sub(7);
        if wx >= 160 { return; }
        if ly as u8 == self.wy { self.window_triggered = true; }
        if !self.window_triggered { return; }
        let map_base: u16 = if self.lcdc & 0x40 != 0 { 0x1C00 } else { 0x1800 };
        let signed = self.lcdc & 0x10 == 0;
        let wy = self.window_line;
        for x in (wx as usize)..SCREEN_W {
            let px = (x - wx as usize) as u8;
            let (color_idx, rgb) = self.get_bg_pixel(px, wy, map_base, signed, x);
            self.scanline_bg_color[x] = color_idx;
            self.set_pixel(x, ly, rgb);
        }
        self.window_line += 1;
    }

    fn get_bg_pixel(&mut self, px: u8, py: u8, map_base: u16, signed: bool, screen_x: usize) -> (u8, (u8, u8, u8)) {
        let tx = (px / 8) as u16;
        let ty = (py / 8) as u16;
        let map_idx = ty * 32 + tx;

        // Tile number always from VRAM bank 0
        let tile_num = self.vram[0][(map_base + map_idx) as usize];

        // CGB: tile attributes from VRAM bank 1
        let attr = if self.model.is_cgb() { self.vram[1][(map_base + map_idx) as usize] } else { 0 };
        let cgb_palette = (attr & 0x07) as usize;
        let tile_bank   = if self.model.is_cgb() && attr & 0x08 != 0 { 1usize } else { 0usize };
        let h_flip      = attr & 0x20 != 0;
        let v_flip      = attr & 0x40 != 0;
        let bg_prio     = attr & 0x80 != 0; // BG has priority over sprites

        if self.model.is_cgb() { self.scanline_bg_prio[screen_x] = bg_prio; }

        let tile_addr: u16 = if signed {
            (0x1000i16 + (tile_num as i8 as i16) * 16) as u16
        } else {
            tile_num as u16 * 16
        };

        let row = if v_flip { 7 - (py % 8) } else { py % 8 };
        let line = row as u16 * 2;
        let lo = self.vram[tile_bank][(tile_addr + line) as usize];
        let hi = self.vram[tile_bank][(tile_addr + line + 1) as usize];
        let bit = if h_flip { px % 8 } else { 7 - (px % 8) };
        let color_idx = ((hi >> bit & 1) << 1) | (lo >> bit & 1);

        let rgb = if self.model.is_cgb() {
            self.cgb_bg_color(cgb_palette, color_idx as usize)
        } else {
            dmg_palette(self.bgp, color_idx)
        };
        (color_idx, rgb)
    }

    fn render_sprites(&mut self, ly: usize) {
        let h: u8 = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let sprites = self.sprite_buffer.clone();
        for sprite in sprites.iter().rev() {
            let sx = sprite.x as i32 - 8;
            let sy = sprite.y as i32 - 16;
            let row = ly as i32 - sy;
            let tile_row = if sprite.y_flip() { (h as i32 - 1 - row) as u8 } else { row as u8 };
            let tile = if h == 16 {
                if tile_row < 8 { sprite.tile & 0xFE } else { sprite.tile | 0x01 }
            } else { sprite.tile };

            let bank = if self.model.is_cgb() { sprite.vram_bank() } else { 0 };
            let tile_line = (tile_row % 8) as u16 * 2;
            let ta = tile as u16 * 16 + tile_line;
            let lo = self.vram[bank][ta as usize];
            let hi = self.vram[bank][(ta + 1) as usize];

            for bit_n in 0..8u8 {
                let screen_x = sx + if sprite.x_flip() { bit_n as i32 } else { 7 - bit_n as i32 };
                if screen_x < 0 || screen_x >= SCREEN_W as i32 { continue; }
                let sx_us = screen_x as usize;
                let color_idx = ((hi >> bit_n & 1) << 1) | (lo >> bit_n & 1);
                if color_idx == 0 { continue; } // transparent

                // Priority check
                if self.model.is_cgb() {
                    // CGB: BG master enable=0 → sprites always on top
                    if self.lcdc & 0x01 != 0 {
                        if self.scanline_bg_prio[sx_us] && self.scanline_bg_color[sx_us] != 0 { continue; }
                        if sprite.priority() && self.scanline_bg_color[sx_us] != 0 { continue; }
                    }
                } else {
                    if sprite.priority() && self.scanline_bg_color[sx_us] != 0 { continue; }
                }

                let rgb = if self.model.is_cgb() {
                    self.cgb_obj_color(sprite.cgb_palette(), color_idx as usize)
                } else {
                    let pal = if sprite.dmg_palette() { self.obp1 } else { self.obp0 };
                    dmg_palette(pal, color_idx)
                };
                self.set_pixel(sx_us, ly, rgb);
            }
        }
    }

    // ─── CGB Palette helpers ─────────────────────────────────────────────────

    fn cgb_bg_color(&self, palette: usize, color: usize) -> (u8, u8, u8) {
        let idx = palette * 8 + color * 2;
        decode_cgb_color(self.bg_palette_ram[idx], self.bg_palette_ram[idx + 1])
    }

    fn cgb_obj_color(&self, palette: usize, color: usize) -> (u8, u8, u8) {
        let idx = palette * 8 + color * 2;
        decode_cgb_color(self.obj_palette_ram[idx], self.obj_palette_ram[idx + 1])
    }

    fn set_pixel(&mut self, x: usize, y: usize, (r, g, b): (u8, u8, u8)) {
        let base = (y * SCREEN_W + x) * 3;
        self.framebuffer[base] = r; self.framebuffer[base+1] = g; self.framebuffer[base+2] = b;
    }

    // ─── Register read/write ─────────────────────────────────────────────────

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => self.stat | 0x80,
            0xFF42 => self.scy,  0xFF43 => self.scx,
            0xFF44 => self.ly,   0xFF45 => self.lyc,
            0xFF47 => self.bgp,  0xFF48 => self.obp0, 0xFF49 => self.obp1,
            0xFF4A => self.wy,   0xFF4B => self.wx,
            // CGB
            0xFF4F => self.vram_bank as u8 | 0xFE,
            0xFF68 => self.bcps,
            0xFF69 => {
                let idx = (self.bcps & 0x3F) as usize;
                self.bg_palette_ram[idx]
            }
            0xFF6A => self.ocps,
            0xFF6B => {
                let idx = (self.ocps & 0x3F) as usize;
                self.obj_palette_ram[idx]
            }
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF40 => {
                let was = self.lcd_enabled();
                self.lcdc = val;
                if was && !self.lcd_enabled() {
                    self.ly = 0; self.dot = 0;
                    self.mode = PpuMode::HBlank;
                    self.stat = (self.stat & 0xFC);
                    self.framebuffer.fill(0xFF);
                }
            }
            0xFF41 => self.stat = (self.stat & 0x07) | (val & 0x78),
            0xFF42 => self.scy  = val, 0xFF43 => self.scx = val,
            0xFF44 => {}
            0xFF45 => { self.lyc = val; self.check_lyc(); }
            0xFF47 => self.bgp  = val,
            0xFF48 => self.obp0 = val,
            0xFF49 => self.obp1 = val,
            0xFF4A => self.wy   = val,
            0xFF4B => self.wx   = val,
            // CGB VRAM bank
            0xFF4F => { if self.model.is_cgb() { self.vram_bank = (val & 0x01) as usize; } }
            // CGB BG palettes
            0xFF68 => self.bcps = val,
            0xFF69 => {
                let idx = (self.bcps & 0x3F) as usize;
                self.bg_palette_ram[idx] = val;
                if self.bcps & 0x80 != 0 { self.bcps = (self.bcps & 0x80) | ((self.bcps + 1) & 0x3F); }
            }
            // CGB OBJ palettes
            0xFF6A => self.ocps = val,
            0xFF6B => {
                let idx = (self.ocps & 0x3F) as usize;
                self.obj_palette_ram[idx] = val;
                if self.ocps & 0x80 != 0 { self.ocps = (self.ocps & 0x80) | ((self.ocps + 1) & 0x3F); }
            }
            _ => {}
        }
    }
}

/// Decode 15-bit CGB color (RGB555 LE) → RGB888
fn decode_cgb_color(lo: u8, hi: u8) -> (u8, u8, u8) {
    let rgb15 = (lo as u16) | ((hi as u16) << 8);
    let r5 = (rgb15 & 0x001F) as u8;
    let g5 = ((rgb15 >> 5) & 0x001F) as u8;
    let b5 = ((rgb15 >> 10) & 0x001F) as u8;
    // Scale 5-bit to 8-bit
    ((r5 << 3) | (r5 >> 2), (g5 << 3) | (g5 >> 2), (b5 << 3) | (b5 >> 2))
}

/// Classic DMG green-tinted palette
fn dmg_palette(palette: u8, idx: u8) -> (u8, u8, u8) {
    let shade = (palette >> (idx * 2)) & 0x03;
    match shade {
        0 => (0xE0, 0xF8, 0xD0),
        1 => (0x88, 0xC0, 0x70),
        2 => (0x34, 0x68, 0x56),
        3 => (0x08, 0x18, 0x20),
        _ => unreachable!(),
    }
}
