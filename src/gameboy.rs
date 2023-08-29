use crate::apu::APU;
use crate::cpu::CPU;
use crate::mmu::MMU;
use crate::ppu::PPU;

pub struct Gameboy {
    cpu: CPU,
    apu: APU,
    ppu: PPU,
    mmu: MMU,
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

pub fn test() -> Gameboy {
    let x = Gameboy::new();

    x
}
