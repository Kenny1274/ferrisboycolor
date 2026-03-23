/// FerrisBoy Serial port stub
pub struct Serial { pub sb: u8, pub sc: u8, pub interrupt_request: bool }
impl Serial {
    pub fn new() -> Self { Serial { sb: 0, sc: 0, interrupt_request: false } }
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF01 => self.sb = val,
            0xFF02 => { self.sc = val; if val == 0x81 { print!("{}", self.sb as char); self.interrupt_request = true; self.sc = 0x01; } }
            _ => {}
        }
    }
    pub fn read(&self, addr: u16) -> u8 { match addr { 0xFF01 => self.sb, 0xFF02 => self.sc, _ => 0xFF } }
}
