use crate::hardware::{Cpu, Hardware, cpu::CpuState};

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
    // The number of ticks per frame.
    const CYCLES_PER_FRAME: usize = 70224;

    let mut total_cycles = 0;

    while total_cycles < CYCLES_PER_FRAME {
      let cycles = if matches!(self.cpu.state(), CpuState::Halted | CpuState::Stopped) {
        4
      } else {
        self.cpu.step(&mut self.hardware)
      };

      self.hardware.step_timer(cycles);

      if self.hardware.get_dma_transfer().is_some() {
        self.hardware.update_dma_transfer(cycles);
      }

      self.hardware.step_ppu(cycles);

      self.hardware.step_apu(cycles);

      if self.hardware.has_pending_interrupts() {
        self.cpu.handle_interrupts(&mut self.hardware);
      }

      total_cycles += cycles;
    }
  }
}
