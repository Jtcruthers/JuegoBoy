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
    pub fn new(rom: Vec<u8>, audio_sample_rate: u32) -> Self {
        Gameboy {
            cpu: CPU::new(),
            mmu: MMU::new(Cartridge::new(rom), audio_sample_rate),
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

    /// Gets the samples since the last call and clears them to prepare a fresh batch.
    /// Returns the samples as interleaved stereo, starting with the left channel.
    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.mmu.apu.samples)
    }

    pub fn get_frame(&mut self) -> [u8; 160 * 144] {
        self.mmu.ppu.get_frame()
    }

    /// The cartridge's external RAM, which is what games write their saves to. Persist this as a
    /// .sav file. Empty if the cartridge has no RAM.
    pub fn get_cartridge_ram(&self) -> &[u8] {
        self.mmu.cartridge.read_ram()
    }

    /// Restore cartridge RAM from a .sav file. Call this before running any frames, since the game
    /// reads its save during startup.
    pub fn load_cartridge_ram(&mut self, data: &[u8]) -> Result<(), String> {
        let ram = self.mmu.cartridge.mbc.ram_mut();
        if data.len() != ram.len() {
            return Err(format!(
                "save is {} bytes but the cartridge has {} bytes of RAM",
                data.len(),
                ram.len()
            ));
        }
        ram.copy_from_slice(data);
        Ok(())
    }
}
