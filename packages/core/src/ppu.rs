use log::*;

#[derive(Debug, PartialEq)]
enum Mode {
    /// HBlank
    ZERO,
    /// VBlank
    ONE,
    /// OAM Scan
    TWO,
    /// Drawing pixels
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
    wlc: u8,
    pub dma: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,

    y_condition: bool,
    sprite_buffer: [u8; 10],
    sprite_counter: u8,
    scan_line: u8,
    pub cycles: u64,
    pub oam_cycles_left: u8,

    pub vram: [u8; 0x2000],
    pub oam: [u8; 160],
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
            wlc: 0,
            dma: 0,
            bgp: 0,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,

            scan_line: 0,
            cycles: 0,
            y_condition: false,
            sprite_buffer: [0; 10],
            sprite_counter: 0,
            oam_cycles_left: 0,

            vram: [0; 0x2000],
            oam: [0; 160],
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

        // TODO - figure out the penalties and stuff
        // But using minimimum mode 3 until then
        if dots_on_row <= 172 + DOTS_PER_MODE_2 {
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

        if self.oam_cycles_left > 0 {
            self.oam_cycles_left -= 1;
        }

        self.scan_line = (self.cycles / 114) as u8;

        // If we just hit a new scan line and its in the visible area, we oam scan
        if self.cycles % 114 == 0 && self.scan_line < 144 {
            if self.stat & 0b0010_0000 > 0 {
                self.interrupt_stat = true;
            }

            self.oam_scan();
        }

        // If start of scan line matches LYC and LY, set stat interrupt
        if self.cycles % 114 == 0 && self.scan_line == self.lyc && self.stat & 0b0100_0000 > 0 {
            self.interrupt_stat = true;
        }

        // We just entered Mode 3, so now we draw the pixels
        if self.cycles % 114 == 20 && self.scan_line < 144 {
            self.draw_scanline();

            // reset sprite buffer
            self.sprite_buffer = [0; 10];
            self.sprite_counter = 0;
        }

        // 80 + 172 dots is 63, which is the shortest period it can be to reach HBlank
        // Using this until I figure out penalties
        if self.cycles % 114 == 63 && self.scan_line < 144 {
            if self.stat & 0b0000_1000 > 0 {
                self.interrupt_stat = true;
            }
        }

        // Only fires the first time it enters VBlank
        if self.cycles == VBLANK_START {
            self.interrupt_vblank = true;
            self.y_condition = false;
            self.wlc = 0;

            // stat has mode 1 int select, so flag it on vblank
            if self.stat & 0b0001_0000 > 0 {
                self.interrupt_stat = true;
            }
        }
    }

    /// Given a tile_row and tile_x, it calculates and returns the color index
    fn get_pixel_color_index(&self, tile_row: u16, tile_x_index: u8) -> u8 {
        debug_assert!(tile_x_index < 8, "tile index is greater than 8");
        // tiles are 16 bits, but we only care about a single pixel which is 2 bits
        let mask = 0b11;
        // X = 0 is the two most signifigant bits, so we need to move them 14 over.
        // Each X after is 2 scoots less
        let shift_distance = 14 - (tile_x_index * 2);

        ((tile_row >> shift_distance) as u8) & mask
    }

    fn get_color_from_pallet(&self, color_index: u8, pallet: u8) -> u8 {
        debug_assert!(color_index < 4, "color index is greater than 3");
        match color_index {
            3 => (pallet & 0b1100_0000) >> 6,
            2 => (pallet & 0b0011_0000) >> 4,
            1 => (pallet & 0b0000_1100) >> 2,
            _ => pallet & 0b0000_0011,
        }
    }

