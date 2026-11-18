use std::io::Write;

use log::*;

use crate::apu::APU;
use crate::cartridge::Cartridge;
use crate::joypad::Joypad;
use crate::ppu::PPU;
use crate::timer::Timer;

pub trait Memory {
    fn read_byte(&self, address: u16) -> u8;
    fn write_byte(&mut self, address: u16, value: u8);
    fn tick(&mut self);
}

/*
const MASK_VBLANK: u8 = 0b00001;
const MASK_LCD: u8 = 0b00010;
const MASK_TIMER: u8 = 0b00100;
const MASK_SERIAL: u8 = 0b01000;
const MASK_JOYPAD: u8 = 0b10000;
*/

const BOOT_ROM: &[u8; 256] = include_bytes!("../../../bootix_dmg.bin");
pub const INTERRUPT_MASK: u8 = 0b0001_1111;

pub struct MMU {
    wram: [u8; 0x2000],
    high_ram: [u8; 0x7F],
    ie_reg: u8,
    if_reg: u8,
    boot_rom_reg: u8,
    cartridge: Cartridge,
    serial_output: u8,
    pub joypad: Joypad,
    pub ppu: PPU,
    pub timer: Timer,
    pub apu: APU,
}

impl MMU {
    pub fn new(cartridge: Cartridge) -> Self {
        MMU {
            wram: [0; 0x2000],
            high_ram: [0; 0x7F],
            ie_reg: 0,
            if_reg: 0,
            boot_rom_reg: 0,
            cartridge,
            serial_output: 0,
            joypad: Joypad::new(),
            ppu: PPU::new(),
            timer: Timer::new(),
            apu: APU::new(),
        }
    }

    fn handle_io_read(&self, address: u16) -> u8 {
        // Unused bits in I/O registers always return 1 when read
        // But IE is an acception, as it's an actual 8-bit register
        match address {
            0xFF00 => self.joypad.read_byte(address),
            0xFF01 => todo!("Serial"), //TODO - Serial
            0xFF02 => todo!("Serial"), //TODO - Serial
            0xFF04..=0xFF07 => self.timer.read_byte(address),
            0xFF0F => self.if_reg | !INTERRUPT_MASK, // Interrupt Flag
            0xFF10..=0xFF3F => self.apu.read_byte(address),
            0xFF40..=0xFF4B => self.ppu.read_byte(address),
            0xFF50 => self.boot_rom_reg,
            _ => 0xFF,
        }
    }

    fn handle_io_write(&mut self, address: u16, value: u8) {
        match address {
            0xFF00 => self.joypad.write_byte(address, value),
            0xFF01 => self.serial_output = value,
            0xFF02 => {
                if value == 0x81 {
                    print!("{}", self.serial_output as char);
                    std::io::stdout().flush().unwrap();
                }
            }
            0xFF04..=0xFF07 => self.timer.write_byte(address, value),
            0xFF0F => self.if_reg = value,
            0xFF10..=0xFF3F => self.apu.write_byte(address, value),
            0xFF40..=0xFF4B => self.ppu.write_byte(address, value),
            0xFF50 => self.boot_rom_reg = value,
            _ => (),
        }
    }

    pub fn boot_rom_active(&self) -> bool {
        self.boot_rom_reg == 0
    }

    fn handle_oam_dma_transfer(&mut self) {
        let dma_address = (self.ppu.dma as u16) << 8;
        match dma_address {
            0xE000..=0xFFFF => {
                warn!("DMA from E000-FFFF is not supported, ignoring");
            }
            0x8000..=0x9FFF => {
                for i in 0..160 {
                    self.ppu.oam[i] = self.ppu.vram[(dma_address + i as u16 - 0x8000) as usize];
                }
            }
            _ => {
                for i in 0..160 {
                    self.ppu.oam[i] = self.read_byte(dma_address + i as u16);
                }
            }
        }
    }
}

impl Memory for MMU {
    /// Ticks all components, except for the CPU, exactly 1 m-cycle
    fn tick(&mut self) {
        // On last cycle of oam, copy it over
        if self.ppu.oam_cycles_left == 1 {
            self.handle_oam_dma_transfer();
        }

        self.ppu.tick();
        if self.ppu.interrupt_vblank {
            self.if_reg |= 0b1;
            self.ppu.interrupt_vblank = false;
        }
        if self.ppu.interrupt_stat {
            self.if_reg |= 0b10;
            self.ppu.interrupt_stat = false;
        }

        self.timer.tick();
        if self.timer.interrupt {
            self.if_reg |= 0b100;
            self.timer.interrupt = false;
        }

        self.joypad.tick();
        if self.joypad.interrupt {
            self.if_reg |= 0b10000;
            self.joypad.interrupt = false;
        }

        self.apu.tick();
    }

    fn read_byte(&self, address: u16) -> u8 {
        let byte = match address {
            0x0000..=0x3FFF => {
                if self.boot_rom_reg == 0 && address < 0x100 {
                    BOOT_ROM[address as usize]
                } else {
                    self.cartridge.read_bank_0(address)
                }
            }
            0x4000..=0x7FFF => self.cartridge.read_bank_n(address),
            0x8000..=0x9FFF => self.ppu.read_byte(address),
            0xA000..=0xBFFF => self.cartridge.read_ram(address),
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize],
            0xE000..=0xFDFF => self.wram[(address - 0xE000) as usize], // echo RAM
            0xFE00..=0xFE9F => self.ppu.read_byte(address),
            0xFEA0..=0xFEFF => todo!("Not Usable {0:4X}", address), //TODO - Not usable
            0xFF00..=0xFF7F => self.handle_io_read(address),
            0xFF80..=0xFFFE => self.high_ram[(address - 0xFF80) as usize],
            0xFFFF => self.ie_reg,
        };

        byte
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        trace!("Writing to {:04X} = {:02X}", address, value);
        match address {
            0x0000..=0x3FFF => self.cartridge.write_bank_0(address, value),
            0x4000..=0x7FFF => self.cartridge.write_bank_n(address, value),
            0x8000..=0x9FFF => self.ppu.write_byte(address, value),
            0xA000..=0xBFFF => self.cartridge.write_ram(address, value),
            0xC000..=0xDFFF => self.wram[(address - 0xC000) as usize] = value,
            0xE000..=0xFDFF => self.wram[(address - 0xE000) as usize] = value, // echo ram
            0xFE00..=0xFE9F => self.ppu.write_byte(address, value),
            0xFEA0..=0xFEFF => (), // Not usable
            0xFF00..=0xFF7F => self.handle_io_write(address, value),
            0xFF80..=0xFFFE => self.high_ram[(address - 0xFF80) as usize] = value,
            0xFFFF => self.ie_reg = value,
        }
    }
}

#[cfg(test)]
pub struct TestMMU {
    pub memory: [u8; 0xFFFF],
}

#[cfg(test)]
impl TestMMU {
    pub fn new() -> Self {
        TestMMU {
            memory: [0; 0xFFFF],
        }
    }
}

#[cfg(test)]
impl Memory for TestMMU {
    fn read_byte(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }

    fn write_byte(&mut self, address: u16, value: u8) {
        if address == 0xFF01 {
            panic!("Serial data");
        }
        if address == 0xFF02 {
            panic!("Serial transfer");
        }
        self.memory[address as usize] = value;
    }

    // noop
    fn tick(&mut self) {}
}
