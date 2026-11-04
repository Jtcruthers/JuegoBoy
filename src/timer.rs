use crate::mmu::Memory;

enum Frequency {
    Hz4096 = 0b00,   // 1024 cycles
    Hz262144 = 0b01, // 16 cycles
    Hz65536 = 0b10,  // 64 cycles
    Hz16384 = 0b11,  // 256 cycles
}

pub struct Timer {
    system_counter: u16, // div is the upper byte
    tima: u8,
    tima_cycles: u64,
    tma: u8,
    previous_tma: u8,
    tac: Frequency,
    enabled: bool,
    pub interrupt: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            system_counter: 0,
            tima: 0,
            tima_cycles: 0,
            tma: 0,
            previous_tma: 0,
            tac: Frequency::Hz4096,
            enabled: true,
            interrupt: false,
        }
    }

    pub fn step(&mut self, cycles: u8) {
        // counter increments on t-cycles, but we use m-cycles
        self.system_counter = self.system_counter.wrapping_add(cycles as u16 * 4);
        self.tima_cycles = self.tima_cycles.wrapping_add(cycles as u64 * 4);

        if self.enabled {
            let max_cycles = match self.tac {
                Frequency::Hz4096 => 256,
                Frequency::Hz262144 => 4,
                Frequency::Hz65536 => 16,
                Frequency::Hz16384 => 64,
            };

            while self.tima_cycles >= max_cycles {
                self.tima_cycles -= max_cycles;
                self.tima = self.tima.wrapping_add(1);

                if self.tima == 0 {
                    // When tima overflows, reset to vlaue in TMA and request interrupt
                    self.tima = self.previous_tma;
                    self.interrupt = true;
                }
            }
        }

        // After ticking, the previous tma is no longer needed, so reset it to tma
        // This way, if a tma write is executed on same m-cycle as content of tma is transffered to
        // tima, the old value is used
        self.previous_tma = self.tma;
    }

    fn div(&self) -> u8 {
        (self.system_counter >> 8) as u8
    }
}

impl Memory for Timer {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF04 => self.div(),
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
            0xFF04 => self.system_counter = 0,
            0xFF05 => self.tima = value,
            0xFF06 => self.tma = value,
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
        assert_eq!(timer.div(), 0);

        timer.step(63);
        assert_eq!(timer.div(), 1);
    }

    #[test]
    fn test_inc_div_when_disabled() {
        let mut timer = Timer::new();
        timer.enabled = false;
        timer.step(1);
        assert_eq!(timer.div(), 0);

        timer.step(63);
        assert_eq!(timer.div(), 1);
    }

    #[test]
    fn test_div_write_resets_counter() {
        let mut timer = Timer::new();
        // halfway to the first DIV increment
        timer.step(32);
        timer.write_byte(0xFF04, 0x01);
        assert_eq!(timer.div(), 0);

        // rest of way to increment
        timer.step(33);
        assert_eq!(timer.div(), 0);
        timer.step(32);
        assert_eq!(timer.div(), 1);
    }
}
