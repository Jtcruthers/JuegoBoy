pub struct Joypad {
    p1: u8,
    pub interrupt: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            p1: 0xFF,
            interrupt: false,
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF00 => self.p1, // TODO - update this field depending on the joypad inputs
            _ => panic!("Invalid joypad read address: {:X}", address),
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0xFF00 => self.p1 |= 0b0011_0000 & value,
            _ => panic!("Invalid joypad write address: {:X}", address),
        }
    }

    pub fn step(&mut self) {
        //self.interrupt = true;
    }
}
