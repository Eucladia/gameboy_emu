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

/// Stores the enabled interrupts and pending interrupts.
#[derive(Debug, Clone)]
pub struct Interrupts {
  /// The `IF` register, which is the currently pending interrupts.
  requested: u8,
  /// The `IE` register, which is the currently enabled interrupts.
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

  /// Sets the internal enabled interrupts to the following value.
  pub fn set_enabled(&mut self, value: u8) {
    // All 8 bits of IE are read/write
    self.enabled = value
  }

  /// Returns a bitfield of the enabled interrupts.
  pub const fn enabled_bitfield(&self) -> u8 {
    self.enabled
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
  pub const fn requested_bitfield(&self) -> u8 {
    // The upper 3 bits of IF return are set when reading
    self.requested | 0b1110_0000
  }

  /// Returns a bitfield of the pending interrupts.
  pub const fn pending_bitfield(&self) -> u8 {
    self.enabled_bitfield() & self.requested_bitfield() & 0b0001_1111
  }

  /// Returns the next pending interrupt, from the bitfield, if any.
  pub const fn next_interrupt_from_bitfield(bitfield: u8) -> Option<Interrupt> {
    // Make sure that the upper 3 bits are cleared
    let pending = bitfield & 0b0001_1111;

    // Interrupts with lower bit values have higher priority
    if is_flag_set!(pending, Interrupt::VBlank as u8) {
      Some(Interrupt::VBlank)
    } else if is_flag_set!(pending, Interrupt::Lcd as u8) {
      Some(Interrupt::Lcd)
    } else if is_flag_set!(pending, Interrupt::Timer as u8) {
      Some(Interrupt::Timer)
    } else if is_flag_set!(pending, Interrupt::Serial as u8) {
      Some(Interrupt::Serial)
    } else if is_flag_set!(pending, Interrupt::Joypad as u8) {
      Some(Interrupt::Joypad)
    } else {
      // This case can only happen if the pending interrupts is 0
      None
    }
  }
}

impl Interrupt {
  /// Converts the [`Interrupt`] to its vector address.
  pub const fn to_vector(self) -> u16 {
    const BASE_INTERRUPT_ADDRESS: u16 = 0x0040;
    const INTERRUPT_OFFSET: u16 = 0x08;

    let leading_zeros = (self as u8).trailing_zeros() as u16;

    BASE_INTERRUPT_ADDRESS + leading_zeros * INTERRUPT_OFFSET
  }
}
