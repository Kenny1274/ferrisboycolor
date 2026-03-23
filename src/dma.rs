/// FerrisBoy DMA — OAM DMA (DMG+CGB) and HDMA/GDMA (CGB only)

// ─── OAM DMA ────────────────────────────────────────────────────────────────

pub struct OamDma {
    pub active: bool,
    pub source: u16,
    pub byte_index: u8,
    delay: u8,
}

impl OamDma {
    pub fn new() -> Self { OamDma { active: false, source: 0, byte_index: 0, delay: 0 } }

    pub fn start(&mut self, val: u8) {
        self.source     = (val as u16) << 8;
        self.byte_index = 0;
        self.active     = true;
        self.delay      = 2;
    }

    /// Returns (src_addr, oam_byte_index) if a byte should be copied this cycle
    pub fn step(&mut self) -> Option<(u16, u8)> {
        if !self.active { return None; }
        if self.delay > 0 { self.delay -= 1; return None; }
        let src = self.source + self.byte_index as u16;
        let dst = self.byte_index;
        self.byte_index += 1;
        if self.byte_index >= 160 { self.active = false; }
        Some((src, dst))
    }
}

// ─── HDMA / GDMA (CGB only) ─────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HdmaMode {
    /// General Purpose DMA — transfers entire block immediately
    GDMA,
    /// H-Blank DMA — transfers 16 bytes each H-Blank
    HDMA,
}

pub struct Hdma {
    pub src: u16,
    pub dst: u16,
    pub length: u16,   // in bytes, always multiple of 16
    pub mode: HdmaMode,
    pub active: bool,
    pub bytes_remaining: u16,
}

impl Hdma {
    pub fn new() -> Self {
        Hdma { src: 0, dst: 0x8000, length: 0, mode: HdmaMode::GDMA, active: false, bytes_remaining: 0 }
    }

    /// Write to HDMA registers (0xFF51–0xFF55)
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF51 => self.src = (self.src & 0x00FF) | ((val as u16) << 8),
            0xFF52 => self.src = (self.src & 0xFF00) | (val as u16 & 0xF0),
            0xFF53 => self.dst = (self.dst & 0x00FF) | ((val as u16 & 0x1F) << 8) | 0x8000,
            0xFF54 => self.dst = (self.dst & 0xFF00) | (val as u16 & 0xF0),
            0xFF55 => {
                self.length = ((val as u16 & 0x7F) + 1) * 16;
                self.bytes_remaining = self.length;
                self.mode = if val & 0x80 != 0 { HdmaMode::HDMA } else { HdmaMode::GDMA };
                self.active = true;
            }
            _ => {}
        }
    }

    /// Read HDMA5 status
    pub fn read_status(&self) -> u8 {
        if !self.active { return 0xFF; }
        let blocks_remaining = (self.bytes_remaining / 16).saturating_sub(1) as u8;
        blocks_remaining & 0x7F
    }

    /// Transfer one 16-byte chunk. Returns true if more data remains.
    pub fn transfer_chunk(&mut self) -> Option<(u16, u16, u16)> {
        if !self.active || self.bytes_remaining == 0 { return None; }
        let chunk = 16.min(self.bytes_remaining);
        let src = self.src;
        let dst = self.dst;
        self.src += chunk;
        self.dst += chunk;
        self.bytes_remaining -= chunk;
        if self.bytes_remaining == 0 { self.active = false; }
        Some((src, dst, chunk))
    }
}
