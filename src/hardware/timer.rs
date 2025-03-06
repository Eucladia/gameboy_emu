use crate::interrupts::{Interrupt, Interrupts};

#[derive(Debug, Clone)]
pub struct Timer {
  /// The internal counter used for tracking cycles.
  counter: u16,
  /// The divider register.
  div: u8,
  /// The timer counter register.
  tima: u8,
  /// The timer modulo amount.
  tma: u8,
  /// The timer control.
  tac: u8,
}

impl Timer {
  /// Creates a new [`Timer`].
  pub const fn new() -> Self {
    Self {
      counter: 0,
      div: 0,
      tima: 0,
      tma: 0,
      tac: 0,
    }
  }

  /// Steps the timer.
  pub fn step(&mut self, interrupts: &mut Interrupts, cycles: u16) {
    self.counter = self.counter.wrapping_add(cycles);

    if self.counter > 0xFF {
      self.div = self.div.wrapping_add(1);
    }

    // Check if the timer is enabled
    if self.tac & 0b100 == 1 {
      // Parse the frequency, in T-cycles, of the clock out of the 2 lower bits
      let threshold = match self.tac & 0x3 {
        0b00 => 1024,
        0b01 => 16,
        0b10 => 64,
        0b11 => 256,
        _ => unreachable!(),
      };

      if self.counter % threshold == 0 {
        if self.tima == 0xFF {
          self.tima = self.tma;

          interrupts.request_interrupt(Interrupt::Timer);
        } else {
          self.tima = self.tima.wrapping_add(1);
        }
      }
    }
  }

  /// Reads from the timer's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address {
      0xFF04 => self.div,
      0xFF05 => self.tima,
      0xFF06 => self.tma,
      0xFF07 => self.tac,
      _ => unreachable!(),
    }
  }

  /// Writes to timer's registers.
  pub fn write_register(&mut self, address: u16, value: u8) {
    match address {
      // Writing to DIV resets it
      0xFF04 => self.div = 0,
      0xFF05 => self.tima = value,
      0xFF06 => self.tma = value,
      // Only the first 3 bits are used
      0xFF07 => self.tac = value & 0x7,
      _ => unreachable!(),
    }
  }
}
