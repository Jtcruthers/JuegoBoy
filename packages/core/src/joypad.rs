use log::debug;

#[derive(Debug)]
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

impl Button {
    fn bit_position(&self) -> u8 {
        match self {
            Button::Start => 7,
            Button::Select => 6,
            Button::B => 5,
            Button::A => 4,
            Button::Down => 3,
            Button::Up => 2,
            Button::Left => 1,
            Button::Right => 0,
        }
    }
}

const P1_ADDR: u16 = 0xFF00;
const BUTTON_MASK: u8 = 0b1101_1111;
const D_PAD_MASK: u8 = 0b1110_1111;

/// Store the state of which buttons are pressed in a separate u8 from p1
/// If pressed, set it to 0, as p1 does. Otherwise, set 1.
/// I'm using the top nibble for the buttons, and the bottom nibble for the d_pad
///
///    [start, select, b, a, down, up, left, right]
///
/// I'm using this ordering so the both nibbles mimics p1's 'bottom nibble which lets me re-use the
/// mask, after moving it up 4 bits.
///
/// When the byte is read, we check which set of buttons the gameboy cares about via p1's top nibble
/// and dynamically return the correct set of pressed buttons/d-pad controls
pub struct Joypad {
    buttons_pressed: u8,
    p1: u8,
    pub interrupt: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            buttons_pressed: 0,
            p1: 0xFF,
            interrupt: false,
        }
    }

    /// is_buttons returns true if the 5th bit is 0 in p1
    /// This means the buttons (SsBA) can be read from the lower nibble
    fn is_buttons(&self) -> bool {
        self.p1 | BUTTON_MASK == BUTTON_MASK
    }

    /// is_d_pad returns true if the 4th bit is 0 in p1
    /// This means the directional keys can be read from the lower nibble
    fn is_d_pad(&self) -> bool {
        self.p1 | D_PAD_MASK == D_PAD_MASK
    }

    pub fn read_byte(&self, address: u16) -> u8 {
        if address != P1_ADDR {
            panic!("Invalid joypad read address: {:X}", address);
        }

        if self.is_buttons() {
            let bottom_nibble = self.buttons_pressed >> 4;
            let top_nibble = (self.p1 & 0xF0) | 0b1100_0000;
            return top_nibble | bottom_nibble;
        }

        if self.is_d_pad() {
            let bottom_nibble = self.buttons_pressed & 0x0F;
            let top_nibble = (self.p1 & 0xF0) | 0b1100_0000;
            return top_nibble | bottom_nibble;
        }

        // If neither buttons or d_pad selected, then the low nibble returns 0xF
        // Since the top 2 bits are always F, and the 5th and 4th bits are 1, it's just 0xFF
        0xFF
    }

    /// Allows writing to P1, but only the 5th and 4th bits are taken
    pub fn write_byte(&mut self, address: u16, value: u8) {
        if address != P1_ADDR {
            panic!("Invalid joypad write address: {:X}", address);
        }

        // Only let them write to bits 5 and 4, keep 7 and 6 as 1
        // Doesn't really matter since we return 1 for 7 and 6 on read
        // But feels better
        let top_nibble = 0b1100_0000 | (0b0011_0000 & value);
        let bottom_nibble = self.p1 & 0x0F;

        self.p1 = top_nibble | bottom_nibble;
    }

    pub fn tick(&mut self) {}

    /// Handle a player pressing a single button
    pub fn press_button(&mut self, button: Button) {
        let bit_to_clear = 1 << button.bit_position();

        // If button is moving from 1 to 0, interrupt
        if self.buttons_pressed & bit_to_clear != 0 {
            self.interrupt = true;
        }
        self.buttons_pressed &= !bit_to_clear;
        debug!(
            "Pressed button {:?}, buttons_pressed: {:08b}",
            button, self.buttons_pressed
        );
    }

    /// Handle player releasing a single button
    pub fn release_button(&mut self, button: Button) {
        let bit_to_set = 1 << button.bit_position();

        self.buttons_pressed |= bit_to_set;
        debug!(
            "Released button {:?}, buttons_pressed: {:08b}",
            button, self.buttons_pressed
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_buttons_is_true_when_5th_bit_not_set() {
        let mut joypad = Joypad::new();

        joypad.p1 = 0b1101_1110;
        assert!(joypad.is_buttons());

        joypad.p1 = 0b0001_0000;
        assert!(joypad.is_buttons());

        joypad.p1 = 0b0001_1111;
        assert!(joypad.is_buttons());

        joypad.p1 = 0b1101_1111;
        assert!(joypad.is_buttons());
    }

    #[test]
    fn test_is_buttons_is_true_when_all_bits_not_set() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b0000_0000;
        assert!(joypad.is_buttons());
    }

    #[test]
    fn test_is_buttons_is_false_when_5th_bit_is_set() {
        let mut joypad = Joypad::new();

        joypad.p1 = 0b0010_0000;
        assert_eq!(joypad.is_buttons(), false);

        joypad.p1 = 0b1111_1111;
        assert_eq!(joypad.is_buttons(), false);
    }

    #[test]
    fn test_is_d_pad_is_true_when_all_bits_not_set() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b0000_0000;
        assert!(joypad.is_d_pad());
    }

    #[test]
    fn test_is_d_pad_is_true_when_4th_bit_is_not_set() {
        let mut joypad = Joypad::new();

        joypad.p1 = 0b1110_1110;
        assert!(joypad.is_d_pad());

        joypad.p1 = 0b0010_0000;
        assert!(joypad.is_d_pad());

        joypad.p1 = 0b0010_1111;
        assert!(joypad.is_d_pad());

        joypad.p1 = 0b1110_1111;
        assert!(joypad.is_d_pad());
    }

    #[test]
    fn test_is_d_pad_is_false_when_4th_bit_is_set() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b0001_0000;
        assert_eq!(joypad.is_d_pad(), false);

        joypad.p1 = 0b1111_1111;
        assert_eq!(joypad.is_d_pad(), false);
    }

    #[test]
    fn test_read_memory_no_selection_is_0xff() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b0011_0000;
        assert_eq!(joypad.read_byte(P1_ADDR), 0xFF);

        joypad.p1 = 0b1111_1111;
        assert_eq!(joypad.read_byte(P1_ADDR), 0xFF);
    }

    #[test]
    fn test_read_memory_buttons_selected_no_buttons_pressed() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b0001_0000;
        joypad.buttons_pressed = 0xFF;
        assert_eq!(joypad.read_byte(P1_ADDR), 0b1101_1111);
    }

    #[test]
    fn test_read_memory_buttons_selected_start_pressed() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b0001_0000;
        joypad.buttons_pressed = 0b0111_1111;
        assert_eq!(joypad.read_byte(P1_ADDR), 0b1101_0111);
    }

    #[test]
    fn test_read_memory_buttons_selected_b_pressed() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b1101_1111;
        joypad.buttons_pressed = 0b1101_1111;
        assert_eq!(joypad.read_byte(P1_ADDR), 0b1101_1101);
    }

    #[test]
    fn test_read_memory_d_pad_selected_no_buttons_pressed() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b1110_1111;
        joypad.buttons_pressed = 0b1111_1111;
        assert_eq!(joypad.read_byte(P1_ADDR), 0b1110_1111);
    }

    #[test]
    fn test_read_memory_d_pad_selected_right_pressed() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b1110_1111;
        joypad.buttons_pressed = 0b1111_1110;
        assert_eq!(joypad.read_byte(P1_ADDR), 0b1110_1110);
    }

    #[test]
    fn test_read_memory_d_pad_selected_up_pressed() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b1110_1111;
        joypad.buttons_pressed = 0b1111_1110;
        assert!(joypad.is_d_pad());
    }

    #[test]
    fn test_write_memory_select_all_only_clears_select_buttons() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0b1111_1111;
        joypad.write_byte(P1_ADDR, 0);
        assert_eq!(joypad.p1, 0b1100_1111);
    }

    #[test]
    fn test_write_memory_select_all_only_sets_select_buttons() {
        let mut joypad = Joypad::new();
        joypad.p1 = 0;
        joypad.write_byte(P1_ADDR, 0xFF);
        assert_eq!(joypad.p1, 0b1111_0000);
    }

    #[test]
    fn test_press_button_a() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0xFF;
        joypad.press_button(Button::A);
        assert_eq!(joypad.buttons_pressed, 0b1110_1111);
    }

    #[test]
    fn test_press_button_start() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0xFF;
        joypad.press_button(Button::Start);
        assert_eq!(joypad.buttons_pressed, 0b0111_1111);
    }

    #[test]
    fn test_press_button_down() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0xFF;
        joypad.press_button(Button::Down);
        assert_eq!(joypad.buttons_pressed, 0b1111_0111);
    }

    #[test]
    fn test_press_button_doesnt_affect_other_buttons() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0b0000_0011;
        joypad.press_button(Button::Right);
        assert_eq!(joypad.buttons_pressed, 0b0000_0010);
    }

    #[test]
    fn test_release_button_b() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0x0;
        joypad.release_button(Button::B);
        assert_eq!(joypad.buttons_pressed, 0b0010_0000);
    }

    #[test]
    fn test_release_button_select() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0b1010_1010;
        joypad.release_button(Button::Select);
        assert_eq!(joypad.buttons_pressed, 0b1110_1010);
    }

    #[test]
    fn test_release_button_up() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0x0;
        joypad.release_button(Button::Up);
        assert_eq!(joypad.buttons_pressed, 0b0000_0100);
    }

    #[test]
    fn test_interrupt_works_on_set() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0xFF;
        joypad.press_button(Button::Start);
        assert_eq!(joypad.interrupt, true);
    }

    #[test]
    fn test_interrupt_doesnt_interrupt_already_pressed() {
        let mut joypad = Joypad::new();

        joypad.buttons_pressed = 0x0;
        joypad.press_button(Button::Start);
        assert_eq!(joypad.interrupt, false);
    }
}
