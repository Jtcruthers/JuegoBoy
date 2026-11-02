use crate::mmu::Memory;

enum Frequency {
    Hz4096 = 0b00,   // 1024 cycles
    Hz262144 = 0b01, // 16 cycles
    Hz65536 = 0b10,  // 64 cycles
    Hz16384 = 0b11,  // 256 cycles
}

pub struct Timer {
    div: u16,
    div_cycles: u8,
    tima: u8,
    tima_cycles: u64,
    tma: u8,
    tac: Frequency,
    enabled: bool,
    pub interrupt: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            div: 0,
            div_cycles: 0,
            tima: 0,
            tima_cycles: 0,
            tma: 0,
            tac: Frequency::Hz4096,
            enabled: true,
            interrupt: false,
        }
    }

    pub fn step(&mut self, cycles: u8) {
        if self.div_cycles.checked_add(cycles).is_none() {
            self.div = self.div.wrapping_add(1);
        }
        self.div_cycles = self.div_cycles.wrapping_add(cycles);

        if self.enabled {
            let max_cycles = match self.tac {
                Frequency::Hz4096 => 1024,
                Frequency::Hz262144 => 16,
                Frequency::Hz65536 => 64,
                Frequency::Hz16384 => 256,
            };

            if self.tima_cycles > max_cycles {
                self.tima_cycles -= max_cycles;
                self.tima = self.tima.wrapping_add(1);

                if self.tima == 0 {
                    self.tima = self.tma;
                    self.interrupt = true;
                }
            }
        }
    }
}

impl Memory for Timer {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF04 => ((self.div >> 8) & 0xFF) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            0xFF07 => {
                let frequency = match self.tac {
                    Frequency::Hz4096 => 0b00,
                    Frequency::Hz262144 => 0b01,
                    Frequency::Hz65536 => 0b10,
                    Frequency::Hz16384 => 0b11,
                };

                let enabled = if self.enabled { 0b100 } else { 0b000 };

                return enabled | frequency;
            }
            _ => panic!("Invalid Timer read address: {:X}", address),
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0xFF04 => self.div &= 0x00FF,
            0xFF05 => (),
            0xFF06 => (),
            0xFF07 => {
                self.tac = match value & 0b11 {
                    0b00 => Frequency::Hz4096,
                    0b01 => Frequency::Hz262144,
                    0b10 => Frequency::Hz65536,
                    0b11 => Frequency::Hz16384,
                    _ => panic!("Invalid Timer frequency?"),
                };

                self.enabled = value & 0b100 > 0;
            }
            _ => panic!("Invalid Timer write address: {:X}", address),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inc_div() {
        let mut timer = Timer::new();
        timer.step(1);
        assert_eq!(timer.div, 0);

        timer.step(255);
        assert_eq!(timer.div, 1);
    }

    #[test]
    fn test_inc_div_when_disabled() {
        let mut timer = Timer::new();
        timer.enabled = false;
        timer.step(1);
        assert_eq!(timer.div, 0);

        timer.step(255);
        assert_eq!(timer.div, 1);
    }
}
