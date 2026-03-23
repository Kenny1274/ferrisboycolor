/// FerrisBoy Timer (DIV, TIMA, TMA, TAC)

pub struct Timer {
    pub div_counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    overflow_pending: bool,
    pub interrupt_request: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer { div_counter: 0xABCC, tima: 0, tma: 0, tac: 0, overflow_pending: false, interrupt_request: false }
    }

    pub fn step(&mut self, t_cycles: u8) {
        if self.overflow_pending {
            self.overflow_pending = false;
            self.tima = self.tma;
            self.interrupt_request = true;
        }
        for _ in 0..t_cycles {
            let old = self.div_counter;
            self.div_counter = self.div_counter.wrapping_add(1);
            if self.timer_enabled() {
                let bit = self.tima_bit();
                if old & bit != 0 && self.div_counter & bit == 0 {
                    self.tima = self.tima.wrapping_add(1);
                    if self.tima == 0 { self.overflow_pending = true; }
                }
            }
        }
    }

    fn timer_enabled(&self) -> bool { self.tac & 0x04 != 0 }

    fn tima_bit(&self) -> u16 {
        match self.tac & 0x03 { 0 => 1 << 9, 1 => 1 << 3, 2 => 1 << 5, 3 => 1 << 7, _ => unreachable!() }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div_counter >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => self.tac | 0xF8,
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF04 => self.div_counter = 0,
            0xFF05 => { if self.overflow_pending { self.overflow_pending = false; } self.tima = val; }
            0xFF06 => { self.tma = val; if self.overflow_pending { self.tima = val; } }
            0xFF07 => {
                let old_en  = self.timer_enabled();
                let old_bit = self.tima_bit();
                self.tac = val & 0x07;
                if old_en && self.div_counter & old_bit != 0 {
                    if !self.timer_enabled() || self.div_counter & self.tima_bit() == 0 {
                        self.tima = self.tima.wrapping_add(1);
                        if self.tima == 0 { self.overflow_pending = true; }
                    }
                }
            }
            _ => {}
        }
    }
}
