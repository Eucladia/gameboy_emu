use crate::{
  flags::is_flag_set,
  interrupts::{Interrupt, Interrupts},
};

#[derive(Debug, Clone)]
pub struct Timer {
  /// The timer counter register.
  tima: u8,
  /// The timer modulo amount.
  tma: u8,
  /// The timer control.
  tac: u8,
  /// The internal 16-bit counter used for DIV (upper 8 bits) and for timing.
  counter: u16,

  /// The number of ticks until an interrupt will be fired, if any.
  ticks_til_interrupt: u8,
  /// The previous AND result.
  prev_and_result: bool,
}

impl Timer {
  /// Creates a new [`Timer`].
  pub const fn new() -> Self {
    Self {
      tima: 0,
      tma: 0,
      tac: 0,
      counter: 0xABCC,

      ticks_til_interrupt: 0,
      prev_and_result: false,
    }
  }

  /// Steps the timer by a T-cycle.
  pub fn step(&mut self, interrupts: &mut Interrupts) {
    self.counter = self.counter.wrapping_add(1);

    // The interrupt gets delayed by 4 T-cycles after TIMA overflows.
    if self.ticks_til_interrupt > 0 {
      self.ticks_til_interrupt -= 1;

      if self.ticks_til_interrupt == 0 {
        self.tima = self.tma;
        interrupts.request_interrupt(Interrupt::Timer);
      }
    } else {
      // Compare the extracted bit of the updated counter with the timer enable bit
      let curr_and_result = is_flag_set!(self.tac, TIMER_ENABLE_MASK)
        & is_flag_set!(self.counter, tac_bit_mask(self.tac));

      if self.prev_and_result && !curr_and_result {
        self.tima = self.tima.wrapping_add(1);
        self.ticks_til_interrupt = if self.tima == 0 { 4 } else { 0 };
      }

      self.prev_and_result = curr_and_result;
    }
  }

  /// Reads from the timer's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address {
      // DIV is stored in the upper 8 bits of the counter
      0xFF04 => ((self.counter & 0xFF00) >> 8) as u8,
      0xFF05 => self.tima,
      0xFF06 => self.tma,
      0xFF07 => self.tac,
      _ => unreachable!(),
    }
  }

  /// Writes to the timer's registers.
  pub fn write_register(&mut self, address: u16, value: u8) {
    match address {
      // Writing to DIV resets the internal counter
      0xFF04 => {
        self.counter = 0;
      }
      0xFF05 => {
        // If TIMA is written to during the 4 T-cycles after a TIMA overflow,
        // then the TMA reload and interrupt request are aborted.
        if self.ticks_til_interrupt > 0 {
          self.tima = value;
          self.ticks_til_interrupt = 0;
        } else {
          self.tima = value;
        }
      }
      // Reloads use the new TMA value, even if it was written to on the same cycle
      0xFF06 => self.tma = value,
      0xFF07 => {
        // We should update the previous AND result when changing TAC because
        // we update the internal clock counter inside `step`, then check against
        // that updated counter.
        self.prev_and_result = is_flag_set!(self.tac, TIMER_ENABLE_MASK)
          & is_flag_set!(self.counter, tac_bit_mask(self.tac));

        self.tac = value & 0x07;
      }
      _ => unreachable!(),
    }
  }
}

/// Gets the clock select bit mask from the TAC register.
const fn tac_bit_mask(tac: u8) -> u16 {
  match tac & 0x3 {
    0b00 => 1 << 9,
    0b01 => 1 << 3,
    0b10 => 1 << 5,
    0b11 => 1 << 7,
    _ => unreachable!(),
  }
}

/// The bit mask for the TAC register for checking if the timer is enabled.
const TIMER_ENABLE_MASK: u8 = 0x04;
