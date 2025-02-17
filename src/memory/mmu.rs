use std::ops::Range;

use crate::memory::{Mbc0, MemoryBankController};

/// The starting address for ROM bank 0.
pub const ROM_BANK_0_START: u16 = 0;
/// The ending address for ROM bank 0.
pub const ROM_BANK_0_END: u16 = 0x4000;
/// The starting address for the switchable ROM bank.
pub const ROM_BANK_N_START: u16 = 0x4000;
/// The ending address  for the switchable ROM bank.
pub const ROM_BANK_N_END: u16 = 0x8000;
/// The starting address for VRAM.
pub const VIDEO_RAM_START: u16 = 0x8000;
/// The ending address  for VRAM.
pub const VIDEO_RAM_END: u16 = 0xA000;
/// The starting address for the cartridges RAM.
pub const EXTERNAL_RAM_START: u16 = 0xA000;
/// The ending address for the cartridges  RAM.
pub const EXTERNAL_RAM_END: u16 = 0xC000;
/// The starting address for internal RAM.
pub const INTERNAL_RAM_START: u16 = 0xC000;
/// The ending address for internal RAM.
pub const INTERNAL_RAM_END: u16 = 0xE000;
/// The starting address for echo RAM.
pub const ECHO_RAM_START: u16 = 0xE000;
/// The ending address for echo RAM.
pub const ECHO_RAM_END: u16 = 0xFE00;
/// The starting address for the OAM (sprite attribute memory).
pub const OAM_START: u16 = 0xFE00;
/// The ending address for the OAM (sprite attribute memory).
pub const OAM_END: u16 = 0xFEA0;
/// The starting address for unused memory.
pub const UNUSED_START: u16 = 0xFEA0;
/// The ending address for unused memory.
pub const UNUSED_END: u16 = 0xFF00;
/// The starting address for I/O registers.
pub const IO_START: u16 = 0xFF00;
/// The ending address for I/O registers.
pub const IO_END: u16 = 0xFF80;
/// The starting address for HRAM.
pub const HIGH_RAM_START: u16 = 0xFF80;
/// The ending address for HRAM.
pub const HIGH_RAM_END: u16 = 0xFFFF;
/// The interrupt enable register.
pub const INTERRUPT_ENABLE_REGISTER: u16 = 0xFFFF;

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
/// The types of interrupts.
pub enum Interrupt {
  VBlank = 1 << 0,
  LCD = 1 << 1,
  Timer = 1 << 2,
  Serial = 1 << 3,
  Joypad = 1 << 4,
}

const CARTRIDGE_TYPE: u16 = 0x147;

/// The memory management unit for the Gameboy.
#[derive(Debug)]
pub struct Mmu {
  /// The available memory to work with.
  memory: [u8; u16::MAX as usize + 1],
  /// The memory bank controller this cartridge uses.
  mbc: MemoryBankController,
}

impl Mmu {
  /// Creates a new memory manager.
  pub fn new(cartridge: Vec<u8>) -> Self {
    let mbc = match cartridge[CARTRIDGE_TYPE as usize] {
      0 => MemoryBankController::Zero(Mbc0::new(cartridge)),
      b => panic!("got invalid memory cartridge type: {b:02X}"),
    };

    Self {
      memory: [0; u16::MAX as usize + 1],
      mbc,
    }
  }

  /// Reads 8-bits of memory from the given address.
  pub fn read_byte(&self, address: u16) -> u8 {
    match address {
      // ROM
      ROM_BANK_0_START..ROM_BANK_0_END => self.mbc.read_rom(address),
      // Switchable ROM bank
      ROM_BANK_N_START..ROM_BANK_N_END => self.mbc.read_rom(address),
      // Video RAM
      VIDEO_RAM_START..VIDEO_RAM_END => self.memory[address as usize],
      // Cartridge RAM
      EXTERNAL_RAM_START..EXTERNAL_RAM_END => self.memory[address as usize],
      // Work RAM
      INTERNAL_RAM_START..INTERNAL_RAM_END => self.memory[address as usize],
      // Echo RAM mirrors work RAM
      ECHO_RAM_START..ECHO_RAM_END => {
        self.memory[(INTERNAL_RAM_START + (address - ECHO_RAM_START)) as usize]
      }
      // Sprite memory
      OAM_START..OAM_END => self.memory[address as usize],
      // Unused, map to 0xFF
      UNUSED_START..UNUSED_END => 0xFF,
      // I/O Registers
      // TODO: Research and impl properly
      IO_START..IO_END => self.memory[address as usize],
      // High RAM
      HIGH_RAM_START..HIGH_RAM_END => self.memory[address as usize],
      // Interrupt register
      INTERRUPT_ENABLE_REGISTER => self.memory[address as usize],
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
      ROM_BANK_0_START..ROM_BANK_0_END => self.mbc.write_rom(address, value),
      // Switchable ROM bank
      ROM_BANK_N_START..ROM_BANK_N_END => self.mbc.write_rom(address, value),
      // Video RAM
      VIDEO_RAM_START..VIDEO_RAM_END => self.memory[address as usize] = value,
      // Cartridge RAM
      EXTERNAL_RAM_START..EXTERNAL_RAM_END => self.memory[address as usize] = value,
      // Work RAM
      INTERNAL_RAM_START..INTERNAL_RAM_END => self.memory[address as usize] = value,
      // Echo RAM mirrors work RAM
      ECHO_RAM_START..ECHO_RAM_END => {
        self.memory[(INTERNAL_RAM_START + (address - ECHO_RAM_START)) as usize] = value
      }
      // Sprite memory
      OAM_START..OAM_END => self.memory[address as usize] = value,
      // Unused, map to 0xFF
      UNUSED_START..UNUSED_END => {}
      // I/O Registers
      IO_START..IO_END => self.memory[address as usize] = value,
      // High RAM
      HIGH_RAM_START..HIGH_RAM_END => self.memory[address as usize] = value,
      // Interrupt register
      INTERRUPT_ENABLE_REGISTER => self.memory[address as usize] = value,
    }
  }
}
