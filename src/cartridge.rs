/// FerrisBoy Cartridge — ROM/RAM banking for DMG & CGB
/// Supports: ROM-only, MBC1, MBC2, MBC3 (RTC stub), MBC5

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MbcType {
    RomOnly,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
    Unknown(u8),
}

impl std::fmt::Display for MbcType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MbcType::RomOnly    => write!(f, "ROM Only"),
            MbcType::Mbc1       => write!(f, "MBC1"),
            MbcType::Mbc2       => write!(f, "MBC2"),
            MbcType::Mbc3       => write!(f, "MBC3"),
            MbcType::Mbc5       => write!(f, "MBC5"),
            MbcType::Unknown(n) => write!(f, "Unknown({:#04x})", n),
        }
    }
}

/// CGB compatibility flag from cartridge header 0x0143
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CgbFlag {
    DMGOnly,
    CGBCompatible, // works on both DMG and CGB
    CGBOnly,
}

pub struct Cartridge {
    rom: Vec<u8>,
    ram: Vec<u8>,
    pub mbc: MbcType,
    pub cgb_flag: CgbFlag,

    rom_bank: usize,
    ram_bank: usize,
    ram_enabled: bool,

    // MBC1
    mbc1_mode: bool,

    // MBC3 RTC stub
    rtc_selected: bool,
    rtc_reg: u8,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Self {
        let mbc_byte = rom[0x0147];
        let mbc = match mbc_byte {
            0x00               => MbcType::RomOnly,
            0x01..=0x03        => MbcType::Mbc1,
            0x05..=0x06        => MbcType::Mbc2,
            0x0F..=0x13        => MbcType::Mbc3,
            0x19..=0x1E        => MbcType::Mbc5,
            n                  => MbcType::Unknown(n),
        };

        let cgb_flag = match rom[0x0143] {
            0x80 => CgbFlag::CGBCompatible,
            0xC0 => CgbFlag::CGBOnly,
            _    => CgbFlag::DMGOnly,
        };

        let ram_size = match rom[0x0149] {
            0x00 => if mbc == MbcType::Mbc2 { 512 } else { 0 },
            0x01 => 2   * 1024,
            0x02 => 8   * 1024,
            0x03 => 32  * 1024,
            0x04 => 128 * 1024,
            0x05 => 64  * 1024,
            _    => 0,
        };

        Cartridge {
            rom,
            ram: vec![0; ram_size],
            mbc,
            cgb_flag,
            rom_bank: 1,
            ram_bank: 0,
            ram_enabled: false,
            mbc1_mode: false,
            rtc_selected: false,
            rtc_reg: 0,
        }
    }

    pub fn title(&self) -> String {
        let end = if self.cgb_flag != CgbFlag::DMGOnly { 0x013F } else { 0x0144 };
        let bytes = &self.rom[0x0134..end];
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).trim().to_string()
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => self.rom_read(0, addr as usize),
            0x4000..=0x7FFF => {
                let bank = self.effective_rom_bank();
                self.rom_read(bank, (addr - 0x4000) as usize)
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled { return 0xFF; }
                if self.mbc == MbcType::Mbc3 && self.rtc_selected { return self.rtc_reg; }
                let offset = self.ram_bank * 0x2000 + (addr - 0xA000) as usize;
                if offset < self.ram.len() { self.ram[offset] } else { 0xFF }
            }
            _ => 0xFF,
        }
    }

    fn rom_read(&self, bank: usize, offset: usize) -> u8 {
        let idx = bank * 0x4000 + offset;
        if idx < self.rom.len() { self.rom[idx] } else { 0xFF }
    }

    fn effective_rom_bank(&self) -> usize {
        match self.mbc {
            MbcType::RomOnly => 1,
            MbcType::Mbc1 => {
                let b = if self.mbc1_mode { self.rom_bank & 0x1F } else { self.rom_bank & 0x7F };
                if b == 0 { 1 } else { b }
            }
            MbcType::Mbc2 => { let b = self.rom_bank & 0x0F; if b == 0 { 1 } else { b } }
            MbcType::Mbc3 => { let b = self.rom_bank & 0x7F; if b == 0 { 1 } else { b } }
            MbcType::Mbc5 => self.rom_bank & 0x1FF,
            MbcType::Unknown(_) => self.rom_bank,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match self.mbc {
            MbcType::RomOnly    => {}
            MbcType::Mbc1       => self.mbc1_write(addr, val),
            MbcType::Mbc2       => self.mbc2_write(addr, val),
            MbcType::Mbc3       => self.mbc3_write(addr, val),
            MbcType::Mbc5       => self.mbc5_write(addr, val),
            MbcType::Unknown(_) => {}
        }
        if (0xA000..=0xBFFF).contains(&addr) && self.ram_enabled {
            if self.mbc == MbcType::Mbc3 && self.rtc_selected { self.rtc_reg = val; return; }
            let offset = self.ram_bank * 0x2000 + (addr - 0xA000) as usize;
            if offset < self.ram.len() { self.ram[offset] = val; }
        }
    }

    fn mbc1_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
            0x2000..=0x3FFF => {
                let lo = (val & 0x1F) as usize;
                self.rom_bank = (self.rom_bank & 0x60) | if lo == 0 { 1 } else { lo };
            }
            0x4000..=0x5FFF => {
                let hi = (val & 0x03) as usize;
                if self.mbc1_mode { self.ram_bank = hi; }
                else { self.rom_bank = (self.rom_bank & 0x1F) | (hi << 5); }
            }
            0x6000..=0x7FFF => self.mbc1_mode = val & 0x01 != 0,
            _ => {}
        }
    }

    fn mbc2_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => { if addr & 0x0100 == 0 { self.ram_enabled = (val & 0x0F) == 0x0A; } }
            0x2000..=0x3FFF => {
                if addr & 0x0100 != 0 {
                    let b = (val & 0x0F) as usize;
                    self.rom_bank = if b == 0 { 1 } else { b };
                }
            }
            _ => {}
        }
    }

    fn mbc3_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
            0x2000..=0x3FFF => { let b = (val & 0x7F) as usize; self.rom_bank = if b == 0 { 1 } else { b }; }
            0x4000..=0x5FFF => {
                if val <= 0x03 { self.ram_bank = val as usize; self.rtc_selected = false; }
                else if (0x08..=0x0C).contains(&val) { self.rtc_selected = true; self.rtc_reg = val; }
            }
            0x6000..=0x7FFF => {}
            _ => {}
        }
    }

    fn mbc5_write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = (val & 0x0F) == 0x0A,
            0x2000..=0x2FFF => self.rom_bank = (self.rom_bank & 0x100) | val as usize,
            0x3000..=0x3FFF => self.rom_bank = (self.rom_bank & 0x0FF) | (((val & 0x01) as usize) << 8),
            0x4000..=0x5FFF => self.ram_bank = (val & 0x0F) as usize,
            _ => {}
        }
    }
}
