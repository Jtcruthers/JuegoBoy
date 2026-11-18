pub struct APU {
    nr52: u8,
    nr51: u8,
    nr50: u8,
    nr10: u8,
    nr11: u8,
    nr12: u8,
    nr13: u8,
    nr14: u8,
    nr21: u8,
    nr22: u8,
    nr23: u8,
    nr24: u8,
    nr30: u8,
    nr31: u8,
    nr32: u8,
    nr33: u8,
    nr34: u8,
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,
    wave_ram: [u8; 16],

    div_apu: u32,
    timer_div_last: u8,
}

impl APU {
    pub fn new() -> Self {
        APU {
            nr52: 0x00,
            nr51: 0x00,
            nr50: 0x00,
            nr10: 0x00,
            nr11: 0x00,
            nr12: 0x00,
            nr13: 0x00,
            nr14: 0x00,
            nr21: 0x00,
            nr22: 0x00,
            nr23: 0x00,
            nr24: 0x00,
            nr30: 0x00,
            nr31: 0x00,
            nr32: 0x00,
            nr33: 0x00,
            nr34: 0x00,
            nr41: 0x00,
            nr42: 0x00,
            nr43: 0x00,
            nr44: 0x00,
            wave_ram: [0; 16],

            div_apu: 0,
            timer_div_last: 0,
        }
    }

    pub fn tick(&mut self, timer_div: u8) {
        // increment div_apu when bit 4 of timer_div goes from 1 to 0
        if self.timer_div_last & 0b0001_0000 == 1 && timer_div & 0b0001_0000 == 0 {
            self.div_apu += 1;
        }

        if self.nr52 & 0b1000_0000 == 0 {
            // APU is disabled, reset all registers but keep wave ram. Timer is uneffected
            self.nr10 = 0x00;
            self.nr11 = 0x00;
            self.nr12 = 0x00;
            self.nr13 = 0x00;
            self.nr14 = 0x00;
            self.nr21 = 0x00;
            self.nr22 = 0x00;
            self.nr23 = 0x00;
            self.nr24 = 0x00;
            self.nr30 = 0x00;
            self.nr31 = 0x00;
            self.nr32 = 0x00;
            self.nr33 = 0x00;
            self.nr34 = 0x00;
            self.nr41 = 0x00;
            self.nr42 = 0x00;
            self.nr43 = 0x00;
            self.nr44 = 0x00;

            return;
        }

        if self.div_apu % 8 == 0 {
            // envelope sweep
        }

        if self.div_apu % 2 == 0 {
            // sound length
        }

        if self.div_apu % 4 == 0 {
            // channel 1 sweep
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0xFF26 => self.nr52 = value & 0b1111_0000, // Only bit 7 can be set
            0xFF25 => self.nr51 = value,
            0xFF24 => self.nr50 = value,
            0xFF10 => self.nr10 = value,
            0xFF11 => self.nr11 = value,
            0xFF12 => self.nr12 = value,
            0xFF13 => self.nr13 = value,
            0xFF14 => self.nr14 = value,
            0xFF16 => self.nr21 = value,
            0xFF17 => self.nr22 = value,
            0xFF18 => self.nr23 = value,
            0xFF19 => self.nr24 = value,
            0xFF1A => self.nr30 = value,
            0xFF1B => self.nr31 = value,
            0xFF1C => self.nr32 = value,
            0xFF1D => self.nr33 = value,
            0xFF1E => self.nr34 = value,
            0xFF20 => self.nr41 = value,
            0xFF21 => self.nr42 = value,
            0xFF22 => self.nr43 = value,
            0xFF23 => self.nr44 = value,
            0xFF30..=0xFF3F => self.wave_ram[(address - 0xFF30) as usize] = value,
            _ => panic!("Invalid APU write address: {:X}", address),
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF26 => self.nr52 | 0b0111_0000, // Bits 6-4 are empty, so set them
            0xFF25 => self.nr51,
            0xFF24 => self.nr50,
            0xFF10 => self.nr10,
            0xFF11 => self.nr11 | 0b0011_1111, // Bits 5-0 are write-only, so set them
            0xFF12 => self.nr12,
            0xFF13 => 0xFF,                    // write-only, return 0xFF
            0xFF14 => self.nr14 | 0b1011_1111, // Bits 7 and 2-0 are write-only, and 5-3 are empty, so set them
            0xFF16 => self.nr21 | 0b0011_1111,
            0xFF17 => self.nr22,
            0xFF18 => 0xFF,                    // write-only, return 0xFF
            0xFF19 => self.nr24 | 0b1011_1111, // Bits 7 and 2-0 are write-only, and 5-3 are empty, so set them
            0xFF1A => self.nr30 | 0b0111_1111, // Bits 6-0 are empty, so set
            0xFF1B => 0xFF,                    // write-only, return 0xFF
            0xFF1C => self.nr32 | 0b1001_1111, // bits 7 and 4-0 are empty
            0xFF1D => 0xFF,                    // write-only, return 0xFF
            0xFF1E => self.nr34 | 0b1011_1111, // Bits 7 and 2-0 are write-only, and 5-3 are empty, so set them
            0xFF20 => 0xFF,                    // write-only, return 0xFF
            0xFF21 => self.nr42,
            0xFF22 => self.nr43,
            0xFF23 => self.nr44 | 0b1011_1111, // Bits 7 is write-only, and 5-0 are empty, so set them
            0xFF30..=0xFF3F => self.wave_ram[(address - 0xFF30) as usize],
            _ => panic!("Invalid APU read address: {:X}", address),
        }
    }
}
