use log::*;

use crate::cartridge::Cartridge;
use crate::cpu::CPU;
use crate::joypad::Button;
use crate::mmu::{Memory, MMU};

pub struct Gameboy {
    cpu: CPU,
    mmu: MMU,
    booting: bool,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>) -> Self {
        Gameboy {
            cpu: CPU::new(),
            mmu: MMU::new(Cartridge::new(rom)),
            booting: true,
        }
    }

    // Step through one full ppu frame
    // The PPU will loop back to 0 after it completes a rendering
    // so we can break the loop when we see the previous cycle count is more than the current cycle count
    pub fn step_frame(&mut self) {
        loop {
            let initial_cycles = self.mmu.ppu.cycles;
            self.cpu.step(&mut self.mmu);

            if self.booting && !self.mmu.boot_rom_active() {
                self.booting = false;
                debug!(
                    "Boot ROM finished\n{}\nIE: {:08b} IF: {:08b} LY: {:02X}",
                    self.cpu,
                    self.mmu.read_byte(0xFFFF),
                    self.mmu.read_byte(0xFF0F),
                    self.mmu.read_byte(0xFF44),
                );
            }

            if self.mmu.ppu.cycles < initial_cycles {
                break;
            }
        }
    }

    pub fn press_button(&mut self, button: Button) {
        self.mmu.joypad.press_button(button);
    }

    pub fn release_button(&mut self, button: Button) {
        self.mmu.joypad.release_button(button);
    }

    pub fn get_frame(&mut self) -> [u8; 160 * 144] {
        self.mmu.ppu.get_frame()
    }
}
