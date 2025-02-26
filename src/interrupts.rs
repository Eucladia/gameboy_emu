/// A kind of interrupt.
#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Interrupt {
  VBlank = 1 << 0,
  LCD = 1 << 1,
  Timer = 1 << 2,
  Serial = 1 << 3,
  Joypad = 1 << 4,
}

/// Stores the currently enabled interrupts and currently set interrupts.
#[derive(Debug, Clone)]
pub struct Interrupts {
  requested: u8,
  enabled: u8,
}

impl Interrupts {
  pub const fn new() -> Self {
    Self {
      requested: 0,
      enabled: 0,
    }
  }

  /// Enables the [`Interrupt`].
  pub fn enable_interrupt(&mut self, interrupt: Interrupt) {
    self.enabled |= interrupt as u8;
  }

  /// Disables the [`Interrupt`].
  pub fn disable_interrupt(&mut self, interrupt: Interrupt) {
    self.enabled &= !(interrupt as u8);
  }

  /// Checks if the [`Interrupt`] is enabled.
  pub fn is_enabled(&mut self, interrupt: Interrupt) -> bool {
    (self.enabled & interrupt as u8) == interrupt as u8
  }

  /// Sets the internal enabled interrupts to the following value.
  pub fn set_enabled(&mut self, value: u8) {
    self.enabled = value & 0b1_1111
  }

  /// Returns a bitfield of the enabled interrupts.
  pub fn enabled_bitfield(&self) -> u8 {
    // Only the first 5 bits have flags
    self.enabled & 0b1_1111
  }

  /// Checks if the following [`Interrupt`] was requested.
  pub fn was_requested(&mut self, interrupt: Interrupt) -> bool {
    (self.requested & interrupt as u8) == interrupt as u8
  }

  /// Requests the following [`Interrupt`].
  pub fn request_interrupt(&mut self, interrupt: Interrupt) {
    self.requested |= interrupt as u8;
  }

  /// Updates the requested interrupts to the following value.
  pub fn set_requested(&mut self, value: u8) {
    self.requested |= value & 0b1_1111;
  }

  /// Clears a requested [`Interrupt`].
  pub fn clear_interrupt(&mut self, interrupt: Interrupt) {
    self.requested &= !(interrupt as u8);
  }

  /// Returns a bitfield of the requested interrupts.
  pub fn requested_bitfield(&self) -> u8 {
    self.requested & 0b1_1111
  }
}
