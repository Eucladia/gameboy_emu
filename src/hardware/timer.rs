use crate::{
  flags::is_flag_set,
  interrupts::{Interrupt, Interrupts},
};

#[derive(Debug)]
pub struct Timer {
  /// The timer counter register.
  tima: u8,
  /// The timer modulo amount.
  tma: u8,
  /// The timer control.
  tac: u8,
  /// The internal 16-bit counter used for DIV (upper 8 bits) and for timing.
  counter: u16,

  /// The timer interrupt delay.
  timer_interrupt_delay: Option<u8>,
}

impl Timer {
  /// Creates a new [`Timer`].
  pub fn new() -> Self {
    Self {
      tima: 0,
      tma: 0,
      tac: 0,
      counter: 0xABCC,

      timer_interrupt_delay: None,
    }
  }

  /// Steps the timer by a T-cycle.
  pub fn step(&mut self, interrupts: &mut Interrupts) {
    // The interrupt gets delayed by 4 T-cycles after TIMA overflows.
    match &self.timer_interrupt_delay {
      &Some(ticks) => 'arm: {
        if ticks == 0 {
          self.timer_interrupt_delay = None;
          break 'arm;
        }

        let new_ticks = ticks - 1;

        // Reload TIMA and request an interrupt
        if new_ticks == 0 {
          self.tima = self.tma;
          interrupts.request_interrupt(Interrupt::Timer);
        }

        self.timer_interrupt_delay = Some(new_ticks);
      }
      None => {}
    }

    let prev_and_result = self.counter_and_result();

    self.counter = self.counter.wrapping_add(1);

    let curr_and_result = self.counter_and_result();

    self.handle_counter_falling_edge(prev_and_result, curr_and_result);
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
        let prev_and_result = self.counter_and_result();

        self.counter = 0;

        let curr_and_result = self.counter_and_result();

        self.handle_counter_falling_edge(prev_and_result, curr_and_result);
      }
      0xFF05 => {
        // Writes to TIMA when it's being reloaded are ignored
        if !self.tima_reloading() {
          self.tima = value;
        }

        // Writes to TIMA when it overflowed cancels the interrupt
        if self.tima_overflowed() {
          self.timer_interrupt_delay = None;
        }
      }
      0xFF06 => {
        self.tma = value;

        // Writes to TMA when it's being reloaded also updates TIMA
        if self.tima_reloading() {
          self.tima = self.tma;
        }
      }

      0xFF07 => {
        let prev_and_result = self.counter_and_result();

        self.tac = value & 0x07;

        let curr_and_result = self.counter_and_result();

        self.handle_counter_falling_edge(prev_and_result, curr_and_result);
      }
      _ => unreachable!(),
    }
  }

  /// Handles a potential falling edge in the timer counter.
  fn handle_counter_falling_edge(&mut self, old_and_result: bool, new_and_result: bool) {
    if old_and_result && !new_and_result {
      self.tima = self.tima.wrapping_add(1);

      if self.tima == 0 {
        self.timer_interrupt_delay = Some(4);
      }
    }
  }

  /// Gets the current and result for the timer counter.
  const fn counter_and_result(&self) -> bool {
    is_flag_set!(self.tac, TIMER_ENABLE_MASK) && is_flag_set!(self.counter, tac_bit_mask(self.tac))
  }

  /// Returns whether the TIMA register is being reloaded.
  const fn tima_reloading(&self) -> bool {
    matches!(&self.timer_interrupt_delay, Some(0))
  }

  /// Returns whether the TIMA register overflowed.
  const fn tima_overflowed(&self) -> bool {
    matches!(&self.timer_interrupt_delay, Some(4))
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
