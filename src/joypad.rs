#[derive(Debug, Clone)]
pub struct Joypad {
  pressed: u8,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Button {
  A,
  B,
  Select,
  Start,
  Right,
  Left,
  Up,
  Down,
}

impl Joypad {
  pub const fn new() -> Self {
    // Mark all buttons as unpressed
    Self { pressed: 0xFF }
  }

  /// Reads the value of the joypad.
  pub fn read(&self, joypad_byte: u8) -> u8 {
    let group = joypad_byte & 0x30;

    match group {
      // Read the select buttons
      0x20 => (self.pressed & 0x0F) | group,
      // Read the d-pad
      0x10 => ((self.pressed >> 4) & 0x0F) | group,
      // No buttons selected
      _ => 0x0F,
    }
  }

  /// Updates the button's state.
  pub fn update_state(&mut self, button: Button, pressed: bool) {
    let mask = button.bit_mask();

    // A set button is set to 0 on the Gameboy
    if pressed {
      self.pressed &= !mask;
    } else {
      self.pressed |= mask;
    }
  }
}

impl Button {
  /// Returns a bitmask of the button.
  pub const fn bit_mask(self) -> u8 {
    let bit_pos = match self {
      Button::A | Button::Right => 0,
      Button::B | Button::Left => 1,
      Button::Select | Button::Up => 2,
      Button::Start | Button::Down => 3,
    };

    1 << bit_pos
  }
}
