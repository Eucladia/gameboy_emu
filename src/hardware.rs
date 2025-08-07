pub mod apu;
pub mod cartridge;
pub mod clock;
pub mod cpu;
pub mod joypad;
pub mod ppu;
pub mod registers;
pub mod timer;

use std::{
  collections::VecDeque,
  sync::{Arc, Mutex},
};

pub use cpu::Cpu;
pub use joypad::Joypad;
pub use timer::Timer;

use crate::{
  hardware::{
    apu::{Apu, AudioSample},
    cartridge::{Cartridge, Mbc1, RomOnly},
    clock::SystemClock,
    joypad::{Button, ButtonAction},
    ppu::{DmaTransfer, DmaTransferProgress, Ppu},
  },
  interrupts::{Interrupt, Interrupts},
};

#[derive(Debug)]
pub struct Hardware {
  /// The internal memory available.
  memory: [u8; MEMORY_SIZE as usize],
  /// The high ram.
  high_ram: [u8; HIGH_RAM_SIZE as usize],
  /// The input joypad.
  pub joypad: Joypad,
  /// The game cartridge.
  pub cartridge: Cartridge,
  /// The timer.
  pub timer: Timer,
  /// The pixel processing unit.
  pub ppu: Ppu,
  /// The audio processing unit.
  pub apu: Apu,
  /// The system clock.
  pub sys_clock: SystemClock,
  /// The enableed and requested interrupts.
  interrupts: Interrupts,
}

impl Hardware {
  /// Creates a new [`Hardware`] instance from the given bytes.
  pub fn new(bytes: Vec<u8>) -> Self {
    let cartridge = match bytes[CARTRIDGE_TYPE as usize] {
      0x0 => Cartridge::RomOnly(RomOnly::new(bytes)),
      0x01..=0x03 => Cartridge::Mbc1(Mbc1::new(bytes)),
      b => panic!("got invalid memory cartridge type: {b:02X}"),
    };

    Self {
      memory: [0; MEMORY_SIZE as usize],
      high_ram: [0; HIGH_RAM_SIZE as usize],
      joypad: Joypad::new(),
      timer: Timer::new(),
      ppu: Ppu::new(),
      apu: Apu::new(),
      interrupts: Interrupts::new(),
      sys_clock: SystemClock::new(),
      cartridge,
    }
  }

  /// Reads 8 bits of memory from the given address.
  pub fn read_byte(&self, address: u16) -> u8 {
    match address {
      // ROM
      0..0x4000 => self.cartridge.read_rom(address),
      // ROM, bank N
      0x4000..0x8000 => self.cartridge.read_rom(address),
      // Video RAM
      0x8000..0xA000 => {
        if self.ppu.can_access_vram() {
          self.ppu.read_ram(address)
        } else {
          0xFF
        }
      }
      // External RAM
      0xA000..0xC000 => self.cartridge.read_ram(address),
      // Work RAM
      0xC000..0xE000 => self.memory[(address - 0xC000) as usize],
      // Echo RAM
      0xE000..0xFE00 => self.memory[(address - 0xE000) as usize],
      // OAM
      0xFE00..0xFEA0 => {
        let ppu_blocked = !self.ppu.can_access_oam();
        // OAM is also blocked if there's a running DMA transfer
        let dma_blocked = self.ppu.dma_transfer_running();

        if dma_blocked || ppu_blocked {
          0xFF
        } else {
          self.ppu.read_oam(address)
        }
      }
      // Unused
      // These values should return 0, per "The Cycle-Accurate Game Boy Docs"
      0xFEA0..0xFF00 => 0x00,
      // I/O Registers
      0xFF00..0xFF80 => self.read_io_register(address),
      // High RAM
      0xFF80..0xFFFF => self.high_ram[(address - 0xFF80) as usize],
      // Interrupt enable register
      0xFFFF => self.interrupts.enabled_bitfield(),
    }
  }

  /// Writes 8-bits to memory at the specified address.
  pub fn write_byte(&mut self, address: u16, value: u8) {
    match address {
      // ROM
      0x0000..0x4000 => self.cartridge.write_rom(address, value),
      // Switchable ROM bank
      0x4000..0x8000 => self.cartridge.write_rom(address, value),
      // Video RAM
      0x8000..0xA000 => {
        if self.ppu.can_access_vram() {
          self.ppu.write_ram(address, value)
        }
      }
      // External RAM
      0xA000..0xC000 => self.cartridge.write_ram(address, value),
      // Work RAM
      0xC000..0xE000 => self.memory[(address - 0xC000) as usize] = value,
      // Echo RAM
      0xE000..0xFE00 => self.memory[(address - 0xE000) as usize] = value,
      // OAM
      0xFE00..0xFEA0 => {
        let ppu_blocked = !self.ppu.can_access_oam();
        // OAM is also blocked if there's a running DMA transfer
        let dma_blocked = self.ppu.dma_transfer_running();

        if !dma_blocked && !ppu_blocked {
          self.ppu.write_oam(address, value)
        }
      }
      // Unused
      0xFEA0..0xFF00 => {}
      // I/O Registers
      0xFF00..0xFF80 => self.write_io_register(address, value),
      // High RAM
      0xFF80..0xFFFF => self.high_ram[(address - 0xFF80) as usize] = value,
      // Interrupt enable register
      0xFFFF => self.interrupts.set_enabled(value),
    }
  }

