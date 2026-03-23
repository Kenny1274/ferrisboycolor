/// FerrisBoy CPU — Sharp LR35902 (DMG + CGB)
///
/// CGB addition: STOP instruction performs double-speed switch when KEY1 is armed

use crate::bus::Bus;

const FLAG_Z: u8 = 0x80;
const FLAG_N: u8 = 0x40;
const FLAG_H: u8 = 0x20;
const FLAG_C: u8 = 0x10;

pub struct Cpu {
    pub a: u8, pub f: u8,
    pub b: u8, pub c: u8,
    pub d: u8, pub e: u8,
    pub h: u8, pub l: u8,
    pub sp: u16,
    pub pc: u16,

    pub ime: bool,
    ime_pending: bool,
    pub halted: bool,
    pub stopped: bool,
    halt_bug: bool,

    pub t_cycles: u8,
}

impl Cpu {
    pub fn new(skip_boot: bool) -> Self {
        if skip_boot {
            Cpu {
                a: 0x01, f: 0xB0,
                b: 0x00, c: 0x13,
                d: 0x00, e: 0xD8,
                h: 0x01, l: 0x4D,
                sp: 0xFFFE, pc: 0x0100,
                ime: false, ime_pending: false,
                halted: false, stopped: false, halt_bug: false,
                t_cycles: 0,
            }
        } else {
            Cpu {
                a: 0, f: 0, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
                sp: 0, pc: 0,
                ime: false, ime_pending: false,
                halted: false, stopped: false, halt_bug: false,
                t_cycles: 0,
            }
        }
    }

    pub fn cgb_init(&mut self) {
        // CGB post-boot register state
        self.a = 0x11; self.f = 0x80;
        self.b = 0x00; self.c = 0x00;
        self.d = 0xFF; self.e = 0x56;
        self.h = 0x00; self.l = 0x0D;
        self.sp = 0xFFFE; self.pc = 0x0100;
    }

    fn af(&self) -> u16 { ((self.a as u16) << 8) | self.f as u16 }
    fn bc(&self) -> u16 { ((self.b as u16) << 8) | self.c as u16 }
    fn de(&self) -> u16 { ((self.d as u16) << 8) | self.e as u16 }
    fn hl(&self) -> u16 { ((self.h as u16) << 8) | self.l as u16 }

    fn set_af(&mut self, v: u16) { self.a = (v >> 8) as u8; self.f = (v & 0xF0) as u8; }
    fn set_bc(&mut self, v: u16) { self.b = (v >> 8) as u8; self.c = v as u8; }
    fn set_de(&mut self, v: u16) { self.d = (v >> 8) as u8; self.e = v as u8; }
    fn set_hl(&mut self, v: u16) { self.h = (v >> 8) as u8; self.l = v as u8; }

    fn flag(&self, f: u8) -> bool { self.f & f != 0 }
    fn set_flag(&mut self, f: u8, on: bool) { if on { self.f |= f; } else { self.f &= !f; } }

    fn fetch(&mut self, bus: &mut Bus) -> u8 {
        let v = bus.read(self.pc);
        if self.halt_bug { self.halt_bug = false; } else { self.pc = self.pc.wrapping_add(1); }
        v
    }

