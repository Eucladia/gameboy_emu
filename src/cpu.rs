use crate::{memory::Memory, registers::Registers};

pub struct Cpu {
  /// The set flags, where the upper nibble contains the relevant set `Flags`.
  flags: u8,
  /// The clock state.
  clock: ClockState,
  /// The memory.
  memory: Memory,
  /// The registers.
  registers: Registers,
}

/// The internal time clock.
struct ClockState {
  /// Machine cycles.
  pub m: u32,
  /// Tick cycles.
  pub t: u32,
}
