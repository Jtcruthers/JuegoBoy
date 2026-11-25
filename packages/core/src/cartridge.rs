use crate::mbc1::MBC1;

pub const HEADER_RAM_SIZE_ADDRESS: usize = 0x0149;
pub const HEADER_RAM_SIZE_8K: u8 = 0x02;
pub const HEADER_RAM_SIZE_32K: u8 = 0x03;

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