    /// Based on scan_line and scy, determines the tilemap Y ID and the row number
    /// Returns a tuple of Y ID, and row index
    ///
    /// If it calculates its the tilemap value at Y = 20 and we're on the second to last row
    /// it would return (20, 6)
    fn get_background_tile_map_y(&self) -> (u8, u8) {
        debug_assert!(
            self.scan_line < 144,
            "Cant get tile_map_x for scanline over 143"
        );
        let number_of_pixels = ((self.scan_line as u16 + self.scy as u16) % 256) as u8;
        let y_id = number_of_pixels / 8;
        let row_index = number_of_pixels % 8;

        (y_id, row_index)
    }

    /// gets the tile's y ID for the window, which always starts at 0,0
    /// Also gets which row within that tile we display for this scanline
    fn get_window_tile_map_y(&self) -> (u8, u8) {
        debug_assert!(
            self.scan_line < 144,
            "Cant get tile_map_x for scanline over 143"
        );
        let number_of_pixels = self.wlc;
        let y_id = number_of_pixels / 8;
        let row_index = number_of_pixels % 8;

        (y_id, row_index)
    }

    fn get_tile_map_address(&self, x_id: u8, y_id: u8, is_window: bool) -> usize {
        // Pick which tile map based on lcdc's 3rd bit for background
        // or 6th for window
        // index it off of our shortened vram
        let mask = if is_window { 0b0100_0000 } else { 0b0000_1000 };

        let base_tile_map_address: usize = if self.lcdc & mask == 0 {
            0x9800
        } else {
            0x9C00
        };

        base_tile_map_address + (y_id as usize * 0x20) + x_id as usize
    }

    fn draw_window_pixel(&mut self, counter: usize) {
        let (tile_map_y_id, tile_row_index) = self.get_window_tile_map_y();

        let pixel_index = counter as u8 + 7 - self.wx; // window doesn't wrap, but is offset by wx
        let tile_map_x_id = pixel_index / 8;
        let tile_x_index = pixel_index % 8;

        let tile_map_address = self.get_tile_map_address(tile_map_x_id, tile_map_y_id, true);
        let tile_id = self.vram[tile_map_address - 0x8000];
        let tile_row = self.get_tile_row(tile_id, tile_row_index);

        let color_index = self.get_pixel_color_index(tile_row, tile_x_index);
        let lcd_index = self.scan_line as usize * LCD_WIDTH + counter;
        self.lcd[lcd_index] = self.get_color_from_pallet(color_index, self.bgp);
    }

    fn draw_background_pixel(&mut self, counter: usize) {
        // Tiles are 8 pixels tall, so scanline + scy to get overall index.
        // Mod by 256 since the map is 256 pixels tall, then divide by 8 to get
        // which row the tile is in
        // Since we mod by 256, the number we get is pixels. So divide by 8 pixels gives us the
        // tile, and the remainder gives us the tile row
        // This will be consistent for the entire scanline, but if it wraps around the X,
        // the address can be lower the futher the scan line writes
        let (tile_map_y_id, tile_row_index) = self.get_background_tile_map_y();

        let pixel_index = ((counter + self.scx as usize) % 256) as u8; // wrap over
        let tile_map_x_id = pixel_index / 8;
        let tile_x_index = pixel_index % 8;

        let tile_map_address = self.get_tile_map_address(tile_map_x_id, tile_map_y_id, false);
        let tile_id = self.vram[tile_map_address - 0x8000];
        let tile_row = self.get_tile_row(tile_id, tile_row_index);

        let color_index = self.get_pixel_color_index(tile_row, tile_x_index);
        let lcd_index = self.scan_line as usize * LCD_WIDTH + counter;
        self.lcd[lcd_index] = self.get_color_from_pallet(color_index, self.bgp);
    }

    fn get_sprite_tile_row(&self, tile_id: u8, y_position: u8) -> u16 {
        // let obj_size = if (self.lcdc & 0b0000_0100) == 0 {
        //     8
        // } else {
        //     todo!("Need to implement 8x16 sprites")
        // };

        // This method only works for 8x8 sprites currently
        // TODO - fix this so it works for 8*16 sprites, tile_index has to change too
        let scanline_with_offset = self.scan_line + 16;
        let row_index = scanline_with_offset - y_position;

        let vram_address = (tile_id as usize) << 4;
        let low_byte = self.vram[vram_address + (2 * row_index) as usize];
        let high_byte = self.vram[vram_address + (2 * row_index) as usize + 1];
        self.combine_u8s_into_tile_row(low_byte, high_byte)
    }

