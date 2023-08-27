use crate::cpu::CPU;
use crate::ppu::PPU;
use crate::apu::APU;
use crate::mmu::MMU;

pub struct Gameboy {
    cpu: CPU,
    apu: APU,
    ppu: PPU,
    mmu: MMU
}

impl Gameboy {
    pub fn new() -> Self {
        Gameboy {
            cpu: CPU::new(),
            ppu: PPU::new(),
            apu: APU::new(),
            mmu: MMU::new(),
        }
    }
}
