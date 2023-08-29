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
        let upper = mmu.read_byte(address) as u16;
        let lower = mmu.read_byte(address + 1) as u16;

        (upper << 8) + lower
    }

    fn inc_8_bit_register(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);

        let z = result == 0;
        let n = false;
        let h = result < value;
        let c = self.carry_flag();
        self.set_flags(z, n, h, c);

        result
    }

    fn dec_8_bit_register(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);

        let z = result == 0;
        let n = true;
        let h = result > value;
        let c = self.carry_flag();
        self.set_flags(z, n, h, c);

        result
    }

    pub fn execute(&mut self, opcode: u8, mmu: &mut impl Memory) -> u8 {
        match opcode {
            0x00 => 1,
            0x01 => {
                self.b = mmu.read_byte(self.pc);
                self.pc += 1;
                self.c = mmu.read_byte(self.pc);
                self.pc += 1;
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
                self.b = self.inc_8_bit_register(self.b);
                1
            }
            0x05 => {
                self.b = self.dec_8_bit_register(self.b);
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
            },
            0x08 => {
                let addr = self.read_u16(self.pc, mmu);
                self.pc += 2;

                let upper = (self.sp >> 8) as u8;
                let lower = (self.sp & 0x00FF) as u8; 
                mmu.write_byte(addr + 1, upper); // little endian
                mmu.write_byte(addr, lower);
                5
            },
            0x09 => {
                let value = self.hl() + self.bc();
                self.h = ((value & 0xFF00) >> 8) as u8;
                self.l = (value & 0x00FF) as u8;
                self.set_flags(self.f & FLAG_MASK_ZERO > 0, false, false, false); //TODO - carry and half carry
                2
            },
            0x0A => {
                self.a = mmu.read_byte(self.bc());
                2
            },
            0x0B => {
                let (upper, lower) = self.u16_to_u8s(self.bc().wrapping_sub(1));
                self.b = upper;
                self.c = lower;
                2
            },
            0x0C => {
                self.c = self.inc_8_bit_register(self.c);
                1
            },
            0x0D => {
                self.c = self.dec_8_bit_register(self.c);
                1
            },
            0x0E => {
                self.c = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            },
            0x0F => {
                let carry_bit = self.a & 1;
                self.a = (self.a >> 1) + (carry_bit << 7);
                self.set_flags(false, false, false, carry_bit > 0);
                1
            },
            0x10 => {
                // TODO - implement stop
                1
            },
            0x11 => {
                let value = self.read_u16(self.pc, mmu);
                self.pc += 2;

                let (upper, lower) = self.u16_to_u8s(value);
                self.d = upper;
                self.e = lower;
                3
            },
            0x12 => {
                mmu.write_byte(self.de(), self.a);
                2
            },
            0x13 => {
                let (upper, lower) = self.u16_to_u8s(self.de().wrapping_add(1));
                self.d = upper;
                self.e = lower;
                2
            },
            0x14 => {
                self.d = self.inc_8_bit_register(self.d);
                1
            },
            0x15 => {
                self.d = self.dec_8_bit_register(self.d);
                1
            },
            0x16 => {
                self.d = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            },
            0x17 => {
                let old_carry = if self.carry_flag() { 1 } else { 0 };
                let new_carry = self.a & 0b10000000 > 0;
                self.a = (self.a << 1) + old_carry;
                self.set_flags(false, false, false, new_carry);
                1
            },
            0x1B => {
                let (upper, lower) = self.u16_to_u8s(self.de().wrapping_sub(1));
                self.d = upper;
                self.e = lower;
                2
            },
            0x1C => {
                self.e = self.inc_8_bit_register(self.e);
                1
            },
            0x1D => {
                self.e = self.dec_8_bit_register(self.e);
                1
            },
            0x1E => {
                self.e = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            },
            0x1F => {
                let old_carry = if self.carry_flag() { 1 } else { 0 };
                let new_carry = self.a & 1 > 0;
                self.a = (self.a >> 1) + (old_carry << 7);
                self.set_flags(false, false, false, new_carry);
                1
            },
            0x23 => {
                let (upper, lower) = self.u16_to_u8s(self.hl().wrapping_add(1));
                self.h = upper;
                self.l = lower;
                2
            },
            0x24 => {
                self.h = self.inc_8_bit_register(self.h);
                1
            },
            0x25 => {
                self.h = self.dec_8_bit_register(self.h);
                1
            },
            0x26 => {
                self.h = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            },
            0x2B => {
                let (upper, lower) = self.u16_to_u8s(self.hl().wrapping_sub(1));
                self.h = upper;
                self.l = lower;
                2
            },
            0x2C => {
                self.l = self.inc_8_bit_register(self.l);
                1
            },
            0x2D => {
                self.l = self.dec_8_bit_register(self.l);
                1
            },
            0x2E => {
                self.l = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            },
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                2
            },
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                2
            },
            _ => panic!("Unknown opcode: 0x{:X}", opcode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmu::TestMMU;

    #[test]
    fn test_0x1f_rra_rotate_a_right_carry() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.f = 0b00000000;
        cpu.a = 0b00000001;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.a, 0b00000000);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), true);

        cpu.f = 0b00010000;
        cpu.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.a, 0b11111111);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), false);

        cpu.f = 0b00000000;
        cpu.a = 0b11111110;

        cpu.execute(0x1F, &mut mmu);

        assert_eq!(cpu.a, 0b01111111);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), false);
    }

    #[test]
    fn test_0x17_rla_rotate_a_left_carry() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.f = 0b00000000;
        cpu.a = 0b10000000;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.a, 0b00000000);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), true);

        cpu.f = 0b00010000;
        cpu.a = 0b01111111;

        cpu.execute(0x17, &mut mmu);

        assert_eq!(cpu.a, 0b11111111);
        assert_eq!(cpu.zero_flag(), false); // Reset
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), false);

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
    fn test_dec_8_bit_registers() {
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
    fn test_inc_8_bit_registers() {
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
        mmu.memory[0x0005] = 0x12;
        mmu.memory[0x0006] = 0x34;

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
        mmu.memory[cpu.pc as usize] = 0xAB;
        mmu.memory[cpu.pc as usize + 1] = 0xCD;
        cpu.sp = 0xDEAD;

        cpu.execute(0x08, &mut mmu);

        assert_eq!(cpu.sp, 0xDEAD);
        assert_eq!(mmu.memory[0xABCD], 0xAD);
        assert_eq!(mmu.memory[0xABCD + 1], 0xDE);
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
        mmu.write_byte(0x100, 0xDE);
        mmu.write_byte(0x101, 0xAD);

        cpu.execute(0x01, &mut mmu);

        assert_eq!(cpu.bc(), 0xDEAD);
        assert_eq!(cpu.pc, 0x102);
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
}
