use crate::mbc1::MBC1;

pub trait MBC {
    fn read_byte(&self, address: u16) -> u8;
    fn write_byte(&mut self, address: u16, value: u8);
}

pub struct Cartridge {
    mbc: Box<dyn MBC>,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Cartridge {
        let cartridge = Cartridge {
            mbc: Box::new(MBC1::new(rom)),
        };

        cartridge
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        self.mbc.read_byte(address)
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        self.mbc.write_byte(address, value);
    }
}
