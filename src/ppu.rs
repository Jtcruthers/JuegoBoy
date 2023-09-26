pub struct PPU {
    scan_line: u8,
    cycles: u64,
    pub interrupt_vblank: bool,
    pub interrupt_stat: bool,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            scan_line: 0,
            cycles: 0,
            interrupt_vblank: false,
            interrupt_stat: false,
        }
    }

    pub fn step(&mut self, cycles: u8) {
        self.cycles += cycles as u64;
        if self.cycles > 114 * 153 {
            self.cycles -= 114 * 153;
        }

        self.scan_line = (self.cycles / 114) as u8;
        if self.scan_line == 144 {
            self.interrupt_vblank = true;
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF40 => 0, // LCDC
            0xFF41 => 0, // STAT
            0xFF42 => 0, // SCY
            0xFF43 => 0, // SCX
            0xFF44 => self.scan_line,
            0xFF45 => 0, // LYC
            0xFF4A => 0, // WY
            0xFF4B => 0, // WX
            _ => panic!("Invalid PPU read address: {:X}", address)
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            _ => ()
        }
    }
}
