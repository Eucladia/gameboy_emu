use crate::{cpu::Cpu, memory::Mmu};

#[derive(Debug)]
pub struct Emulator {
  /// The CPU for the emulator.
  cpu: Cpu,
  /// The MMU for the emulator.
  mmu: Mmu,
}