    fn get_sprite_tile_x_index(&self, counter: usize, tile_x_position: u8) -> u8 {
        8 - (tile_x_position - counter as u8)
    }

    /// Given the x counter and sprite_buffer's index, draw the sprite to the screen
    fn draw_sprite_pixel(&mut self, counter: usize, sprite_index: u8) {
        let y_position = self.oam[sprite_index as usize];
        let x_position = self.oam[sprite_index as usize + 1];
        let tile_index = self.oam[sprite_index as usize + 2];

        let tile_row = self.get_sprite_tile_row(tile_index, y_position);
        let tile_x_index = self.get_sprite_tile_x_index(counter, x_position);
        // TODO - figure out priority on attribute. pandocs says if priority is 1, bg
        // and window color are drawn over? Seems backwards.
        let attributes = self.oam[sprite_index as usize + 3];

        let pallet = if attributes & 0b0001_0000 == 0 {
            self.obp0
        } else {
            self.obp1
        };
        let color_index = self.get_pixel_color_index(tile_row, tile_x_index);
        let lcd_index = self.scan_line as usize * LCD_WIDTH + counter;
        self.lcd[lcd_index] = self.get_color_from_pallet(color_index, pallet);
    }

    /// Scans oam for any objects to display and adds their index to the sprite_buffer
    fn oam_scan(&mut self) {
        let obj_enable = (self.lcdc & 0b0000_0010) > 0;
        if !obj_enable {
            return;
        }

        // If true, sprites are 16 pixels tall. Otherwise, just 1 tile, so 8 pixels
        let obj_size = if (self.lcdc & 0b0000_0100) > 0 { 16 } else { 8 };

        for i in 0..40 {
            let sprite_index = i * 4;
            let y_position = self.oam[sprite_index];

            let bottom_y_position = y_position + obj_size;
            let scanline_with_offset = self.scan_line + 16;
            if y_position <= scanline_with_offset && bottom_y_position >= scanline_with_offset {
                self.sprite_buffer[self.sprite_counter as usize] = sprite_index as u8;
                self.sprite_counter += 1;
            }

            // buffer is full, quit scanning
            if self.sprite_counter >= 10 {
                return;
            }
        }
    }

    fn is_sprite_within_x_bounds(&self, counter: usize, x_position: u8) -> bool {
        // the oam scan to load the sprites already checked for y position
        // sprites are 8 pixels wide and they start 8 pixels off the screen
        // So if the counter is
        let counter_with_offset = counter as u8 + 8;
        x_position <= counter_with_offset && counter_with_offset <= x_position + 8
    }

