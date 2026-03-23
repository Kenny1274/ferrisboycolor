/// FerrisBoy APU stub
pub struct Apu { regs: [u8; 0x30] }
impl Apu {
    pub fn new() -> Self { Apu { regs: [0; 0x30] } }
    pub fn read(&self, addr: u16) -> u8 { let i = (addr - 0xFF10) as usize; if i < self.regs.len() { self.regs[i] } else { 0xFF } }
    pub fn write(&mut self, addr: u16, val: u8) { let i = (addr - 0xFF10) as usize; if i < self.regs.len() { self.regs[i] = val; } }
    pub fn step(&mut self, _t: u8) {}
}
