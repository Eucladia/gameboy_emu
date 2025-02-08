use crate::{memory::Memory, registers::Registers};

pub struct Cpu {
  /// The set flags.
  ///
  /// Note: The upper nibble contains the set flags, the lower nibble is always
  /// zeroed.
  flags: u8,
  /// The clock state.
  clock: ClockState,
  /// The memory.
  memory: Memory,
  /// The registers.
  registers: Registers,
}

impl Cpu {
  pub fn new(mmu: Memory) -> Self {
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
