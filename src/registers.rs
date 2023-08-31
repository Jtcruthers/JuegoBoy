use std::ops::{Index, IndexMut};

const FLAG_MASK_ZERO: u8 = 0b10000000;
const FLAG_MASK_SUBTRACT: u8 = 0b01000000;
const FLAG_MASK_HALF_CARRY: u8 = 0b00100000;
const FLAG_MASK_CARRY: u8 = 0b00010000;

#[derive(Clone, Copy)]
pub enum Register {
    A,
    B,
    C,
    D,
    E,
    F,
    H,
    L,
}

pub enum Register16 {
    AF,
    BC,
    DE,
    HL,
}

pub enum Flag {
    Zero,
    Subtract,
    HalfCarry,
    Carry,
}

pub struct Registers {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub f: u8,
    pub h: u8,
    pub l: u8,
}

impl Index<Register> for Registers {
    type Output = u8;

    fn index(&self, register: Register) -> &Self::Output {
        use Register::*;
        match register {
            A => &self.a,
            B => &self.b,
            C => &self.c,
            D => &self.d,
            E => &self.e,
            F => &self.f,
            H => &self.h,
            L => &self.l,
        }
    }
}

impl IndexMut<Register> for Registers {
    fn index_mut(&mut self, register: Register) -> &mut Self::Output {
        use Register::*;
        match register {
            A => &mut self.a,
            B => &mut self.b,
            C => &mut self.c,
            D => &mut self.d,
            E => &mut self.e,
            F => &mut self.f,
            H => &mut self.h,
            L => &mut self.l,
        }
    }
}

impl Registers {
    pub fn new() -> Self {
        Registers {
            a: 0x01,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            f: 0xB0,
            h: 0x01,
            l: 0x4D,
        }
    }

    pub fn af(&self) -> u16 {
        ((self.a as u16) << 8) + self.f as u16
    }

    pub fn bc(&self) -> u16 {
        ((self.b as u16) << 8) + self.c as u16
    }

    pub fn de(&self) -> u16 {
        ((self.d as u16) << 8) + self.e as u16
    }

    pub fn hl(&self) -> u16 {
        ((self.h as u16) << 8) + self.l as u16
    }

    pub fn set_16(&mut self, register: Register16, value: u16) {
        use Register16::*;

        let (upper, lower) = self.u16_to_u8s(value);
        match register {
            AF => {
                self.a = upper;
                self.f = lower;
            }
            BC => {
                self.b = upper;
                self.c = lower;
            }
            DE => {
                self.d = upper;
                self.e = lower;
            }
            HL => {
                self.h = upper;
                self.l = lower;
            }
        }
    }

    pub fn zero_flag(&self) -> bool {
        self.f & FLAG_MASK_ZERO > 0
    }

    pub fn subtract_flag(&self) -> bool {
        self.f & FLAG_MASK_SUBTRACT > 0
    }

    pub fn half_carry_flag(&self) -> bool {
        self.f & FLAG_MASK_HALF_CARRY > 0
    }

    pub fn carry_flag(&self) -> bool {
        self.f & FLAG_MASK_CARRY > 0
    }

    pub fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        let z_flag = if z { 0b1000 } else { 0 };
        let n_flag = if n { 0b0100 } else { 0 };
        let h_flag = if h { 0b0010 } else { 0 };
        let c_flag = if c { 0b0001 } else { 0 };

        self.f = (z_flag | n_flag | h_flag | c_flag) << 4;
    }

    fn u16_to_u8s(&self, value: u16) -> (u8, u8) {
        let upper = (value >> 8) as u8;
        let lower = (value & 0x00FF) as u8;

        (upper, lower)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call() {}
}
