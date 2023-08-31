use crate::mmu::Memory;
use crate::registers::Register::*;
use crate::registers::{Register, Registers};

pub struct CPU {
    registers: Registers,
    sp: u16,
    pc: u16,
    ime: bool,
    halted: bool
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            registers: Registers::new(),
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            halted: false,
        }
    }

    fn fetch(&mut self, mmu: &mut impl Memory) -> u8 {
        let opcode = mmu.read_byte(self.pc);
        self.pc += 1;
        opcode
    }

    pub fn step(&mut self, mmu: &mut impl Memory) -> u8 {
        if self.halted {
            self.halted = false;
            return 1
        }
        let opcode = self.fetch(mmu);
        let cycles = self.execute(opcode, mmu);

        cycles
    }

    fn u16_to_u8s(&self, value: u16) -> (u8, u8) {
        let upper = (value >> 8) as u8;
        let lower = (value & 0x00FF) as u8;

        (upper, lower)
    }

    fn rlc(&mut self, reg: Register) -> u8 {
        let carry_bit = (self.registers[reg] & 0b10000000) >> 7;
        self.registers[reg] = (self.registers[reg] << 1) + carry_bit;
        self.registers.set_flags(false, false, false, carry_bit > 0);
        2
    }

    fn rrc(&mut self, reg: Register) -> u8 {
        let carry_bit = self.registers[reg] & 1;
        self.registers[reg] = (self.registers[reg] >> 1) + (carry_bit << 7);
        self.registers.set_flags(false, false, false, carry_bit > 0);
        2
    }

    fn rl(&mut self, reg: Register) -> u8 {
        let carry_bit = (self.registers[reg] & 0b10000000) >> 7;
        self.registers[reg] = (self.registers[reg] << 1) + (self.registers.carry_flag() as u8);
        self.registers.set_flags(false, false, false, carry_bit > 0);
        2
    }

    fn rr(&mut self, reg: Register) -> u8 {
        let carry_bit = self.registers[reg] & 1;
        self.registers[reg] = (self.registers[reg] >> 1) + (self.registers.carry_flag() as u8);
        self.registers.set_flags(false, false, false, carry_bit > 0);
        2
    }

    fn sla(&mut self, reg: Register) -> u8 {
        let carry_bit = (self.registers[reg] & 0b10000000) >> 7;
        self.registers[reg] = (self.registers[reg] << 1);
        self.registers.set_flags(false, false, false, carry_bit > 0);
        2
    }

    fn sra(&mut self, reg: Register) -> u8 {
        let msb = (self.registers[reg] & 0b10000000);
        let carry_bit = self.registers[reg] & 1;
        self.registers[reg] = (self.registers[reg] >> 1) + msb;
        self.registers.set_flags(false, false, false, carry_bit > 0);
        2
    }

    fn srl(&mut self, reg: Register) -> u8 {
        let carry_bit = self.registers[reg] & 1;
        self.registers[reg] = self.registers[reg] >> 1;
        self.registers.set_flags(false, false, false, carry_bit > 0);
        2
    }

    fn bit(&mut self, bit: u8, reg: Register) -> u8 {
        let bit_is_set = self.registers[reg] & (1 << bit) > 0;
        self.registers
            .set_flags(bit_is_set, false, true, self.registers.carry_flag());
        2
    }

    fn res(&mut self, bit: u8, reg: Register) -> u8 {
        let mask = !(1 << bit);
        self.registers[reg] &= mask;
        2
    }

    fn set(&mut self, bit: u8, reg: Register) -> u8 {
        let mask = 1 << bit;
        self.registers[reg] |= mask;
        2
    }

    fn swap(&mut self, reg: Register) {
        let carry_bit = (self.registers[reg] & 0b10000000) >> 7;
        self.registers[reg] = (self.registers[reg] << 1) + (self.registers.carry_flag() as u8);
        self.registers.set_flags(false, false, false, carry_bit > 0);
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
        let c = self.registers.carry_flag();
        self.registers.set_flags(z, n, h, c);

        result
    }

    fn dec_u8_and_set_flags(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);

        let z = result == 0;
        let n = true;
        let h = result > value;
        let c = self.registers.carry_flag();
        self.registers.set_flags(z, n, h, c);

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
        let old_hl = self.registers.hl();
        let new_hl = self.registers.hl().wrapping_add(value);
        let (upper, lower) = self.u16_to_u8s(new_hl);
        let half_carry = (old_hl & 0x00FF) > (lower as u16);
        self.registers.set_flags(
            self.registers.zero_flag(),
            false,
            half_carry,
            new_hl < old_hl,
        );
        self.registers.h = upper;
        self.registers.l = lower;
        2
    }

    fn halt(&mut self) {
        self.halted = true;
    }

    fn add_a(&mut self, value: u8) {
        let old_a = self.registers.a;
        let new_a = self.registers.a.wrapping_add(value);
        let half_carry = (old_a & 0x0F) + (value & 0x0F) > 0x0F;
        self.registers
            .set_flags(new_a == 0, false, half_carry, new_a < old_a);
        self.registers.a = new_a;
    }

    fn sub_a(&mut self, value: u8) {
        let old_a = self.registers.a;
        let new_a = self.registers.a.wrapping_sub(value);
        let half_carry = (old_a & 0x0F) < (value & 0x0F);
        self.registers
            .set_flags(new_a == 0, true, half_carry, new_a > old_a);
        self.registers.a = new_a;
    }

    fn and_a(&mut self, value: u8) {
        self.registers.a &= value;
        self.registers
            .set_flags(self.registers.a == 0, false, true, false);
    }

    fn xor_a(&mut self, value: u8) {
        self.registers.a ^= value;
        self.registers
            .set_flags(self.registers.a == 0, false, false, false);
    }

    fn or_a(&mut self, value: u8) {
        self.registers.a |= value;
        self.registers
            .set_flags(self.registers.a == 0, false, false, false);
    }

    fn cp_a(&mut self, value: u8) {
        let h = (self.registers.a & 0x0F) < (value & 0x0F);
        self.registers
            .set_flags(self.registers.a == value, true, h, self.registers.a < value)
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

    /** Get value from the stack and increment sp */
    fn pop(&mut self, mmu: &mut impl Memory) -> (u8, u8) {
        let value = self.read_u16(self.sp, mmu);
        self.sp += 2;
        self.u16_to_u8s(value)
    }

    /** Put value onto the stack and decrement sp */
    fn push(&mut self, value: u16, mmu: &mut impl Memory) {
        let (upper, lower) = self.u16_to_u8s(value);
        self.sp -= 1;
        mmu.write_byte(self.sp, upper);
        self.sp -= 1;
        mmu.write_byte(self.sp, lower);
    }

    fn interrupt_service_routine(&mut self, mmu: &mut impl Memory) -> u8 {
        self.push(self.pc, mmu);
        0
    }

    fn load_reg_to_reg(&mut self, to: Register, from: Register) -> u8 {
        self.registers[to] = self.registers[from];
        1
    }

    pub fn execute(&mut self, opcode: u8, mmu: &mut impl Memory) -> u8 {
        if opcode != 0x00 {
            println!("Executing opcode: {:02X} from {:04X}", opcode, self.pc);
        }
        match opcode {
            0x00 => 1,
            0x01 => {
                self.registers.c = mmu.read_byte(self.pc);
                self.registers.b = mmu.read_byte(self.pc + 1);
                self.pc += 2;
                3
            }
            0x02 => {
                mmu.write_byte(self.registers.bc(), self.registers.a);
                2
            }
            0x03 => {
                let (upper, lower) = self.u16_to_u8s(self.registers.bc().wrapping_add(1));
                self.registers.b = upper;
                self.registers.c = lower;
                2
            }
            0x04 => {
                self.registers.b = self.inc_u8_and_set_flags(self.registers.b);
                1
            }
            0x05 => {
                self.registers.b = self.dec_u8_and_set_flags(self.registers.b);
                1
            }
            0x06 => {
                self.registers.b = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x07 => {
                self.rlc(A);
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
            0x09 => self.add_u16_to_hl(self.registers.bc()),
            0x0A => {
                self.registers.a = mmu.read_byte(self.registers.bc());
                2
            }
            0x0B => {
                let (upper, lower) = self.u16_to_u8s(self.registers.bc().wrapping_sub(1));
                self.registers.b = upper;
                self.registers.c = lower;
                2
            }
            0x0C => {
                self.registers.c = self.inc_u8_and_set_flags(self.registers.c);
                1
            }
            0x0D => {
                self.registers.c = self.dec_u8_and_set_flags(self.registers.c);
                1
            }
            0x0E => {
                self.registers.c = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x0F => {
                self.rrc(A);
                1
            }
            0x10 => {
                self.halt();
                1
            }
            0x11 => {
                self.registers.e = mmu.read_byte(self.pc);
                self.registers.d = mmu.read_byte(self.pc + 1);
                self.pc += 2;
                3
            }
            0x12 => {
                mmu.write_byte(self.registers.de(), self.registers.a);
                2
            }
            0x13 => {
                let (upper, lower) = self.u16_to_u8s(self.registers.de().wrapping_add(1));
                self.registers.d = upper;
                self.registers.e = lower;
                2
            }
            0x14 => {
                self.registers.d = self.inc_u8_and_set_flags(self.registers.d);
                1
            }
            0x15 => {
                self.registers.d = self.dec_u8_and_set_flags(self.registers.d);
                1
            }
            0x16 => {
                self.registers.d = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x17 => {
                let old_carry = if self.registers.carry_flag() { 1 } else { 0 };
                let new_carry = self.registers.a & 0b10000000 > 0;
                self.registers.a = (self.registers.a << 1) + old_carry;
                self.registers.set_flags(false, false, false, new_carry);
                1
            }
            0x18 => self.jump_relative(true, mmu),
            0x19 => self.add_u16_to_hl(self.registers.de()),
            0x1A => {
                self.registers.a = mmu.read_byte(self.registers.de());
                2
            }
            0x1B => {
                let (upper, lower) = self.u16_to_u8s(self.registers.de().wrapping_sub(1));
                self.registers.d = upper;
                self.registers.e = lower;
                2
            }
            0x1C => {
                self.registers.e = self.inc_u8_and_set_flags(self.registers.e);
                1
            }
            0x1D => {
                self.registers.e = self.dec_u8_and_set_flags(self.registers.e);
                1
            }
            0x1E => {
                self.registers.e = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x1F => {
                let old_carry = if self.registers.carry_flag() { 1 } else { 0 };
                let new_carry = self.registers.a & 1 > 0;
                self.registers.a = (self.registers.a >> 1) + (old_carry << 7);
                self.registers.set_flags(false, false, false, new_carry);
                1
            }
            0x20 => self.jump_relative(!self.registers.zero_flag(), mmu),
            0x21 => {
                self.registers.l = mmu.read_byte(self.pc);
                self.pc += 1;
                self.registers.h = mmu.read_byte(self.pc);
                self.pc += 1;
                3
            }
            0x22 => {
                mmu.write_byte(self.registers.hl(), self.registers.a);
                let new_hl = self.registers.hl().wrapping_add(1);
                self.registers.h = (new_hl >> 8) as u8;
                self.registers.l = (new_hl & 0x00FF) as u8;
                2
            }
            0x23 => {
                let (upper, lower) = self.u16_to_u8s(self.registers.hl().wrapping_add(1));
                self.registers.h = upper;
                self.registers.l = lower;
                2
            }
            0x24 => {
                self.registers.h = self.inc_u8_and_set_flags(self.registers.h);
                1
            }
            0x25 => {
                self.registers.h = self.dec_u8_and_set_flags(self.registers.h);
                1
            }
            0x26 => {
                self.registers.h = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x27 => {
                todo!(); // DAA
            }
            0x28 => self.jump_relative(self.registers.zero_flag(), mmu),
            0x29 => self.add_u16_to_hl(self.registers.hl()),
            0x2A => {
                self.registers.a = mmu.read_byte(self.registers.hl());
                let new_hl = self.registers.hl().wrapping_add(1);
                self.registers.h = (new_hl >> 8) as u8;
                self.registers.l = (new_hl & 0x00FF) as u8;
                2
            }
            0x2B => {
                let (upper, lower) = self.u16_to_u8s(self.registers.hl().wrapping_sub(1));
                self.registers.h = upper;
                self.registers.l = lower;
                2
            }
            0x2C => {
                self.registers.l = self.inc_u8_and_set_flags(self.registers.l);
                1
            }
            0x2D => {
                self.registers.l = self.dec_u8_and_set_flags(self.registers.l);
                1
            }
            0x2E => {
                self.registers.l = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x2F => {
                self.registers.a = !self.registers.a;
                self.registers.set_flags(
                    self.registers.zero_flag(),
                    true,
                    true,
                    self.registers.carry_flag(),
                );
                1
            }
            0x30 => self.jump_relative(!self.registers.carry_flag(), mmu),
            0x31 => {
                self.sp = self.read_u16(self.pc, mmu);
                self.pc += 2;
                3
            }
            0x32 => {
                mmu.write_byte(self.registers.hl(), self.registers.a);
                let new_hl = self.registers.hl().wrapping_sub(1);
                self.registers.h = (new_hl >> 8) as u8;
                self.registers.l = (new_hl & 0x00FF) as u8;
                2
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                2
            }
            0x34 => {
                let value = mmu.read_byte(self.registers.hl());
                let new_value = self.inc_u8_and_set_flags(value);
                mmu.write_byte(self.registers.hl(), new_value);
                3
            }
            0x35 => {
                let value = mmu.read_byte(self.registers.hl());
                let new_value = self.dec_u8_and_set_flags(value);
                mmu.write_byte(self.registers.hl(), new_value);
                3
            }
            0x36 => {
                let value = mmu.read_byte(self.pc);
                mmu.write_byte(self.registers.hl(), value);
                self.pc += 1;
                2
            }
            0x37 => {
                self.registers
                    .set_flags(self.registers.zero_flag(), false, false, true);
                1
            }
            0x38 => self.jump_relative(self.registers.carry_flag(), mmu),
            0x39 => self.add_u16_to_hl(self.sp),
            0x3A => {
                self.registers.a = mmu.read_byte(self.registers.hl());
                let new_hl = self.registers.hl().wrapping_sub(1);
                let (upper, lower) = self.u16_to_u8s(new_hl);
                self.registers.h = upper;
                self.registers.l = lower;
                2
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                2
            }
            0x3C => {
                self.registers.a = self.inc_u8_and_set_flags(self.registers.a);
                1
            }
            0x3D => {
                self.registers.a = self.dec_u8_and_set_flags(self.registers.a);
                1
            }
            0x3E => {
                self.registers.a = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x3F => {
                self.registers.set_flags(
                    self.registers.zero_flag(),
                    false,
                    false,
                    !self.registers.carry_flag(),
                );
                1
            }
            0x40 => self.load_reg_to_reg(B, B),
            0x41 => self.load_reg_to_reg(B, C),
            0x42 => self.load_reg_to_reg(B, D),
            0x43 => self.load_reg_to_reg(B, E),
            0x44 => self.load_reg_to_reg(B, H),
            0x45 => self.load_reg_to_reg(B, L),
            0x46 => {
                self.registers.b = mmu.read_byte(self.registers.hl());
                2
            }
            0x47 => self.load_reg_to_reg(B, A),

            0x48 => self.load_reg_to_reg(C, A),
            0x49 => self.load_reg_to_reg(C, B),
            0x4A => {
                self.registers.c = self.registers.d;
                1
            }
            0x4B => {
                self.registers.c = self.registers.e;
                1
            }
            0x4C => {
                self.registers.c = self.registers.h;
                1
            }
            0x4D => {
                self.registers.c = self.registers.l;
                1
            }
            0x4E => {
                self.registers.c = mmu.read_byte(self.registers.hl());
                2
            }
            0x4F => {
                self.registers.c = self.registers.a;
                1
            }

            0x50 => {
                self.registers.d = self.registers.b;
                1
            }
            0x51 => {
                self.registers.d = self.registers.c;
                1
            }
            0x52 => {
                self.registers.d = self.registers.d;
                1
            }
            0x53 => {
                self.registers.d = self.registers.e;
                1
            }
            0x54 => {
                self.registers.d = self.registers.h;
                1
            }
            0x55 => {
                self.registers.d = self.registers.l;
                1
            }
            0x56 => {
                self.registers.d = mmu.read_byte(self.registers.hl());
                2
            }
            0x57 => {
                self.registers.d = self.registers.a;
                1
            }

            0x58 => {
                self.registers.e = self.registers.b;
                1
            }
            0x59 => {
                self.registers.e = self.registers.c;
                1
            }
            0x5A => {
                self.registers.e = self.registers.d;
                1
            }
            0x5B => {
                self.registers.e = self.registers.e;
                1
            }
            0x5C => {
                self.registers.e = self.registers.h;
                1
            }
            0x5D => {
                self.registers.e = self.registers.l;
                1
            }
            0x5E => {
                self.registers.e = mmu.read_byte(self.registers.hl());
                2
            }
            0x5F => {
                self.registers.e = self.registers.a;
                1
            }

            0x60 => {
                self.registers.h = self.registers.b;
                1
            }
            0x61 => {
                self.registers.h = self.registers.c;
                1
            }
            0x62 => {
                self.registers.h = self.registers.d;
                1
            }
            0x63 => {
                self.registers.h = self.registers.e;
                1
            }
            0x64 => {
                self.registers.h = self.registers.h;
                1
            }
            0x65 => {
                self.registers.h = self.registers.l;
                1
            }
            0x66 => {
                self.registers.h = mmu.read_byte(self.registers.hl());
                2
            }
            0x67 => {
                self.registers.h = self.registers.a;
                1
            }

            0x68 => {
                self.registers.l = self.registers.b;
                1
            }
            0x69 => {
                self.registers.l = self.registers.c;
                1
            }
            0x6A => {
                self.registers.l = self.registers.d;
                1
            }
            0x6B => {
                self.registers.l = self.registers.e;
                1
            }
            0x6C => {
                self.registers.l = self.registers.h;
                1
            }
            0x6D => {
                self.registers.l = self.registers.l;
                1
            }
            0x6E => {
                self.registers.l = mmu.read_byte(self.registers.hl());
                2
            }
            0x6F => {
                self.registers.l = self.registers.a;
                1
            }

            0x70 => {
                mmu.write_byte(self.registers.hl(), self.registers.b);
                2
            }
            0x71 => {
                mmu.write_byte(self.registers.hl(), self.registers.c);
                2
            }
            0x72 => {
                mmu.write_byte(self.registers.hl(), self.registers.d);
                2
            }
            0x73 => {
                mmu.write_byte(self.registers.hl(), self.registers.e);
                2
            }
            0x74 => {
                mmu.write_byte(self.registers.hl(), self.registers.h);
                2
            }
            0x75 => {
                mmu.write_byte(self.registers.hl(), self.registers.l);
                2
            }
            0x76 => {
                self.halt();
                1
            }
            0x77 => {
                mmu.write_byte(self.registers.hl(), self.registers.a);
                2
            }

            0x78 => {
                self.registers.a = self.registers.b;
                1
            }
            0x79 => {
                self.registers.a = self.registers.c;
                1
            }
            0x7A => {
                self.registers.a = self.registers.d;
                1
            }
            0x7B => {
                self.registers.a = self.registers.e;
                1
            }
            0x7C => {
                self.registers.a = self.registers.h;
                1
            }
            0x7D => {
                self.registers.a = self.registers.l;
                1
            }
            0x7E => {
                self.registers.a = mmu.read_byte(self.registers.hl());
                2
            }
            0x7F => {
                self.registers.a = self.registers.a;
                1
            }

            0x80 => {
                self.add_a(self.registers.b);
                1
            }
            0x81 => {
                self.add_a(self.registers.c);
                1
            }
            0x82 => {
                self.add_a(self.registers.d);
                1
            }
            0x83 => {
                self.add_a(self.registers.e);
                1
            }
            0x84 => {
                self.add_a(self.registers.h);
                1
            }
            0x85 => {
                self.add_a(self.registers.l);
                1
            }
            0x86 => {
                self.add_a(mmu.read_byte(self.registers.hl()));
                2
            }
            0x87 => {
                self.add_a(self.registers.a);
                1
            }

            0x88 => {
                self.add_a(self.registers.b + self.registers.carry_flag() as u8);
                1
            }
            0x89 => {
                self.add_a(self.registers.c + self.registers.carry_flag() as u8);
                1
            }
            0x8A => {
                self.add_a(self.registers.d + self.registers.carry_flag() as u8);
                1
            }
            0x8B => {
                self.add_a(self.registers.e + self.registers.carry_flag() as u8);
                1
            }
            0x8C => {
                self.add_a(self.registers.h + self.registers.carry_flag() as u8);
                1
            }
            0x8D => {
                self.add_a(self.registers.l + self.registers.carry_flag() as u8);
                1
            }
            0x8E => {
                self.add_a(mmu.read_byte(self.registers.hl()) + self.registers.carry_flag() as u8);
                2
            }
            0x8F => {
                self.add_a(self.registers.a + self.registers.carry_flag() as u8);
                1
            }

            0x90 => {
                self.sub_a(self.registers.b);
                1
            }
            0x91 => {
                self.sub_a(self.registers.c);
                1
            }
            0x92 => {
                self.sub_a(self.registers.d);
                1
            }
            0x93 => {
                self.sub_a(self.registers.e);
                1
            }
            0x94 => {
                self.sub_a(self.registers.h);
                1
            }
            0x95 => {
                self.sub_a(self.registers.l);
                1
            }
            0x96 => {
                self.sub_a(mmu.read_byte(self.registers.hl()));
                2
            }
            0x97 => {
                self.sub_a(self.registers.a);
                1
            }

            0x98 => {
                self.sub_a(self.registers.b + self.registers.carry_flag() as u8);
                1
            }
            0x99 => {
                self.sub_a(self.registers.c + self.registers.carry_flag() as u8);
                1
            }
            0x9A => {
                self.sub_a(self.registers.d + self.registers.carry_flag() as u8);
                1
            }
            0x9B => {
                self.sub_a(self.registers.e + self.registers.carry_flag() as u8);
                1
            }
            0x9C => {
                self.sub_a(self.registers.h + self.registers.carry_flag() as u8);
                1
            }
            0x9D => {
                self.sub_a(self.registers.l + self.registers.carry_flag() as u8);
                1
            }
            0x9E => {
                self.sub_a(mmu.read_byte(self.registers.hl()) + self.registers.carry_flag() as u8);
                2
            }
            0x9F => {
                self.sub_a(self.registers.a + self.registers.carry_flag() as u8);
                1
            }

            0xA0 => {
                self.and_a(self.registers.b);
                1
            }
            0xA1 => {
                self.and_a(self.registers.c);
                1
            }
            0xA2 => {
                self.and_a(self.registers.d);
                1
            }
            0xA3 => {
                self.and_a(self.registers.e);
                1
            }
            0xA4 => {
                self.and_a(self.registers.h);
                1
            }
            0xA5 => {
                self.and_a(self.registers.l);
                1
            }
            0xA6 => {
                self.and_a(mmu.read_byte(self.registers.hl()));
                2
            }
            0xA7 => {
                self.and_a(self.registers.a);
                1
            }

            0xA8 => {
                self.xor_a(self.registers.b);
                1
            }
            0xA9 => {
                self.xor_a(self.registers.c);
                1
            }
            0xAA => {
                self.xor_a(self.registers.d);
                1
            }
            0xAB => {
                self.xor_a(self.registers.e);
                1
            }
            0xAC => {
                self.xor_a(self.registers.h);
                1
            }
            0xAD => {
                self.xor_a(self.registers.l);
                1
            }
            0xAE => {
                self.xor_a(mmu.read_byte(self.registers.hl()));
                2
            }
            0xAF => {
                self.xor_a(self.registers.a);
                1
            }

            0xB0 => {
                self.or_a(self.registers.b);
                1
            }
            0xB1 => {
                self.or_a(self.registers.c);
                1
            }
            0xB2 => {
                self.or_a(self.registers.d);
                1
            }
            0xB3 => {
                self.or_a(self.registers.e);
                1
            }
            0xB4 => {
                self.or_a(self.registers.h);
                1
            }
            0xB5 => {
                self.or_a(self.registers.l);
                1
            }
            0xB6 => {
                self.or_a(mmu.read_byte(self.registers.hl()));
                2
            }
            0xB7 => {
                self.or_a(self.registers.a);
                1
            }

            0xB8 => {
                self.cp_a(self.registers.b);
                1
            }
            0xB9 => {
                self.cp_a(self.registers.c);
                1
            }
            0xBA => {
                self.cp_a(self.registers.d);
                1
            }
            0xBB => {
                self.cp_a(self.registers.e);
                1
            }
            0xBC => {
                self.cp_a(self.registers.h);
                1
            }
            0xBD => {
                self.cp_a(self.registers.l);
                1
            }
            0xBE => {
                self.cp_a(mmu.read_byte(self.registers.hl()));
                2
            }
            0xBF => {
                self.cp_a(self.registers.a);
                1
            }
            0xC0 => self.ret(!self.registers.zero_flag(), mmu),
            0xC1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.b = upper;
                self.registers.c = lower;
                3
            }
            0xC2 => self.jump_abs(!self.registers.zero_flag(), mmu),
            0xC3 => self.jump_abs(true, mmu),
            0xC4 => self.call(!self.registers.zero_flag(), mmu),
            0xC5 => {
                self.push(self.registers.bc(), mmu);
                4
            }
            0xC6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.add_a(value);
                2
            }
            0xC7 => self.restart(0, mmu),
            0xC8 => self.ret(self.registers.zero_flag(), mmu),
            0xC9 => {
                self.ret(true, mmu);
                4
            }
            0xCA => self.jump_abs(self.registers.zero_flag(), mmu),
            0xCB => self.execute_cb_opcodes(mmu), // Other opcode sheet
            0xCC => self.call(self.registers.zero_flag(), mmu),
            0xCD => self.call(true, mmu),
            0xCE => {
                let value = mmu.read_byte(self.pc) + self.registers.carry_flag() as u8;
                self.pc += 1;
                self.add_a(value);
                2
            }
            0xCF => self.restart(0x08, mmu),
            0xD0 => self.ret(!self.registers.carry_flag(), mmu),
            0xD1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.d = upper;
                self.registers.e = lower;
                3
            }
            0xD2 => self.jump_abs(!self.registers.carry_flag(), mmu),
            0xD3 => {
                panic!("Unsupported OPCODE")
            }
            0xD4 => self.call(!self.registers.carry_flag(), mmu),
            0xD5 => {
                self.push(self.registers.de(), mmu);
                4
            }
            0xD6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.sub_a(value);
                2
            }
            0xD7 => self.restart(0x10, mmu),
            0xD8 => self.ret(self.registers.carry_flag(), mmu),
            0xD9 => {
                self.ime = true;
                self.ret(true, mmu);
                4
            }
            0xDA => self.jump_abs(self.registers.carry_flag(), mmu),
            0xDB => {
                panic!("Unsupported OPCODE")
            }
            0xDC => self.call(self.registers.carry_flag(), mmu),
            0xDD => {
                panic!("Unsupported OPCODE")
            }
            0xDE => {
                let value = mmu.read_byte(self.pc) + self.registers.carry_flag() as u8;
                self.pc += 1;
                self.sub_a(value);
                2
            }
            0xDF => self.restart(0x18, mmu),
            0xE0 => {
                let address = 0xFF00 + mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                mmu.write_byte(address, self.registers.a);
                3
            }
            0xE1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.h = upper;
                self.registers.l = lower;
                3
            }
            0xE2 => {
                let address = 0xFF00 + self.registers.c as u16;
                mmu.write_byte(address, self.registers.a);
                2
            }
            0xE3 => {
                panic!("Unsupported OPCODE")
            }
            0xE4 => {
                panic!("Unsupported OPCODE")
            }
            0xE5 => {
                self.push(self.registers.hl(), mmu);
                4
            }
            0xE6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.and_a(value);
                2
            }
            0xE7 => self.restart(0x20, mmu),
            0xE8 => {
                let offset = mmu.read_byte(self.pc) as i16;
                self.pc += 1;
                self.sp = (self.sp as i16 + offset as i16) as u16;
                4
            }
            0xE9 => {
                self.pc = self.read_u16(self.registers.hl(), mmu);
                4
            }
            0xEA => {
                let address = self.read_u16(self.pc, mmu);
                self.pc += 2;
                mmu.write_byte(address, self.registers.a);
                4
            }
            0xEB => {
                panic!("Unsupported OPCODE")
            }
            0xEC => {
                panic!("Unsupported OPCODE")
            }
            0xED => {
                panic!("Unsupported OPCODE")
            }
            0xEE => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.xor_a(value);
                2
            }
            0xEF => self.restart(0x28, mmu),
            0xF0 => {
                let address = 0xFF00 + mmu.read_byte(self.pc) as u16;
                self.pc += 1;
                self.registers.a = mmu.read_byte(address);
                3
            }
            0xF1 => {
                let (upper, lower) = self.pop(mmu);
                self.registers.a = upper;
                self.registers.f = lower;
                3
            }
            0xF2 => {
                let address = 0xFF00 + self.registers.c as u16;
                self.registers.a = mmu.read_byte(address);
                2
            }
            0xF3 => {
                self.ime = false;
                1
            }
            0xF4 => {
                panic!("Unsupported OPCODE")
            }
            0xF5 => {
                self.push(self.registers.af(), mmu);
                4
            }
            0xF6 => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.or_a(value);
                2
            }
            0xF7 => self.restart(0x30, mmu),
            0xF8 => {
                let offset = mmu.read_byte(self.pc) as i16;
                self.pc += 1;
                let value = (self.sp as i16 + offset) as u16;
                let (upper, lower) = self.u16_to_u8s(value);
                self.registers.h = upper;
                self.registers.l = lower;
                3
            }
            0xF9 => {
                self.sp = self.registers.hl();
                2
            }
            0xFA => {
                let address = self.read_u16(self.pc, mmu);
                self.pc += 2;
                self.registers.a = mmu.read_byte(address);
                4
            }
            0xFB => {
                self.ime = true;
                1
            }
            0xFC => {
                panic!("Unsupported OPCODE")
            }
            0xFD => {
                panic!("Unsupported OPCODE")
            }
            0xFE => {
                let value = mmu.read_byte(self.pc);
                self.pc += 1;
                self.cp_a(value);
                2
            }
            0xFF => self.restart(0x38, mmu),
        }
    }

    fn execute_cb_opcodes(&mut self, mmu: &mut impl Memory) -> u8 {
        let opcode = mmu.read_byte(self.pc);
        self.pc += 1;

        match opcode {
            0x00 => {
                self.rlc(B);
                2
            }
            0x01 => {
                self.rlc(C);
                2
            }
            0x02 => {
                self.rlc(D);
                2
            }
            0x03 => {
                self.rlc(E);
                2
            }
            0x04 => {
                self.rlc(H);
                2
            }
            0x05 => {
                self.rlc(L);
                2
            }
            0x06 => {
                let old_value = mmu.read_byte(self.registers.hl());
                let carry_bit = (old_value & 0b10000000) >> 7;
                let value = (old_value << 1) + carry_bit;
                mmu.write_byte(self.registers.hl(), value);
                self.registers.set_flags(false, false, false, carry_bit > 0);
                4
            }
            0x07 => {
                self.rlc(A);
                2
            }

            0x08 => {
                self.rrc(B);
                2
            }
            0x09 => {
                self.rrc(C);
                2
            }
            0x0A => {
                self.rrc(D);
                2
            }
            0x0B => {
                self.rrc(E);
                2
            }
            0x0C => {
                self.rrc(H);
                2
            }
            0x0D => {
                self.rrc(L);
                2
            }
            0x0E => {
                let old_value = mmu.read_byte(self.registers.hl());
                let carry_bit = old_value & 1;
                let value = (old_value >> 1) + (carry_bit << 7);
                mmu.write_byte(self.registers.hl(), value);
                self.registers.set_flags(false, false, false, carry_bit > 0);
                4
            }
            0x0F => {
                self.rrc(A);
                2
            }

            0x10 => {
                self.rl(B);
                2
            }
            0x11 => {
                self.rl(C);
                2
            }
            0x12 => {
                self.rl(D);
                2
            }
            0x13 => {
                self.rl(E);
                2
            }
            0x14 => {
                self.rl(H);
                2
            }
            0x15 => {
                self.rl(L);
                2
            }
            0x16 => {
                let old_value = mmu.read_byte(self.registers.hl());
                let carry_bit = (old_value & 0b10000000) >> 7;
                let value = (old_value << 1) + self.registers.carry_flag() as u8;
                mmu.write_byte(self.registers.hl(), value);
                self.registers.set_flags(false, false, false, carry_bit > 0);
                4
            }
            0x17 => {
                self.rlc(A);
                2
            }

            0x18 => {
                self.rr(B);
                2
            }
            0x19 => {
                self.rr(C);
                2
            }
            0x1A => {
                self.rr(D);
                2
            }
            0x1B => {
                self.rr(E);
                2
            }
            0x1C => {
                self.rr(H);
                2
            }
            0x1D => {
                self.rr(L);
                2
            }
            0x1E => {
                let old_value = mmu.read_byte(self.registers.hl());
                let carry_bit = old_value & 1;
                let value = (old_value >> 1) + ((self.registers.carry_flag() as u8) << 7);
                mmu.write_byte(self.registers.hl(), value);
                self.registers.set_flags(false, false, false, carry_bit > 0);
                4
            }
            0x1F => {
                self.rrc(A);
                2
            }

            0x20 => {
                self.sla(B);
                2
            }
            0x21 => {
                self.sla(C);
                2
            }
            0x22 => {
                self.sla(D);
                2
            }
            0x23 => {
                self.sla(E);
                2
            }
            0x24 => {
                self.sla(H);
                2
            }
            0x25 => {
                self.sla(L);
                2
            }
            0x26 => {
                todo!("SLA (HL)");
            }
            0x27 => {
                self.sla(A);
                2
            }

            0x28 => {
                self.sra(B);
                2
            }
            0x29 => {
                self.sra(C);
                2
            }
            0x2A => {
                self.sra(D);
                2
            }
            0x2B => {
                self.sra(E);
                2
            }
            0x2C => {
                self.sra(H);
                2
            }
            0x2D => {
                self.sra(L);
                2
            }
            0x2E => {
                todo!("SRA (HL)");
            }
            0x2F => {
                self.sra(A);
                2
            }

            0x30 => {
                self.swap(B);
                2
            }
            0x31 => {
                self.swap(C);
                2
            }
            0x32 => {
                self.swap(D);
                2
            }
            0x33 => {
                self.swap(E);
                2
            }
            0x34 => {
                self.swap(H);
                2
            }
            0x35 => {
                self.swap(L);
                2
            }
            0x36 => {
                todo!("swap (HL)");
            }
            0x37 => {
                self.swap(A);
                2
            }

            0x38 => {
                self.srl(B);
                2
            }
            0x39 => {
                self.srl(C);
                2
            }
            0x3A => {
                self.srl(D);
                2
            }
            0x3B => {
                self.srl(E);
                2
            }
            0x3C => {
                self.srl(H);
                2
            }
            0x3D => {
                self.srl(L);
                2
            }
            0x3E => {
                todo!("SRL (HL)");
            }
            0x3F => {
                self.srl(A);
                2
            }

            0x40 => self.bit(0, B),
            0x41 => self.bit(0, C),
            0x42 => self.bit(0, D),
            0x43 => self.bit(0, E),
            0x44 => self.bit(0, H),
            0x45 => self.bit(0, L),
            0x46 => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 0) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x47 => self.bit(0, A),

            0x48 => self.bit(1, B),
            0x49 => self.bit(1, C),
            0x4A => self.bit(1, D),
            0x4B => self.bit(1, E),
            0x4C => self.bit(1, H),
            0x4D => self.bit(1, L),
            0x4E => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 1) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x4F => self.bit(1, A),

            0x50 => self.bit(2, B),
            0x51 => self.bit(2, C),
            0x52 => self.bit(2, D),
            0x53 => self.bit(2, E),
            0x54 => self.bit(2, H),
            0x55 => self.bit(2, L),
            0x56 => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 2) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x57 => self.bit(2, A),

            0x58 => self.bit(3, B),
            0x59 => self.bit(3, C),
            0x5A => self.bit(3, D),
            0x5B => self.bit(3, E),
            0x5C => self.bit(3, H),
            0x5D => self.bit(3, L),
            0x5E => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 3) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x5F => self.bit(3, A),

            0x60 => self.bit(4, B),
            0x61 => self.bit(4, C),
            0x62 => self.bit(4, D),
            0x63 => self.bit(4, E),
            0x64 => self.bit(4, H),
            0x65 => self.bit(4, L),
            0x66 => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 4) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x67 => self.bit(4, A),

            0x68 => self.bit(5, B),
            0x69 => self.bit(5, C),
            0x6A => self.bit(5, D),
            0x6B => self.bit(5, E),
            0x6C => self.bit(5, H),
            0x6D => self.bit(5, L),
            0x6E => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 5) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x6F => self.bit(5, A),

            0x70 => self.bit(6, B),
            0x71 => self.bit(6, C),
            0x72 => self.bit(6, D),
            0x73 => self.bit(6, E),
            0x74 => self.bit(6, H),
            0x75 => self.bit(6, L),
            0x76 => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 6) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x77 => self.bit(6, A),

            0x78 => self.bit(7, B),
            0x79 => self.bit(7, C),
            0x7A => self.bit(7, D),
            0x7B => self.bit(7, E),
            0x7C => self.bit(7, H),
            0x7D => self.bit(7, L),
            0x7E => {
                let value = mmu.read_byte(self.registers.hl());
                let bit_is_set = value & (1 << 7) > 0;
                self.registers
                    .set_flags(bit_is_set, false, true, self.registers.carry_flag());
                4
            }
            0x7F => self.bit(7, A),

            0x80 => self.res(0, B),
            0x81 => self.res(0, C),
            0x82 => self.res(0, D),
            0x83 => self.res(0, E),
            0x84 => self.res(0, H),
            0x85 => self.res(0, L),
            0x86 => {
                let mask = !(1 << 0);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0x87 => self.res(0, A),

            0x88 => self.res(1, B),
            0x89 => self.res(1, C),
            0x8A => self.res(1, D),
            0x8B => self.res(1, E),
            0x8C => self.res(1, H),
            0x8D => self.res(1, L),
            0x8E => {
                let mask = !(1 << 1);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0x8F => self.res(1, A),

            0x90 => self.res(2, B),
            0x91 => self.res(2, C),
            0x92 => self.res(2, D),
            0x93 => self.res(2, E),
            0x94 => self.res(2, H),
            0x95 => self.res(2, L),
            0x96 => {
                let mask = !(1 << 2);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0x97 => self.res(2, A),

            0x98 => self.res(3, B),
            0x99 => self.res(3, C),
            0x9A => self.res(3, D),
            0x9B => self.res(3, E),
            0x9C => self.res(3, H),
            0x9D => self.res(3, L),
            0x9E => {
                let mask = !(1 << 3);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0x9F => self.res(3, A),

            0xA0 => self.res(4, B),
            0xA1 => self.res(4, C),
            0xA2 => self.res(4, D),
            0xA3 => self.res(4, E),
            0xA4 => self.res(4, H),
            0xA5 => self.res(4, L),
            0xA6 => {
                let mask = !(1 << 4);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xA7 => self.res(4, A),

            0xA8 => self.res(5, B),
            0xA9 => self.res(5, C),
            0xAA => self.res(5, D),
            0xAB => self.res(5, E),
            0xAC => self.res(5, H),
            0xAD => self.res(5, L),
            0xAE => {
                let mask = !(1 << 5);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xAF => self.res(5, A),

            0xB0 => self.res(6, B),
            0xB1 => self.res(6, C),
            0xB2 => self.res(6, D),
            0xB3 => self.res(6, E),
            0xB4 => self.res(6, H),
            0xB5 => self.res(6, L),
            0xB6 => {
                let mask = !(1 << 6);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xB7 => self.res(6, A),

            0xB8 => self.res(7, B),
            0xB9 => self.res(7, C),
            0xBA => self.res(7, D),
            0xBB => self.res(7, E),
            0xBC => self.res(7, H),
            0xBD => self.res(7, L),
            0xBE => {
                let mask = !(1 << 7);
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value & mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xBF => self.res(7, A),

            0xC0 => self.set(0, B),
            0xC1 => self.set(0, C),
            0xC2 => self.set(0, D),
            0xC3 => self.set(0, E),
            0xC4 => self.set(0, H),
            0xC5 => self.set(0, L),
            0xC6 => {
                let mask = 1 << 0;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xC7 => self.set(0, A),

            0xC8 => self.set(1, B),
            0xC9 => self.set(1, C),
            0xCA => self.set(1, D),
            0xCB => self.set(1, E),
            0xCC => self.set(1, H),
            0xCD => self.set(1, L),
            0xCE => {
                let mask = 1 << 1;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xCF => self.set(1, A),

            0xD0 => self.set(2, B),
            0xD1 => self.set(2, C),
            0xD2 => self.set(2, D),
            0xD3 => self.set(2, E),
            0xD4 => self.set(2, H),
            0xD5 => self.set(2, L),
            0xD6 => {
                let mask = 1 << 2;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xD7 => self.set(2, A),

            0xD8 => self.set(3, B),
            0xD9 => self.set(3, C),
            0xDA => self.set(3, D),
            0xDB => self.set(3, E),
            0xDC => self.set(3, H),
            0xDD => self.set(3, L),
            0xDE => {
                let mask = 1 << 3;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xDF => self.set(3, A),

            0xE0 => self.set(4, B),
            0xE1 => self.set(4, C),
            0xE2 => self.set(4, D),
            0xE3 => self.set(4, E),
            0xE4 => self.set(4, H),
            0xE5 => self.set(4, L),
            0xE6 => {
                let mask = 1 << 4;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xE7 => self.set(4, A),

            0xE8 => self.set(5, B),
            0xE9 => self.set(5, C),
            0xEA => self.set(5, D),
            0xEB => self.set(5, E),
            0xEC => self.set(5, H),
            0xED => self.set(5, L),
            0xEE => {
                let mask = 1 << 5;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xEF => self.set(5, A),

            0xF0 => self.set(6, B),
            0xF1 => self.set(6, C),
            0xF2 => self.set(6, D),
            0xF3 => self.set(6, E),
            0xF4 => self.set(6, H),
            0xF5 => self.set(6, L),
            0xF6 => {
                let mask = 1 << 6;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xF7 => self.set(6, A),

            0xF8 => self.set(7, B),
            0xF9 => self.set(7, C),
            0xFA => self.set(7, D),
            0xFB => self.set(7, E),
            0xFC => self.set(7, H),
            0xFD => self.set(7, L),
            0xFE => {
                let mask = 1 << 7;
                let value = mmu.read_byte(self.registers.hl());
                let new_value = value | mask;
                mmu.write_byte(self.registers.hl(), new_value);
                4
            }
            0xFF => self.set(7, A),
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
    fn test_srl() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b10001010;
        cpu.srl(A);
        assert_eq!(cpu.registers[A], 0b01000101);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.sra(A);
        assert_eq!(cpu.registers[A], 0b00100010);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_sra() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b10001010;
        cpu.sra(A);
        assert_eq!(cpu.registers[A], 0b11000101);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.sra(A);
        assert_eq!(cpu.registers[A], 0b11100010);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_sla() {
        let mut cpu = CPU::new();
        cpu.registers[A] = 0b10001010;
        cpu.sla(A);
        assert_eq!(cpu.registers[A], 0b00010100);
        assert_eq!(cpu.registers.carry_flag(), true);

        cpu.sla(A);
        assert_eq!(cpu.registers[A], 0b00101000);
        assert_eq!(cpu.registers.carry_flag(), false);
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

        cpu.registers.a = 1;
        cpu.add_a(2);
        assert_eq!(cpu.registers.a, 3);
        assert_eq!(cpu.registers.f, 0);

        cpu.registers.a = 0x0F;
        cpu.add_a(0x01);
        assert_eq!(cpu.registers.a, 0x10);
        assert_eq!(cpu.registers.f, 0b00100000);

        cpu.registers.a = 0xF0;
        cpu.add_a(0x10);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, 0b10010000);

        cpu.registers.a = 0xF0;
        cpu.add_a(0x11);
        assert_eq!(cpu.registers.a, 0x01);
        assert_eq!(cpu.registers.f, 0b00010000);
    }

    #[test]
    fn test_sub_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 1;
        cpu.sub_a(2);
        assert_eq!(cpu.registers.a, 0xFF);
        assert_eq!(cpu.registers.f, 0b01110000);

        cpu.registers.a = 0x10;
        cpu.sub_a(0x01);
        assert_eq!(cpu.registers.a, 0x0F);
        assert_eq!(cpu.registers.f, 0b01100000);

        cpu.registers.a = 0x10;
        cpu.sub_a(0x10);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, 0b11000000);

        cpu.registers.a = 0xF0;
        cpu.sub_a(0x10);
        assert_eq!(cpu.registers.a, 0xE0);
        assert_eq!(cpu.registers.f, 0b01000000);
    }

    #[test]
    fn test_xor_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 0b0111;
        cpu.xor_a(0b1101);
        assert_eq!(cpu.registers.a, 0b1010);
        assert_eq!(cpu.registers.f, 0);

        cpu.registers.a = 0x10;
        cpu.xor_a(0x10);
        assert_eq!(cpu.registers.a, 0);
        assert_eq!(cpu.registers.f, 0b10000000);
    }

    #[test]
    fn test_or_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 0b0111;
        cpu.or_a(0b1101);
        assert_eq!(cpu.registers.a, 0b1111);
        assert_eq!(cpu.registers.f, 0);

        cpu.registers.a = 0x00;
        cpu.or_a(0x00);
        assert_eq!(cpu.registers.a, 0);
        assert_eq!(cpu.registers.f, 0b10000000);
    }

    #[test]
    fn test_cp_a() {
        let mut cpu = CPU::new();

        cpu.registers.a = 5;
        cpu.cp_a(5);
        assert_eq!(cpu.registers.f, 0b11000000);

        cpu.registers.a = 0x03;
        cpu.cp_a(0x01);
        assert_eq!(cpu.registers.f, 0b01000000);

        cpu.registers.a = 0x10;
        cpu.cp_a(0x03);
        assert_eq!(cpu.registers.f, 0b01100000);

        cpu.registers.a = 0x00;
        cpu.cp_a(0x33);
        assert_eq!(cpu.registers.f, 0b01110000);
    }

    #[test]
    fn test_0x3a_load_hl_ptr_into_a_dec() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.a = 0;
        mmu.memory[0x1234] = 0xAA;

        cpu.execute(0x3A, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1233);
        assert_eq!(cpu.registers.a, 0xAA);
    }

    #[test]
    fn test_0x38_jump_relative_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0 | 0b00010000;
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;

        let cycles = cpu.execute(0x38, &mut mmu);

        assert_eq!(cpu.pc, 0x10);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_0x37_jump_relative_carry() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0;

        cpu.execute(0x37, &mut mmu);

        assert_eq!(cpu.registers.f, 0b00010000);
    }

    #[test]
    fn test_0x36_load_u8_into_hl_ptr() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        mmu.memory[cpu.pc as usize] = 0xAA;
        mmu.memory[cpu.registers.hl() as usize] = 0;

        cpu.execute(0x36, &mut mmu);

        assert_eq!(mmu.memory[cpu.registers.hl() as usize], 0xAA);
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
        cpu.registers.f = 0;
        cpu.pc = 0;
        mmu.memory[cpu.pc as usize] = 0x10;

        let cycles = cpu.execute(0x30, &mut mmu);

        assert_eq!(cpu.pc, 0x10);
        assert_eq!(cycles, 3);
    }

    #[test]
    fn test_0x2f_compliment() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.a = 0b10101010;
        cpu.registers.f = 0b10010000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.registers.a, 0b01010101);
        assert_flags(&cpu, true, true, true, true);

        cpu.registers.a = 0b11110000;
        cpu.registers.f = 0b10010000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.registers.a, 0b00001111);
        assert_flags(&cpu, true, true, true, true);

        cpu.registers.a = 0b00001111;
        cpu.registers.f = 0b00000000;
        cpu.execute(0x2F, &mut mmu);
        assert_eq!(cpu.registers.a, 0b11110000);
        assert_flags(&cpu, false, true, true, false);
    }

    #[test]
    fn test_0x2a_load_hl_ptr_into_a() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.a = 0;
        mmu.memory[0x1234] = 0xAA;

        cpu.execute(0x2A, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1235);
        assert_eq!(cpu.registers.a, 0xAA);
    }

    #[test]
    fn test_0x29_add_hl_hl() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x2468);
        assert_flags(&cpu, true, false, false, false);

        cpu.registers.h = 0x00;
        cpu.registers.l = 0xFF;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x01FE);
        assert_flags(&cpu, true, false, true, false);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0xFFFE);
        assert_flags(&cpu, false, false, true, true);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0x00;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x29, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0xFE00);
        assert_flags(&cpu, true, false, false, true);
    }

    #[test]
    fn test_0x28_jump_relative_if_z() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0b10000000; // zero, jump_relative
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

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x1235);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x0);
    }

    #[test]
    fn test_0x22_load_a_into_hl_ptr() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.a = 0xAA;
        mmu.memory[0x1234] = 0x00;

        cpu.execute(0x22, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1235);
        assert_eq!(mmu.memory[0x1234], 0xAA);
    }

    #[test]
    fn test_0x21_load_u16_into_hl() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();

        cpu.registers.h = 0x00;
        cpu.registers.l = 0x00;
        cpu.pc = 0x0000;
        mmu.memory[cpu.pc as usize] = 0x34; // little endian
        mmu.memory[cpu.pc as usize + 1] = 0x12;

        cpu.execute(0x21, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1234);
        assert_eq!(cpu.pc, 0x0002);
    }

    #[test]
    fn test_0x20_jump_relative_not_zero() {
        let (mut cpu, mut mmu) = get_cpu_and_mmu();
        cpu.registers.f = 0b00000000;
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

        cpu.registers.a = 0x00;
        cpu.registers.d = 0x56;
        cpu.registers.e = 0x78;
        mmu.memory[0x5678] = 0x12;

        cpu.execute(0x1A, &mut mmu);

        assert_eq!(cpu.registers.a, 0x12);
    }

    #[test]
    fn test_0x1f_rra_rotate_a_right_carry() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b00000001;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b00000000);
        assert_flags(&cpu, false, false, false, true);

        cpu.registers.f = 0b00010000;
        cpu.registers.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111111);
        assert_flags(&cpu, false, false, false, false);

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b01111111);
        assert_flags(&cpu, false, false, false, false);
    }

    #[test]
    fn test_0x19_add_hl_de() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.d = 0x56;
        cpu.registers.e = 0x78;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1234 + 0x5678);
        assert_flags(&cpu, true, false, false, false);

        cpu.registers.h = 0x00;
        cpu.registers.l = 0xFF;
        cpu.registers.d = 0x12;
        cpu.registers.e = 0x01;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x1300);
        assert_flags(&cpu, true, false, true, false);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.registers.d = 0x00;
        cpu.registers.e = 0x01;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x0);
        assert_flags(&cpu, true, false, true, true);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0x00;
        cpu.registers.d = 0x01;
        cpu.registers.e = 0x00;
        cpu.registers.f = 0b11000000;

        cpu.execute(0x19, &mut mmu);

        assert_eq!(cpu.registers.hl(), 0x0);
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

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b10000000;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.registers.a, 0b00000000);
        assert_flags(&cpu, false, false, false, true);

        cpu.registers.f = 0b00010000;
        cpu.registers.a = 0b01111111;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111111);
        assert_flags(&cpu, false, false, false, false);

        cpu.registers.f = 0b00000000;
        cpu.registers.a = 0b01111111;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111110);
        assert_eq!(cpu.registers.zero_flag(), false); // Reset
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
        assert_eq!(cpu.registers.half_carry_flag(), false); // Reset
        assert_eq!(cpu.registers.carry_flag(), false);
    }

    #[test]
    fn test_load_u8_into_register() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x16, &mut mmu);

        assert_eq!(cpu.registers.d, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.registers.e = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x1E, &mut mmu);

        assert_eq!(cpu.registers.e, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.registers.l = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x2E, &mut mmu);

        assert_eq!(cpu.registers.l, 0xAB);
        assert_eq!(cpu.pc, 0x0101);

        cpu.registers.h = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x26, &mut mmu);

        assert_eq!(cpu.registers.l, 0xAB);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn test_dec_u8_and_set_flagss() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0x12;
        cpu.execute(0x15, &mut mmu);
        assert_eq!(cpu.registers.d, 0x11);

        cpu.registers.d = 0x00;
        cpu.execute(0x15, &mut mmu);
        assert_eq!(cpu.registers.d, 0xFF);

        cpu.registers.e = 0x12;
        cpu.execute(0x1D, &mut mmu);
        assert_eq!(cpu.registers.e, 0x11);

        cpu.registers.e = 0x00;
        cpu.execute(0x1D, &mut mmu);
        assert_eq!(cpu.registers.e, 0xFF);

        cpu.registers.h = 0x12;
        cpu.execute(0x25, &mut mmu);
        assert_eq!(cpu.registers.h, 0x11);

        cpu.registers.h = 0x00;
        cpu.execute(0x25, &mut mmu);
        assert_eq!(cpu.registers.h, 0xFF);

        cpu.registers.l = 0x12;
        cpu.execute(0x2D, &mut mmu);
        assert_eq!(cpu.registers.l, 0x11);

        cpu.registers.l = 0x00;
        cpu.execute(0x2D, &mut mmu);
        assert_eq!(cpu.registers.l, 0xFF);
    }

    #[test]
    fn test_inc_u8_and_set_flagss() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0x12;
        cpu.execute(0x14, &mut mmu);
        assert_eq!(cpu.registers.d, 0x13);

        cpu.registers.d = 0xFF;
        cpu.execute(0x14, &mut mmu);
        assert_eq!(cpu.registers.d, 0x00);

        cpu.registers.e = 0x12;
        cpu.execute(0x1C, &mut mmu);
        assert_eq!(cpu.registers.e, 0x13);

        cpu.registers.e = 0xFF;
        cpu.execute(0x1C, &mut mmu);
        assert_eq!(cpu.registers.e, 0x00);

        cpu.registers.h = 0x12;
        cpu.execute(0x24, &mut mmu);
        assert_eq!(cpu.registers.h, 0x13);

        cpu.registers.h = 0xFF;
        cpu.execute(0x24, &mut mmu);
        assert_eq!(cpu.registers.h, 0x00);

        cpu.registers.l = 0x12;
        cpu.execute(0x2C, &mut mmu);
        assert_eq!(cpu.registers.l, 0x13);

        cpu.registers.l = 0xFF;
        cpu.execute(0x2C, &mut mmu);
        assert_eq!(cpu.registers.l, 0x00);
    }

    #[test]
    fn test_dec_16_bit_registers() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.d = 0x12;
        cpu.registers.e = 0x34;
        cpu.execute(0x1B, &mut mmu);
        assert_eq!(cpu.registers.de(), 0x1233);

        cpu.registers.d = 0x00;
        cpu.registers.e = 0x00;
        cpu.execute(0x1B, &mut mmu);
        assert_eq!(cpu.registers.de(), 0xFFFF);

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.execute(0x2B, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x1233);

        cpu.registers.h = 0x00;
        cpu.registers.l = 0x00;
        cpu.execute(0x2B, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0xFFFF);

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

        cpu.registers.d = 0x12;
        cpu.registers.e = 0x34;
        cpu.execute(0x13, &mut mmu);
        assert_eq!(cpu.registers.de(), 0x1235);

        cpu.registers.d = 0xFF;
        cpu.registers.e = 0xFF;
        cpu.execute(0x13, &mut mmu);
        assert_eq!(cpu.registers.de(), 0);

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0x1235);

        cpu.registers.h = 0xFF;
        cpu.registers.l = 0xFF;
        cpu.execute(0x23, &mut mmu);
        assert_eq!(cpu.registers.hl(), 0);

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

        cpu.registers.a = 0xAB;
        cpu.registers.d = 0x12;
        cpu.registers.e = 0x34;
        mmu.memory[0x1234] = 0x00;

        cpu.execute(0x12, &mut mmu);

        assert_eq!(mmu.memory[0x1234], 0xAB);
        assert_eq!(cpu.registers.de(), 0x1234);
    }

    #[test]
    fn test_0x11_load_u16_into_de() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.pc = 0x0005;
        mmu.memory[0x0005] = 0x34;
        mmu.memory[0x0006] = 0x12;

        cpu.execute(0x11, &mut mmu);

        assert_eq!(cpu.registers.de(), 0x1234);
        assert_eq!(cpu.pc, 0x0007);
    }

    #[test]
    fn test_0x0f_rrca_right_rotate_a_circular() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0b11111110;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b01111111);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), false);

        cpu.registers.a = 0b00000001;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.registers.a, 0b10000000);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true);
    }

    #[test]
    fn test_0x0e_load_u8_into_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.c = 0x0;
        cpu.pc = 0x1234;
        mmu.memory[0x1234] = 0xAB;

        cpu.execute(0x0E, &mut mmu);

        assert_eq!(cpu.registers.c, 0xAB);
        assert_eq!(cpu.pc, 0x1235);
    }

    #[test]
    fn test_0x0d_decrement_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.c = 0x01;
        cpu.registers.f = 0b00110000; // Set carry flag

        cpu.execute(0x0D, &mut mmu);

        assert_eq!(cpu.registers.c, 0x00);
        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.subtract_flag(), true);
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified

        cpu.registers.c = 0x00;
        cpu.registers.f = 0b00010000; // Set carry flag
        cpu.execute(0x0D, &mut mmu);

        assert_eq!(cpu.registers.c, 0xFF);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.subtract_flag(), true);
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified
    }

    #[test]
    fn test_0x0c_increment_c() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.c = 0x12;
        cpu.registers.f = 0b11110000; // Set carry flag
        cpu.execute(0x0C, &mut mmu);

        assert_eq!(cpu.registers.c, 0x13);
        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.subtract_flag(), false); // Resets
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified

        cpu.registers.c = 0xFF;
        cpu.registers.f = 0b01010000; // Set carry flag
        cpu.execute(0x0C, &mut mmu);

        assert_eq!(cpu.registers.c, 0x00);
        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.subtract_flag(), false); // Resets
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), true); // Unmodified
    }

    #[test]
    fn test_0x0b_decrement_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0x12;
        cpu.registers.c = 0x34;

        cpu.execute(0x0B, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0x1233);
    }

    #[test]
    fn test_0x0a_load_bc_addr_to_a() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0x00;
        cpu.registers.b = 0x56;
        cpu.registers.c = 0x78;
        mmu.memory[0x5678] = 0x12;

        cpu.execute(0x0A, &mut mmu);

        assert_eq!(cpu.registers.a, 0x12);
    }

    #[test]
    fn test_0x09_add_hl_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.h = 0x12;
        cpu.registers.l = 0x34;
        cpu.registers.b = 0x56;
        cpu.registers.c = 0x78;
        cpu.registers.f = 0b01000000;

        cpu.execute(0x09, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0x5678);
        assert_eq!(cpu.registers.hl(), 0x1234 + 0x5678);
        assert_eq!(cpu.registers.subtract_flag(), false); // Resets
        assert_eq!(cpu.registers.half_carry_flag(), false); //TODO
        assert_eq!(cpu.registers.carry_flag(), false); //TODO
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

        cpu.registers.a = 0b10000000;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.registers.a, 0b00000001);
        assert_eq!(cpu.registers.zero_flag(), false); // Reset
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
        assert_eq!(cpu.registers.half_carry_flag(), false); // Reset
        assert_eq!(cpu.registers.carry_flag(), true);

        cpu.registers.a = 0b01111111;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.registers.a, 0b11111110);
        assert_eq!(cpu.registers.zero_flag(), false); // Reset
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
        assert_eq!(cpu.registers.half_carry_flag(), false); // Reset
        assert_eq!(cpu.registers.carry_flag(), false);
    }

    #[test]
    fn test_0x06_load_u8_into_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xFF;
        cpu.pc = 0x0100;
        mmu.write_byte(0x0100, 0xAB);

        cpu.execute(0x06, &mut mmu);

        assert_eq!(cpu.registers.b, 0xAB);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn test_0x05_decrement_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xFF;
        cpu.registers.f = 0b11110000;

        cpu.execute(0x05, &mut mmu);

        assert_eq!(cpu.registers.b, 0xFE);
        assert_eq!(cpu.registers.zero_flag(), false); // Not zero
        assert_eq!(cpu.registers.subtract_flag(), true); // Always true
                                                         //TODO - assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unaffected

        cpu.registers.b = 0x01;
        cpu.registers.f = 0b11100000;

        cpu.execute(0x06, &mut mmu);

        assert_eq!(cpu.registers.b, 0x00);
        assert_eq!(cpu.registers.zero_flag(), true); // Not zero
        assert_eq!(cpu.registers.subtract_flag(), true); // Always true
                                                         //TODO - assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), false); // Unaffected
    }

    #[test]
    fn test_0x04_increment_b() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xEE;
        cpu.registers.f = 0b11110000;

        cpu.execute(0x04, &mut mmu);

        assert_eq!(cpu.registers.b, 0xEF);
        assert_eq!(cpu.registers.zero_flag(), false); // Not zero
        assert_eq!(cpu.registers.subtract_flag(), false); // Reset
                                                          //TODO - assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true); // Unaffected
    }

    #[test]
    fn test_0x03_increment_bc() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.b = 0xEE;
        cpu.registers.c = 0xE0;

        cpu.execute(0x03, &mut mmu);

        assert_eq!(cpu.registers.bc(), 0xEEE1);
    }

    #[test]
    fn test_0x02_load_a_into_bc_addr() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.registers.a = 0xFF;
        cpu.registers.b = 0x12;
        cpu.registers.c = 0x34;
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

        assert_eq!(cpu.registers.bc(), 0x1234);
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

        cpu.registers.a = 0xDE;
        cpu.registers.f = 0xAD;

        cpu.registers.b = 0xBE;
        cpu.registers.c = 0xEF;

        cpu.registers.d = 0x12;
        cpu.registers.e = 0x34;

        cpu.registers.h = 0xAB;
        cpu.registers.l = 0xCD;

        assert_eq!(cpu.registers.af(), 0xDEAD);
        assert_eq!(cpu.registers.bc(), 0xBEEF);
        assert_eq!(cpu.registers.de(), 0x1234);
        assert_eq!(cpu.registers.hl(), 0xABCD);
    }

    #[test]
    fn test_set_flags() {
        let mut cpu = CPU::new();

        cpu.registers.f = 0x00;
        cpu.registers.set_flags(true, false, false, true);

        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.subtract_flag(), false);
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), true);

        cpu.registers.set_flags(true, true, true, false);

        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.subtract_flag(), true);
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), false);
    }

    #[test]
    fn test_flags() {
        let mut cpu = CPU::new();

        cpu.registers.a = 0xDE;
        cpu.registers.f = 0xF0;

        assert_eq!(cpu.registers.zero_flag(), true);
        assert_eq!(cpu.registers.subtract_flag(), true);
        assert_eq!(cpu.registers.half_carry_flag(), true);
        assert_eq!(cpu.registers.carry_flag(), true);

        cpu.registers.f = 0x00;

        assert_eq!(cpu.registers.zero_flag(), false);
        assert_eq!(cpu.registers.subtract_flag(), false);
        assert_eq!(cpu.registers.half_carry_flag(), false);
        assert_eq!(cpu.registers.carry_flag(), false);
    }

    #[test]
    fn test_initial_register_values() {
        let cpu = CPU::new();

        assert_eq!(cpu.registers.af(), 0x01B0);
        assert_eq!(cpu.registers.bc(), 0x0013);
        assert_eq!(cpu.registers.de(), 0x00D8);
        assert_eq!(cpu.registers.hl(), 0x014D);
        assert_eq!(cpu.sp, 0xFFFE);
        assert_eq!(cpu.pc, 0x0100);
    }

    fn assert_flags(cpu: &CPU, zero: bool, subtract: bool, half_carry: bool, carry: bool) {
        assert_eq!(cpu.registers.zero_flag(), zero);
        assert_eq!(cpu.registers.subtract_flag(), subtract);
        assert_eq!(cpu.registers.half_carry_flag(), half_carry);
        assert_eq!(cpu.registers.carry_flag(), carry);
    }
}
