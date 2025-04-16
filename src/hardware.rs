pub mod apu;
pub mod cartridge;
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
  flags::is_flag_set,
  hardware::{
    apu::{Apu, AudioSample},
    cartridge::{Cartridge, Mbc1, RomOnly},
    joypad::Button,
    ppu::{DmaTransfer, Ppu, PpuMode},
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
        // VRAM is only accessible if the LCD is off or the PPU is not in pixel transfer
        //
        // See https://gbdev.io/pandocs/Accessing_VRAM_and_OAM.html for more.
        if !self.ppu.display_enabled() || !matches!(self.ppu.current_mode(), PpuMode::PixelTransfer)
        {
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
        // OAM is only accessible if the LCD is off or the PPU is not in OAM scan
        // and not pixel transfer modes
        //
        // See https://gbdev.io/pandocs/Accessing_VRAM_and_OAM.html for more.
        if !self.ppu.display_enabled()
          || !matches!(
            self.ppu.current_mode(),
            PpuMode::OamScan | PpuMode::PixelTransfer
          )
        {
          self.ppu.read_oam(address)
        } else {
          0xFF
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

  /// Reads 16-bits in memory, in little endian, from the given address.
  pub fn read_word(&self, address: u16) -> u16 {
    let lower = self.read_byte(address) as u16;
    let upper = self.read_byte(address.wrapping_add(1)) as u16;

    (upper << 8) | lower
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
        // VRAM is only accessible when the LCD is off or the PPU is not in pixel transfer.
        //
        // See https://gbdev.io/pandocs/Accessing_VRAM_and_OAM.html for more.
        if !self.ppu.display_enabled() || !matches!(self.ppu.current_mode(), PpuMode::PixelTransfer)
        {
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
        // OAM is only accessible when the LCD is off or the PPU is not in pixel transfer
        // and not in OAM scan.
        //
        // See https://gbdev.io/pandocs/Accessing_VRAM_and_OAM.html for more.
        if !self.ppu.display_enabled()
          || !matches!(
            self.ppu.current_mode(),
            PpuMode::OamScan | PpuMode::PixelTransfer
          )
        {
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

  /// Progresses the DMA transfer, copying the next byte in to the OAM.
  pub fn update_dma_transfer(&mut self, cycles: usize) {
    match self.ppu.dma_transfer {
      Some(DmaTransfer::Requested) => {
        // There's a delay of 1 M-cycle when executing DMA, so we have a filler state
        self.ppu.dma_transfer = Some(DmaTransfer::Starting);
      }
      Some(DmaTransfer::Starting) => {
        self.ppu.dma_transfer = Some(DmaTransfer::Transferring { current_pos: 0 });
      }
      Some(DmaTransfer::Transferring { current_pos }) => {
        const DMA_MAX_BYTES_TRANSFERRABLE: u16 = 160;

        let mut index = current_pos as u16;
        let starting_address = (self.ppu.dma as u16) << 8;
        let remaining_bytes = DMA_MAX_BYTES_TRANSFERRABLE - index;
        let iterations = (cycles as u16 / 4).min(remaining_bytes);

        for _ in 0..iterations {
          let src_byte = self.read_byte(starting_address + index);

          // Use `Ppu::write_oam` method because Hardware::write_byte` checks for
          // active DMA transfers.
          self.ppu.write_oam(0xFE00 + index, src_byte);

          index += 1;
        }

        if index >= DMA_MAX_BYTES_TRANSFERRABLE {
          self.ppu.dma_transfer = None;
        } else {
          self.ppu.dma_transfer = Some(DmaTransfer::Transferring {
            current_pos: index as u8,
          })
        }
      }
      None => {}
    }
  }

  /// Presses or releases a [`Button`].
  pub fn update_button(&mut self, button: Button, pressed: bool) {
    self
      .joypad
      .update_button_state(&mut self.interrupts, button, pressed);
  }

  /// Steps the timer with the following number of cycles.
  pub fn step_timer(&mut self, cycles: usize) {
    self.timer.step(&mut self.interrupts, cycles);
  }

  /// Steps the PPU with the following number of cycles.
  pub fn step_ppu(&mut self, cycles: usize) {
    self.ppu.step(&mut self.interrupts, cycles);
  }

  /// Steps the APU with the following number of cycles.
  pub fn step_apu(&mut self, cycles: usize) {
    self.apu.step(cycles);
  }

  pub fn audio_buffer(&self) -> Arc<Mutex<VecDeque<AudioSample>>> {
    self.apu.audio_buffer()
  }

  /// Checks if there are any pending interrupts.
  pub fn has_pending_interrupts(&self) -> bool {
    (self.interrupts.enabled_bitfield() & self.interrupts.requested_bitfield()) != 0
  }

  /// Checks if the following interrupt has been requested.
  pub fn is_interrupt_requested(&self, interrupt: Interrupt) -> bool {
    is_flag_set!(self.interrupts.enabled_bitfield(), interrupt as u8)
      && is_flag_set!(self.interrupts.requested_bitfield(), interrupt as u8)
  }

  /// Clears a requested [`Interrupt`].
  pub fn clear_interrupt(&mut self, interrupt: Interrupt) {
    self.interrupts.clear_interrupt(interrupt);
  }

  /// Gets the active DMA transfer.
  pub fn get_dma_transfer(&self) -> Option<&DmaTransfer> {
    self.ppu.dma_transfer.as_ref()
  }

  /// Gets the frame buffer from the PPU.
  pub fn frame_buffer(&self) -> &[[u8; 160]; 144] {
    self.ppu.buffer()
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
}

/// The amount of working memory.
const MEMORY_SIZE: u16 = 0x2000;
/// The amount of fast, high memory.
const HIGH_RAM_SIZE: u16 = 0x7F;

/// The address where the cartridge type is stored.
const CARTRIDGE_TYPE: u16 = 0x147;
