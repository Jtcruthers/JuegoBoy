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

const BUTTON_MASK: u8 = 0b1101_1111;
const D_PAD_MASK: u8 = 0b1110_1111;
const START_DOWN_MASK: u8 = 0b1111_0111;
const SELECT_UP_MASK: u8 = 0b1111_1011;
const A_RIGHT_MASK: u8 = 0b1111_1110;
const B_LEFT_MASK: u8 = 0b1111_1101;

pub struct Joypad {
    p1: u8,
    pub interrupt: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            p1: 0xFF,
            interrupt: false,
        }
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        match address {
            0xFF00 => self.p1 | 0b1100_0000, // Have unused bits always return 1
            _ => panic!("Invalid joypad read address: {:X}", address),
        }
    }

    pub fn write_byte(&mut self, address: u16, value: u8) {
        match address {
            0xFF00 => self.p1 |= 0b0011_0000 & value,
            _ => panic!("Invalid joypad write address: {:X}", address),
        }
    }

    pub fn tick(&mut self) {}

    // TODO - figure out if we need to change top nibble, or if the writes take care of that
    // If we don't set top nibble, then how can we tell if it was Select or Up?
    pub fn press_button(&mut self, button: Button) {
        self.interrupt = true;

        self.p1 = match button {
            Button::Start => BUTTON_MASK & START_DOWN_MASK,
            Button::Select => BUTTON_MASK & SELECT_UP_MASK,
            Button::A => BUTTON_MASK & A_RIGHT_MASK,
            Button::B => BUTTON_MASK & B_LEFT_MASK,
            Button::Up => D_PAD_MASK & SELECT_UP_MASK,
            Button::Down => D_PAD_MASK & START_DOWN_MASK,
            Button::Right => D_PAD_MASK & A_RIGHT_MASK,
            Button::Left => D_PAD_MASK & B_LEFT_MASK,
        }
    }

    // TODO - figure out if we need to change the top nibble
    // If you release it, i'm just setting that bit back to 1,
    // which is considered off
    //
    // Also unsure if I need to take away the interrupt? I dont think so
    pub fn release_button(&mut self, button: Button) {
        self.p1 |= match button {
            Button::Start => !START_DOWN_MASK,
            Button::Select => !SELECT_UP_MASK,
            Button::A => !A_RIGHT_MASK,
            Button::B => !B_LEFT_MASK,
            Button::Up => !SELECT_UP_MASK,
            Button::Down => !START_DOWN_MASK,
            Button::Right => !A_RIGHT_MASK,
            Button::Left => !B_LEFT_MASK,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_press_button() {
        let mut joypad = Joypad::new();

        joypad.press_button(Button::A);
        assert_eq!(joypad.p1, 0b1101_1110);
    }
}
