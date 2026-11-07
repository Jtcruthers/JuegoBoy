use winit::event::KeyEvent;
use winit::keyboard::KeyCode;

use gameboy_core::{Button, Gameboy};

pub fn handle_user_input(gameboy: &mut Gameboy, event: KeyEvent) {
    if event.repeat {
        return;
    }

    let physical_key = event.physical_key;
    let event_button: Option<Button> = match physical_key {
        winit::keyboard::PhysicalKey::Code(KeyCode::KeyW) => Some(Button::Up),
        winit::keyboard::PhysicalKey::Code(KeyCode::KeyA) => Some(Button::Left),
        winit::keyboard::PhysicalKey::Code(KeyCode::KeyS) => Some(Button::Down),
        winit::keyboard::PhysicalKey::Code(KeyCode::KeyD) => Some(Button::Right),
        winit::keyboard::PhysicalKey::Code(KeyCode::ArrowUp) => Some(Button::A),
        winit::keyboard::PhysicalKey::Code(KeyCode::ArrowLeft) => Some(Button::B),
        winit::keyboard::PhysicalKey::Code(KeyCode::ArrowRight) => Some(Button::Start),
        winit::keyboard::PhysicalKey::Code(KeyCode::ArrowDown) => Some(Button::Select),
        _ => None,
    };

    let state = event.state;
    if let Some(button) = event_button {
        match state {
            winit::event::ElementState::Pressed => gameboy.press_button(button),
            winit::event::ElementState::Released => gameboy.release_button(button),
        };
    }
}
