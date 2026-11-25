/*!
* The glue for the wasm bundle. Provides wasm bindings for the gameboy and buttons, which keeps the
* core library free from any knowledge of consumers. We could put these bindings on the core
* itself, but I didn't want to mix concerns and keep the core as clean as possible
*/
use gameboy_core::{Gameboy, LCD_HEIGHT, LCD_WIDTH};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub enum Button {
    Start,
    Select,
    A,
    B,
    Up,
    Down,
    Left,
    Right,
}

impl From<Button> for gameboy_core::Button {
    fn from(button: Button) -> Self {
        match button {
            Button::Start => Self::Start,
            Button::Select => Self::Select,
            Button::A => Self::A,
            Button::B => Self::B,
            Button::Up => Self::Up,
            Button::Down => Self::Down,
            Button::Left => Self::Left,
            Button::Right => Self::Right,
        }
    }
}

#[wasm_bindgen]
pub struct Emulator {
    gameboy: Gameboy,
}

#[wasm_bindgen]
impl Emulator {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: Vec<u8>) -> Emulator {
        Emulator {
            gameboy: Gameboy::new(rom),
        }
    }

    pub fn step_frame(&mut self) {
        self.gameboy.step_frame();
    }

    /// Returns the RGBA bytes for the frame. Since canvas takes a separate byte array for R, G, B, and A,
    /// we convert the gameboy's color to those
    pub fn frame(&mut self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(LCD_WIDTH * LCD_HEIGHT * 4);
        for color in self.gameboy.get_frame() {
            let color_slice: [u8; 4] = match color {
                0b00 => [0xE0, 0xE0, 0xE0, 0xFF],
                0b01 => [0x88, 0xC0, 0x70, 0xFF],
                0b10 => [0x34, 0x68, 0x56, 0xFF],
                0b11 => [0x08, 0x18, 0x20, 0xFF],
                _ => panic!("Invalid color from gameboy"),
            };
            rgba.extend_from_slice(&color_slice);
        }
        rgba
    }

    /// The cartridge RAM (the game's save data). Empty if the cartridge has no RAM.
    pub fn get_cartridge_ram(&self) -> Vec<u8> {
        self.gameboy.get_cartridge_ram().to_vec()
    }

    /// Restore cartridge RAM from a previous `cartridge_ram()`. Throws if the size doesn't match.
    pub fn load_cartridge_ram(&mut self, data: &[u8]) -> Result<(), JsError> {
        self.gameboy
            .load_cartridge_ram(data)
            .map_err(|err| JsError::new(&err))
    }

    pub fn press_button(&mut self, button: Button) {
        self.gameboy.press_button(button.into());
    }

    pub fn release_button(&mut self, button: Button) {
        self.gameboy.release_button(button.into());
    }
}
