// EDITED
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

  /// Whether the previous step produced an overflow.
  overflowed: bool,
  /// The previous AND result.
  ///
  /// The AND result is described as the extracted bit from doing the following:
  /// `counter_bit & timer_enable_bit`.
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

      overflowed: false,
      prev_and_result: false,
    }
  }

  /// Steps the timer forward by the given number of cycles.
  pub fn step(&mut self, interrupts: &mut Interrupts, cycles: usize) {
    for _ in 0..cycles {
      self.counter = self.counter.wrapping_add(1);

      // Technically the reload and timer request should be delayed by 4 T-cycles,
      // but we only call this fn at most every 4 T-cycles, so this should be good.
      if self.overflowed {
        self.overflowed = false;
        self.tima = self.tma;

        interrupts.request_interrupt(Interrupt::Timer);
      } else {
        // Compare the extracted bit of the updated counter with the timer enable bit
        let curr_and_result = is_flag_set!(self.tac, TIMER_ENABLE_MASK)
          & is_flag_set!(self.counter, tac_bit_mask(self.tac));

        if self.prev_and_result && !curr_and_result {
          self.tima = self.tima.wrapping_add(1);
          self.overflowed = self.tima == 0;
        }

        self.prev_and_result = curr_and_result;
      }
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
        if self.overflowed {
          self.tima = value;
          self.overflowed = false;
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

#[cfg(test)]
mod test {
  use crate::{
    hardware::{Interrupts, Timer},
    interrupts::Interrupt,
  };

  // Tests https://github.com/Hacktix/GBEDG/blob/master/timers/index.md#an-edge-case.
  #[test]
  fn disabled_tima_inc() {
    let mut timer = Timer::new();
    let mut interrupts = Interrupts::new();

    timer.tac = 0b101;
    timer.counter = 0b11000;

    timer.step(&mut interrupts, 4);

    assert_eq!(timer.tima, 0);

    timer.write_register(0xFF07, 0b001);
    timer.step(&mut interrupts, 4);

    // It should've incremented TIMA because there was a falling edge
    assert_eq!(timer.tima, 1);
  }

  // Tests https://gbdev.io/pandocs/Timer_Obscure_Behaviour.html.
  #[test]
  fn select_bit_changed_inc() {
    let mut timer = Timer::new();
    let mut interrupts = Interrupts::new();

    timer.counter = 0x3FF0;
    timer.tac = 0xFC;

    timer.write_register(0xFF07, 0x05);
    // This step should increment TIMA because there was a falling edge
    // despite having a new clock select mask
    timer.step(&mut interrupts, 4);

    assert_eq!(timer.tima, 1);
  }

  // Tests https://gbdev.io/pandocs/Timer_Obscure_Behaviour.html.
  #[test]
  fn timer_select_bit_changed() {
    let mut timer = Timer::new();
    let mut interrupts = Interrupts::new();

    // TIMA should not be incremented in these cases
    timer.counter = 0x3FF0;
    timer.tac = 0xFC;

    timer.write_register(0xFF07, 0x04);
    timer.step(&mut interrupts, 4);

    assert_eq!(timer.tima, 0);

    timer.prev_and_result = false;
    timer.counter = 0x3FF0;
    timer.tac = 0xFC;

    timer.write_register(0xFF07, 0x07);
    timer.step(&mut interrupts, 4);

    assert_eq!(timer.tima, 0);
  }

  // Test to make sure that a reload and timer Interrupt get aborted, if written to
  // during the 4 T-cycles after the TIMA overflow.
  #[test]
  fn reload_interrupt_aborted() {
    let mut timer = Timer::new();
    let mut interrupts = Interrupts::new();

    timer.counter = 0x3FF0;
    timer.tac = 0xFC;
    // Mock an overflow
    timer.tima = 0xFF;

    timer.write_register(0xFF07, 0x05);
    // This should increment TIMA while keeping overflow as true, since we're
    // only stepping a cycle
    timer.step(&mut interrupts, 1);

    assert_eq!(timer.tima, 0);
    assert!(timer.overflowed);

    // Write to TIMA to cancel the interrupt request and reload
    timer.write_register(0xFF05, 0x0F);

    timer.step(&mut interrupts, 4);

    assert_eq!(timer.tima, 0x0F);
    assert!(!timer.overflowed);
    assert!(!interrupts.is_requested(Interrupt::Timer));
  }

  // Test to make sure that the reload uses the updated TMA value.
  #[test]
  fn reload_uses_updated_tma() {
    let mut timer = Timer::new();
    let mut interrupts = Interrupts::new();

    timer.counter = 0x3FF0;
    timer.tac = 0xFC;
    // Mock an overflow
    timer.tima = 0xFF;

    timer.write_register(0xFF07, 0x05);
    // This should increment TIMA, but leave the overflow since we're only steppinga a cycle
    timer.step(&mut interrupts, 1);

    assert_eq!(timer.tima, 0);
    assert!(timer.overflowed);

    // Write a new value to TMA
    timer.write_register(0xFF06, 0x1C);

    timer.step(&mut interrupts, 4);

    assert_eq!(timer.tima, 0x1C);
    assert!(interrupts.is_requested(Interrupt::Timer));
  }
}
