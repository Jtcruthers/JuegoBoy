pub struct PPU {
    cycles: u64,
    pub interrupt_vblank: bool,
    pub interrupt_stat: bool,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            cycles: 0,
            interrupt_vblank: false,
            interrupt_stat: false,
        }
    }

    pub fn step(&mut self, cycles: u8) {
        self.cycles += cycles as u64;
        if self.cycles >= 114 * 144 {
            self.interrupt_vblank = true;

            if self.cycles >= 114 * 153 {
                self.cycles = 0;
            }
        }
    }
}
