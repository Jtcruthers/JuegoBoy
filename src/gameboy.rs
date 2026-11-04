use log::*;

use crate::cartridge::Cartridge;
use crate::cpu::CPU;
use crate::mmu::{Memory, MMU};

pub struct Gameboy {
    cpu: CPU,
    mmu: MMU,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>) -> Self {
        Gameboy {
            cpu: CPU::new(),
            mmu: MMU::new(Cartridge::new(rom)),
        }
    }

    pub fn run(&mut self) {
        let mut booting = self.mmu.boot_rom_active();
        loop {
            self.cpu.step(&mut self.mmu);

            if booting && !self.mmu.boot_rom_active() {
                booting = false;
                debug!(
                    "Boot ROM finished\n{}\nIE: {:08b} IF: {:08b} LY: {:02X}",
                    self.cpu,
                    self.mmu.read_byte(0xFFFF),
                    self.mmu.read_byte(0xFF0F),
                    self.mmu.read_byte(0xFF44),
                );
            }
        }
    }
}
