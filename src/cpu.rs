use crate::mmu::{Memory, MMU};

pub struct CPU {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
    ime: bool
}

const FLAG_MASK_ZERO: u8 = 0b10000000;
const FLAG_MASK_SUBTRACT: u8 = 0b01000000;
const FLAG_MASK_HALF_CARRY: u8 = 0b00100000;
const FLAG_MASK_CARRY: u8 = 0b00010000;

impl CPU {
    pub fn new() -> CPU {
        CPU {
            a: 0x01,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            f: 0xB0,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false
        }
    }

    fn af(&self) -> u16 {
        ((self.a as u16) << 8) + self.f as u16
    }

    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) + self.c as u16
    }

    fn de(&self) -> u16 {
        ((self.d as u16) << 8) + self.e as u16
    }

    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) + self.l as u16
    }

    fn zero_flag(&self) -> bool {
        self.f & FLAG_MASK_ZERO > 0
    }

    fn subtract_flag(&self) -> bool {
        self.f & FLAG_MASK_SUBTRACT > 0
    }

    fn half_carry_flag(&self) -> bool {
        self.f & FLAG_MASK_HALF_CARRY > 0
    }

    fn carry_flag(&self) -> bool {
        self.f & FLAG_MASK_CARRY > 0
    }

    fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
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

    fn read_u16(&mut self, address: u16, mmu: &mut impl Memory) -> u16 {
        let upper = mmu.read_byte(address + 1) as u16;
        let lower = mmu.read_byte(address) as u16;

        (upper << 8) + lower
    }

    fn inc_u8_and_set_flags(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);

        let z = result == 0;
        let n = false;
        let h = result < value;
        let c = self.carry_flag();
        self.set_flags(z, n, h, c);

        result
    }

    fn dec_u8_and_set_flags(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);

        let z = result == 0;
        let n = true;
        let h = result > value;
        let c = self.carry_flag();
        self.set_flags(z, n, h, c);

        result
    }

    fn call(&mut self, condition: bool, mmu: &mut impl Memory) -> u8 {
        if condition {
            let address = self.read_u16(self.pc, mmu);
            self.push(address, mmu);
            self.pc += 2;

            6
        } else {
            self.pc += 2;
            3
        }
    }

    fn jump_relative(&mut self, condition: bool, mmu: &mut impl Memory) -> u8 {
        let offset = mmu.read_byte(self.pc) as i8;
        if condition {
            self.pc = ((self.pc as i16) + offset as i16) as u16;
            3
        } else {
            self.pc += 1;
            2
        }
    }

    fn jump_abs(&mut self, condition: bool, mmu: &mut impl Memory) -> u8 {
        if condition {
            let address = self.read_u16(self.pc, mmu);
            self.pc = address;
            4
        } else {
            self.pc += 2;
            3
        }
    }

    fn add_u16_to_hl(&mut self, value: u16) -> u8 {
        let old_hl = self.hl();
        let new_hl = self.hl().wrapping_add(value);
        let (upper, lower) = self.u16_to_u8s(new_hl);
        let half_carry = (old_hl & 0x00FF) > (lower as u16);
        self.set_flags(self.zero_flag(), false, half_carry, new_hl < old_hl);
        self.h = upper;
        self.l = lower;
        2
    }

    fn halt(&mut self) {
        todo!();
    }

    fn add_a(&mut self, value: u8) {
        let old_a = self.a;
        let new_a = self.a.wrapping_add(value);
        let half_carry = (old_a & 0x0F) + (value & 0x0F) > 0x0F;
        self.set_flags(new_a == 0, false, half_carry, new_a < old_a);
        self.a = new_a;
    }

    fn sub_a(&mut self, value: u8) {
        let old_a = self.a;
        let new_a = self.a.wrapping_sub(value);
        let half_carry = (old_a & 0x0F) < (value & 0x0F);
        self.set_flags(new_a == 0, true, half_carry, new_a > old_a);
        self.a = new_a;
    }

    fn and_a(&mut self, value: u8) {
        self.a &= value;
        self.set_flags(self.a == 0, false, true, false);
    }

    fn xor_a(&mut self, value: u8) {
        self.a ^= value;
        self.set_flags(self.a == 0, false, false, false);
    }

    fn or_a(&mut self, value: u8) {
        self.a |= value;
        self.set_flags(self.a == 0, false, false, false);
    }

    fn cp_a(&mut self, value: u8) {
        let h = (self.a & 0x0F) < (value & 0x0F);
        self.set_flags(self.a == value, true, h, self.a < value)
    }

    fn ret(&mut self, condition: bool, mmu: &mut impl Memory) -> u8 {
        if condition {
            self.pc = self.read_u16(self.sp, mmu);
            return 5;
        }
        2
    }

    fn restart(&mut self, address: u16, mmu: &mut impl Memory) -> u8 {
        self.push(self.pc, mmu);
        self.pc = address;
        4
    }

    fn pop(&mut self, mmu: &mut impl Memory) -> (u8, u8) {
        let value = self.read_u16(self.sp, mmu);
        self.sp += 2;
        self.u16_to_u8s(value)
    }

    fn push(&mut self, value: u16, mmu: &mut impl Memory) {
        let (upper, lower) = self.u16_to_u8s(value);
        self.sp -= 1;
        mmu.write_byte(self.sp, upper);
        self.sp -= 1;
        mmu.write_byte(self.sp, lower);
    }

    pub fn execute(&mut self, opcode: u8, mmu: &mut impl Memory) -> u8 {
        match opcode {
            0x00 => 1,
            0x01 => {
                self.c = mmu.read_byte(self.pc);
                self.b = mmu.read_byte(self.pc + 1);
                self.pc += 2;
                3
            }
            0x02 => {
                mmu.write_byte(self.bc(), self.a);
                2
            }
            0x03 => {
                let (upper, lower) = self.u16_to_u8s(self.bc().wrapping_add(1));
                self.b = upper;
                self.c = lower;
                2
            }
            0x04 => {
                self.b = self.inc_u8_and_set_flags(self.b);
                1
            }
            0x05 => {
                self.b = self.dec_u8_and_set_flags(self.b);
                1
            }
            0x06 => {
                self.b = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x07 => {
                let carry_bit = (self.a & 0b10000000) >> 7;
                self.a = (self.a << 1) + carry_bit;
                self.set_flags(false, false, false, carry_bit > 0);
                1
            }
            0x08 => {
                let addr = self.read_u16(self.pc, mmu);
                self.pc += 2;

                let upper = (self.sp >> 8) as u8;
                let lower = (self.sp & 0x00FF) as u8;
                mmu.write_byte(addr + 1, upper); // little endian
                mmu.write_byte(addr, lower);
                5
            }
            0x09 => self.add_u16_to_hl(self.bc()),
            0x0A => {
                self.a = mmu.read_byte(self.bc());
                2
            }
            0x0B => {
                let (upper, lower) = self.u16_to_u8s(self.bc().wrapping_sub(1));
                self.b = upper;
                self.c = lower;
                2
            }
            0x0C => {
                self.c = self.inc_u8_and_set_flags(self.c);
                1
            }
            0x0D => {
                self.c = self.dec_u8_and_set_flags(self.c);
                1
            }
            0x0E => {
                self.c = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x0F => {
                let carry_bit = self.a & 1;
                self.a = (self.a >> 1) + (carry_bit << 7);
                self.set_flags(false, false, false, carry_bit > 0);
                1
            }
            0x10 => { todo!(); } // STOP
            0x11 => {
                self.e = mmu.read_byte(self.pc);
                self.d = mmu.read_byte(self.pc + 1);
                self.pc += 2;
                3
            }
            0x12 => {
                mmu.write_byte(self.de(), self.a);
                2
            }
            0x13 => {
                let (upper, lower) = self.u16_to_u8s(self.de().wrapping_add(1));
                self.d = upper;
                self.e = lower;
                2
            }
            0x14 => {
                self.d = self.inc_u8_and_set_flags(self.d);
                1
            }
            0x15 => {
                self.d = self.dec_u8_and_set_flags(self.d);
                1
            }
            0x16 => {
                self.d = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x17 => {
                let old_carry = if self.carry_flag() { 1 } else { 0 };
                let new_carry = self.a & 0b10000000 > 0;
                self.a = (self.a << 1) + old_carry;
                self.set_flags(false, false, false, new_carry);
                1
            }
            0x18 => self.jump_relative(true, mmu),
            0x19 => self.add_u16_to_hl(self.de()),
            0x1A => {
                self.a = mmu.read_byte(self.de());
                2
            }
            0x1B => {
                let (upper, lower) = self.u16_to_u8s(self.de().wrapping_sub(1));
                self.d = upper;
                self.e = lower;
                2
            }
            0x1C => {
                self.e = self.inc_u8_and_set_flags(self.e);
                1
            }
            0x1D => {
                self.e = self.dec_u8_and_set_flags(self.e);
                1
            }
            0x1E => {
                self.e = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x1F => {
                let old_carry = if self.carry_flag() { 1 } else { 0 };
                let new_carry = self.a & 1 > 0;
                self.a = (self.a >> 1) + (old_carry << 7);
                self.set_flags(false, false, false, new_carry);
                1
            }
            0x20 => self.jump_relative(!self.zero_flag(), mmu),
            0x21 => {
                self.l = mmu.read_byte(self.pc);
                self.pc += 1;
                self.h = mmu.read_byte(self.pc);
                self.pc += 1;
                3
            }
            0x22 => {
                mmu.write_byte(self.hl(), self.a);
                let new_hl = self.hl().wrapping_add(1);
                self.h = (new_hl >> 8) as u8;
                self.l = (new_hl & 0x00FF) as u8;
                2
            }
            0x23 => {
                let (upper, lower) = self.u16_to_u8s(self.hl().wrapping_add(1));
                self.h = upper;
                self.l = lower;
                2
            }
            0x24 => {
                self.h = self.inc_u8_and_set_flags(self.h);
                1
            }
            0x25 => {
                self.h = self.dec_u8_and_set_flags(self.h);
                1
            }
            0x26 => {
                self.h = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x27 => {
                todo!(); // DAA
            }
            0x28 => self.jump_relative(self.zero_flag(), mmu),
            0x29 => self.add_u16_to_hl(self.hl()),
            0x2A => {
                self.a = mmu.read_byte(self.hl());
                let new_hl = self.hl().wrapping_add(1);
                self.h = (new_hl >> 8) as u8;
                self.l = (new_hl & 0x00FF) as u8;
                2
            }
            0x2B => {
                let (upper, lower) = self.u16_to_u8s(self.hl().wrapping_sub(1));
                self.h = upper;
                self.l = lower;
                2
            }
            0x2C => {
                self.l = self.inc_u8_and_set_flags(self.l);
                1
            }
            0x2D => {
                self.l = self.dec_u8_and_set_flags(self.l);
                1
            }
            0x2E => {
                self.l = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x2F => {
                self.a = !self.a;
                self.set_flags(self.zero_flag(), true, true, self.carry_flag());
                1
            }
            0x30 => self.jump_relative(!self.carry_flag(), mmu),
            0x31 => {
                self.sp = self.read_u16(self.pc, mmu);
                self.pc += 2;
                3
            }
            0x32 => {
                mmu.write_byte(self.hl(), self.a);
                let new_hl = self.hl().wrapping_sub(1);
                self.h = (new_hl >> 8) as u8;
                self.l = (new_hl & 0x00FF) as u8;
                2
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                2
            }
            0x34 => {
                let value = mmu.read_byte(self.hl());
                let new_value = self.inc_u8_and_set_flags(value);
                mmu.write_byte(self.hl(), new_value);
                3
            }
            0x35 => {
                let value = mmu.read_byte(self.hl());
                let new_value = self.dec_u8_and_set_flags(value);
                mmu.write_byte(self.hl(), new_value);
                3
            }
            0x36 => {
                let value = mmu.read_byte(self.pc);
                mmu.write_byte(self.hl(), value);
                self.pc += 1;
                2
            }
            0x37 => {
                self.set_flags(self.zero_flag(), false, false, true);
                1
            }
            0x38 => self.jump_relative(self.carry_flag(), mmu),
            0x39 => self.add_u16_to_hl(self.sp),
            0x3A => {
                self.a = mmu.read_byte(self.hl());
                let new_hl = self.hl().wrapping_sub(1);
                let (upper, lower) = self.u16_to_u8s(new_hl);
                self.h = upper;
                self.l = lower;
                2
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                2
            }
            0x3C => {
                self.a = self.inc_u8_and_set_flags(self.a);
                1
            }
            0x3D => {
                self.a = self.dec_u8_and_set_flags(self.a);
                1
            }
            0x3E => {
                self.a = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x3F => {
                self.set_flags(self.zero_flag(), false, false, !self.carry_flag());
                1
            }
            0x40 => {
                self.b = self.b;
                1
            }
            0x41 => {
                self.b = self.c;
                1
            }
            0x42 => {
                self.b = self.d;
                1
            }
            0x43 => {
                self.b = self.e;
                1
            }
            0x44 => {
                self.b = self.h;
                1
            }
            0x45 => {
                self.b = self.l;
                1
            }
            0x46 => {
                self.b = mmu.read_byte(self.hl());
                2
            }
            0x47 => {
                self.b = self.a;
                1
            }

            0x48 => {
                self.c = self.b;
                1
            }
            0x49 => {
                self.c = self.c;
                1
            }
            0x4A => {
                self.c = self.d;
                1
            }
            0x4B => {
                self.c = self.e;
                1
            }
            0x4C => {
                self.c = self.h;
                1
            }
            0x4D => {
                self.c = self.l;
                1
            }
            0x4E => {
                self.c = mmu.read_byte(self.hl());
                2
            }
            0x4F => {
                self.c = self.a;
                1
            }

            0x50 => {
                self.d = self.b;
                1
            }
            0x51 => {
                self.d = self.c;
                1
            }
            0x52 => {
                self.d = self.d;
                1
            }
            0x53 => {
                self.d = self.e;
                1
            }
            0x54 => {
                self.d = self.h;
                1
            }
            0x55 => {
                self.d = self.l;
                1
            }
            0x56 => {
                self.d = mmu.read_byte(self.hl());
                2
            }
            0x57 => {
                self.d = self.a;
                1
            }

            0x58 => {
                self.e = self.b;
                1
            }
            0x59 => {
                self.e = self.c;
                1
            }
            0x5A => {
                self.e = self.d;
                1
            }
            0x5B => {
                self.e = self.e;
                1
            }
            0x5C => {
                self.e = self.h;
                1
            }
            0x5D => {
                self.e = self.l;
                1
            }
            0x5E => {
                self.e = mmu.read_byte(self.hl());
                2
            }
            0x5F => {
                self.e = self.a;
                1
            }

            0x60 => {
                self.h = self.b;
                1
            }
            0x61 => {
                self.h = self.c;
                1
            }
            0x62 => {
                self.h = self.d;
                1
            }
            0x63 => {
                self.h = self.e;
                1
            }
            0x64 => {
                self.h = self.h;
                1
            }
            0x65 => {
                self.h = self.l;
                1
            }
            0x66 => {
                self.h = mmu.read_byte(self.hl());
                2
            }
            0x67 => {
                self.h = self.a;
                1
            }

            0x68 => {
                self.l = self.b;
                1
            }
            0x69 => {
                self.l = self.c;
                1
            }
            0x6A => {
                self.l = self.d;
                1
            }
            0x6B => {
                self.l = self.e;
                1
            }
            0x6C => {
                self.l = self.h;
                1
            }
            0x6D => {
                self.l = self.l;
                1
            }
            0x6E => {
                self.l = mmu.read_byte(self.hl());
                2
            }
            0x6F => {
                self.l = self.a;
                1
            }

            0x70 => {
                mmu.write_byte(self.hl(), self.b);
                2
            }
            0x71 => {
                mmu.write_byte(self.hl(), self.c);
                2
            }
            0x72 => {
                mmu.write_byte(self.hl(), self.d);
                2
            }
            0x73 => {
                mmu.write_byte(self.hl(), self.e);
                2
            }
            0x74 => {
                mmu.write_byte(self.hl(), self.h);
                2
            }
            0x75 => {
                mmu.write_byte(self.hl(), self.l);
                2
            }
            0x76 => {
                self.halt();
                1
            }
            0x77 => {
                mmu.write_byte(self.hl(), self.a);
                2
            }

            0x78 => {
                self.a = self.b;
                1
            }
            0x79 => {
                self.a = self.c;
                1
            }
            0x7A => {
                self.a = self.d;
                1
            }
            0x7B => {
                self.a = self.e;
                1
            }
            0x7C => {
                self.a = self.h;
                1
            }
            0x7D => {
                self.a = self.l;
                1
            }
            0x7E => {
                self.a = mmu.read_byte(self.hl());
                2
            }
            0x7F => {
                self.a = self.a;
                1
            }

            0x80 => {
                self.add_a(self.b);
                1
            }
            0x81 => {
                self.add_a(self.c);
                1
            }
            0x82 => {
                self.add_a(self.d);
                1
            }
            0x83 => {
                self.add_a(self.e);
                1
            }
            0x84 => {
                self.add_a(self.h);
                1
            }
            0x85 => {
                self.add_a(self.l);
                1
            }
            0x86 => {
                self.add_a(mmu.read_byte(self.hl()));
                2
            }
            0x87 => {
                self.add_a(self.a);
                1
            }

            0x88 => {
                self.add_a(self.b + self.carry_flag() as u8);
                1
            }
            0x89 => {
                self.add_a(self.c + self.carry_flag() as u8);
                1
            }
            0x8A => {
                self.add_a(self.d + self.carry_flag() as u8);
                1
            }
            0x8B => {
                self.add_a(self.e + self.carry_flag() as u8);
                1
            }
            0x8C => {
                self.add_a(self.h + self.carry_flag() as u8);
                1
            }
            0x8D => {
                self.add_a(self.l + self.carry_flag() as u8);
                1
            }
            0x8E => {
                self.add_a(mmu.read_byte(self.hl()) + self.carry_flag() as u8);
                2
            }
            0x8F => {
                self.add_a(self.a + self.carry_flag() as u8);
                1
            }

            0x90 => {
                self.sub_a(self.b);
                1
            }
            0x91 => {
                self.sub_a(self.c);
                1
            }
            0x92 => {
                self.sub_a(self.d);
                1
            }
            0x93 => {
                self.sub_a(self.e);
                1
            }
            0x94 => {
                self.sub_a(self.h);
                1
            }
            0x95 => {
                self.sub_a(self.l);
                1
            }
            0x96 => {
                self.sub_a(mmu.read_byte(self.hl()));
                2
            }
            0x97 => {
                self.sub_a(self.a);
                1
            }

            0x98 => {
                self.sub_a(self.b + self.carry_flag() as u8);
                1
            }
            0x99 => {
                self.sub_a(self.c + self.carry_flag() as u8);
                1
            }
            0x9A => {
                self.sub_a(self.d + self.carry_flag() as u8);
                1
            }
            0x9B => {
                self.sub_a(self.e + self.carry_flag() as u8);
                1
            }
            0x9C => {
                self.sub_a(self.h + self.carry_flag() as u8);
                1
            }
            0x9D => {
                self.sub_a(self.l + self.carry_flag() as u8);
                1
            }
            0x9E => {
                self.sub_a(mmu.read_byte(self.hl()) + self.carry_flag() as u8);
                2
            }
            0x9F => {
                self.sub_a(self.a + self.carry_flag() as u8);
                1
            }

            0xA0 => {
                self.and_a(self.b);
                1
            }
            0xA1 => {
                self.and_a(self.c);
                1
            }
            0xA2 => {
                self.and_a(self.d);
                1
            }
            0xA3 => {
                self.and_a(self.e);
                1
            }
            0xA4 => {
                self.and_a(self.h);
                1
            }
            0xA5 => {
                self.and_a(self.l);
                1
            }
            0xA6 => {
                self.and_a(mmu.read_byte(self.hl()));
                2
            }
            0xA7 => {
                self.and_a(self.a);
                1
            }

            0xA8 => {
                self.xor_a(self.b);
                1
            }
            0xA9 => {
                self.xor_a(self.c);
                1
            }
            0xAA => {
                self.xor_a(self.d);
                1
            }
            0xAB => {
                self.xor_a(self.e);
                1
            }
            0xAC => {
                self.xor_a(self.h);
                1
            }
            0xAD => {
                self.xor_a(self.l);
                1
            }
            0xAE => {
                self.xor_a(mmu.read_byte(self.hl()));
                2
            }
            0xAF => {
                self.xor_a(self.a);
                1
            }

            0xB0 => {
                self.or_a(self.b);
                1
            }
            0xB1 => {
                self.or_a(self.c);
                1
            }
            0xB2 => {
                self.or_a(self.d);
                1
            }
            0xB3 => {
                self.or_a(self.e);
                1
            }
            0xB4 => {
                self.or_a(self.h);
                1
            }
            0xB5 => {
                self.or_a(self.l);
                1
            }
            0xB6 => {
                self.or_a(mmu.read_byte(self.hl()));
                2
            }
            0xB7 => {
                self.or_a(self.a);
                1
            }

            0xB8 => {
                self.cp_a(self.b);
                1
            }
            0xB9 => {
                self.cp_a(self.c);
                1
            }
            0xBA => {
                self.cp_a(self.d);
                1
            }
            0xBB => {
                self.cp_a(self.e);
                1
            }
            0xBC => {
                self.cp_a(self.h);
                1
            }
            0xBD => {
                self.cp_a(self.l);
                1
            }
            0xBE => {
                self.cp_a(mmu.read_byte(self.hl()));
                2
            }
            0xBF => {
                self.cp_a(self.a);
                1
            }
            0xC0 => self.ret(!self.zero_flag(), mmu),
            0xC1 => {
                let (upper, lower) = self.pop(mmu);
                self.b = upper;
                self.c = lower;
                3
            }
            0xC2 => { self.jump_abs(!self.zero_flag(), mmu) }
            0xC3 => {
                self.jump_abs(true, mmu)
            }
            0xC4 => {
                self.call(!self.zero_flag(), mmu)
            }
            0xC5 => {
                self.push(self.bc(), mmu);
                4
            }
            0xC6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.add_a(value);
                2
            }
            0xC7 => { self.restart(0, mmu) }
            0xC8 => self.ret(self.zero_flag(), mmu),
            0xC9 => {
                self.ret(true, mmu);
                4
            }
            0xCA => { self.jump_abs(self.zero_flag(), mmu) }
            0xCB => { todo!() } // Other opcode sheet
            0xCC => { self.call(self.zero_flag(), mmu) }
            0xCD => { self.call(true, mmu) }
            0xCE => {
                let value = mmu.read_byte(self.pc) + self.carry_flag() as u8;
                self.pc += 1;
                self.add_a(value);
                2
            }
            0xCF => { self.restart(0x08, mmu) }
            0xD0 => self.ret(!self.carry_flag(), mmu),
            0xD1 => {
                let (upper, lower) = self.pop(mmu);
                self.d = upper;
                self.e = lower;
                3
            }
            0xD2 => { self.jump_abs(!self.carry_flag(), mmu) }
            0xD3 => { panic!("Unsupported OPCODE") }
            0xD4 => {
                self.call(!self.carry_flag(), mmu)
            }
            0xD5 => {
                self.push(self.de(), mmu);
                4
            }
            0xD6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.sub_a(value);
                2
            }
            0xD7 => { self.restart(0x10, mmu) }
            0xD8 => self.ret(self.carry_flag(), mmu),
            0xD9 => {
                self.ime = true;
                self.ret(true, mmu);
                4
            }
            0xDA => { self.jump_abs(self.carry_flag(), mmu) }
            0xDB => { panic!("Unsupported OPCODE") }
            0xDC => {
                self.call(self.carry_flag(), mmu)
            }
            0xDD => { panic!("Unsupported OPCODE") }
            0xDE => {
                let value = mmu.read_byte(self.pc) + self.carry_flag() as u8;
                self.pc += 1;
                self.sub_a(value);
                2
            }
            0xDF => { self.restart(0x18, mmu) }
            0xE0 => {
                let address = 0xFF00 + mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                mmu.write_byte(address, self.a);
                3
            }
            0xE1 => {
                let (upper, lower) = self.pop(mmu);
                self.h = upper;
                self.l = lower;
                3
            }
            0xE2 => {
                let address = 0xFF00 + self.c as u16;
                mmu.write_byte(address, self.a);
                2
            }
            0xE3 => { panic!("Unsupported OPCODE") }
            0xE4 => { panic!("Unsupported OPCODE") }
            0xE5 => {
                self.push(self.hl(), mmu);
                4
            }
            0xE6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.and_a(value);
                2
            }
            0xE7 => { self.restart(0x20, mmu) }
            0xE8 => {
                let offset = mmu.read_byte(self.pc) as i16;
                self.pc += 1;
                self.sp = (self.sp as i16 + offset as i16) as u16;
                4
            }
            0xE9 => {
                self.pc = self.read_u16(self.hl(), mmu);
                4
            }
            0xEA => {
                let address = self.read_u16(self.pc, mmu);
                self.pc += 2;
                mmu.write_byte(address, self.a);
                4
            }
            0xEB => { panic!("Unsupported OPCODE") }
            0xEC => { panic!("Unsupported OPCODE") }
            0xED => { panic!("Unsupported OPCODE") }
            0xEE => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.xor_a(value);
                2
            }
            0xEF => { self.restart(0x28, mmu) }
            0xF0 => {
                let address = 0xFF00 + mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                self.a = mmu.read_byte(address);
                3
            }
            0xF1 => {
                let (upper, lower) = self.pop(mmu);
                self.a = upper;
                self.f = lower;
                3
            }
            0xF2 => {
                let address = 0xFF00 + self.c as u16;
                self.a = mmu.read_byte(address);
                2
            }
            0xF3 => { self.ime = false; 1 }
            0xF4 => { panic!("Unsupported OPCODE") }
            0xF5 => {
                self.push(self.af(), mmu);
                4
            }
            0xF6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.or_a(value);
                2
            }
            0xF7 => { self.restart(0x30, mmu) }
            0xF8 => {
                let offset = mmu.read_byte(self.pc) as i16;
                self.pc += 1;
                let value = (self.sp as i16 + offset) as u16;
                let (upper, lower) = self.u16_to_u8s(value);
                self.h = upper;
                self.l = lower;
                3
            }
            0xF9 => {
                self.sp = self.hl();
                2
            }
            0xFA => {
                let address = self.read_u16(self.pc, mmu);
                self.pc += 2;
                self.a = mmu.read_byte(address);
                4
            }
            0xFB => { self.ime = true; 1 }
            0xFC => { panic!("Unsupported OPCODE") }
            0xFD => { panic!("Unsupported OPCODE") }
            0xFE => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.cp_a(value);
                2
            }
            0xFF => { self.restart(0x38, mmu) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::TestMMU;

    fn get_cpu_and_mmu() -> (CPU, TestMMU) {
        (CPU::new(), TestMMU::new())
    }

    #[test]
    fn test_call() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0x0;
        mmu.memory[0x0] = 0xCD;
        mmu.memory[0x1] = 0xAB;
        cpu.sp = 0x102;
        mmu.memory[0x100] = 0x00;
        mmu.memory[0x101] = 0x00;

        let cycles = cpu.call(true, &mut mmu);

        assert_eq!(cycles, 6);
        assert_eq!(cpu.pc, 2);
        assert_eq!(cpu.sp, 0x100);
        assert_eq!(mmu.memory[0x100], 0xCD);
        assert_eq!(mmu.memory[0x101], 0xAB);

        cpu.pc = 0x0;
        mmu.memory[0x0] = 0xCD;
        mmu.memory[0x1] = 0xAB;
        cpu.sp = 0x102;
        mmu.memory[0x100] = 0x00;
        mmu.memory[0x101] = 0x00;

        let cycles = cpu.call(false, &mut mmu);

        assert_eq!(cycles, 3);
        assert_eq!(cpu.pc, 2);
        assert_eq!(cpu.sp, 0x102);
        assert_eq!(mmu.memory[0x100], 0x00);
        assert_eq!(mmu.memory[0x101], 0x00);
    }

    #[test]
    fn test_pop() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.sp = 0;
        mmu.memory[0x0000] = 0xCD;
        mmu.memory[0x0001] = 0xAB;

        let popped_u8s = cpu.pop(&mut mmu);

        assert_eq!(popped_u8s, (0xAB, 0xCD));
        assert_eq!(cpu.sp, 2);
    }

    #[test]
    fn test_push() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.sp = 2;
        mmu.memory[0x0000] = 0x00;
        mmu.memory[0x0001] = 0x00;
        cpu.push(0xABCD, &mut mmu);

        assert_eq!(mmu.memory[0x0000], 0xCD);
        assert_eq!(mmu.memory[0x0001], 0xAB);
        assert_eq!(cpu.sp, 0);
    }

    #[test]
    fn test_restart() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0x1234;
        cpu.sp = 2;
        mmu.memory[0] = 0x00;
        mmu.memory[1] = 0x00;
        cpu.restart(0, &mut mmu);
        assert_eq!(cpu.pc, 0);
        assert_eq!(cpu.sp, 0);
        assert_eq!(mmu.memory[1], 0x12);
        assert_eq!(mmu.memory[0], 0x34);

        cpu.pc = 0x1234;
        cpu.sp = 2;
        mmu.memory[0] = 0x00;
        mmu.memory[1] = 0x00;
        cpu.restart(0x40, &mut mmu);
        assert_eq!(cpu.pc, 0x40);
        assert_eq!(mmu.memory[1], 0x12);
        assert_eq!(mmu.memory[0], 0x34);
    }

    #[test]
    fn test_ret() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0;
        cpu.sp = 0;
        mmu.memory[0x0000] = 0xCD;
        mmu.memory[0x0001] = 0xAB;
        let cycles = cpu.ret(true, &mut mmu);
        assert_eq!(cpu.pc, 0xABCD);
        assert_eq!(cycles, 5);

        cpu.pc = 0;
        cpu.sp = 0;
        mmu.memory[0x0000] = 0xCD;
        mmu.memory[0x0001] = 0xAB;
        let cycles = cpu.ret(false, &mut mmu);
        assert_eq!(cpu.pc, 0);
        assert_eq!(cycles, 2);
    }

    #[test]
    fn test_add_a() {
        let mut cpu = CPU::new();

        cpu.a = 1;
        cpu.add_a(2);
        assert_eq!(cpu.a, 3);
        assert_eq!(cpu.f, 0);

        cpu.a = 0x0F;
        cpu.add_a(0x01);
        assert_eq!(cpu.a, 0x10);
        assert_eq!(cpu.f, 0b00100000);

        cpu.a = 0xF0;
        cpu.add_a(0x10);
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.f, 0b10010000);

        cpu.a = 0xF0;
        cpu.add_a(0x11);
        assert_eq!(cpu.a, 0x01);
        assert_eq!(cpu.f, 0b00010000);
    }

    #[test]
    fn test_sub_a() {
        let mut cpu = CPU::new();

        cpu.a = 1;
        cpu.sub_a(2);
        assert_eq!(cpu.a, 0xFF);
        assert_eq!(cpu.f, 0b01110000);

        cpu.a = 0x10;
        cpu.sub_a(0x01);
        assert_eq!(cpu.a, 0x0F);
        assert_eq!(cpu.f, 0b01100000);

        cpu.a = 0x10;
        cpu.sub_a(0x10);
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.f, 0b11000000);

        cpu.a = 0xF0;
        cpu.sub_a(0x10);
        assert_eq!(cpu.a, 0xE0);
        assert_eq!(cpu.f, 0b01000000);
    }

    #[test]
    fn test_xor_a() {
        let mut cpu = CPU::new();

        cpu.a = 0b0111;
        cpu.xor_a(0b1101);
        assert_eq!(cpu.a, 0b1010);
        assert_eq!(cpu.f, 0);

        cpu.a = 0x10;
        cpu.xor_a(0x10);
        assert_eq!(cpu.a, 0);
        assert_eq!(cpu.f, 0b10000000);
    }

    #[test]
    fn test_or_a() {
        let mut cpu = CPU::new();

        cpu.a = 0b0111;
        cpu.or_a(0b1101);
        assert_eq!(cpu.a, 0b1111);
        assert_eq!(cpu.f, 0);

        cpu.a = 0x00;
        cpu.or_a(0x00);
        assert_eq!(cpu.a, 0);
        assert_eq!(cpu.f, 0b10000000);
    }

    #[test]
    fn test_cp_a() {
        let mut cpu = CPU::new();

        cpu.a = 5;
        cpu.cp_a(5);
        assert_eq!(cpu.f, 0b11000000);

        cpu.a = 0x03;
        cpu.cp_a(0x01);
        assert_eq!(cpu.f, 0b01000000);

        cpu.a = 0x10;
        cpu.cp_a(0x03);
        assert_eq!(cpu.f, 0b01100000);

        cpu.a = 0x00;
        cpu.cp_a(0x33);
        assert_eq!(cpu.f, 0b01110000);
    }

    #[test]
    fn test_0x3a_load_hl_ptr_into_a_dec() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.a = 0;
        mmu.memory[0x1234] = 0xAA;

        cpu.execute(0x3A, &mut mmu);

        assert_eq!(cpu.hl(), 0x1233);
        assert_eq!(cpu.a, 0xAA);
    }

    #[test]
    fn test_0x38_jump_relative_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.f = 0 | FLAG_MASK_CARRY;
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;

        let cycles = cpu.execute(0x38, &mut mmu);

        assert_eq!(cpu.pc, 0x10);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_0x37_jump_relative_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.f = 0;

        cpu.execute(0x37, &mut mmu);

        assert_eq!(cpu.f, 0b00010000);
    }

    #[test]
    fn test_0x36_load_u8_into_hl_ptr() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.h = 0x12;
        cpu.l = 0x34;
        mmu.memory[cpu.pc as usize] = 0xAA;
        mmu.memory[cpu.hl() as usize] = 0;

        cpu.execute(0x36, &mut mmu);

        assert_eq!(mmu.memory[cpu.hl() as usize], 0xAA);
    }

    #[test]
    fn test_0x31_load_u16_into_sp() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.sp = 0x0000;
        cpu.pc = 0x0000;
        mmu.memory[cpu.pc as usize] = 0x34; // little endian
        mmu.memory[cpu.pc as usize + 1] = 0x12;

        cpu.execute(0x31, &mut mmu);

        assert_eq!(cpu.sp, 0x1234);
        assert_eq!(cpu.pc, 0x0002);
    }

    #[test]
    fn test_0x30_jump_relative_not_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.f = 0;
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;

        let cycles = cpu.execute(0x30, &mut mmu);

        assert_eq!(cpu.pc, 0x10);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_0x2f_compliment() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.a = 0b10101010;
        cpu.f = 0b10010000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.a, 0b01010101);
        assert_flags(&cpu, true, true, true, true);

        cpu.a = 0b11110000;
        cpu.f = 0b10010000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.a, 0b00001111);
        assert_flags(&cpu, true, true, true, true);

        cpu.a = 0b00001111;
        cpu.f = 0b00000000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.a, 0b11110000);
        assert_flags(&cpu, false, true, true, false);
    }

    #[test]
    fn test_0x2a_load_hl_ptr_into_a() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.a = 0;
        mmu.memory[0x1234] = 0xAA;

        cpu.execute(0x2A, &mut mmu);

        assert_eq!(cpu.hl(), 0x1235);
        assert_eq!(cpu.a, 0xAA);
    }

    #[test]
    fn test_0x29_add_hl_hl() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.hl(), 0x2468);
        assert_flags(&cpu, true, false, false, false);

        cpu.h = 0x00;
        cpu.l = 0xFF;
        cpu.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.hl(), 0x01FE);
        assert_flags(&cpu, true, false, true, false);

        cpu.h = 0xFF;
        cpu.l = 0xFF;
        cpu.f = 0b01000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.hl(), 0xFFFE);
        assert_flags(&cpu, false, false, true, true);

        cpu.h = 0xFF;
        cpu.l = 0x00;
        cpu.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.hl(), 0xFE00);
        assert_flags(&cpu, true, false, false, true);
    }

    #[test]
    fn test_0x28_jump_relative_if_z() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.f = 0b10000000; // zero, jump_relative
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;
        let cycles = cpu.execute(0x28, &mut mmu);
        assert_eq!(cpu.pc, 0x10);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_0x27_daa_decimal_adjust_acc() {
        //TODO
    }

    #[test]
    fn test_0x23_inc_hl() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.hl(), 0x1235);

        cpu.h = 0xFF;
        cpu.l = 0xFF;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.hl(), 0x0);
    }

    #[test]
    fn test_0x22_load_a_into_hl_ptr() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.a = 0xAA;
        mmu.memory[0x1234] = 0x00;

        cpu.execute(0x22, &mut mmu);

        assert_eq!(cpu.hl(), 0x1235);
        assert_eq!(mmu.memory[0x1234], 0xAA);
    }

    #[test]
    fn test_0x21_load_u16_into_hl() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.h = 0x00;
        cpu.l = 0x00;
        cpu.pc = 0x0000;
        mmu.memory[cpu.pc as usize] = 0x34; // little endian
        mmu.memory[cpu.pc as usize + 1] = 0x12;

        cpu.execute(0x21, &mut mmu);

        assert_eq!(cpu.hl(), 0x1234);
        assert_eq!(cpu.pc, 0x0002);
    }

    #[test]
    fn test_0x20_jump_relative_not_zero() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.f = 0b00000000;
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;

        let cycles = cpu.execute(0x20, &mut mmu);

        assert_eq!(cpu.pc, 0x10);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_0x1a_load_de_addr_to_a() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0x00;
        cpu.d = 0x56;
        cpu.e = 0x78;
        mmu.memory[0x5678] = 0x12;

        cpu.execute(0x1A, &mut mmu);

        assert_eq!(cpu.a, 0x12);
    }

    #[test]
    fn test_0x1f_rra_rotate_a_right_carry() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.f = 0b00000000;
        cpu.a = 0b00000001;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.a, 0b00000000);
        assert_flags(&cpu, false, false, false, true);

        cpu.f = 0b00010000;
        cpu.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.a, 0b11111111);
        assert_flags(&cpu, false, false, false, false);

        cpu.f = 0b00000000;
        cpu.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.a, 0b01111111);
        assert_flags(&cpu, false, false, false, false);
    }

    #[test]
    fn test_0x19_add_hl_de() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.d = 0x56;
        cpu.e = 0x78;
        cpu.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.hl(), 0x1234 + 0x5678);
        assert_flags(&cpu, true, false, false, false);

        cpu.h = 0x00;
        cpu.l = 0xFF;
        cpu.d = 0x12;
        cpu.e = 0x01;
        cpu.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.hl(), 0x1300);
        assert_flags(&cpu, true, false, true, false);

        cpu.h = 0xFF;
        cpu.l = 0xFF;
        cpu.d = 0x00;
        cpu.e = 0x01;
        cpu.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.hl(), 0x0);
        assert_flags(&cpu, true, false, true, true);

        cpu.h = 0xFF;
        cpu.l = 0x00;
        cpu.d = 0x01;
        cpu.e = 0x00;
        cpu.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.hl(), 0x0);
        assert_flags(&cpu, true, false, false, true);
    }

    #[test]
    fn test_0x18_jr_i8() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x0000;
        mmu.memory[cpu.pc as usize] = 0x50;

        cpu.execute(0x18, &mut mmu);

        assert_eq!(cpu.pc, 0x50);
    }

    #[test]
    fn test_0x17_rla_rotate_a_left_carry() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.f = 0b00000000;
        cpu.a = 0b10000000;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.a, 0b00000000);
        assert_flags(&cpu, false, false, false, true);

        cpu.f = 0b00010000;
        cpu.a = 0b01111111;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.a, 0b11111111);
        assert_flags(&cpu, false, false, false, false);

        cpu.f = 0b00000000;
        cpu.a = 0b01111111;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.a, 0b11111110);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), false);
    }

    #[test]
    fn test_load_u8_into_register() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.d = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x16, &mut mmu);

        assert_eq!(cpu.d, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.e = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x1E, &mut mmu);

        assert_eq!(cpu.e, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.l = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x2E, &mut mmu);

        assert_eq!(cpu.l, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.h = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x26, &mut mmu);

        assert_eq!(cpu.l, 0xAB);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn test_dec_u8_and_set_flagss() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.d = 0x12;
        cpu.execute(0x15, &mut mmu);
        assert_eq!(cpu.d, 0x11);

        cpu.d = 0x00;
        cpu.execute(0x15, &mut mmu);
        assert_eq!(cpu.d, 0xFF);

        cpu.e = 0x12;
        cpu.execute(0x1D, &mut mmu);
        assert_eq!(cpu.e, 0x11);

        cpu.e = 0x00;
        cpu.execute(0x1D, &mut mmu);
        assert_eq!(cpu.e, 0xFF);

        cpu.h = 0x12;
        cpu.execute(0x25, &mut mmu);
        assert_eq!(cpu.h, 0x11);

        cpu.h = 0x00;
        cpu.execute(0x25, &mut mmu);
        assert_eq!(cpu.h, 0xFF);

        cpu.l = 0x12;
        cpu.execute(0x2D, &mut mmu);
        assert_eq!(cpu.l, 0x11);

        cpu.l = 0x00;
        cpu.execute(0x2D, &mut mmu);
        assert_eq!(cpu.l, 0xFF);
    }

    #[test]
    fn test_inc_u8_and_set_flagss() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.d = 0x12;
        cpu.execute(0x14, &mut mmu);
        assert_eq!(cpu.d, 0x13);

        cpu.d = 0xFF;
        cpu.execute(0x14, &mut mmu);
        assert_eq!(cpu.d, 0x00);

        cpu.e = 0x12;
        cpu.execute(0x1C, &mut mmu);
        assert_eq!(cpu.e, 0x13);

        cpu.e = 0xFF;
        cpu.execute(0x1C, &mut mmu);
        assert_eq!(cpu.e, 0x00);

        cpu.h = 0x12;
        cpu.execute(0x24, &mut mmu);
        assert_eq!(cpu.h, 0x13);

        cpu.h = 0xFF;
        cpu.execute(0x24, &mut mmu);
        assert_eq!(cpu.h, 0x00);

        cpu.l = 0x12;
        cpu.execute(0x2C, &mut mmu);
        assert_eq!(cpu.l, 0x13);

        cpu.l = 0xFF;
        cpu.execute(0x2C, &mut mmu);
        assert_eq!(cpu.l, 0x00);
    }

    #[test]
    fn test_dec_16_bit_registers() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.d = 0x12;
        cpu.e = 0x34;
        cpu.execute(0x1B, &mut mmu);
        assert_eq!(cpu.de(), 0x1233);

        cpu.d = 0x00;
        cpu.e = 0x00;
        cpu.execute(0x1B, &mut mmu);
        assert_eq!(cpu.de(), 0xFFFF);

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.execute(0x2B, &mut mmu);
        assert_eq!(cpu.hl(), 0x1233);

        cpu.h = 0x00;
        cpu.l = 0x00;
        cpu.execute(0x2B, &mut mmu);
        assert_eq!(cpu.hl(), 0xFFFF);

        cpu.sp = 0x1234;
        cpu.execute(0x3B, &mut mmu);
        assert_eq!(cpu.sp, 0x1233);

        cpu.sp = 0x0;
        cpu.execute(0x3B, &mut mmu);
        assert_eq!(cpu.sp, 0xFFFF);
    }

    #[test]
    fn test_inc_16_bit_registers() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.d = 0x12;
        cpu.e = 0x34;
        cpu.execute(0x13, &mut mmu);
        assert_eq!(cpu.de(), 0x1235);

        cpu.d = 0xFF;
        cpu.e = 0xFF;
        cpu.execute(0x13, &mut mmu);
        assert_eq!(cpu.de(), 0);

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.hl(), 0x1235);

        cpu.h = 0xFF;
        cpu.l = 0xFF;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.hl(), 0);

        cpu.sp = 0x1234;
        cpu.execute(0x33, &mut mmu);
        assert_eq!(cpu.sp, 0x1235);

        cpu.sp = 0xFFFF;
        cpu.execute(0x33, &mut mmu);
        assert_eq!(cpu.sp, 0);
    }

    #[test]
    fn test_0x12_load_a_into_de_ptr() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0xAB;
        cpu.d = 0x12;
        cpu.e = 0x34;
        mmu.memory[0x1234] = 0x00;

        cpu.execute(0x12, &mut mmu);

        assert_eq!(mmu.memory[0x1234], 0xAB);
        assert_eq!(cpu.de(), 0x1234);
    }

    #[test]
    fn test_0x11_load_u16_into_de() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x0005;
        mmu.memory[0x0005] = 0x34;
        mmu.memory[0x0006] = 0x12;

        cpu.execute(0x11, &mut mmu);

        assert_eq!(cpu.de(), 0x1234);
        assert_eq!(cpu.pc, 0x0007);
    }

    #[test]
    fn test_0x0f_rrca_right_rotate_a_circular() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0b11111110;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.a, 0b01111111);
        assert_eq!(cpu.zero_flag(), false);
        assert_eq!(cpu.carry_flag(), false);

        cpu.a = 0b00000001;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.a, 0b10000000);
        assert_eq!(cpu.zero_flag(), false);
        assert_eq!(cpu.carry_flag(), true);
    }

    #[test]
    fn test_0x0e_load_u8_into_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.c = 0x0;
        cpu.pc = 0x1234;
        mmu.memory[0x1234] = 0xAB;

        cpu.execute(0x0E, &mut mmu);

        assert_eq!(cpu.c, 0xAB);
        assert_eq!(cpu.pc, 0x1235);
    }

    #[test]
    fn test_0x0d_decrement_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.c = 0x01;
        cpu.f = 0b00110000; // Set carry flag

        cpu.execute(0x0D, &mut mmu);

        assert_eq!(cpu.c, 0x00);
        assert_eq!(cpu.zero_flag(), true);
        assert_eq!(cpu.subtract_flag(), true);
        assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), true); // Unmodified

        cpu.c = 0x00;
        cpu.f = 0b00010000; // Set carry flag
        cpu.execute(0x0D, &mut mmu);

        assert_eq!(cpu.c, 0xFF);
        assert_eq!(cpu.zero_flag(), false);
        assert_eq!(cpu.subtract_flag(), true);
        assert_eq!(cpu.half_carry_flag(), true);
        assert_eq!(cpu.carry_flag(), true); // Unmodified
    }

    #[test]
    fn test_0x0c_increment_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.c = 0x12;
        cpu.f = 0b11110000; // Set carry flag
        cpu.execute(0x0C, &mut mmu);

        assert_eq!(cpu.c, 0x13);
        assert_eq!(cpu.zero_flag(), false);
        assert_eq!(cpu.subtract_flag(), false); // Resets
        assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), true); // Unmodified

        cpu.c = 0xFF;
        cpu.f = 0b01010000; // Set carry flag
        cpu.execute(0x0C, &mut mmu);

        assert_eq!(cpu.c, 0x00);
        assert_eq!(cpu.zero_flag(), true);
        assert_eq!(cpu.subtract_flag(), false); // Resets
        assert_eq!(cpu.half_carry_flag(), true);
        assert_eq!(cpu.carry_flag(), true); // Unmodified
    }

    #[test]
    fn test_0x0b_decrement_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.b = 0x12;
        cpu.c = 0x34;

        cpu.execute(0x0B, &mut mmu);

        assert_eq!(cpu.bc(), 0x1233);
    }

    #[test]
    fn test_0x0a_load_bc_addr_to_a() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0x00;
        cpu.b = 0x56;
        cpu.c = 0x78;
        mmu.memory[0x5678] = 0x12;

        cpu.execute(0x0A, &mut mmu);

        assert_eq!(cpu.a, 0x12);
    }

    #[test]
    fn test_0x09_add_hl_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.h = 0x12;
        cpu.l = 0x34;
        cpu.b = 0x56;
        cpu.c = 0x78;
        cpu.f = 0b01000000;

        cpu.execute(0x09, &mut mmu);

        assert_eq!(cpu.bc(), 0x5678);
        assert_eq!(cpu.hl(), 0x1234 + 0x5678);
        assert_eq!(cpu.subtract_flag(), false); // Resets
        assert_eq!(cpu.half_carry_flag(), false); //TODO
        assert_eq!(cpu.carry_flag(), false); //TODO
    }

    #[test]
    fn test_0x08_load_sp_into_addr() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x100;
        mmu.memory[cpu.pc as usize] = 0x34;
        mmu.memory[cpu.pc as usize + 1] = 0x12;
        cpu.sp = 0xDEAD;

        cpu.execute(0x08, &mut mmu);

        assert_eq!(cpu.sp, 0xDEAD);
        assert_eq!(mmu.memory[0x1234], 0xAD);
        assert_eq!(mmu.memory[0x1234 + 1], 0xDE);
        assert_eq!(cpu.pc, 0x102);
    }

    #[test]
    fn test_0x07_rlca_rotate_a_left_circular() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0b10000000;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.a, 0b00000001);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), true);

        cpu.a = 0b01111111;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.a, 0b11111110);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), false);
    }

    #[test]
    fn test_0x06_load_u8_into_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.b = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x06, &mut mmu);

        assert_eq!(cpu.b, 0xAB);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn test_0x05_decrement_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.b = 0xFF;
        cpu.f = 0b11110000;

        cpu.execute(0x05, &mut mmu);

        assert_eq!(cpu.b, 0xFE);
        assert_eq!(cpu.zero_flag(), false); // Not zero
        assert_eq!(cpu.subtract_flag(), true); // Always true
                                               //TODO - assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), true); // Unaffected

        cpu.b = 0x01;
        cpu.f = 0b11100000;

        cpu.execute(0x06, &mut mmu);

        assert_eq!(cpu.b, 0x00);
        assert_eq!(cpu.zero_flag(), true); // Not zero
        assert_eq!(cpu.subtract_flag(), true); // Always true
                                               //TODO - assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), false); // Unaffected
    }

    #[test]
    fn test_0x04_increment_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.b = 0xEE;
        cpu.f = 0b11110000;

        cpu.execute(0x04, &mut mmu);

        assert_eq!(cpu.b, 0xEF);
        assert_eq!(cpu.zero_flag(), false); // Not zero
        assert_eq!(cpu.subtract_flag(), false); // Reset
                                                //TODO - assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), true); // Unaffected
    }

    #[test]
    fn test_0x03_increment_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.b = 0xEE;
        cpu.c = 0xE0;

        cpu.execute(0x03, &mut mmu);

        assert_eq!(cpu.bc(), 0xEEE1);
    }

    #[test]
    fn test_0x02_load_a_into_bc_addr() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0xFF;
        cpu.b = 0x12;
        cpu.c = 0x34;
        mmu.write_byte(0x1234, 0);

        cpu.execute(0x02, &mut mmu);

        assert_eq!(mmu.read_byte(0x1234), 0xFF);
    }

    #[test]
    fn test_0x01_load_bc_u16() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x100;
        mmu.memory[0x100] = 0x34;
        mmu.memory[0x101] = 0x12;

        cpu.execute(0x01, &mut mmu);

        assert_eq!(cpu.bc(), 0x1234);
        assert_eq!(cpu.pc, 0x102);
    }

    #[test]
    fn test_jump_abs() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0;
        let cycles = cpu.jump_abs(false, &mut mmu);
        assert_eq!(cpu.pc, 2);
        assert_eq!(cycles, 3);

        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0xCD;
        mmu.memory[1 + cpu.pc as usize] = 0xAB;
        let cycles = cpu.jump_abs(true, &mut mmu);
        assert_eq!(cpu.pc, 0xABCD);
        assert_eq!(cycles, 4);

        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0xFF;
        mmu.memory[1 + cpu.pc as usize] = 0xFF;
        let cycles = cpu.jump_abs(true, &mut mmu);
        assert_eq!(cpu.pc, 0xFFFF);
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_jump_relative() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.pc = 0;
        let cycles = cpu.jump_relative(false, &mut mmu);
        assert_eq!(cpu.pc, 1);
        assert_eq!(cycles, 2);

        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;
        let cycles = cpu.jump_relative(true, &mut mmu);
        assert_eq!(cpu.pc, 0x10);
        assert_eq!(cycles, 3);

        cpu.pc = 0x10;
        mmu.memory[cpu.pc as usize] = (-0x10 as i8) as u8;
        let cycles = cpu.jump_relative(true, &mut mmu);
        assert_eq!(cpu.pc, 0);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_8bit_regs_to_16_bit_regs() {
        let mut cpu = CPU::new();

        cpu.a = 0xDE;
        cpu.f = 0xAD;

        cpu.b = 0xBE;
        cpu.c = 0xEF;

        cpu.d = 0x12;
        cpu.e = 0x34;

        cpu.h = 0xAB;
        cpu.l = 0xCD;

        assert_eq!(cpu.af(), 0xDEAD);
        assert_eq!(cpu.bc(), 0xBEEF);
        assert_eq!(cpu.de(), 0x1234);
        assert_eq!(cpu.hl(), 0xABCD);
    }

    #[test]
    fn test_set_flags() {
        let mut cpu = CPU::new();

        cpu.f = 0x00;
        cpu.set_flags(true, false, false, true);

        assert_eq!(cpu.zero_flag(), true);
        assert_eq!(cpu.subtract_flag(), false);
        assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), true);

        cpu.set_flags(true, true, true, false);

        assert_eq!(cpu.zero_flag(), true);
        assert_eq!(cpu.subtract_flag(), true);
        assert_eq!(cpu.half_carry_flag(), true);
        assert_eq!(cpu.carry_flag(), false);
    }

    #[test]
    fn test_flags() {
        let mut cpu = CPU::new();

        cpu.a = 0xDE;
        cpu.f = 0xF0;

        assert_eq!(cpu.zero_flag(), true);
        assert_eq!(cpu.subtract_flag(), true);
        assert_eq!(cpu.half_carry_flag(), true);
        assert_eq!(cpu.carry_flag(), true);

        cpu.f = 0x00;

        assert_eq!(cpu.zero_flag(), false);
        assert_eq!(cpu.subtract_flag(), false);
        assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), false);
    }

    #[test]
    fn test_initial_register_values() {
        let cpu = CPU::new();

        assert_eq!(cpu.af(), 0x01B0);
        assert_eq!(cpu.bc(), 0x0013);
        assert_eq!(cpu.de(), 0x00D8);
        assert_eq!(cpu.hl(), 0x014D);
        assert_eq!(cpu.sp, 0xFFFE);
        assert_eq!(cpu.pc, 0x0100);
    }

    fn assert_flags(cpu: &CPU, zero: bool, subtract: bool, half_carry: bool, carry: bool) {
        assert_eq!(cpu.zero_flag(), zero);
        assert_eq!(cpu.subtract_flag(), subtract);
        assert_eq!(cpu.half_carry_flag(), half_carry);
        assert_eq!(cpu.carry_flag(), carry);
    }
}
