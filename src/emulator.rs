use crate::hardware::Cpu;

#[derive(Debug)]
pub struct Emulator {
  /// The CPU for the emulator.
  cpu: Cpu,
}
