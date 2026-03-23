/// FerrisBoy Joypad

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Button { A, B, Start, Select, Up, Down, Left, Right, None }

pub struct Joypad {
    select_action: bool,
    select_direction: bool,
    action_state: u8,
    direction_state: u8,
    pub interrupt_request: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad { select_action: false, select_direction: false, action_state: 0x0F, direction_state: 0x0F, interrupt_request: false }
    }

    pub fn write(&mut self, val: u8) {
        self.select_action    = val & 0x20 == 0;
        self.select_direction = val & 0x10 == 0;
    }

    pub fn read(&self) -> u8 {
        let mut result = 0xCF;
        if self.select_action    { result &= !0x20; result = (result & 0xF0) | (self.action_state    & 0x0F); }
        if self.select_direction { result &= !0x10; result = (result & 0xF0) | (self.direction_state & 0x0F); }
        result
    }

    pub fn press(&mut self, btn: Button) {
        if btn == Button::None { return; }
        let was = self.is_released(btn);
        self.set_state(btn, true);
        if was { self.interrupt_request = true; }
    }

    pub fn release(&mut self, btn: Button) {
        if btn == Button::None { return; }
        self.set_state(btn, false);
    }

    fn is_released(&self, btn: Button) -> bool {
        match btn {
            Button::A      => self.action_state    & 0x01 != 0,
            Button::B      => self.action_state    & 0x02 != 0,
            Button::Select => self.action_state    & 0x04 != 0,
            Button::Start  => self.action_state    & 0x08 != 0,
            Button::Right  => self.direction_state & 0x01 != 0,
            Button::Left   => self.direction_state & 0x02 != 0,
            Button::Up     => self.direction_state & 0x04 != 0,
            Button::Down   => self.direction_state & 0x08 != 0,
            Button::None   => true,
        }
    }

    fn set_state(&mut self, btn: Button, pressed: bool) {
        let bit = !pressed as u8;
        match btn {
            Button::A      => self.action_state    = (self.action_state    & !0x01) | (bit),
            Button::B      => self.action_state    = (self.action_state    & !0x02) | (bit << 1),
            Button::Select => self.action_state    = (self.action_state    & !0x04) | (bit << 2),
            Button::Start  => self.action_state    = (self.action_state    & !0x08) | (bit << 3),
            Button::Right  => self.direction_state = (self.direction_state & !0x01) | (bit),
            Button::Left   => self.direction_state = (self.direction_state & !0x02) | (bit << 1),
            Button::Up     => self.direction_state = (self.direction_state & !0x04) | (bit << 2),
            Button::Down   => self.direction_state = (self.direction_state & !0x08) | (bit << 3),
            Button::None   => {}
        }
    }
}
