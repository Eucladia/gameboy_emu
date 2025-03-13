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
    }
  }

  /// Steps the timer forward by the given number of cycles.
  pub fn step(&mut self, interrupts: &mut Interrupts, cycles: usize) {
    const TIMER_ENABLE_MASK: u8 = 0b100;

    for _ in 0..cycles {
      // Handle the delayed overflow
      if self.overflowed {
        self.overflowed = false;

        self.tima = self.tma;
        interrupts.request_interrupt(Interrupt::Timer);
      }

      // The counter still gets updated even if the timer isn't enabled
      let old_counter = self.counter;
      self.counter = self.counter.wrapping_add(1);

      if !is_flag_set!(self.tac, TIMER_ENABLE_MASK) {
        continue;
      }

      let mask = match self.tac & 0b11 {
        // 256 M-cycles (4096 Hz)
        0b00 => 1 << 9,
        // 4 M-cycles (262144 Hz)
        0b01 => 1 << 3,
        // 16 M-cycles (65536 Hz)
        0b10 => 1 << 5,
        // 64 M-cycles (16384 Hz)
        0b11 => 1 << 7,
        _ => unreachable!(),
      };

      // Check for a falling edge (bit goes from 1 to 0)
      if is_flag_set!(old_counter, mask) && !is_flag_set!(self.counter, mask) {
        if self.tima == 0xFF {
          self.tima = 0;
          self.overflowed = true;
        } else {
          self.tima = self.tima.wrapping_add(1);
        }
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
      // Writing to DIV resets the entire internal counter
      0xFF04 => self.counter = 0,
      0xFF05 => self.tima = value,
      0xFF06 => self.tma = value,
      // Only lower 3 bits of TAC are used.
      0xFF07 => self.tac = value & 0x7,
      _ => unreachable!(),
    }
  }
}
