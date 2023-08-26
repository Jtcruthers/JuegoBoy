pub struct CPU {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    f: u8,
    h: u8,
    l: u8,
    sp: usize, // sp and pc should be u16, but since they are pointers (indexing
    pc: usize, // an array) we use usize
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
            memory: [0; 0xFFFF],
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
        self.f & 0b10000000 > 0
    }

    fn subtract_flag(&self) -> bool {
        self.f & 0b01000000 > 0
    }

    fn half_carry_flag(&self) -> bool {
        self.f & 0b00100000 > 0
    }

    fn carry_flag(&self) -> bool {
        self.f & 0b00010000 > 0
    }

    fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        let z_flag = if z { 0b1000 } else { 0 };
        let n_flag = if n { 0b0100 } else { 0 };
        let h_flag = if h { 0b0010 } else { 0 };
        let c_flag = if c { 0b0001 } else { 0 };

        self.f = (z_flag | n_flag | h_flag | c_flag) << 4;
    }

    fn execute(&mut self, opcode: u8) {
        match opcode {
            0x00 => (),
            0x01 => {
                self.b = self.memory[self.pc];
                self.pc += 1;
                self.c = self.memory[self.pc];
                self.pc += 1;
            },
            0x02 => {
                self.memory[self.bc() as usize] = self.a;
            },
            0x03 => {
                let bc = self.bc() + 1;
                self.b = ((bc & 0xFF00) >> 8) as u8;
                self.c = (bc & 0xFF) as u8;
            },
            0x04 => {
                self.b += 1;

                let z = self.b == 0;
                let n = false;
                let h = false; //TODO 
                let c = self.carry_flag();
                self.set_flags(z, n, h, c);
            },
            0x05 => {
                self.b -= 1;

                let z = self.b == 0;
                let n = true;
                let h = false; //TODO 
                let c = self.carry_flag();
                self.set_flags(z, n, h, c);
            },
            0x06 => {
                self.b = self.memory[self.pc];
                self.pc += 1;
            },
            0x07 => {
                let c = self.a & 0b10000000 > 0;
                self.a = self.a << 1;

                let z = self.a == 0;
                let n = false;
                let h = false;
                self.set_flags(z, n, h, c)
            },
            _ => ()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_0x07_rotate_a_left() {
        let mut cpu = CPU::new();

        cpu.a = 0b10000000;

        cpu.execute(0x07);

        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.zero_flag(), true);
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), true);

        cpu.a = 0b01111111;

        cpu.execute(0x07);

        assert_eq!(cpu.a, 0b11111110);
        assert_eq!(cpu.zero_flag(), false);
        assert_eq!(cpu.subtract_flag(), false); // Reset
        assert_eq!(cpu.half_carry_flag(), false); // Reset
        assert_eq!(cpu.carry_flag(), false);
    }

    #[test]
    fn test_0x06_load_u8_into_b() {
        let mut cpu = CPU::new();

        cpu.b = 0xFF;
        cpu.pc = 0x0100;
        cpu.memory[0x0100] = 0xAB;

        cpu.execute(0x06);

        assert_eq!(cpu.b, 0xAB);
        assert_eq!(cpu.pc, 0x0101);
    }

    #[test]
    fn test_0x05_decrement_b() {
        let mut cpu = CPU::new();

        cpu.b = 0xFF;
        cpu.f = 0b11110000;

        cpu.execute(0x05);

        assert_eq!(cpu.b, 0xFE);
        assert_eq!(cpu.zero_flag(), false); // Not zero
        assert_eq!(cpu.subtract_flag(), true); // Always true
        //TODO - assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), true); // Unaffected

        cpu.b = 0x01;
        cpu.f = 0b11100000;

        cpu.execute(0x05);

        assert_eq!(cpu.b, 0x00);
        assert_eq!(cpu.zero_flag(), true); // Not zero
        assert_eq!(cpu.subtract_flag(), true); // Always true
        //TODO - assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), false); // Unaffected
    }

    #[test]
    fn test_0x04_increment_b() {
        let mut cpu = CPU::new();

        cpu.b = 0xEE;
        cpu.f = 0b11110000;

        cpu.execute(0x04);

        assert_eq!(cpu.b, 0xEF);
        assert_eq!(cpu.zero_flag(), false); // Not zero
        assert_eq!(cpu.subtract_flag(), false); // Reset
        //TODO - assert_eq!(cpu.half_carry_flag(), false);
        assert_eq!(cpu.carry_flag(), true); // Unaffected
    }

    #[test]
    fn test_0x03_increment_bc() {
        let mut cpu = CPU::new();

        cpu.b = 0xEE;
        cpu.c = 0xE0;

        cpu.execute(0x03);

        assert_eq!(cpu.bc(), 0xEEE1);
    }

    #[test]
    fn test_0x02_load_a_into_bc_addr() {
        let mut cpu = CPU::new();

        cpu.a = 0xFF;
        cpu.b = 0x12;
        cpu.c = 0x34;
        cpu.memory[0x1234] = 0;

        cpu.execute(0x02);

        assert_eq!(cpu.memory[0x1234], 0xFF);
    }

    #[test]
    fn test_0x01_load_bc_u16() {
        let mut cpu = CPU::new();

        cpu.pc = 0x100;
        cpu.memory[0x100] = 0xDE;
        cpu.memory[0x101] = 0xAD;

        cpu.execute(0x01);

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
