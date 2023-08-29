use crate::mmu::{Memory, MMU};

enum Register8 {
    A,
    B,
    C,
    D,
    E,
    F,
    H,
    L,
}

enum Register16 {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC,
}

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

fn add_16_reg() {

}

fn add(a: u8) {

}

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
                let bc = self.bc() + 1;
                self.b = ((bc & 0xFF00) >> 8) as u8;
                self.c = (bc & 0xFF) as u8;
                2
            }
            0x04 => {
                self.b += 1;

                let z = self.b == 0;
                let n = false;
                let h = false; //TODO
                let c = self.carry_flag();
                self.set_flags(z, n, h, c);
                1
            }
            0x05 => {
                self.b -= 1;

                let z = self.b == 0;
                let n = true;
                let h = false; //TODO
                let c = self.carry_flag();
                self.set_flags(z, n, h, c);
                1
            }
            0x06 => {
                self.b = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            }
            0x07 => {
                let c = self.a & 0b10000000 > 0;
                self.a = self.a << 1;
                self.set_flags(self.a == 0, false, false, c);
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
                let (upper, lower) = self.u16_to_u8s(self.bc() - 1);
                self.b = upper;
                self.c = lower;
                2
            },
            0x0C => {
                let old_c = self.c;
                self.c = self.c.wrapping_add(1);
                self.set_flags(self.c == 0, false, old_c > self.c, self.f & FLAG_MASK_CARRY > 0);
                1
            },
            0x0D => {
                let old_c = self.c;
                self.c = self.c.wrapping_sub(1);
                self.set_flags(self.c == 0, true, old_c < self.c, self.f & FLAG_MASK_CARRY > 0);
                1
            },
            0x0E => {
                self.c = mmu.read_byte(self.pc);
                self.pc += 1;
                2
            },
            0x0F => {
                let carry = self.a & 1 > 0;
                self.a = self.a >> 1;
                self.set_flags(self.a == 0, false, false, carry);
                1
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
    fn test_0x0f_right_rotate_a() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0b11111110;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.a, 0b01111111);
        assert_eq!(cpu.carry_flag(), false);

        cpu.a = 0b00000001;

        cpu.execute(0x0F, &mut mmu);

        assert_eq!(cpu.a, 0b00000000);
        assert_eq!(cpu.zero_flag(), true);
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
    fn test_0x07_rotate_a_left() {
        let mut cpu = CPU::new();
        let mut mmu = TestMMU::new();

        cpu.a = 0b10000000;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.zero_flag(), true);
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), true);

        cpu.a = 0b01111111;

        cpu.execute(0x07, &mut mmu);

        assert_eq!(cpu.a, 0b11111110);
        assert_eq!(cpu.zero_flag(), false);
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
