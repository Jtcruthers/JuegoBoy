use crate::mbc1::MBC1;

pub trait MBC {
    fn read_bank_0(&self, address: u16) -> u8;
    fn read_bank_n(&self, address: u16) -> u8;
    fn read_ram(&self, address: u16) -> u8;
    fn write_bank_0(&mut self, address: u16, value: u8);
    fn write_bank_n(&mut self, address: u16, value: u8);
    fn write_ram(&mut self, address: u16, value: u8);
}
pub struct Cartridge {
    mbc: Box<dyn MBC>
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Cartridge {
        let mapper_type = rom[0x147];
        let mut cartridge = Cartridge {
            mbc: Box::new(MBC1::new(rom))
        };
        
        cartridge
    }
    
    pub fn read_bank_0(&self, address: u16) -> u8 {
        self.mbc.read_bank_0(address)
    }

    pub fn read_bank_n(&self, address: u16) -> u8 {
        self.mbc.read_bank_n(address)
    }

    pub fn read_ram(&self, address: u16) -> u8 {
        self.mbc.read_ram(address)
    }

    pub fn write_bank_0(&mut self, address: u16, value: u8) {
        self.mbc.write_bank_0(address, value);
    }

    pub fn write_bank_n(&mut self, address: u16, value: u8) {
        self.mbc.write_bank_n(address, value);
    }

    pub fn write_ram(&mut self, address: u16, value: u8) {
        self.mbc.write_ram(address, value);
    }
}
