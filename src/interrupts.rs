use crate::flags::{add_flag, is_flag_set, remove_flag};

/// A kind of interrupt.
#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Interrupt {
  VBlank = 1 << 0,
  Lcd = 1 << 1,
  Timer = 1 << 2,
  Serial = 1 << 3,
  Joypad = 1 << 4,
}

/// Stores the currently enabled interrupts and currently set interrupts.
#[derive(Debug, Clone)]
pub struct Interrupts {
  /// The currently requested interrupts.
  requested: u8,
  /// The currently enabled interrupts.
  enabled: u8,
}

impl Interrupts {
  /// Creates a new [`Interrupts`], with no enabled or requested interrupts.
  pub const fn new() -> Self {
    Self {
      requested: 0,
      enabled: 0,
    }
  }

  /// Checks if the [`Interrupt`] is enabled.
  pub fn is_enabled(&self, interrupt: Interrupt) -> bool {
    is_flag_set!(self.enabled_bitfield(), interrupt as u8)
  }

  /// Sets the internal enabled interrupts to the following value.
  pub fn set_enabled(&mut self, value: u8) {
    // All 8 bits of IE are read/write
    self.enabled = value
  }

  /// Returns a bitfield of the enabled interrupts.
  pub fn enabled_bitfield(&self) -> u8 {
    self.enabled
  }

  /// Checks if the following [`Interrupt`] was requested.
  pub fn is_requested(&self, interrupt: Interrupt) -> bool {
    is_flag_set!(self.requested_bitfield(), interrupt as u8)
  }

  /// Requests the following [`Interrupt`].
  pub fn request_interrupt(&mut self, interrupt: Interrupt) {
    add_flag!(&mut self.requested, interrupt as u8);
  }

  /// Sets the requested interrupts to the following value.
  pub fn set_requested(&mut self, value: u8) {
    // Only the lower 5 bits of IF are read/write
    self.requested = value & 0b0001_1111;
  }

  /// Clears a requested [`Interrupt`].
  pub fn clear_interrupt(&mut self, interrupt: Interrupt) {
    remove_flag!(&mut self.requested, interrupt as u8);
  }

  /// Returns a bitfield of the requested interrupts.
  pub fn requested_bitfield(&self) -> u8 {
    // The upper 3 bits of IF return are set when reading
    self.requested | 0b1110_0000
  }
}
