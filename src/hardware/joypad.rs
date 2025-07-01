use crate::{
  flags::{add_flag, remove_flag},
  interrupts::{Interrupt, Interrupts},
};

/// The input controller used to interact with the game.
#[derive(Debug, Clone)]
pub struct Joypad {
  /// The buttons that are pressed.
  pressed: u8,
  /// The group of buttons that are pressed.
  button_group: u8,
}

/// The set of buttons on the joypad.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Button {
  /// The `A` button.
  A = 1 << 0,
  /// The `B` button.
  B = 1 << 1,
  /// The `Select` button.
  Select = 1 << 2,
  /// The `Start` button.
  Start = 1 << 3,
  /// The `Right` button.
  Right = 1 << 4,
  /// The `Left` button.
  Left = 1 << 5,
  /// The `Up` button.
  Up = 1 << 6,
  /// The `Down` button.
  Down = 1 << 7,
}

impl Joypad {
  /// Creates a new [`Joypad`] in an unreleased state.
  pub const fn new() -> Self {
    Self {
      // Mark all buttons as released
      pressed: 0xFF,
      // Mark the groups as unselected
      button_group: 0x30,
    }
  }

  /// Reads the value of the [`Joypad`].
  pub fn read_register(&self) -> u8 {
    let lower_nibble = self.register_value();

    // The upper 2 bits are always set
    0b1100_0000 | self.button_group | lower_nibble
  }

  /// Updates the [`Joypad`] button group.
  pub fn write_register(&mut self, value: u8) {
    // Only bits 4 and 5 are writeable
    self.button_group = value & 0b0011_0000;
  }

  /// Updates the button's state.
  pub fn update_button_state(
    &mut self,
    interrupts: &mut Interrupts,
    button: Button,
    button_state: ButtonAction,
  ) {
    let before_lower_nibble = self.register_value();

    match button_state {
      // A button is pressed if its bit is set to 0
      ButtonAction::Pressed => remove_flag!(&mut self.pressed, button as u8),
      ButtonAction::Released => add_flag!(&mut self.pressed, button as u8),
    }

    let after_lower_nibble = self.register_value();

    // Interrupts are ONLY fired if there is a falling edge in the lower nibble
    if before_lower_nibble & !after_lower_nibble != 0 {
      interrupts.request_interrupt(Interrupt::Joypad);
    }
  }

  /// Returns the lower nibble of the selected group of buttons.
  const fn register_value(&self) -> u8 {
    match (self.button_group >> 4) & 0x3 {
      // The action group was selected, if the 5th bit was 0
      0b01 => self.pressed & 0x0F,
      // The d-pad group was selected, if the 4th bit was 0
      0b10 => (self.pressed & 0xF0) >> 4,
      // If the 4th and 5th bits are 0, then both groups are combined
      0b00 => (self.pressed & 0x0F) & ((self.pressed & 0xF0) >> 4),
      // No button group was selected
      0b11 => 0x0F,
      _ => unreachable!(),
    }
  }
}

/// A button action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
  /// The button was pressed.
  Pressed,
  /// The button was released.
  Released,
}
