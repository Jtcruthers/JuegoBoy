pub struct Cartridge {
    bank_0: [u8; 0x4000],
    bank_n: [u8; 0x4000],
    ram: [u8; 0x2000],
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Cartridge {
        let mut cartridge = Cartridge {
            bank_0: [0; 0x4000],
            bank_n: [0; 0x4000],
            ram: [0; 0x2000],
        };

        for (offset, byte) in rom.iter().enumerate() {
            match offset {
                0x0000..=0x3FFF => cartridge.bank_0[offset] = *byte,
                0x4000..=0x7FFF => cartridge.bank_n[(offset - 0x4000) as usize] = *byte,
                _ => panic!("ROM too large until memory banking"),
            }
        }

        cartridge
    }

    pub fn read_bank_0(&self, address: u16) -> u8 {
        self.bank_0[address as usize]
    }

    pub fn read_bank_n(&self, address: u16) -> u8 {
        self.bank_n[(address - 0x4000) as usize]
    }

    pub fn read_ram(&self, address: u16) -> u8 {
        self.ram[(address - 0xA000) as usize]
    }

    pub fn write_bank_0(&mut self, address: u16, value: u8) {
        self.bank_0[address as usize] = value;
    }

    pub fn write_bank_n(&mut self, address: u16, value: u8) {
        self.bank_n[(address - 0x4000) as usize] = value;
    }

    pub fn write_ram(&mut self, address: u16, value: u8) {
        self.ram[(address - 0xA000) as usize] = value;
    }
}
