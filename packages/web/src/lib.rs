/*!
* The glue for the wasm bundle. Provides wasm bindings for the gameboy and buttons, which keeps the
* core library free from any knowledge of consumers. We could put these bindings on the core
* itself, but I didn't want to mix concerns and keep the core as clean as possible
*/
use std::time::Duration;

use gameboy_core::{LCD_HEIGHT, LCD_WIDTH};
use ui_core::Player;
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
    player: Player,
}

#[wasm_bindgen]
impl Emulator {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: Vec<u8>) -> Emulator {
        Emulator {
            player: Player::new(rom),
        }
    }

    /// Runs however many frames are due after `elapsed_ms` of real time. Returns true if any ran,
    /// so JS knows to draw.
    pub fn advance(&mut self, elapsed_ms: f64) -> bool {
        // requestAnimationFrame's timestamp can be slightly earlier than a performance.now()
        // taken just before it, and Duration panics on negative values
        let elapsed = Duration::from_secs_f64(elapsed_ms.max(0.0) / 1000.0);
        self.player.advance(elapsed)
    }

    /// Browsers keep audio suspended until the user interacts with the page, so JS calls this
    /// from key and click handlers.
    pub fn resume_audio(&self) {
        self.player.play_audio();
    }

    pub fn pause_audio(&self) {
        self.player.pause_audio();
    }

    /// Returns the RGBA bytes for the frame. Since canvas takes a separate byte array for R, G, B, and A,
    /// we convert the gameboy's color to those
    pub fn frame(&mut self) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(LCD_WIDTH * LCD_HEIGHT * 4);
        for color in self.player.gameboy.get_frame() {
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
        self.player.gameboy.get_cartridge_ram().to_vec()
    }

    /// Restore cartridge RAM from a previous `cartridge_ram()`. Throws if the size doesn't match.
    pub fn load_cartridge_ram(&mut self, data: &[u8]) -> Result<(), JsError> {
        self.player
            .gameboy
            .load_cartridge_ram(data)
            .map_err(|err| JsError::new(&err))
    }

    pub fn press_button(&mut self, button: Button) {
        self.player.gameboy.press_button(button.into());
    }

    pub fn release_button(&mut self, button: Button) {
        self.player.gameboy.release_button(button.into());
    }
}
