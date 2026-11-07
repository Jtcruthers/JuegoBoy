const VBLANK_START: u64 = 114 * 144;

const LCD_WIDTH: usize = 160;
const LCD_HEIGHT: usize = 144;
const TOTAL_PIXELS: usize = LCD_WIDTH * LCD_HEIGHT;

const DOTS_PER_M_CYCLE: u64 = 4;
const DOTS_PER_ROW: u64 = 456;
const DOTS_PER_MODE_2: u64 = 80;

pub struct PPU {
    scan_line: u8,
    cycles: u64,
    vram: [u8; 0x2000],
    oam: [u8; 160],
    pub interrupt_vblank: bool,
    pub interrupt_stat: bool,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            vram: [0; 0x2000],
            oam: [0; 160],
            scan_line: 0,
            cycles: 0,
            interrupt_vblank: false,
            interrupt_stat: false,
        }
    }

    pub fn tick(&mut self) {
        self.cycles += 1;
        if self.cycles == 114 * 154 {
            self.cycles -= 114 * 154;
        }

        self.scan_line = (self.cycles / 114) as u8;
        // Only fires the first time it enters VBlank
        if self.cycles == VBLANK_START {
            self.interrupt_vblank = true;
        }
    }

    /// get_frame returns the array of pixel for the given frame
    ///
    /// The idea is the emulator will step over the exact number of M-Cycles
    /// to have a frame ready, and then the consumer will call this to render
    /// the frame.
    pub fn get_frame(&self) -> [u8; TOTAL_PIXELS] {
        let pixel_buffer = [0; TOTAL_PIXELS];

        // TODO - calculate a single frame

        return pixel_buffer;
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x8000..=0x9FFF => self.vram[(address - 0x8000) as usize],
            0xFE00..=0xFE9F => self.oam[(address - 0xFE00) as usize],
            0xFF40 => 0, // LCDC
            0xFF41 => 0, // STAT
            0xFF42 => 0, // SCY
            0xFF43 => 0, // SCX
            0xFF44 => self.scan_line,
            0xFF45 => 0, // LYC
            0xFF4A => 0, // WY
            0xFF4B => 0, // WX
            _ => panic!("Invalid PPU read address: {:X}", address),
        }
    }

    pub fn write_byte(&mut self, address: u16, _value: u8) {
        match address {
            _ => (),
        }
    }
}
