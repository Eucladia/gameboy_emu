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

    for _ in 0..CYCLES_PER_FRAME {
      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer(1);
      self.hardware.step_ppu(1);
      self.hardware.step_apu(1);

      if self.hardware.dma_transfer_running() {
        self.hardware.step_dma_transfer();
      }
    }
  }
}
