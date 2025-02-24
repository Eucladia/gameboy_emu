use crate::interrupts::{Interrupt, Interrupts};

#[derive(Debug, Clone)]
pub struct Joypad {
  /// The buttons that are pressed.
  pressed: u8,
  /// The group of buttons that are pressed.
  button_group: u8,
}

/// The Gameboy's buttons.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Button {
  A = 1 << 0,
  B = 1 << 1,
  Select = 1 << 2,
  Start = 1 << 3,
  Right = 1 << 4,
  Left = 1 << 5,
  Up = 1 << 6,
  Down = 1 << 7,
}

impl Joypad {
  pub const fn new() -> Self {
    Self {
      // Mark all buttons as released, because a value of 0 means its pressed
      pressed: 0xFF,
      // Mark the groups as unselected
      button_group: 0xF0,
    }
  }

  /// Reads the value of the [`Joypad`].
  pub fn read(&self) -> u8 {
    let lower_nibble = match (self.button_group >> 4) & 0b11 {
      // The action group was selected, if the 5th bit was 0
      0b01 | 0b00 => self.pressed & 0x0F,
      // The d-pad group was selected, if the 4th bit was 0
      0b10 => (self.pressed & 0xF0) >> 4,
      // No buttons selected
      _ => 0x0F,
    };

    // The upper 2 bits are always set
    0xC0 | self.button_group | lower_nibble
  }

  /// Updates the [`Joypad`] button group.
  pub fn write(&mut self, value: u8) {
    // Only bits 4 and 5 are writeable
    self.button_group = value & 0x30;
  }

  /// Updates the button's state.
  pub fn update_state(&mut self, interrupts: &mut Interrupts, button: Button, pressed: bool) {
    let before = self.pressed;
    let mask = button as u8;

    // A button is pressed if its bit is set to 0
    if pressed {
      self.pressed &= !mask;
    } else {
      self.pressed |= mask;
    }

    if self.pressed != before {
      interrupts.request_interrupt(Interrupt::Joypad);
    }
  }
}