    // Draw the entire scanline we are on
    // Not cycle accurate, but it should be good enough for a first iteration
    fn draw_scanline(&mut self) {
        if self.scan_line == self.wy {
            self.y_condition = true;
        }
        let mut wrote_from_window = false;

        // TODO - optimize this so we dont have to get the same tile row every iteration
        for counter in 0..LCD_WIDTH {
            if self.lcdc & 0b1 == 0 {
                // bg/window is disabled, print blank
                let lcd_index = self.scan_line as usize * LCD_WIDTH + counter;
                self.lcd[lcd_index] = 0;
                // TODO - optomize this to not draw window/bg if it will be replaced by an obj
            } else {
                if counter + 7 >= self.wx as usize
                    && self.y_condition
                    && self.lcdc & 0b0010_0000 != 0
                {
                    self.draw_window_pixel(counter);
                    wrote_from_window = true;
                } else {
                    self.draw_background_pixel(counter);
                }
            }

            // TODO - optimize not to check objects every pixel
            if self.lcdc & 0b0000_0010 > 0 {
                for sprite_buffer_index in 0..self.sprite_counter {
                    let sprite_index = self.sprite_buffer[sprite_buffer_index as usize];
                    let x_position = self.oam[sprite_index as usize + 1];
                    // let attributes = self.oam[sprite_index as usize + 1];
                    // TODO - figure out priority on attribute. pandocs says if priority is 1, bg
                    // and window color are drawn over? Seems backwards.

                    // the oam scan to load the sprites already checked for y position, so just
                    // check the x
                    if self.is_sprite_within_x_bounds(counter, x_position) {
                        self.draw_sprite_pixel(counter, sprite_index);
                        break; // TODO - figure out prioritization of which sprite to draw
                    }
                }
            }
        }

        if wrote_from_window {
            self.wlc += 1;
            // 6 dot penalty here
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
                // bit 7 is unused so set to 1, clear bottom 3 bits
                let mut stat = (self.stat | 0b1000_0000) & !(0b11);

                let lyc_compare = self.scan_line == self.lyc;
                if lyc_compare {
                    stat |= 1 << 2
                } else {
                    stat &= !(1 << 2)
                }

                if self.lcdc & 0b1000_0000 > 0 {
                    stat |= match self.get_mode() {
                        Mode::ZERO => 0,
                        Mode::ONE => 0b1,
                        Mode::TWO => 0b10,
                        Mode::THREE => 0b11,
                    }
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
            0xFF41 => self.stat = value, // Can write whatever, as the read-only bits are handled on read
            0xFF42 => self.scy = value,
            0xFF43 => self.scx = value,
            0xFF44 => (), // ly is read-only
            0xFF45 => self.lyc = value,
            0xFF46 => {
                self.oam_cycles_left = 160; // DMA transfer takes 160 cycles to complete
                self.dma = value
            }
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
        assert_eq!(ppu.get_pixel_color_index(tile_row, 0), 0b00);
        assert_eq!(ppu.get_pixel_color_index(tile_row, 1), 0b10);
        assert_eq!(ppu.get_pixel_color_index(tile_row, 2), 0b11);
        assert_eq!(ppu.get_pixel_color_index(tile_row, 3), 0b01);
        assert_eq!(ppu.get_pixel_color_index(tile_row, 4), 0b11);
        assert_eq!(ppu.get_pixel_color_index(tile_row, 5), 0b11);
        assert_eq!(ppu.get_pixel_color_index(tile_row, 6), 0b10);
        assert_eq!(ppu.get_pixel_color_index(tile_row, 7), 0b01);
    }

    #[test]
    fn test_get_color_from_pallet() {
        let ppu = PPU::new();

        let mut bgp = 0b1111_1111;
        assert_eq!(ppu.get_color_from_pallet(0, bgp), 0b11);

        bgp = 0b0000_0000;
        assert_eq!(ppu.get_color_from_pallet(0, bgp), 0b00);

        bgp = 0b1110_0100;
        assert_eq!(ppu.get_color_from_pallet(0, bgp), 0b00);
        assert_eq!(ppu.get_color_from_pallet(1, bgp), 0b01);
        assert_eq!(ppu.get_color_from_pallet(2, bgp), 0b10);
        assert_eq!(ppu.get_color_from_pallet(3, bgp), 0b11);

        bgp = 0b0001_1011;
        assert_eq!(ppu.get_color_from_pallet(0, bgp), 0b11);
        assert_eq!(ppu.get_color_from_pallet(1, bgp), 0b10);
        assert_eq!(ppu.get_color_from_pallet(2, bgp), 0b01);
        assert_eq!(ppu.get_color_from_pallet(3, bgp), 0b00);
    }

    #[test]
    fn test_get_tile_map_y_scy_is_zero() {
        let mut ppu = PPU::new();
        ppu.scy = 0;

        ppu.scan_line = 0;
        assert_eq!(ppu.get_background_tile_map_y(), (0, 0));

        ppu.scan_line = 1;
        assert_eq!(ppu.get_background_tile_map_y(), (0, 1));

        ppu.scan_line = 7;
        assert_eq!(ppu.get_background_tile_map_y(), (0, 7));

        ppu.scan_line = 8;
        assert_eq!(ppu.get_background_tile_map_y(), (1, 0));

        ppu.scan_line = 143;
        assert_eq!(ppu.get_background_tile_map_y(), (17, 7));
    }

    #[test]
    fn test_get_tile_map_y_scy_is_10() {
        let mut ppu = PPU::new();
        ppu.scy = 10; // starts displaying at 10 pixels down

        ppu.scan_line = 0;
        assert_eq!(ppu.get_background_tile_map_y(), (1, 2));

        ppu.scan_line = 5;
        assert_eq!(ppu.get_background_tile_map_y(), (1, 7));

        ppu.scan_line = 6;
        assert_eq!(ppu.get_background_tile_map_y(), (2, 0));

        ppu.scan_line = 143;
        assert_eq!(ppu.get_background_tile_map_y(), (19, 1));
    }

    #[test]
    fn test_get_tile_map_y_scy_is_255() {
        let mut ppu = PPU::new();
        ppu.scy = 255; // starts displaying at 255 pixels down

        ppu.scan_line = 0;
        assert_eq!(ppu.get_background_tile_map_y(), (31, 7));

        ppu.scan_line = 1;
        assert_eq!(ppu.get_background_tile_map_y(), (0, 0));

        ppu.scan_line = 143;
        assert_eq!(ppu.get_background_tile_map_y(), (17, 6));
    }

    #[test]
    fn tet_get_background_tilemap_address_tilemap_0() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_0000;

        assert_eq!(ppu.get_tile_map_address(0, 0, false), 0x9800);
        assert_eq!(ppu.get_tile_map_address(15, 15, false), 0x99EF);
        assert_eq!(ppu.get_tile_map_address(31, 31, false), 0x9BFF);
        assert_eq!(ppu.get_tile_map_address(5, 20, false), 0x9A85);
    }

    #[test]
    fn tet_get_background_tilemap_address_tilemap_1() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_1000;

        assert_eq!(ppu.get_tile_map_address(0, 0, false), 0x9C00);
        assert_eq!(ppu.get_tile_map_address(15, 15, false), 0x9DEF);
        assert_eq!(ppu.get_tile_map_address(31, 31, false), 0x9FFF);
        assert_eq!(ppu.get_tile_map_address(5, 20, false), 0x9E85);
    }

