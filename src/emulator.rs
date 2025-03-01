use crate::hardware::{Cpu, Hardware};

#[derive(Debug)]
pub struct Emulator {
  /// The CPU for the Gameboy.
  cpu: Cpu,
  /// The hardware components of the Gameboy.
  hardware: Hardware,
}
