// A kind of cartridge.
#[derive(Debug)]
pub enum Cartridge {
  /// A game cartridge that only has 32kB of ROM and no RAM.
  RomOnly(RomOnly),
}

impl Cartridge {
  /// Reads the value specified by the address in ROM.
  pub fn read_rom(&self, address: u16) -> u8 {
    match self {
      Cartridge::RomOnly(cartridge) => cartridge.read_rom(address),
    }
  }

  /// Writes the value to the address in ROM.
  pub fn write_rom(&self, address: u16, value: u8) {
    match self {
      // This cartridge type does not have any ROM
      Cartridge::RomOnly(cartridge) => {}
    }
  }

  /// Reads the value in RAM of the specified address in RAM.
  pub fn read_ram(&self, address: u16) -> u8 {
    match self {
      // This cartridge type does not have any RAM
      Cartridge::RomOnly(_) => 0xFF,
    }
  }

  /// Writes to the value to address in RAM.
  pub fn write_ram(&mut self, address: u16, value: u8) {
    match self {
      // No-op because this cartridge type has no RAM
      Cartridge::RomOnly(_) => {}
    }
  }
}

/// A cartridge that only has ROM and no memory bank controller.
#[derive(Debug)]
pub struct RomOnly {
  /// The ROM of the cartridge.
  // NOTE: Can't we make this an array of 32kB since this game has a fixed amount of ROM?
  rom: Vec<u8>,
}

impl RomOnly {
  pub fn new(rom: Vec<u8>) -> Self {
    Self { rom }
  }

  /// Reads from the ROM.
  pub fn read_rom(&self, address: u16) -> u8 {
    self.rom.get(address as usize).copied().unwrap()
  }
}
