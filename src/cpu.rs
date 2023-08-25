pub struct Cpu {
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
    memory: [u16; 0xFFFF],
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_8bit_regs_to_16_bit_regs() {
        let mut cpu = Cpu::new();

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
    fn test_flags() {
        let mut cpu = Cpu::new();

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
        let mut cpu = Cpu::new();

        assert_eq!(cpu.af(), 0x01B0);
        assert_eq!(cpu.bc(), 0x0013);
        assert_eq!(cpu.de(), 0x00D8);
        assert_eq!(cpu.hl(), 0x014D);
        assert_eq!(cpu.sp, 0xFFFE);
        assert_eq!(cpu.pc, 0x0100);
    }
}
