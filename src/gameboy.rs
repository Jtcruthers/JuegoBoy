use crate::apu::APU;
use crate::cpu::CPU;
use crate::mmu::MMU;
use crate::ppu::PPU;
use crate::cartridge::Cartridge;

pub struct Gameboy {
    cpu: CPU,
    apu: APU,
    ppu: PPU,
    mmu: MMU,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>) -> Self {
        Gameboy {
            cpu: CPU::new(),
            ppu: PPU::new(),
            apu: APU::new(),
            mmu: MMU::new(
                Cartridge::new(rom)
            ),
        }
    }

    pub fn run(&mut self) {
        loop {
            self.cpu.step(&mut self.mmu);
        }
    }
}
