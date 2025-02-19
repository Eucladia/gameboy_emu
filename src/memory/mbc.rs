use crate::memory::Mbc0;

/// The kind of memory bank controller.
#[derive(Debug)]
pub enum Cartridge {
  Zero(Mbc0),
}

impl Cartridge {
  /// Reads the value specified by the address in ROM.
  pub fn read_rom(&self, address: u16) -> u8 {
    match self {
      Cartridge::Zero(mbc) => mbc.read_rom(address),
    }
  }

  /// Writes the value to the address in ROM.
  pub fn write_rom(&self, address: u16, value: u8) {
    match self {
      // This cartridge type does not have any ROM
      Cartridge::Zero(mbc) => {}
    }
  }

  /// Reads the value in RAM of the specified address in RAM.
  pub fn read_ram(&self, address: u16) -> u8 {
    match self {
      // This cartridge type does not have any RAM
      Cartridge::Zero(_) => 0xFF,
    }
  }

  /// Writes to the value to address in RAM.
  pub fn write_ram(&mut self, address: u16, value: u8) {
    match self {
      // No-op because this cartridge has no RAM
      Cartridge::Zero(_) => {}
    }
  }
}
