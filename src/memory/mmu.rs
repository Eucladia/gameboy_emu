use crate::memory::{Mbc0, MemoryBankController};

const CARTRIDGE_TYPE: u16 = 0x147;

/// The memory management unit for the Gameboy.
#[derive(Debug)]
pub struct Mmu {
  /// The available work RAM.
  ram: [u8; 0x2000],
  /// The high RAM.
  hram: [u8; 0x7F],
  /// The memory bank controller this cartridge uses.
  mbc: MemoryBankController,
  /// The interrupt enable register
  interrupt_enable: u8,
}

impl Mmu {
  /// Creates a new memory manager.
  pub fn new(rom: Vec<u8>) -> Self {
    let mbc = match rom[CARTRIDGE_TYPE as usize] {
      0 => MemoryBankController::Zero(Mbc0::new(rom)),
      b => panic!("got invalid memory cartridge type: {b:02X}"),
    };

    Self {
      mbc,
      ram: [0; 0x2000],
      hram: [0; 0x7f],
      interrupt_enable: 0,
    }
  }

  /// Reads 8-bits of memory from the given address.
  pub fn read_byte(&self, address: u16) -> u8 {
    match address {
      // ROM
      0x0000..0x8000 => self.mbc.read_rom(address),
      // Working RAM
      0xC000..0xE000 => self.ram[(address - 0xC000) as usize],
      // Echo RAM - A copy of working RAM
      0xE000..0xFE00 => self.ram[(address - 0xE000) as usize],
      // High RAM
      0xFF80..0xFFFF => self.hram[(address - 0xFF80) as usize],
      // Interrupt enable register
      0xFFFF => self.interrupt_enable,
      // Unmapped memory
      _ => 0xFF,
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
      // Working RAM
      0xC000..0xE000 => self.ram[(address - 0xC000) as usize] = value,
      // Echo of working RAM
      0xE000..0xFE00 => self.ram[(address - 0xE000) as usize] = value,
      // High RAM
      0xFF80..0xFFFF => self.hram[(address - 0xFF80) as usize] = value,
      // Interrupt enable
      0xFFFF => self.interrupt_enable = value,
      // Unwritable regions
      _ => (),
    }
  }
}