    #[test]
    fn tet_get_window_tilemap_address_tilemap_0() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0000_0000;

        assert_eq!(ppu.get_tile_map_address(0, 0, true), 0x9800);
        assert_eq!(ppu.get_tile_map_address(15, 15, true), 0x99EF);
        assert_eq!(ppu.get_tile_map_address(31, 31, true), 0x9BFF);
        assert_eq!(ppu.get_tile_map_address(5, 20, true), 0x9A85);
    }

    #[test]
    fn tet_get_window_tilemap_address_tilemap_1() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b0100_0000;

        assert_eq!(ppu.get_tile_map_address(0, 0, true), 0x9C00);
        assert_eq!(ppu.get_tile_map_address(15, 15, true), 0x9DEF);
        assert_eq!(ppu.get_tile_map_address(31, 31, true), 0x9FFF);
        assert_eq!(ppu.get_tile_map_address(5, 20, true), 0x9E85);
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

    #[test]
    fn tet_read_stat_first_bit_set() {
        let mut ppu = PPU::new();
        let stat_addr = 0xFF41;

        ppu.stat = 0;
        assert_eq!(ppu.read_byte(stat_addr) & 0b1000_0000, 0b1000_0000);

        ppu.stat = 0xFF;
        assert_eq!(ppu.read_byte(stat_addr) & 0b1000_0000, 0b1000_0000);
    }

    #[test]
    fn tet_read_stat_3rd_bit_set_when_lyc_equal_lc() {
        let mut ppu = PPU::new();
        let stat_addr = 0xFF41;

        ppu.scan_line = 0;
        ppu.lyc = 0;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0100, 0b0000_0100);

        ppu.scan_line = 100;
        ppu.lyc = 100;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0100, 0b0000_0100);
    }

    #[test]
    fn tet_read_stat_3rd_bit_not_set_when_lyc_equal_lc() {
        let mut ppu = PPU::new();
        let stat_addr = 0xFF41;

        ppu.scan_line = 0;
        ppu.lyc = 1;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0100, 0);

        ppu.scan_line = 1;
        ppu.lyc = 0;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0100, 0);
    }

    #[test]
    fn test_read_stat_0_and_1_bits_equal_mode() {
        let mut ppu = PPU::new();
        let stat_addr = 0xFF41;
        ppu.lcdc = 0b1000_0000;

        ppu.cycles = 0;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 2);

        ppu.cycles = 21;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 3);

        ppu.cycles = 100;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 0);

        ppu.cycles = 70_000;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 1);
    }

    #[test]
    fn test_read_stat_0_and_1_bits_not_set_ppu_off() {
        let mut ppu = PPU::new();
        let stat_addr = 0xFF41;
        ppu.lcdc = 0b0000_0000;

        ppu.cycles = 0;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 0);

        ppu.cycles = 21;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 0);

        ppu.cycles = 100;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 0);

        ppu.cycles = 70_000;
        assert_eq!(ppu.read_byte(stat_addr) & 0b0000_0011, 0);
    }

    #[test]
    fn test_oam_scan_doesnt_add_obj_off_screen() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0010;

        ppu.oam_scan();

        assert_eq!(ppu.sprite_counter, 0);
        for i in 0..10 {
            assert_eq!(ppu.sprite_buffer[i], 0);
        }
    }

    #[test]
    fn test_oam_scan_adds_obj_half_off_screen() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0010;

        ppu.oam[0] = 10;
        ppu.oam[4] = 14;
        ppu.oam_scan();

        assert_eq!(ppu.sprite_counter, 2);
        assert_eq!(ppu.sprite_buffer[0], 0);
        assert_eq!(ppu.sprite_buffer[1], 4);
        assert_eq!(ppu.sprite_buffer[2], 0);
    }

    #[test]
    fn test_oam_scan_doesnt_add_obj_when_off() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0000;

        ppu.oam[0] = 10;
        ppu.oam[4] = 14;
        ppu.oam[8] = 16;
        ppu.oam[12] = 17;
        ppu.oam_scan();

        assert_eq!(ppu.sprite_counter, 0);
        assert_eq!(ppu.sprite_buffer[0], 0);
        assert_eq!(ppu.sprite_buffer[1], 0);
        assert_eq!(ppu.sprite_buffer[2], 0);
    }

    #[test]
    fn test_oam_scan_adds_obj_on_screen() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0010;
        ppu.scan_line = 0;

        ppu.oam[4] = 16;
        ppu.oam[16] = 16;
        ppu.oam[156] = 16;
        ppu.oam_scan();

        assert_eq!(ppu.sprite_counter, 3);
        assert_eq!(ppu.sprite_buffer[0], 4);
        assert_eq!(ppu.sprite_buffer[1], 16);
        assert_eq!(ppu.sprite_buffer[2], 156);
    }

    #[test]
    fn test_oam_scan_adds_obj_on_screen_scanline_moved() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0010;
        ppu.scan_line = 50;

        ppu.oam[4] = 66;
        ppu.oam[16] = 66;
        ppu.oam[156] = 66;
        ppu.oam_scan();

        assert_eq!(ppu.sprite_counter, 3);
        assert_eq!(ppu.sprite_buffer[0], 4);
        assert_eq!(ppu.sprite_buffer[1], 16);
        assert_eq!(ppu.sprite_buffer[2], 156);
    }

    #[test]
    fn test_oam_scan_adds_obj_half_off_bottom() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0010;
        ppu.scan_line = 141;

        ppu.oam[4] = 152;
        ppu.oam[16] = 154;
        ppu.oam[152] = 156;
        ppu.oam[156] = 160;
        ppu.oam_scan();

        assert_eq!(ppu.sprite_counter, 3);
        assert_eq!(ppu.sprite_buffer[0], 4);
        assert_eq!(ppu.sprite_buffer[1], 16);
        assert_eq!(ppu.sprite_buffer[2], 152);
    }

    #[test]
    fn test_oam_scan_only_adds_first_10_objects() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0010;
        ppu.scan_line = 0;

        ppu.oam[0] = 16;
        ppu.oam[4] = 16;
        ppu.oam[8] = 16;
        ppu.oam[12] = 16;
        ppu.oam[16] = 16;
        ppu.oam[20] = 16;
        ppu.oam[24] = 16;
        ppu.oam[28] = 16;
        ppu.oam[32] = 16;
        ppu.oam[36] = 16;
        ppu.oam[40] = 16;
        ppu.oam[156] = 16;
        ppu.oam_scan();

        assert_eq!(ppu.sprite_counter, 10);
        assert_eq!(ppu.sprite_buffer[0], 0);
        assert_eq!(ppu.sprite_buffer[1], 4);
        assert_eq!(ppu.sprite_buffer[2], 8);
        assert_eq!(ppu.sprite_buffer[3], 12);
        assert_eq!(ppu.sprite_buffer[4], 16);
        assert_eq!(ppu.sprite_buffer[5], 20);
        assert_eq!(ppu.sprite_buffer[6], 24);
        assert_eq!(ppu.sprite_buffer[7], 28);
        assert_eq!(ppu.sprite_buffer[8], 32);
        assert_eq!(ppu.sprite_buffer[9], 36);
    }

    #[test]
    fn test_get_8x8_sprite_tile_row() {
        let mut ppu = PPU::new();
        ppu.lcdc = 0b1000_0000;
        ppu.scan_line = 0;
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

        assert_eq!(ppu.get_sprite_tile_row(0x50, 16), 0b00101111_11111000);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 15), 0b00110000_00001100);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 14), 0b00110000_00001100);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 13), 0b00110000_00001100);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 12), 0b00110111_11111100);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 11), 0b00010101_11011100);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 10), 0b00110111_01111000);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 9), 0b00101111_11100000);
        assert_eq!(ppu.get_sprite_tile_row(0x50, 8), 0);
    }

    #[test]
    fn test_get_sprite_tile_x_index() {
        let ppu = PPU::new();

        assert_eq!(ppu.get_sprite_tile_x_index(0, 1), 7);
        assert_eq!(ppu.get_sprite_tile_x_index(0, 8), 0);
        assert_eq!(ppu.get_sprite_tile_x_index(0, 4), 4);

        assert_eq!(ppu.get_sprite_tile_x_index(7, 8), 7);
    }

    #[test]
    fn test_is_sprite_within_x_bounds() {
        let ppu = PPU::new();

        assert_eq!(ppu.is_sprite_within_x_bounds(0, 0), false);
        assert_eq!(ppu.is_sprite_within_x_bounds(0, 1), true);
        assert_eq!(ppu.is_sprite_within_x_bounds(0, 8), true);
        assert_eq!(ppu.is_sprite_within_x_bounds(0, 9), false);

        assert_eq!(ppu.is_sprite_within_x_bounds(10, 0), false);
        assert_eq!(ppu.is_sprite_within_x_bounds(10, 10), false);
        assert_eq!(ppu.is_sprite_within_x_bounds(10, 11), true);
        assert_eq!(ppu.is_sprite_within_x_bounds(10, 18), true);
        assert_eq!(ppu.is_sprite_within_x_bounds(10, 19), false);
    }
}
