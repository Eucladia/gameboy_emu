/// The internal system clock.
#[derive(Debug, Clone)]
pub struct SystemClock(usize);

/// A possible cycle.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TCycle {
  /// T-cycle 1.
  T1,
  /// T-cycle 2.
  T2,
  /// T-cycle 3.
  T3,
  /// T-cycle 4.
  T4,
}

impl SystemClock {
  /// Creates a new system clock.
  pub const fn new() -> Self {
    Self(0)
  }

  /// Returns the current T-cycle.
  pub const fn t_cycle(&self) -> TCycle {
    const CYCLES_PER_CLOCK: usize = 4;

    match self.0 % CYCLES_PER_CLOCK {
      0 => TCycle::T4,
      1 => TCycle::T1,
      2 => TCycle::T2,
      3 => TCycle::T3,
      _ => unreachable!(),
    }
  }

  /// Increments the internal system clock by a T-cycle.
  pub fn increment_clock(&mut self) {
    self.0 = self.0.wrapping_add(1);
  }
}
