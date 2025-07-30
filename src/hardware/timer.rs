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

  /// The timer interrupt.
  timer_interrupt: Option<TimerInterrupt>,
  /// The previous AND result.
  prev_and_result: bool,
}

/// A timer interrupt to be fired.
#[derive(Debug, Clone)]
enum TimerInterrupt {
  /// The TMA register is being reloaded.
  Reloading,
  /// The number of ticks left until the interrupt is fired.
  Delay { ticks: u8 },
}

impl Timer {
  /// Creates a new [`Timer`].
  pub fn new() -> Self {
    Self {
      tima: 0,
      tma: 0,
      tac: 0,
      counter: 0xABCC,

      timer_interrupt: None,
      prev_and_result: false,
    }
  }

  /// Steps the timer by a T-cycle.
  pub fn step(&mut self, interrupts: &mut Interrupts) {
    self.counter = self.counter.wrapping_add(1);

    // The interrupt gets delayed by 4 T-cycles after TIMA overflows.
    if let Some(interrupt_delay) = &self.timer_interrupt {
      match interrupt_delay {
        TimerInterrupt::Reloading => self.timer_interrupt = None,
        &TimerInterrupt::Delay { ticks } => {
          let new_ticks = ticks - 1;

          if new_ticks == 0 {
            self.tima = self.tma;
            self.timer_interrupt = Some(TimerInterrupt::Reloading);

            interrupts.request_interrupt(Interrupt::Timer);
          } else {
            self.timer_interrupt = Some(TimerInterrupt::Delay { ticks: new_ticks })
          }
        }
      }
    }

    // Compare the extracted bit of the updated counter with the timer enable bit
    let curr_and_result = is_flag_set!(self.tac, TIMER_ENABLE_MASK)
      & is_flag_set!(self.counter, tac_bit_mask(self.tac));

    if self.prev_and_result && !curr_and_result {
      self.tima = self.tima.wrapping_add(1);

      if self.tima == 0 {
        self.timer_interrupt = Some(TimerInterrupt::Delay { ticks: 4 });
      }
    }

    self.prev_and_result = curr_and_result;
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
        // Writes to TIMA when it's being reloaded are ignored
        if !matches!(self.timer_interrupt, Some(TimerInterrupt::Reloading)) {
          self.tima = value;
        }

        // Writes to TIMA when it overflowed cancels the interrupt
        if matches!(
          self.timer_interrupt,
          Some(TimerInterrupt::Delay { ticks: 4 })
        ) {
          self.timer_interrupt = None;
        }
      }
      0xFF06 => {
        self.tma = value;

        // Writes to TMA when it's being reloaded also updates TIMA
        if matches!(self.timer_interrupt, Some(TimerInterrupt::Reloading)) {
          self.tima = self.tma;
        }
      }

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
