use crate::cartridge::MBC;

pub struct MBC1 {
    /// 1-bit register, write only to 0x0000-1FFF
    ram_enabled: bool,

    /// 5-bit register, write only to 0x2000-3FFF. Selects which ROM bank number for the 4000-7FFF
    /// region. If this register is set to 0x00, it behaves as if it's 0x01 to prevent duplicating
    /// 0x0000-3FFF data in the higher bank.
    rom_bank: u8,

    /// 2-bit register, write only to 0x4000-5FFF. Used to select a RAM bank between 0x00-03 when
    /// its a 32 KiB ram cart, OR to specify the upper two bits (bits 5 and 6) of the ROM Bank
    /// number (rom_bank register). If neither ROM nor RAM is big enough, it does nothing.
    ram_bank: u8,

    /// 1-bit register, write-only to 0x6000-7FFF. If not set, bank 0 is truly bank 0. If set, bank
    /// 0 uses the ram_bank to determine which bank to use for both ROM bank 0 and RAM bank 0.
    banking_mode: u8,

    rom: Vec<u8>,
    /// External RAM. It will either be 8K or 32K in the form of 4 8K banks. To use the 32 KiB ram,
    /// the ROM must be <= 512 KiB. Currently, i'm just setting it to the max size, but I should
    /// check the size of the ROM and dynamically set this.
    /// TODO - set this based on the actual size of the ROM
    ram: [u8; 0x8000],
}

impl MBC1 {
    pub fn new(rom: Vec<u8>) -> Self {
        MBC1 {
            rom_bank: 0,
            ram_bank: 0,
            ram_enabled: false,
            banking_mode: 0,
            rom: rom,
            ram: [0; 0x8000],
        }
    }
}

impl MBC for MBC1 {
    fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = (value & 0x0F) == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0b1_1111;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0b11;
            }
            0x6000..=0x7FFF => {
                self.banking_mode = value & 0b1;
            }
            _ => panic!("Invalid address for writes"),
        }
    }

    fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                // For 1 MiB or larger carts, entering mode 1 allows the second banking register to
                // apply to reads from this address. So you can access 0x20, 0x40, and 0x60 here.
                // TODO - I'm not supporting the MBC1M multi-cart yet
                let bank_number = if self.banking_mode == 0 {
                    0
                } else {
                    self.ram_bank << 5 // Move these bits to 6/5
                };
                self.rom[address as usize + bank_number as usize * 0x4000]
            }
            0x4000..=0x7FFF => {
                // This area contains the other 16 KiB banks of the ROM. If the main 5-bit ROM
                // banking register is 0, it reads the bank as if it was set to 1.
                let low_bank = if self.rom_bank == 0 {
                    0
                } else {
                    self.rom_bank - 1
                };
                let high_bank = self.ram_bank << 5;
                let bank_number = ((high_bank | low_bank) & 0b111_1111) as usize;

                self.rom[address as usize + bank_number * 0x4000]
            }
            0xA000..=0xBFFF => {
                if !self.ram_enabled {
                    return 0xFF;
                }
                let bank = if self.banking_mode == 0 {
                    0
                } else {
                    self.ram_bank
                } as usize;
                self.ram[bank * 0x2000 + (address - 0xA000) as usize]
            }
            _ => 0xFF, // just return 0xFF if it doesn't isn't supported
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reading_rom_bank_nn() {
        let mut rom = vec![0; 0xFFFF];
        rom[0x8000] = 0xFF; // 8000 is first byte of rom bank 02

        let mut mbc1 = MBC1::new(rom);
        mbc1.banking_mode = 1;

        mbc1.rom_bank = 1;
        assert_eq!(mbc1.read_byte(0x4000), 0x00);
        mbc1.rom_bank = 2;
        assert_eq!(mbc1.read_byte(0x4000), 0xFF);
        mbc1.rom_bank = 3;
        assert_eq!(mbc1.read_byte(0x4000), 0x00);
    }

    #[test]
    fn test_setting_ram_enable() {
        let mut mbc1 = MBC1::new(vec![0; 0x1]);
        mbc1.ram_enabled = false;
        mbc1.write_byte(0x0000, 0xA);
        assert_eq!(mbc1.ram_enabled, true);
        mbc1.write_byte(0x1FFF, 0);
        assert_eq!(mbc1.ram_enabled, false);
    }

    #[test]
    fn test_setting_rom_bank() {
        let mut mbc1 = MBC1::new(vec![0; 0x1]);
        mbc1.rom_bank = 0;
        mbc1.write_byte(0x2000, 10);
        assert_eq!(mbc1.rom_bank, 10);
        mbc1.write_byte(0x3FFF, 5);
        assert_eq!(mbc1.rom_bank, 5);
    }

    #[test]
    fn test_setting_ram_bank() {
        let mut mbc1 = MBC1::new(vec![0; 0x1]);
        mbc1.ram_bank = 0;
        mbc1.write_byte(0x4000, 3);
        assert_eq!(mbc1.ram_bank, 3);
        mbc1.write_byte(0x5FFF, 1);
        assert_eq!(mbc1.ram_bank, 1);
    }

    #[test]
    fn test_setting_banking_mode() {
        let mut mbc1 = MBC1::new(vec![0; 0x1]);
        mbc1.banking_mode = 0;
        mbc1.write_byte(0x6000, 1);
        assert_eq!(mbc1.banking_mode, 1);
        mbc1.write_byte(0x7FFF, 0);
        assert_eq!(mbc1.banking_mode, 0);
    }

    #[test]
    fn test_write_ram_bank_sets_rom() {
        // 2 MiB ROM = 128 banks, so all 7 bank bits are used
        let mut rom = vec![0; 0x200000];
        rom[0x20 * 0x4000] = 0x20;
        rom[0x21 * 0x4000] = 0x21;
        rom[0x61 * 0x4000] = 0x61;
        rom[0x65 * 0x4000 + 0x123] = 0x65;

        let mut mbc1 = MBC1::new(rom);

        mbc1.write_byte(0x2000, 1);
        mbc1.write_byte(0x4000, 0b11);
        assert_eq!(mbc1.read_byte(0x4000), 0x61);

        // Offsets within the bank still work
        mbc1.write_byte(0x2000, 5);
        assert_eq!(mbc1.read_byte(0x4123), 0x65);
    }

    #[test]
    fn test_read_ram_bank_nn() {
        let mut mbc1 = MBC1::new(vec![0; 0x1]);
        mbc1.ram_enabled = true;
        mbc1.banking_mode = 1;
        mbc1.ram[0] = 0xAA;
        mbc1.ram[0x2000] = 0xBB;
        mbc1.ram[0x4000] = 0xCC;
        mbc1.ram[0x6000] = 0xDD;

        mbc1.ram_bank = 0;
        assert_eq!(mbc1.read_byte(0xA000), 0xAA);

        mbc1.ram_bank = 1;
        assert_eq!(mbc1.read_byte(0xA000), 0xBB);

        mbc1.ram_bank = 2;
        assert_eq!(mbc1.read_byte(0xA000), 0xCC);

        mbc1.ram_bank = 3;
        assert_eq!(mbc1.read_byte(0xA000), 0xDD);
    }
}
