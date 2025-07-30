use crate::hardware::{Cpu, Hardware};

/// The Gameboy emulator.
#[derive(Debug)]
pub struct Emulator {
  /// The CPU for the Gameboy.
  cpu: Cpu,
  /// The hardware components of the Gameboy.
  pub hardware: Hardware,
}

impl Emulator {
  /// Creates a new [`Emulator`].
  pub fn new(cpu: Cpu, hardware: Hardware) -> Self {
    Self { cpu, hardware }
  }

  /// Steps one frame of the Gameboy.
  pub fn step(&mut self) {
    // The number of T-cycles per frame.
    const CYCLES_PER_FRAME: usize = 70224;

    for _ in 0..(CYCLES_PER_FRAME / 4) {
      // ---------------------------------- T1 ----------------------------------
      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer();
      self.hardware.step_ppu();
      self.hardware.step_apu();

      if self.hardware.dma_transfer_exists() {
        self.hardware.step_dma_transfer();
      }

      // ---------------------------------- T2 ----------------------------------
      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer();
      self.hardware.step_ppu();
      self.hardware.step_apu();

      if self.hardware.dma_transfer_exists() {
        self.hardware.step_dma_transfer();
      }

      // ---------------------------------- T3 ----------------------------------

      // NOTE: Step the timer first because of the timing sensitive test `rapid_toggle`.
      //
      // If we don't do this, then the timer interrupt won't be ready in time for the CPU,
      // since the CPU does that during T3.
      //
      // The alternative is to set the timer delay to 3 T-cycles when there's an overflow
      // from TAC writes, but that feels hacky and inconsistent.
      self.hardware.step_timer();
      self.cpu.step(&mut self.hardware);
      self.hardware.step_ppu();
      self.hardware.step_apu();

      if self.hardware.dma_transfer_exists() {
        self.hardware.step_dma_transfer();
      }

      // ---------------------------------- T4 ----------------------------------
      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer();
      self.hardware.step_ppu();
      self.hardware.step_apu();

      if self.hardware.dma_transfer_exists() {
        self.hardware.step_dma_transfer();
      }
    }
  }
}
