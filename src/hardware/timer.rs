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
  pub const fn new() -> Self {
    Self {
      counter: 0,
      div: 0,
      tima: 0,
      tma: 0,
      tac: 0,
    }
  }

  /// Steps the [`Timer`].
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

  /// Reads from the Timer's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address {
      DIVIDER_REGISTER => self.div,
      TIMER_COUNTER_REGISTER => self.tima,
      TIMER_MODULO_REGISTER => self.tma,
      TIMER_CONTROL_REGISTER => self.tac,
      _ => unreachable!(),
    }
  }

  /// Writes to Timer's registers.
  pub fn write_register(&mut self, address: u16, value: u8) {
    match address {
      // Writing to DIV resets it
      DIVIDER_REGISTER => self.div = 0,
      TIMER_COUNTER_REGISTER => self.tima = value,
      TIMER_MODULO_REGISTER => self.tma = value,
      // Only the first 3 bits are used
      TIMER_CONTROL_REGISTER => self.tac = value & 0x7,
      _ => unreachable!(),
    }
  }
}

/// The address of the divider register.
const DIVIDER_REGISTER: u16 = 0xFF04;
/// The address of the timer counter register.
const TIMER_COUNTER_REGISTER: u16 = 0xFF05;
/// The address of the timer modulo register.
const TIMER_MODULO_REGISTER: u16 = 0xFF06;
/// The address of the timer control register.
const TIMER_CONTROL_REGISTER: u16 = 0xFF07;
