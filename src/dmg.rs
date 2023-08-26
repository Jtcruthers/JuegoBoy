use crate::cpu::CPU;
use crate::ppu::PPU;
use crate::apu::APU;
use crate::mmu::MMU;

pub struct DMG {
    cpu: CPU,
    apu: APU,
    ppu: PPU,
    mmu: MMU
}

impl DMG {
    pub fn new() -> Self {
        DMG {
            cpu: CPU::new(),
            ppu: PPU::new(),
            apu: APU::new(),
            mmu: MMU::new(),
        }
    }
}
