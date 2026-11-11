use log::*;

#[derive(Debug, PartialEq)]
enum Mode {
    ZERO,
    ONE,
    TWO,
    THREE,
}

const VBLANK_START: u64 = 114 * 144;

pub const LCD_WIDTH: usize = 160;
pub const LCD_HEIGHT: usize = 144;
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

        let dots = self.cycles * 4;
        let dots_on_row = dots % DOTS_PER_ROW;

        if dots_on_row < DOTS_PER_MODE_2 {
            return Mode::TWO;
        }

        // Need to figure out the penalties and stuff
        // Mode 3 can be variable, nut never mor than 289
        if dots_on_row <= 289 + DOTS_PER_MODE_2 {
            return Mode::THREE;
        }

        return Mode::ZERO;
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

    fn get_tile_row(&self, tile_id: u8, row_index: u8) -> u16 {
        let address = self.get_tile_address(tile_id);
        let vram_address = address as usize - 0x8000;

        let low_byte = self.vram[vram_address + (2 * row_index) as usize];
        let high_byte = self.vram[vram_address + (2 * row_index) as usize + 1];

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

    fn get_pixel_data(&self, tile_row: u16, tile_x_index: u8) -> u8 {
        debug_assert!(tile_x_index < 8, "tile index is greater than 8");
        let shift_distance = 14 - (tile_x_index * 2);
        let mask = 0b11 << shift_distance;

        ((tile_row & mask) >> shift_distance) as u8
    }

    /// Based on scan_line and scy, determines the tilemap Y ID and the row number
    /// Returns a tuple of Y ID, and row index
    ///
    /// If it calculates its the tilemap value at Y = 20 and we're on the second to last row
    /// it would return (20, 6)
    fn get_tile_map_y(&self) -> (u8, u8) {
        debug_assert!(
            self.scan_line < 144,
            "Cant get tile_map_x for scanline over 143"
        );
        let number_of_pixels = ((self.scan_line as u16 + self.scy as u16) % 256) as u8;
        let y_id = number_of_pixels / 8;
        let row_index = number_of_pixels % 8;

        (y_id, row_index)
    }

    fn get_tile_map_address(&self, x_id: u8, y_id: u8) -> usize {
        // Pick which tile map based on lcdc's 3rd bit
        // index it off of our shortened vram
        let base_tile_map_address: usize = if self.lcdc & 0b0000_1000 == 0 {
            0x9800
        } else {
            0x9C00
        };

        base_tile_map_address + (y_id as usize * 0x20) + x_id as usize
    }

    // Draw the entire scanline we are on
    // Not cycle accurate, but it should be good enough for a first iteration
    fn draw_scanline(&mut self) {
        // Tiles are 8 pixels tall, so scanline + scy to get overall index.
        // Mod by 144 since there are only 144 rows shown, then divide by 8 to get
        // which row the tile is in
        // This will be consistent for the entire scanline, but if it wraps around the X,
        // the address can be lower the futher the scan line writes
        let (tile_map_y_id, tile_row_index) = self.get_tile_map_y();

        // TODO - optimize this so we dont have to get the same tile row every iteration
        for lcd_x_index in 0..LCD_WIDTH {
            let pixel_index = ((lcd_x_index + self.scx as usize) % 256) as u8; // wrap over
            let tile_map_x_id = pixel_index / 8;
            let tile_x_index = pixel_index % 8;

            let tile_map_address = self.get_tile_map_address(tile_map_x_id, tile_map_y_id);
            let tile_id = self.vram[tile_map_address - 0x8000];
            let tile_row = self.get_tile_row(tile_id, tile_row_index);

            let pixel = self.get_pixel_data(tile_row, tile_x_index);
            let lcd_index = self.scan_line as usize * LCD_WIDTH + lcd_x_index;
            self.lcd[lcd_index] = pixel;
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
                // match self.get_mode() {
                //     Mode::THREE => (), // Do nothing
                //     _ => self.vram[(address - 0x8000) as usize] = value,
                // }
                self.vram[(address - 0x8000) as usize] = value;
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

        assert_eq!(ppu.get_tile_row(0x50, 0), 0b00101111_11111000);
        assert_eq!(ppu.get_tile_row(0x50, 1), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0x50, 2), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0x50, 3), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0x50, 4), 0b00110111_11111100);
        assert_eq!(ppu.get_tile_row(0x50, 5), 0b00010101_11011100);
        assert_eq!(ppu.get_tile_row(0x50, 6), 0b00110111_01111000);
        assert_eq!(ppu.get_tile_row(0x50, 7), 0b00101111_11100000);
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

        assert_eq!(ppu.get_tile_row(0xC6, 0), 0b00101111_11111000);
        assert_eq!(ppu.get_tile_row(0xC6, 1), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0xC6, 2), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0xC6, 3), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0xC6, 4), 0b00110111_11111100);
        assert_eq!(ppu.get_tile_row(0xC6, 5), 0b00010101_11011100);
        assert_eq!(ppu.get_tile_row(0xC6, 6), 0b00110111_01111000);
        assert_eq!(ppu.get_tile_row(0xC6, 7), 0b00101111_11100000);
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

        assert_eq!(ppu.get_tile_row(0x50, 0), 0b00101111_11111000);
        assert_eq!(ppu.get_tile_row(0x50, 1), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0x50, 2), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0x50, 3), 0b00110000_00001100);
        assert_eq!(ppu.get_tile_row(0x50, 4), 0b00110111_11111100);
        assert_eq!(ppu.get_tile_row(0x50, 5), 0b00010101_11011100);
        assert_eq!(ppu.get_tile_row(0x50, 6), 0b00110111_01111000);
        assert_eq!(ppu.get_tile_row(0x50, 7), 0b00101111_11100000);
    }

    #[test]
    fn test_get_pixel_data() {
        let ppu = PPU::new();

        let tile_row = 0b00101101_11111001;
        assert_eq!(ppu.get_pixel_data(tile_row, 0), 0b00);
        assert_eq!(ppu.get_pixel_data(tile_row, 1), 0b10);
        assert_eq!(ppu.get_pixel_data(tile_row, 2), 0b11);
        assert_eq!(ppu.get_pixel_data(tile_row, 3), 0b01);
        assert_eq!(ppu.get_pixel_data(tile_row, 4), 0b11);
        assert_eq!(ppu.get_pixel_data(tile_row, 5), 0b11);
        assert_eq!(ppu.get_pixel_data(tile_row, 6), 0b10);
        assert_eq!(ppu.get_pixel_data(tile_row, 7), 0b01);
    }

    #[test]
    fn test_get_tile_map_y_scy_is_zero() {
        let mut ppu = PPU::new();
        ppu.scy = 0;

        ppu.scan_line = 0;
        assert_eq!(ppu.get_tile_map_y(), (0, 0));

        ppu.scan_line = 1;
        assert_eq!(ppu.get_tile_map_y(), (0, 1));

        ppu.scan_line = 7;
        assert_eq!(ppu.get_tile_map_y(), (0, 7));

        ppu.scan_line = 8;
        assert_eq!(ppu.get_tile_map_y(), (1, 0));

        ppu.scan_line = 143;
        assert_eq!(ppu.get_tile_map_y(), (17, 7));
    }

    #[test]
    fn test_get_tile_map_y_scy_is_10() {
        let mut ppu = PPU::new();
        ppu.scy = 10; // starts displaying at 10 pixels down

        ppu.scan_line = 0;
        assert_eq!(ppu.get_tile_map_y(), (1, 2));

        ppu.scan_line = 5;
        assert_eq!(ppu.get_tile_map_y(), (1, 7));

        ppu.scan_line = 6;
        assert_eq!(ppu.get_tile_map_y(), (2, 0));

        ppu.scan_line = 143;
        assert_eq!(ppu.get_tile_map_y(), (19, 1));
    }

    #[test]
    fn test_get_tile_map_y_scy_is_255() {
        let mut ppu = PPU::new();
        ppu.scy = 255; // starts displaying at 255 pixels down

        ppu.scan_line = 0;
        assert_eq!(ppu.get_tile_map_y(), (31, 7));

        ppu.scan_line = 1;
        assert_eq!(ppu.get_tile_map_y(), (0, 0));

        ppu.scan_line = 143;
        assert_eq!(ppu.get_tile_map_y(), (17, 6));
    }

    #[test]
    fn tet_get_tilemap_address_tilemap_0() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_0000;

        assert_eq!(ppu.get_tile_map_address(0, 0), 0x9800);
        assert_eq!(ppu.get_tile_map_address(15, 15), 0x99EF);
        assert_eq!(ppu.get_tile_map_address(31, 31), 0x9BFF);
        assert_eq!(ppu.get_tile_map_address(5, 20), 0x9A85);
    }

    #[test]
    fn tet_get_tilemap_address_tilemap_1() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_1000;

        assert_eq!(ppu.get_tile_map_address(0, 0), 0x9C00);
        assert_eq!(ppu.get_tile_map_address(15, 15), 0x9DEF);
        assert_eq!(ppu.get_tile_map_address(31, 31), 0x9FFF);
        assert_eq!(ppu.get_tile_map_address(5, 20), 0x9E85);
    }

    #[test]
    fn tet_get_mode_two_line_0() {
        let mut ppu = PPU::new();

        ppu.cycles = 0;
        assert_eq!(ppu.get_mode(), Mode::TWO);

        ppu.cycles = 19;
        assert_eq!(ppu.get_mode(), Mode::TWO);

        ppu.cycles = 20;
        assert_ne!(ppu.get_mode(), Mode::TWO);
    }

    #[test]
    fn tet_get_mode_two_line_51() {
        let mut ppu = PPU::new();

        ppu.cycles = 5814;
        assert_eq!(ppu.get_mode(), Mode::TWO);

        ppu.cycles = 5833;
        assert_eq!(ppu.get_mode(), Mode::TWO);

        ppu.cycles = 5834;
        assert_ne!(ppu.get_mode(), Mode::TWO);
    }

    #[test]
    fn tet_get_mode_one() {
        let mut ppu = PPU::new();

        ppu.cycles = 16_415;
        assert_ne!(ppu.get_mode(), Mode::ONE);

        ppu.cycles = 16_416;
        assert_eq!(ppu.get_mode(), Mode::ONE);

        ppu.cycles = 17_556;
        assert_eq!(ppu.get_mode(), Mode::ONE);
    }
}
