use crate::memory::{Mbc0, MemoryBankController};

const CARTRIDGE_TYPE: u16 = 0x147;

/// The memory management unit for the Gameboy.
#[derive(Debug)]
pub struct Mmu {
  /// The available work RAM.
  ram: [u8; 0x2000],
  /// The high RAM.
  hram: [u8; 0x7F],
  /// The I/O registers.
  io_registers: IoRegisters,
  /// The interrupt enable register
  interrupt_enable: u8,
  /// The memory bank controller this cartridge uses.
  mbc: MemoryBankController,
}

#[derive(Debug)]
struct IoRegisters {
  /// The joypad.
  // 0xFF00
  joypad: u8,
  /// Timer registers.
  // 0xFF04 - 0xFF07
  timer: [u8; 4],
  /// Whether there was an interrupt.
  // 0xFF0F
  interrupt_flag: u8,
  /// LCD registers.
  // 0xFF40 - 0xFF48
  lcd: [u8; 12],
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
      // I/O Registers
      0xFF00..0xFF80 => self.io_registers.read_byte(address),
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
      // ROM
      0x0000..0x8000 => self.mbc.write_rom(address, value),
      // Working RAM
      0xC000..0xE000 => self.ram[(address - 0xC000) as usize] = value,
      // Echo of working RAM
      0xE000..0xFE00 => self.ram[(address - 0xE000) as usize] = value,
      // I/O Registers
      0xFF00..0xFF90 => self.io_registers.write_byte(address, value),
      // High RAM
      0xFF80..0xFFFF => self.hram[(address - 0xFF80) as usize] = value,
      // Interrupt enable
      0xFFFF => self.interrupt_enable = value,
      // Unwritable regions
      _ => {}
    }
  }
}

impl IoRegisters {
  pub fn new() -> Self {
    Self {
      joypad: 0xFF,
      timer: [0; 4],
      interrupt_flag: 0,
      lcd: [0; 12],
    }
  }

  pub fn read_byte(&self, address: u16) -> u8 {
    match address {
      // Set the unused bits
      0xFF00 => self.joypad | 0xCF,
      0xFF04..0xFF08 => self.timer[(address - 0xFF04) as usize],
      0xFF0F => self.interrupt_flag,
      0xFF40..0xFF4C => self.lcd[(address - 0xFF40) as usize],
      _ => 0xFF,
    }
  }
  pub fn write_byte(&mut self, address: u16, value: u8) {
    match address {
      // Make sure that only bits 4 and 5 can be written to
      0xFF00 => self.joypad = value & 0b110000,
      0xFF04 => self.timer[0] = 0,
      0xFF05..0xFF08 => self.timer[(address - 0xFF04) as usize] = value,
      // Only the first 5 LSB are valid
      0xFF0F => self.interrupt_flag = value & 0b11111,
      0xFF40..0xFF4C => self.lcd[(address - 0xFF40) as usize] = value,
      _ => {}
    }
  }
}
