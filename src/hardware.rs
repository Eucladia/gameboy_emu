pub mod cartridge;
pub mod cpu;
pub mod joypad;
pub mod ppu;
pub mod registers;
pub mod timer;

pub use cpu::Cpu;
pub use joypad::Joypad;
use ppu::PpuMode;
pub use timer::Timer;

use crate::{
  hardware::cartridge::{Cartridge, RomOnly},
  hardware::ppu::{DmaTransfer, Ppu},
  interrupts::Interrupts,
};

#[derive(Debug)]
pub struct Hardware {
  /// The internal memory available.
  memory: [u8; MEMORY_SIZE as usize],
  /// The high ram.
  high_ram: [u8; HIGH_RAM_SIZE as usize],
  /// The input joypad.
  joypad: Joypad,
  /// The game cartridge.
  cartridge: Cartridge,
  /// The timer.
  timer: Timer,
  /// The pixel processing unit.
  ppu: Ppu,
  /// The set interrupts
  interrupts: Interrupts,
}

impl Hardware {
  pub fn new(bytes: Vec<u8>) -> Self {
    let cartridge = match bytes[CARTRIDGE_TYPE as usize] {
      0 => Cartridge::RomOnly(RomOnly::new(bytes)),
      b => panic!("got invalid memory cartridge type: {b:02X}"),
    };

    Self {
      memory: [0; MEMORY_SIZE as usize],
      high_ram: [0; HIGH_RAM_SIZE as usize],
      joypad: Joypad::new(),
      timer: Timer::new(),
      ppu: Ppu::new(),
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
        // VRAM is inaccessible during pixel transfer mode
        if matches!(self.ppu.current_mode(), PpuMode::PixelTransfer) {
          0xFF
        } else {
          self.ppu.read_ram(address)
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
        // OAM is inaccessible during OAM and pixel transfer modes
        if matches!(
          self.ppu.current_mode(),
          PpuMode::OamScan | PpuMode::PixelTransfer
        ) {
          0xFF
        } else {
          self.ppu.read_oam(address)
        }
      }
      // Unused
      0xFEA0..0xFF00 => 0xFF,
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
    let upper = self.read_byte(address + 1) as u16;

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
        // Writing to VRAM is undefined when in pixel transfer mode
        if !matches!(self.ppu.current_mode(), PpuMode::PixelTransfer) {
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
        // Writing to OAM is undefined when in OAM and pixel transfer mode
        if !matches!(
          self.ppu.current_mode(),
          PpuMode::OamScan | PpuMode::PixelTransfer
        ) {
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
        let mut index = current_pos as u16;
        let starting_address = (self.ppu.dma as u16) << 8;
        let ending_address = starting_address + 160;
        let remaining_bytes = ending_address - index;
        let iterations = (cycles as u16 / 4).min(remaining_bytes);

        for _ in 0..iterations {
          let src_byte = self.read_byte(starting_address + index);

          // Use PPU's write_oam method because Hardware's write_byte fn checks for
          // active DMA transfers.
          self.ppu.write_oam(0xFE00 + index, src_byte);

          index += 1;
        }

        if index >= 160 {
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

  /// Gets the active DMA transfer.
  pub fn get_dma_transfer(&self) -> Option<&DmaTransfer> {
    self.ppu.dma_transfer.as_ref()
  }

  fn read_io_register(&self, address: u16) -> u8 {
    match address {
      0xFF00 => self.joypad.read_register(),
      0xFF04..0xFF08 => self.timer.read_register(address),
      0xFF40..0xFF4B => self.ppu.read_register(address),
      0xFF0F => self.interrupts.requested_bitfield(),
      0xFF10..0xFF27 | 0xFF30..0xFF40 => todo!("audio is unimplemented"),
      _ => unreachable!(),
    }
  }

  fn write_io_register(&mut self, address: u16, value: u8) {
    match address {
      0xFF00 => self.joypad.write_register(value),
      0xFF04..0xFF08 => self.timer.write_register(address, value),
      0xFF40..0xFF4B => self.ppu.write_register(address, value),
      0xFF0F => self.interrupts.set_requested(value),
      0xFF10..0xFF27 | 0xFF30..0xFF40 => todo!("audio is unimplemented"),
      _ => unreachable!(),
    }
  }
}

/// The amount of working memory.
const MEMORY_SIZE: u16 = 0x2000;
/// The amount of fast, high memory.
const HIGH_RAM_SIZE: u16 = 0x7F;

const CARTRIDGE_TYPE: u16 = 0x147;
