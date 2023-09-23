use log::*;

use crate::cartridge::MBC;

pub struct MBC1 {
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank: u8,
    banking_mode: u8,
    rom: [u8; 0x20000],
    ram: [u8; 0x8000]
}

impl MBC1 {
  pub fn new(rom: Vec<u8>) -> Self {
    let mut mbc1 = MBC1 {
        rom_bank: 1,
        ram_bank: 0,
        ram_enabled: false,
        banking_mode: 0,
        rom: [0; 0x20000],
        ram: [0; 0x8000]
    };

    trace!("Loading rom into MBC1");
    for (offset, byte) in rom.iter().enumerate() {
      mbc1.rom[offset] = *byte;
    }
    debug!("Done loading rom into MBC1");

     mbc1
  }
}

impl MBC for MBC1 {
    fn read_bank_0(&self, address: u16) -> u8 {
        self.rom[address as usize]
    }

    fn read_bank_n(&self, address: u16) -> u8 {
        let multiplier = match self.rom_bank {
            0 => 1,
            _ => self.rom_bank
        } as u16;
        self.rom[(address + multiplier * 0x4000) as usize]
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        self.ram[(address - 0xA000) as usize]
    }

    fn write_bank_0(&mut self, address: u16, value: u8) {
      match address {
        0x0000..=0x1FFF => {
            self.ram_enabled = (value & 0x0F) == 0x0A;
        },
        0x2000..=0x3FFF => {
            self.rom_bank = value & 0b11111;
        },
        _ => panic!("Invalid address for bank 0")
      }
    }

    fn write_bank_n(&mut self, address: u16, value: u8) {
      match address {
         0x4000..=0x5FFF => {
          self.ram_bank = value & 0b11;
        },
        0x6000..=0x7FFF => {
            self.banking_mode = value & 0b1;
        },
        _ => panic!("Invalid address for bank n")
      }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
      if self.ram_enabled {
        self.ram[(address - 0xA000) as usize] = value;
      }
    }
}
