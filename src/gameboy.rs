use crate::apu::APU;
use crate::cpu::CPU;
use crate::mmu::TestMMU;
use crate::ppu::PPU;

pub struct Gameboy {
    cpu: CPU,
    apu: APU,
    ppu: PPU,
    mmu: TestMMU,
}

impl Gameboy {
    pub fn new() -> Self {
        Gameboy {
            cpu: CPU::new(),
            ppu: PPU::new(),
            apu: APU::new(),
            mmu: TestMMU::new(),
        }
    }

    fn load_rom(&mut self, rom: Vec<u8>) {
        for (offset, byte) in rom.iter().enumerate() {
            self.mmu.memory[offset] = *byte;
        }
    }

    pub fn run(&mut self, rom: Vec<u8>) {
        self.load_rom(rom);
        loop {
            self.cpu.step(&mut self.mmu);
        }
    }
}
