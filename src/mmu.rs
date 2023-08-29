pub trait Memory {
    fn read_byte(&self, address: u16) -> u8;
    fn write_byte(&mut self, address: u16, value: u8);
}

pub struct MMU {
    wram: [u8; 0x2000],
    vram: [u8; 0x2000],
}

impl MMU {
    pub fn new() -> Self {
        MMU {
            wram: [0; 0x2000],
            vram: [0; 0x2000],
        }
    }
}

impl Memory for MMU {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => 0, //TODO - cartridge rom bank 00
            0x4000..=0x7FFF => 0, //TODO - cartridge rom bank 01~NN
            0x8000..=0x9FFF => self.vram[address as usize],
            0xA000..=0xBFFF => 0, //TODO - cartridge ram
            0xC000..=0xDFFF => self.wram[address as usize],
            0xE000..=0xFDFF => self.wram[address as usize - 0x2000], // echo RAM
            0xFE00..=0xFE9F => 0, //TODO - OAM
            0xFEA0..=0xFEFF => 0, // Not usable
            0xFF00..=0xFF7F => 0, //TODO - I/O
            0xFF80..=0xFFFE => 0, //TODO - High ram (?)
            0xFFFF => 0, //TODO - Interrupt enable register
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x3FFF => (), //TODO - cartridge rom bank 00
            0x4000..=0x7FFF => (), //TODO - cartridge rom bank 01~NN
            0x8000..=0x9FFF => self.vram[address as usize] = value,
            0xA000..=0xBFFF => (), //TODO - cartridge ram
            0xC000..=0xDFFF => self.wram[address as usize] = value,
            0xE000..=0xFDFF => self.wram[address as usize - 0x2000] = value, // should echo set?
            0xFE00..=0xFE9F => (), //TODO - OAM
            0xFEA0..=0xFEFF => (), // Not usable
            0xFF00..=0xFF7F => (), //TODO - I/O
            0xFF80..=0xFFFE => (), //TODO - High ram (?)
            0xFFFF => (), //TODO - Interrupt enable register
        }
    }
}

pub struct TestMMU {
    pub memory: [u8; 0xFFFF],
}

impl TestMMU {
    pub fn new() -> Self {
        TestMMU {
            memory: [0; 0xFFFF],
        }
    }
}

impl Memory for TestMMU {
    fn read_byte(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        self.memory[address as usize] = value;
    }
}