    fn fetch16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        (hi << 8) | lo
    }

    fn push(&mut self, bus: &mut Bus, v: u16) { self.sp = self.sp.wrapping_sub(2); bus.write16(self.sp, v); }
    fn pop(&mut self, bus: &mut Bus) -> u16 { let v = bus.read16(self.sp); self.sp = self.sp.wrapping_add(2); v }

    pub fn handle_interrupts(&mut self, bus: &mut Bus) -> bool {
        let pending = bus.interrupt_flag & bus.interrupt_enable & 0x1F;
        if self.halted && pending != 0 { self.halted = false; }
        if !self.ime { return false; }
        if let Some(bit) = bus.pending_interrupt() {
            self.ime = false;
            bus.clear_interrupt(bit);
            self.push(bus, self.pc);
            self.pc = 0x0040 + bit as u16 * 8;
            self.t_cycles = 20;
            return true;
        }
        false
    }

    pub fn step(&mut self, bus: &mut Bus) {
        if self.ime_pending { self.ime_pending = false; self.ime = true; }
        if self.stopped || self.halted { self.t_cycles = 4; return; }
        let op = self.fetch(bus);
        self.execute(bus, op);
    }

    fn execute(&mut self, bus: &mut Bus, op: u8) {
        self.t_cycles = CYCLE_TABLE[op as usize];
        match op {
            0x00 => {}
            0x01 => { let v = self.fetch16(bus); self.set_bc(v); }
            0x11 => { let v = self.fetch16(bus); self.set_de(v); }
            0x21 => { let v = self.fetch16(bus); self.set_hl(v); }
            0x31 => { self.sp = self.fetch16(bus); }
            0x02 => bus.write(self.bc(), self.a),
            0x12 => bus.write(self.de(), self.a),
            0x22 => { bus.write(self.hl(), self.a); let v = self.hl().wrapping_add(1); self.set_hl(v); }
            0x32 => { bus.write(self.hl(), self.a); let v = self.hl().wrapping_sub(1); self.set_hl(v); }
            0x03 => { let v = self.bc().wrapping_add(1); self.set_bc(v); }
            0x13 => { let v = self.de().wrapping_add(1); self.set_de(v); }
            0x23 => { let v = self.hl().wrapping_add(1); self.set_hl(v); }
            0x33 => self.sp = self.sp.wrapping_add(1),
            0x04 => self.b = self.inc8(self.b),
            0x0C => self.c = self.inc8(self.c),
            0x14 => self.d = self.inc8(self.d),
            0x1C => self.e = self.inc8(self.e),
            0x24 => self.h = self.inc8(self.h),
            0x2C => self.l = self.inc8(self.l),
            0x34 => { let v = bus.read(self.hl()); let r = self.inc8(v); bus.write(self.hl(), r); }
            0x3C => self.a = self.inc8(self.a),
            0x05 => self.b = self.dec8(self.b),
            0x0D => self.c = self.dec8(self.c),
            0x15 => self.d = self.dec8(self.d),
            0x1D => self.e = self.dec8(self.e),
            0x25 => self.h = self.dec8(self.h),
            0x2D => self.l = self.dec8(self.l),
            0x35 => { let v = bus.read(self.hl()); let r = self.dec8(v); bus.write(self.hl(), r); }
            0x3D => self.a = self.dec8(self.a),
            0x06 => { self.b = self.fetch(bus); }
            0x0E => { self.c = self.fetch(bus); }
            0x16 => { self.d = self.fetch(bus); }
            0x1E => { self.e = self.fetch(bus); }
            0x26 => { self.h = self.fetch(bus); }
            0x2E => { self.l = self.fetch(bus); }
            0x36 => { let v = self.fetch(bus); bus.write(self.hl(), v); }
            0x3E => { self.a = self.fetch(bus); }
            0x07 => { let c = self.a >> 7; self.a = (self.a << 1)|c; self.f = if c!=0{FLAG_C}else{0}; }
            0x17 => { let c = self.a >> 7; self.a = (self.a << 1)|(self.flag(FLAG_C) as u8); self.f = if c!=0{FLAG_C}else{0}; }
            0x0F => { let c = self.a & 1; self.a = (self.a >> 1)|(c<<7); self.f = if c!=0{FLAG_C}else{0}; }
            0x1F => { let c = self.a & 1; self.a = (self.a >> 1)|((self.flag(FLAG_C) as u8)<<7); self.f = if c!=0{FLAG_C}else{0}; }
            0x08 => { let a = self.fetch16(bus); bus.write16(a, self.sp); }
            0x09 => { let v = self.add_hl(self.bc()); self.set_hl(v); }
            0x19 => { let v = self.add_hl(self.de()); self.set_hl(v); }
            0x29 => { let v = self.add_hl(self.hl()); self.set_hl(v); }
            0x39 => { let v = self.add_hl(self.sp); self.set_hl(v); }
            0x0A => self.a = bus.read(self.bc()),
            0x1A => self.a = bus.read(self.de()),
            0x2A => { self.a = bus.read(self.hl()); let v = self.hl().wrapping_add(1); self.set_hl(v); }
            0x3A => { self.a = bus.read(self.hl()); let v = self.hl().wrapping_sub(1); self.set_hl(v); }
            0x0B => { let v = self.bc().wrapping_sub(1); self.set_bc(v); }
            0x1B => { let v = self.de().wrapping_sub(1); self.set_de(v); }
            0x2B => { let v = self.hl().wrapping_sub(1); self.set_hl(v); }
            0x3B => self.sp = self.sp.wrapping_sub(1),
            0x10 => {
                // STOP — on CGB, may trigger speed switch
                self.fetch(bus);
                if bus.speed_switch_armed {
                    bus.perform_speed_switch();
                } else {
                    self.stopped = true;
                }
            }
            0x18 => { let e = self.fetch(bus) as i8; self.jr(e); }
            0x20 => { let e = self.fetch(bus) as i8; if !self.flag(FLAG_Z) { self.jr(e); self.t_cycles += 4; } }
            0x28 => { let e = self.fetch(bus) as i8; if  self.flag(FLAG_Z) { self.jr(e); self.t_cycles += 4; } }
            0x30 => { let e = self.fetch(bus) as i8; if !self.flag(FLAG_C) { self.jr(e); self.t_cycles += 4; } }
            0x38 => { let e = self.fetch(bus) as i8; if  self.flag(FLAG_C) { self.jr(e); self.t_cycles += 4; } }
            0x27 => self.daa(),
            0x2F => { self.a = !self.a; self.f |= FLAG_N | FLAG_H; }
            0x37 => { self.f = (self.f & FLAG_Z) | FLAG_C; }
            0x3F => { let c = !self.flag(FLAG_C); self.f = self.f & FLAG_Z; self.set_flag(FLAG_C, c); }
            0x76 => {
                if !self.ime && (bus.interrupt_flag & bus.interrupt_enable & 0x1F) != 0 {
                    self.halt_bug = true;
                } else { self.halted = true; }
            }
            0x40..=0x7F => { let s = self.reg8(bus, op & 0x07); self.set_reg8(bus, (op >> 3) & 0x07, s); }
            0x80..=0xBF => { let o = self.reg8(bus, op & 0x07); self.alu(op >> 3 & 0x07, o); }
            0xC6 => { let v = self.fetch(bus); self.alu(0, v); }
            0xCE => { let v = self.fetch(bus); self.alu(1, v); }
            0xD6 => { let v = self.fetch(bus); self.alu(2, v); }
            0xDE => { let v = self.fetch(bus); self.alu(3, v); }
            0xE6 => { let v = self.fetch(bus); self.alu(4, v); }
            0xEE => { let v = self.fetch(bus); self.alu(5, v); }
            0xF6 => { let v = self.fetch(bus); self.alu(6, v); }
            0xFE => { let v = self.fetch(bus); self.alu(7, v); }
            0xC1 => { let v = self.pop(bus); self.set_bc(v); }
            0xD1 => { let v = self.pop(bus); self.set_de(v); }
            0xE1 => { let v = self.pop(bus); self.set_hl(v); }
            0xF1 => { let v = self.pop(bus); self.set_af(v); }
            0xC5 => self.push(bus, self.bc()),
            0xD5 => self.push(bus, self.de()),
            0xE5 => self.push(bus, self.hl()),
            0xF5 => self.push(bus, self.af()),
            0xC0 => { if !self.flag(FLAG_Z) { self.pc = self.pop(bus); self.t_cycles += 12; } }
            0xC8 => { if  self.flag(FLAG_Z) { self.pc = self.pop(bus); self.t_cycles += 12; } }
            0xD0 => { if !self.flag(FLAG_C) { self.pc = self.pop(bus); self.t_cycles += 12; } }
            0xD8 => { if  self.flag(FLAG_C) { self.pc = self.pop(bus); self.t_cycles += 12; } }
            0xC9 => { self.pc = self.pop(bus); }
            0xD9 => { self.pc = self.pop(bus); self.ime = true; }
            0xC2 => { let a = self.fetch16(bus); if !self.flag(FLAG_Z) { self.pc = a; self.t_cycles += 4; } }
            0xCA => { let a = self.fetch16(bus); if  self.flag(FLAG_Z) { self.pc = a; self.t_cycles += 4; } }
            0xD2 => { let a = self.fetch16(bus); if !self.flag(FLAG_C) { self.pc = a; self.t_cycles += 4; } }
            0xDA => { let a = self.fetch16(bus); if  self.flag(FLAG_C) { self.pc = a; self.t_cycles += 4; } }
            0xC3 => { self.pc = self.fetch16(bus); }
            0xE9 => { self.pc = self.hl(); }
            0xC4 => { let a = self.fetch16(bus); if !self.flag(FLAG_Z) { self.push(bus, self.pc); self.pc = a; self.t_cycles += 12; } }
            0xCC => { let a = self.fetch16(bus); if  self.flag(FLAG_Z) { self.push(bus, self.pc); self.pc = a; self.t_cycles += 12; } }
            0xD4 => { let a = self.fetch16(bus); if !self.flag(FLAG_C) { self.push(bus, self.pc); self.pc = a; self.t_cycles += 12; } }
            0xDC => { let a = self.fetch16(bus); if  self.flag(FLAG_C) { self.push(bus, self.pc); self.pc = a; self.t_cycles += 12; } }
            0xCD => { let a = self.fetch16(bus); self.push(bus, self.pc); self.pc = a; }
            0xC7 => { self.push(bus, self.pc); self.pc = 0x0000; }
            0xCF => { self.push(bus, self.pc); self.pc = 0x0008; }
            0xD7 => { self.push(bus, self.pc); self.pc = 0x0010; }
            0xDF => { self.push(bus, self.pc); self.pc = 0x0018; }
            0xE7 => { self.push(bus, self.pc); self.pc = 0x0020; }
            0xEF => { self.push(bus, self.pc); self.pc = 0x0028; }
            0xF7 => { self.push(bus, self.pc); self.pc = 0x0030; }
            0xFF => { self.push(bus, self.pc); self.pc = 0x0038; }
            0xE0 => { let o = self.fetch(bus); bus.write(0xFF00 | o as u16, self.a); }
            0xF0 => { let o = self.fetch(bus); self.a = bus.read(0xFF00 | o as u16); }
            0xE2 => bus.write(0xFF00 | self.c as u16, self.a),
            0xF2 => self.a = bus.read(0xFF00 | self.c as u16),
            0xEA => { let a = self.fetch16(bus); bus.write(a, self.a); }
            0xFA => { let a = self.fetch16(bus); self.a = bus.read(a); }
            0xE8 => {
                let s = self.fetch(bus) as i8 as i32;
                let sp = self.sp as i32;
                let r = sp.wrapping_add(s);
                self.f = 0;
                self.set_flag(FLAG_H, (sp ^ s ^ r) & 0x10 != 0);
                self.set_flag(FLAG_C, (sp ^ s ^ r) & 0x100 != 0);
                self.sp = r as u16;
            }
            0xF8 => {
                let s = self.fetch(bus) as i8 as i32;
                let sp = self.sp as i32;
                let r = sp.wrapping_add(s);
                self.f = 0;
                self.set_flag(FLAG_H, (sp ^ s ^ r) & 0x10 != 0);
                self.set_flag(FLAG_C, (sp ^ s ^ r) & 0x100 != 0);
                self.set_hl(r as u16);
            }
            0xF9 => self.sp = self.hl(),
            0xF3 => { self.ime = false; self.ime_pending = false; }
            0xFB => { self.ime_pending = true; }
            0xCB => { let cb = self.fetch(bus); self.execute_cb(bus, cb); }
            _ => { log::warn!("Unknown opcode {:#04x} @ {:#06x}", op, self.pc.wrapping_sub(1)); }
        }
    }

    fn execute_cb(&mut self, bus: &mut Bus, op: u8) {
        self.t_cycles = if op & 0x07 == 6 { 16 } else { 8 };
        let ri = op & 0x07;
        let bit = (op >> 3) & 0x07;
        let grp = op >> 6;
        let mut v = self.reg8(bus, ri);
        v = match grp {
            0 => match bit {
                0 => self.rlc(v), 1 => self.rrc(v), 2 => self.rl(v), 3 => self.rr(v),
                4 => self.sla(v), 5 => self.sra(v), 6 => self.swap(v), 7 => self.srl(v),
                _ => unreachable!()
            },
            1 => { self.set_flag(FLAG_Z, v & (1<<bit)==0); self.set_flag(FLAG_N,false); self.set_flag(FLAG_H,true); v }
            2 => v & !(1<<bit),
            3 => v |  (1<<bit),
            _ => unreachable!()
        };
        if grp != 1 { self.set_reg8(bus, ri, v); }
    }

    fn reg8(&self, bus: &Bus, i: u8) -> u8 {
        match i { 0=>self.b, 1=>self.c, 2=>self.d, 3=>self.e, 4=>self.h, 5=>self.l, 6=>bus.read(self.hl()), 7=>self.a, _=>unreachable!() }
    }
    fn set_reg8(&mut self, bus: &mut Bus, i: u8, v: u8) {
        match i { 0=>{self.b=v} 1=>{self.c=v} 2=>{self.d=v} 3=>{self.e=v} 4=>{self.h=v} 5=>{self.l=v} 6=>bus.write(self.hl(),v), 7=>{self.a=v} _=>unreachable!() }
    }

    fn alu(&mut self, op: u8, operand: u8) {
        match op {
            0 => self.add_a(operand, false),
            1 => self.add_a(operand, self.flag(FLAG_C)),
            2 => self.sub_a(operand, false),
            3 => self.sub_a(operand, self.flag(FLAG_C)),
            4 => { let r = self.a & operand; self.f = FLAG_H | if r==0{FLAG_Z}else{0}; self.a=r; }
            5 => { let r = self.a ^ operand; self.f = if r==0{FLAG_Z}else{0}; self.a=r; }
            6 => { let r = self.a | operand; self.f = if r==0{FLAG_Z}else{0}; self.a=r; }
            7 => { let s = self.a; self.sub_a(operand, false); self.a = s; } // CP
            _ => unreachable!()
        }
    }

    fn add_a(&mut self, v: u8, c: bool) {
        let c = c as u8;
        let r = self.a as u16 + v as u16 + c as u16;
        self.set_flag(FLAG_Z, r as u8 == 0);
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (self.a & 0xF) + (v & 0xF) + c > 0xF);
        self.set_flag(FLAG_C, r > 0xFF);
        self.a = r as u8;
    }

    fn sub_a(&mut self, v: u8, b: bool) {
        let b = b as u8;
        let r = self.a as i16 - v as i16 - b as i16;
        self.set_flag(FLAG_Z, r as u8 == 0);
        self.set_flag(FLAG_N, true);
        self.set_flag(FLAG_H, (self.a & 0xF) < (v & 0xF) + b);
        self.set_flag(FLAG_C, r < 0);
        self.a = r as u8;
    }

    fn inc8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        self.set_flag(FLAG_Z, r==0); self.set_flag(FLAG_N, false); self.set_flag(FLAG_H, v&0xF==0xF); r
    }
    fn dec8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        self.set_flag(FLAG_Z, r==0); self.set_flag(FLAG_N, true); self.set_flag(FLAG_H, v&0xF==0); r
    }

    fn add_hl(&mut self, v: u16) -> u16 {
        let hl = self.hl();
        let r = hl as u32 + v as u32;
        self.set_flag(FLAG_N, false);
        self.set_flag(FLAG_H, (hl^v^r as u16) & 0x1000 != 0);
        self.set_flag(FLAG_C, r > 0xFFFF);
        r as u16
    }

    fn jr(&mut self, o: i8) { self.pc = (self.pc as i32 + o as i32) as u16; }

    fn daa(&mut self) {
        let mut a = self.a as u16;
        if !self.flag(FLAG_N) {
            if self.flag(FLAG_H) || (a & 0xF) > 9  { a = a.wrapping_add(0x06); }
            if self.flag(FLAG_C) || a > 0x9F        { a = a.wrapping_add(0x60); }
        } else {
            if self.flag(FLAG_H) { a = a.wrapping_sub(0x06) & 0xFF; }
            if self.flag(FLAG_C) { a = a.wrapping_sub(0x60); }
        }
        self.set_flag(FLAG_C, a & 0x100 != 0);
        self.set_flag(FLAG_H, false);
        self.a = a as u8;
        self.set_flag(FLAG_Z, self.a == 0);
    }

    fn rlc (&mut self, v: u8) -> u8 { let c=v>>7;   let r=(v<<1)|c;          self.f=if r==0{FLAG_Z}else{0}|if c!=0{FLAG_C}else{0}; r }
    fn rrc (&mut self, v: u8) -> u8 { let c=v&1;    let r=(v>>1)|(c<<7);      self.f=if r==0{FLAG_Z}else{0}|if c!=0{FLAG_C}else{0}; r }
    fn rl  (&mut self, v: u8) -> u8 { let oc=self.flag(FLAG_C) as u8; let c=v>>7; let r=(v<<1)|oc; self.f=if r==0{FLAG_Z}else{0}|if c!=0{FLAG_C}else{0}; r }
    fn rr  (&mut self, v: u8) -> u8 { let oc=self.flag(FLAG_C) as u8; let c=v&1;  let r=(v>>1)|(oc<<7); self.f=if r==0{FLAG_Z}else{0}|if c!=0{FLAG_C}else{0}; r }
    fn sla (&mut self, v: u8) -> u8 { let c=v>>7; let r=v<<1;          self.f=if r==0{FLAG_Z}else{0}|if c!=0{FLAG_C}else{0}; r }
    fn sra (&mut self, v: u8) -> u8 { let c=v&1;  let r=(v>>1)|(v&0x80); self.f=if r==0{FLAG_Z}else{0}|if c!=0{FLAG_C}else{0}; r }
    fn srl (&mut self, v: u8) -> u8 { let c=v&1;  let r=v>>1;           self.f=if r==0{FLAG_Z}else{0}|if c!=0{FLAG_C}else{0}; r }
    fn swap(&mut self, v: u8) -> u8 { let r=(v>>4)|(v<<4); self.f=if r==0{FLAG_Z}else{0}; r }
}

#[rustfmt::skip]
const CYCLE_TABLE: [u8; 256] = [
    4,12, 8, 8, 4, 4, 8, 4,20, 8, 8, 8, 4, 4, 8, 4,
    4,12, 8, 8, 4, 4, 8, 4,12, 8, 8, 8, 4, 4, 8, 4,
    8,12, 8, 8, 4, 4, 8, 4, 8, 8, 8, 8, 4, 4, 8, 4,
    8,12, 8, 8,12,12,12, 4, 8, 8, 8, 8, 4, 4, 8, 4,
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    8, 8, 8, 8, 8, 8, 4, 8, 4, 4, 4, 4, 4, 4, 8, 4,
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    4, 4, 4, 4, 4, 4, 8, 4, 4, 4, 4, 4, 4, 4, 8, 4,
    8,12,12,16,12,16, 8,16, 8,16,12, 4,12,24, 8,16,
    8,12,12, 0,12,16, 8,16, 8,16,12, 0,12, 0, 8,16,
   12,12, 8, 0, 0,16, 8,16,16, 4,16, 0, 0, 0, 8,16,
   12,12, 8, 4, 0,16, 8,16,12, 8,16, 4, 0, 0, 8,16,
];
