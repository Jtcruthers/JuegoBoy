use crate::cpu::CPU;
use crate::mmu::MMU;
use crate::cartridge::Cartridge;

pub struct Gameboy {
    cpu: CPU,
    mmu: MMU,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>) -> Self {
        Gameboy {
            cpu: CPU::new(),
            mmu: MMU::new(
                Cartridge::new(rom)
            ),
        }
    }

    pub fn run(&mut self) {
        loop {
            let cycles = self.cpu.step(&mut self.mmu);
            self.mmu.step(cycles);
        }
    }
}
