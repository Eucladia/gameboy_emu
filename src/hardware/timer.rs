use crate::{
  flags::{is_falling_edge, is_flag_set},
  hardware::clock::{SystemClock, TCycle},
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
  /// The internal counter, used for the DIV register.
  counter: u16,
  /// The timer interrupt.
  timer_interrupt: TimerInterrupt,
}

/// The timer interrupt.
#[derive(Debug, Clone, Copy)]
enum TimerInterrupt {
  /// There is currently no timer interrupt.
  None,
  /// The TIMA register overflowed and an interrupt will be fired.
  Overflowed,
  /// The TIMA register is being reloaded.
  Reloading,
}

impl Timer {
  /// Creates a new [`Timer`].
  pub fn new() -> Self {
    Self {
      tima: 0,
      tma: 0,
      tac: 0,
      counter: 0xABCC,
      timer_interrupt: TimerInterrupt::None,
    }
  }

  /// Steps the timer by a T-cycle.
  pub fn step(&mut self, interrupts: &mut Interrupts, sys_clock: &SystemClock) {
    match sys_clock.t_cycle() {
      TCycle::T1 | TCycle::T2 | TCycle::T3 => {}
      // The timer gets clocked every M-cycle, not T-cycle.
      TCycle::T4 => {
        match &self.timer_interrupt {
          TimerInterrupt::Overflowed => {
            // Reload TIMA and request an interrupt, since an M-cycle has elapsed.
            self.tima = self.tma;
            interrupts.request_interrupt(Interrupt::Timer);

            self.timer_interrupt = TimerInterrupt::Reloading;
          }
          TimerInterrupt::Reloading => {
            self.timer_interrupt = TimerInterrupt::None;
          }
          TimerInterrupt::None => {}
        }

        let prev_and_result = self.counter_and_result();

        self.counter = self.counter.wrapping_add(1);

        let curr_and_result = self.counter_and_result();

        if is_falling_edge!(prev_and_result, curr_and_result) {
          self.increment_tima();
        }
      }
    }
  }

  /// Reads from the timer's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address {
      0xFF04 => self.div_value(),
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

        if is_falling_edge!(prev_and_result, curr_and_result) {
          self.increment_tima();
        }
      }
      0xFF05 => {
        // Writes to TIMA when it's being reloaded are ignored
        if !matches!(self.timer_interrupt, TimerInterrupt::Reloading) {
          self.tima = value;
        }

        // Writing to TIMA when it overflowed cancels the interrupt
        if matches!(self.timer_interrupt, TimerInterrupt::Overflowed) {
          self.timer_interrupt = TimerInterrupt::None;
        }
      }
      0xFF06 => {
        self.tma = value;

        // Writes to TMA when it's being reloaded also updates TIMA
        if matches!(self.timer_interrupt, TimerInterrupt::Reloading) {
          self.tima = self.tma;
        }
      }

      0xFF07 => {
        let prev_and_result = self.counter_and_result();

        self.tac = value & 0x07;

        let curr_and_result = self.counter_and_result();

        if is_falling_edge!(prev_and_result, curr_and_result) {
          self.increment_tima();
        }
      }
      _ => unreachable!(),
    }
  }

  /// Increments the TIMA register.
  fn increment_tima(&mut self) {
    self.tima = self.tima.wrapping_add(1);

    if self.tima == 0 {
      self.timer_interrupt = TimerInterrupt::Overflowed;
    }
  }

  /// Returns the value of the DIV register
  const fn div_value(&self) -> u8 {
    // DIV is actually bits 6-13, not bits 8-15. The top 2 bits have to do
    // with `STOP` shenanigans.
    (self.counter >> 6) as u8
  }

  /// Gets the current and result for the timer counter.
  const fn counter_and_result(&self) -> bool {
    is_flag_set!(self.tac, TIMER_ENABLE_MASK) && is_flag_set!(self.counter, tac_bit_mask(self.tac))
  }
}

/// Gets the clock select bit mask from the TAC register.
const fn tac_bit_mask(tac: u8) -> u16 {
  match tac & 0x3 {
    0b00 => 1 << 7,
    0b01 => 1 << 1,
    0b10 => 1 << 3,
    0b11 => 1 << 5,
    _ => unreachable!(),
  }
}

/// The bit mask for the TAC register for checking if the timer is enabled.
const TIMER_ENABLE_MASK: u8 = 0x04;