  /// Steps the DMA transfer by one T-cycle.
  pub fn step_dma_transfer(&mut self) {
    const DMA_TRANSFER_DELAY: u8 = 4;

    // This looks ugly to satisfy the borrow checker, it struggles with
    // mutable disjoint borrows :(
    match self.ppu.dma_transfer {
      Some(DmaTransfer {
        source,
        ref progress,
      }) => {
        match progress {
          &DmaTransferProgress::Requested { delay_ticks: ticks } => {
            let new_ticks = ticks + 1;

            if new_ticks == DMA_TRANSFER_DELAY {
              self.ppu.dma_transfer = Some(DmaTransfer::starting(source));
            } else {
              self.ppu.dma_transfer = Some(DmaTransfer::requested_with_ticks(source, new_ticks));
            }
          }
          &DmaTransferProgress::Transferring { ticks } => 'arm: {
            const CYCLES_PER_TRANSFER: u16 = 4;
            const DMA_TRANSFER_MAX_BYTES: u16 = 160;
            const DMA_TRANSFER_DURATION: u16 = DMA_TRANSFER_MAX_BYTES * CYCLES_PER_TRANSFER;

            // Check for this at the start, otherwise we would end the DMA transfer 1 T-cycle
            // before it should actually be over. This is important to pass `oam_dma_timing`.
            if ticks == DMA_TRANSFER_DURATION {
              self.ppu.dma_transfer = None;
              break 'arm;
            }

            let new_ticks = ticks + 1;

            // An M-cycle has occured, so transfer a byte now
            if new_ticks % CYCLES_PER_TRANSFER == 0 {
              let starting_address = (source as u16) << 8;
              let index = ticks / CYCLES_PER_TRANSFER;
              let src_byte = self.read_byte(starting_address + index);

              // Use `Ppu::write_oam` because Hardware::write_byte` checks for active DMA transfers.
              self.ppu.write_oam(0xFE00 + index, src_byte);
            }

            self.ppu.dma_transfer = Some(DmaTransfer::starting_with_ticks(source, new_ticks))
          }
        }
      }
      None => {}
    }

    // Restarted DMA transfers overwrite the previous one 4 T-cycles after requested.
    if let Some(restarted_dma) = &mut self.ppu.restarted_dma_transfer {
      restarted_dma.delay_ticks += 1;

      if restarted_dma.delay_ticks == DMA_TRANSFER_DELAY {
        self.ppu.dma_transfer = Some(DmaTransfer::starting(restarted_dma.source));
        self.ppu.restarted_dma_transfer = None;
      }
    }
  }

  /// Reads the I/O registers.
  fn read_io_register(&self, address: u16) -> u8 {
    match address {
      0xFF00 => self.joypad.read_register(),
      // Serial transfer
      0xFF01 | 0xFF02 => 0x0,
      0xFF04..0xFF08 => self.timer.read_register(address),
      0xFF10..0xFF27 | 0xFF30..0xFF40 => self.apu.read_register(address),
      0xFF40..0xFF4C => self.ppu.read_register(address),
      0xFF0F => self.interrupts.requested_bitfield(),
      _ => 0xFF,
    }
  }

  /// Writes to the I/O registers.
  fn write_io_register(&mut self, address: u16, value: u8) {
    match address {
      0xFF00 => self.joypad.write_register(value),
      // Serial transfer
      0xFF01 | 0xFF02 => {}
      0xFF04..0xFF08 => self.timer.write_register(address, value),
      0xFF10..0xFF27 | 0xFF30..0xFF40 => self.apu.write_register(address, value),
      0xFF40..0xFF4C => self.ppu.write_register(address, value),
      0xFF0F => self.interrupts.set_requested(value),
      _ => {}
    }
  }

  /// Updates the joypad's button state for the [`Button`].
  pub fn update_button(&mut self, button: Button, button_state: ButtonAction) {
    self
      .joypad
      .update_button_state(&mut self.interrupts, button, button_state);
  }

  /// Steps the timer by a T-cycle.
  pub fn step_timer(&mut self) {
    self.timer.step(&mut self.interrupts, &self.sys_clock);
  }

  /// Steps the PPU by a T-cycle.
  pub fn step_ppu(&mut self) {
    self.ppu.step(&mut self.interrupts);
  }

  /// Steps the APU by a T-cycle.
  pub fn step_apu(&mut self) {
    self.apu.step();
  }

  /// Steps the system clock by a T-cycle.
  pub fn step_sys_clock(&mut self) {
    self.sys_clock.increment_clock()
  }

  /// Returns the audio buffer.
  pub fn audio_buffer(&self) -> Arc<Mutex<VecDeque<AudioSample>>> {
    self.apu.audio_buffer()
  }

  /// Gets the frame buffer from the PPU.
  pub fn frame_buffer(&self) -> &[[u8; 160]; 144] {
    self.ppu.buffer()
  }

  /// Checks if there are any pending interrupts.
  pub fn has_pending_interrupts(&self) -> bool {
    self.interrupts.pending_bitfield() != 0
  }

  /// Returns the next pending [`Interrupt`] to be handled, if any.
  pub fn next_pending_interrupt(&self) -> Option<Interrupt> {
    let pending = self.interrupts.pending_bitfield();

    Interrupts::next_interrupt_from_bitfield(pending)
  }

  /// Clears a requested [`Interrupt`].
  pub fn clear_interrupt(&mut self, interrupt: Interrupt) {
    self.interrupts.clear_interrupt(interrupt);
  }
}

/// The amount of working memory.
const MEMORY_SIZE: u16 = 0x2000;
/// The amount of fast, high memory.
const HIGH_RAM_SIZE: u16 = 0x7F;
/// The address where the cartridge type is stored.
const CARTRIDGE_TYPE: u16 = 0x147;
