use crate::cartridge::{Cartridge, self};

pub trait Memory {
    fn read_byte(&self, address: u16) -> u8;
    fn write_byte(&mut self, address: u16, value: u8);
}

const MASK_VBLANK: u8 = 0b00001;
const MASK_LCD: u8 = 0b00010;
const MASK_TIMER: u8 = 0b00100;
const MASK_SERIAL: u8 = 0b01000;
const MASK_JOYPAD: u8 = 0b10000;

pub struct MMU {
    wram: [u8; 0x2000],
    vram: [u8; 0x2000],
    ie_reg: u8,
    if_reg: u8,
    cartridge: Cartridge,
}

impl MMU {
    pub fn new(cartridge: Cartridge) -> Self {
        MMU {
            wram: [0; 0x2000],
            vram: [0; 0x2000],
            ie_reg: 0,
            if_reg: 0,
            cartridge
        }
    }
}

impl Memory for MMU {
    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => self.cartridge.read_bank_0(address),
            0x4000..=0x7FFF => self.cartridge.read_bank_n(address),
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize],
            0xA000..=0xBFFF => self.cartridge.read_ram(address),
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(address - 0xE000 - 0x2000) as usize], // echo RAM
            0xFE00..=0xFE9F => todo!("OAM"),        //TODO - OAM
            0xFEA0..=0xFEFF => todo!("Not Usable"), //TODO - Not usable
            0xFF00..=0xFF7F => todo!("I/O"),        //TODO - I/O
            0xFF80..=0xFFFE => todo!("High RAM"),   //TODO - High ram (?)
            0xFFFF => 0, //TODO - Interrupt enable register
        }
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x3FFF => self.cartridge.write_bank_0(address, value),
            0x4000..=0x7FFF => self.cartridge.write_bank_n(address, value),
            0x8000..=0x9FFF => self.vram[(address -0x8000) as usize] = value,
            0xA000..=0xBFFF => self.cartridge.write_ram(address, value),
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.wram[(address - 0xE000 - 0x2000) as usize] = value, // echo ram
            0xFE00..=0xFE9F => (),                                           //TODO - OAM
            0xFEA0..=0xFEFF => (),                                           // Not usable
            0xFF00..=0xFF7F => (),                                           //TODO - I/O
            0xFF80..=0xFFFE => (),                                           //TODO - High ram (?)
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
        println!("Writing {:04X} to {:04X}", value, address);
        if (address == 0xFF01) {
            println!("SERIAL DATA: {:04X}", value);
            panic!("Serial data");
        }
        if (address == 0xFF02) {
            println!("SERIAL TRANSFER: {:04X}", value);
            panic!("Serial transfer");
        }
        self.memory[address as usize] = value;
    }
}
