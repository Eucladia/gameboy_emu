use crate::hardware::{Cpu, Hardware, clock::TCycle};

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

    debug_assert_eq!(self.hardware.sys_clock.t_cycle(), TCycle::T4);

    for _ in 0..(CYCLES_PER_FRAME / 4) {
      // ---------------------------------- T1 ----------------------------------
      self.hardware.step_sys_clock();

      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer();
      self.hardware.step_ppu();
      self.hardware.step_apu();
      self.hardware.step_dma_transfer();

      // ---------------------------------- T2 ----------------------------------
      self.hardware.step_sys_clock();

      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer();
      self.hardware.step_ppu();
      self.hardware.step_apu();
      self.hardware.step_dma_transfer();

      // ---------------------------------- T3 ----------------------------------
      self.hardware.step_sys_clock();

      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer();
      self.hardware.step_ppu();
      self.hardware.step_apu();
      self.hardware.step_dma_transfer();

      // ---------------------------------- T4 ----------------------------------
      self.hardware.step_sys_clock();

      self.cpu.step(&mut self.hardware);
      self.hardware.step_timer();
      self.hardware.step_ppu();
      self.hardware.step_apu();
      self.hardware.step_dma_transfer();
    }
  }
}
