use crate::{memory::Mmu, registers::Registers};

#[derive(Debug)]
pub struct Cpu {
  /// The set flags.
  ///
  /// Note: The upper nibble contains the set flags, the lower nibble is always zeroed.
  flags: u8,
  /// The clock state.
  clock: ClockState,
  /// The memory.
  memory: Mmu,
  /// The registers.
  registers: Registers,
}

impl Cpu {
  pub fn new(mmu: Mmu) -> Self {
    Self {
      memory: mmu,
      clock: ClockState::default(),
      flags: 0,
      registers: Registers::default(),
    }
  }
}

/// The internal time clock.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct ClockState {
  /// Machine cycles.
  pub m: u32,
  /// Tick cycles.
  pub t: u32,
}
