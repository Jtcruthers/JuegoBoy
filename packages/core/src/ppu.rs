use log::*;

enum Mode {
    ZERO,
    ONE,
    TWO,
    THREE,
}

const VBLANK_START: u64 = 114 * 144;

const LCD_WIDTH: usize = 160;
const LCD_HEIGHT: usize = 144;
const TOTAL_PIXELS: usize = LCD_WIDTH * LCD_HEIGHT;

// const DOTS_PER_M_CYCLE: u64 = 4;
const DOTS_PER_ROW: u64 = 456;
const DOTS_PER_MODE_2: u64 = 80;

pub struct PPU {
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    lyc: u8,
    dma: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,

    scan_line: u8,
    pub cycles: u64,
    vram: [u8; 0x2000],
    oam: [u8; 160],

    lcd: [u8; TOTAL_PIXELS],

    pub interrupt_vblank: bool,
    pub interrupt_stat: bool,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            lcdc: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            lyc: 0,
            dma: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            vram: [0; 0x2000],
            oam: [0; 160],
            scan_line: 0,
            cycles: 0,
            lcd: [0; TOTAL_PIXELS],
            interrupt_vblank: false,
            interrupt_stat: false,
        }
    }

    fn get_mode(&self) -> Mode {
        if self.cycles >= VBLANK_START {
            return Mode::ONE;
        }

        let dots_on_row = self.cycles % DOTS_PER_ROW;
        if dots_on_row <= DOTS_PER_MODE_2 {
            return Mode::TWO;
        }

        // Need to figure out the penalties and stuff
        // Mode 3 can be variable, nut never mor than 289
        if dots_on_row <= 289 + DOTS_PER_MODE_2 {
            return Mode::ZERO;
        }

        return Mode::THREE;
    }

    fn get_tile_address(&self, tile_id: u8) -> u16 {
        let id: u16 = (tile_id as u16) << 4; // tile_id is middle two bytes of address

        if tile_id >= 0x80 {
            id + 0x8000
        } else if self.lcdc & 0b0001_0000 != 0 {
            id + 0x8000
        } else {
            id + 0x9000
        }
    }

    fn combine_u8s_into_tile_row(&self, low_byte: u8, high_byte: u8) -> u16 {
        let mut row: u16 = 0;
        for bit_index in 0..8 {
            let sig_bit = (high_byte & (1 << (7 - bit_index))) >> (7 - bit_index);
            let least_bit = (low_byte & (1 << (7 - bit_index))) >> (7 - bit_index);
            let bit = ((sig_bit << 1) + least_bit) as u16;

            row |= bit << ((7 - bit_index) * 2);
        }

        return row;
    }

    fn get_tile(&self, tile_id: u8) -> [u16; 8] {
        let mut tile: [u16; 8] = [0; 8];

        let address = self.get_tile_address(tile_id);
        let mut vram_address = address as usize - 0x8000;
        for i in 0..8 {
            // loop over two bytes at a time to combine them into one row of tile
            let low_byte = self.vram[vram_address];
            let high_byte = self.vram[vram_address + 1];

            tile[i] = self.combine_u8s_into_tile_row(low_byte, high_byte);

            vram_address += 2;
        }

        return tile;
    }

    fn get_tile_row(&self, tile_id: u8, row_index: u8) -> u16 {
        let address = self.get_tile_address(tile_id);
        let vram_address = address as usize - 0x8000 + row_index as usize;
        // loop over two bytes at a time to combine them into one row of tile
        let low_byte = self.vram[vram_address];
        let high_byte = self.vram[vram_address + 1];

        return self.combine_u8s_into_tile_row(low_byte, high_byte);
    }

    pub fn tick(&mut self) {
        self.cycles += 1;
        if self.cycles == 114 * 154 {
            self.cycles -= 114 * 154;
        }

        self.scan_line = (self.cycles / 114) as u8;

        // If we just hit a new scan line and its in the visible area, draw it
        if self.cycles % 114 == 0 && self.scan_line < 144 {
            self.draw_scanline();
        }

        // Only fires the first time it enters VBlank
        if self.cycles == VBLANK_START {
            self.interrupt_vblank = true;
        }
    }

    // Draw the entire scanline we are on
    // Not cycle accurate, but it should be good enough for a first iteration
    fn draw_scanline(&mut self) {
        // Tiles are 8 pixels tall, so scanline + scy to get overall index, divide by 8 to get
        // which row the tile is in
        let tile_map_y_index = (self.scan_line + self.scy) as usize / 8;

        // Pick which tile map based on lcdc's 6th bit
        let mut tile_map_address: usize = if self.lcdc & 0b0100_0000 > 0 {
            0x9800 - 0x8000
        } else {
            0x9C00 - 0x8000
        };

        // the y addresses increment the address by 0x20
        tile_map_address += (tile_map_y_index * 0x20) as usize;

        // Which row inside of the tile are we on. scanline divide 8 gives us the tilemap row, but
        // the remainder is which row within the tile we're on
        let tile_y_index = self.scan_line % 8;

        let tiles_per_lcd_width = LCD_WIDTH / 8;

        for tile_map_x_index in 0..tiles_per_lcd_width {
            let tile_id = self.vram[(tile_map_address + tile_map_x_index) as usize];
            let tile_row = self.get_tile_row(tile_id, tile_y_index);
            for tile_x_index in 0..8 {
                // iterate through the tile row we're on and put it on the lcd
                let pixel = tile_row & (0b11 << tile_x_index * 2);
                let lcd_index =
                    self.scan_line as usize * LCD_WIDTH + tile_map_x_index * 8 + tile_x_index;
                warn!(
                    "Putting pixel {} on lcd for y index {} and x index {}",
                    pixel,
                    self.scan_line as usize * LCD_WIDTH,
                    tile_x_index
                );
                self.lcd[lcd_index] = (pixel >> (tile_x_index * 2)) as u8;
            }
        }
    }

    /// get_frame returns the array of pixels for the given frame
    ///
    /// The idea is the emulator will step over the exact number of M-Cycles
    /// to have a frame ready, and then the consumer will call this to render
    /// the frame.
    pub fn get_frame(&self) -> [u8; TOTAL_PIXELS] {
        debug!("getting frame: {:?}", self.lcd);
        self.lcd
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0x8000..=0x9FFF => {
                match self.get_mode() {
                    Mode::THREE => 0xFF, // Return garbage as PPU needs it
                    _ => self.vram[(address - 0x8000) as usize],
                }
            }
            0xFE00..=0xFE9F => {
                match self.get_mode() {
                    // Apparently DMA can access this during this time, but nothing else?
                    Mode::TWO => 0xFF,
                    Mode::THREE => 0xFF,
                    _ => self.oam[(address - 0xFE00) as usize],
                }
            }
            0xFF40 => self.lcdc,
            0xFF41 => {
                let mut stat = self.stat | 0b1000_0000; // bit 7 is unused
                let lyc_compare = self.scan_line == self.lyc;

                if lyc_compare {
                    stat |= 1 << 2
                } else {
                    stat &= !(1 << 2)
                }

                stat
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.scan_line,
            0xFF45 => self.lyc,
            0xFF46 => self.dma,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            _ => panic!("Invalid PPU read address: {:X}", address),
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0x8000..=0x9FFF => {
                match self.get_mode() {
                    Mode::THREE => (), // Do nothing
                    _ => self.vram[(address - 0x8000) as usize] = value,
                }
            }
            0xFE00..=0xFE9F => {
                match self.get_mode() {
                    // Apparently DMA can access this during this time, but nothing else?
                    Mode::TWO => (),
                    Mode::THREE => (),
                    _ => self.oam[(address - 0xFE00) as usize] = value,
                }
            }
            0xFF40 => self.lcdc = value,
            0xFF41 => self.stat |= value & 0b1111_1000, // last 3 bits are read-only
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => (), // ly is read-only
            0xFF45 => self.lyc = value,
            0xFF46 => self.dma = value,
            0xFF47 => self.bgp = value,
            0xFF48 => self.obp0 = value,
            0xFF49 => self.obp1 = value,
            0xFF4A => self.wy = value,
            0xFF4B => self.wx = value,
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lyc_equals_ly_sets_stat_bit_on() {
        let mut ppu = PPU::new();

        ppu.lyc = 100;
        ppu.scan_line = 100;

        assert_eq!(ppu.read_byte(0xFF41), 0b1000_0100);
    }

    #[test]
    fn test_lyc_not_equals_ly_sets_stat_bit_off() {
        let mut ppu = PPU::new();

        ppu.lyc = 12;
        ppu.scan_line = 34;

        assert_eq!(ppu.read_byte(0xFF41), 0b1000_0000);
    }

    #[test]
    fn test_stat_last_3_bits_are_read_only() {
        let mut ppu = PPU::new();

        ppu.lyc = 12;
        ppu.scan_line = 34;

        ppu.write_byte(0xFF41, 0b0111_1111);
        assert_eq!(ppu.read_byte(0xFF41), 0b1111_1000);
    }

    #[test]
    fn test_get_tile_address_8000_block_0() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0001_0000; // bit 4 set to 1

        assert_eq!(ppu.get_tile_address(0x0), 0x8000);
        assert_eq!(ppu.get_tile_address(0x7F), 0x87F0);
    }

    #[test]
    fn test_get_tile_address_8000_block_1() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0001_0000; // bit 4 set to 1

        assert_eq!(ppu.get_tile_address(0x80), 0x8800);
        assert_eq!(ppu.get_tile_address(0xFF), 0x8FF0);
    }

    #[test]
    fn test_get_tile_address_8800_block_2() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_0000; // bit 4 set to 0

        assert_eq!(ppu.get_tile_address(0), 0x9000);
        assert_eq!(ppu.get_tile_address(127), 0x97F0);
    }

    #[test]
    fn test_get_tile_address_8800_block_1() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_0000; // bit 4 set to 0

        assert_eq!(ppu.get_tile_address(0x80), 0x8800);
        assert_eq!(ppu.get_tile_address(0xFF), 0x8FF0);
    }

    #[test]
    fn test_get_tile_block_0() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0001_0000; // bit 4 set to 1

        // test case from pandocs
        ppu.write_byte(0x8500, 0x3C);
        ppu.write_byte(0x8501, 0x7E);
        ppu.write_byte(0x8502, 0x42);
        ppu.write_byte(0x8503, 0x42);
        ppu.write_byte(0x8504, 0x42);
        ppu.write_byte(0x8505, 0x42);
        ppu.write_byte(0x8506, 0x42);
        ppu.write_byte(0x8507, 0x42);
        ppu.write_byte(0x8508, 0x7E);
        ppu.write_byte(0x8509, 0x5E);
        ppu.write_byte(0x850A, 0x7E);
        ppu.write_byte(0x850B, 0x0A);
        ppu.write_byte(0x850C, 0x7C);
        ppu.write_byte(0x850D, 0x56);
        ppu.write_byte(0x850E, 0x38);
        ppu.write_byte(0x850F, 0x7C);

        let tile = ppu.get_tile(0x50);
        assert_eq!(tile[0], 0b00101111_11111000);
        assert_eq!(tile[1], 0b00110000_00001100);
        assert_eq!(tile[2], 0b00110000_00001100);
        assert_eq!(tile[3], 0b00110000_00001100);
        assert_eq!(tile[4], 0b00110111_11111100);
        assert_eq!(tile[5], 0b00010101_11011100);
        assert_eq!(tile[6], 0b00110111_01111000);
        assert_eq!(tile[7], 0b00101111_11100000);
    }

    #[test]
    fn test_get_tile_block_1() {
        let mut ppu = PPU::new();

        ppu.write_byte(0x8C60, 0x3C);
        ppu.write_byte(0x8C61, 0x7E);
        ppu.write_byte(0x8C62, 0x42);
        ppu.write_byte(0x8C63, 0x42);
        ppu.write_byte(0x8C64, 0x42);
        ppu.write_byte(0x8C65, 0x42);
        ppu.write_byte(0x8C66, 0x42);
        ppu.write_byte(0x8C67, 0x42);
        ppu.write_byte(0x8C68, 0x7E);
        ppu.write_byte(0x8C69, 0x5E);
        ppu.write_byte(0x8C6A, 0x7E);
        ppu.write_byte(0x8C6B, 0x0A);
        ppu.write_byte(0x8C6C, 0x7C);
        ppu.write_byte(0x8C6D, 0x56);
        ppu.write_byte(0x8C6E, 0x38);
        ppu.write_byte(0x8C6F, 0x7C);

        let tile = ppu.get_tile(0xC6);
        assert_eq!(tile[0], 0b00101111_11111000);
        assert_eq!(tile[1], 0b00110000_00001100);
        assert_eq!(tile[2], 0b00110000_00001100);
        assert_eq!(tile[3], 0b00110000_00001100);
        assert_eq!(tile[4], 0b00110111_11111100);
        assert_eq!(tile[5], 0b00010101_11011100);
        assert_eq!(tile[6], 0b00110111_01111000);
        assert_eq!(tile[7], 0b00101111_11100000);
    }

    #[test]
    fn test_get_tile_block_2() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_0000; // bit 4 set to 0

        ppu.write_byte(0x9500, 0x3C);
        ppu.write_byte(0x9501, 0x7E);
        ppu.write_byte(0x9502, 0x42);
        ppu.write_byte(0x9503, 0x42);
        ppu.write_byte(0x9504, 0x42);
        ppu.write_byte(0x9505, 0x42);
        ppu.write_byte(0x9506, 0x42);
        ppu.write_byte(0x9507, 0x42);
        ppu.write_byte(0x9508, 0x7E);
        ppu.write_byte(0x9509, 0x5E);
        ppu.write_byte(0x950A, 0x7E);
        ppu.write_byte(0x950B, 0x0A);
        ppu.write_byte(0x950C, 0x7C);
        ppu.write_byte(0x950D, 0x56);
        ppu.write_byte(0x950E, 0x38);
        ppu.write_byte(0x950F, 0x7C);

        let tile = ppu.get_tile(0x50);
        assert_eq!(tile[0], 0b00101111_11111000);
        assert_eq!(tile[1], 0b00110000_00001100);
        assert_eq!(tile[2], 0b00110000_00001100);
        assert_eq!(tile[3], 0b00110000_00001100);
        assert_eq!(tile[4], 0b00110111_11111100);
        assert_eq!(tile[5], 0b00010101_11011100);
        assert_eq!(tile[6], 0b00110111_01111000);
        assert_eq!(tile[7], 0b00101111_11100000);
    }
}
