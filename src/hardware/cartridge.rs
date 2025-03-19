// A kind of cartridge.
#[derive(Debug)]
pub enum Cartridge {
  /// A game cartridge that only has 32kB of ROM and no RAM.
  RomOnly(RomOnly),
  /// A game cartridge with memory bank controller 1.
  Mbc1(Mbc1),
}

impl Cartridge {
  /// Reads the value specified by the address in ROM.
  pub fn read_rom(&self, address: u16) -> u8 {
    match self {
      Cartridge::RomOnly(cartridge) => cartridge.read_rom(address),
      Cartridge::Mbc1(cartridge) => cartridge.read_rom(address),
    }
  }

  /// Writes the value to the address in ROM.
  pub fn write_rom(&mut self, address: u16, value: u8) {
    match self {
      // This cartridge type does not have any ROM
      Cartridge::RomOnly(_) => {}
      Cartridge::Mbc1(cartridge) => cartridge.write_rom(address, value),
    }
  }

  /// Reads the value in RAM of the specified address in RAM.
  pub fn read_ram(&self, address: u16) -> u8 {
    match self {
      // This cartridge type does not have any RAM
      Cartridge::RomOnly(_) => 0xFF,
      Cartridge::Mbc1(cartridge) => cartridge.read_ram(address),
    }
  }

  /// Writes to the value to address in RAM.
  pub fn write_ram(&mut self, address: u16, value: u8) {
    match self {
      // No-op because this cartridge type has no RAM
      Cartridge::RomOnly(_) => {}
      Cartridge::Mbc1(cartridge) => cartridge.write_ram(address, value),
    }
  }
}

/// A cartridge with MBC1 controller.
#[derive(Debug)]
pub struct Mbc1 {
  rom: Vec<u8>,
  ram: Vec<u8>,
  rom_bank: usize,
  ram_bank: usize,
  ram_enabled: bool,
  banking_mode: BankingMode,
}

/// The possible banking modes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BankingMode {
  /// Address space 0x0000-0x3FFF and 0xA000-0xBFFF are locked to bank 0 & SRAM.
  Simple,
  /// Address space 0x0000-0x3FFF and 0xA000-0xBFFF can be bank switched.
  Advanced,
}

impl Mbc1 {
  pub fn new(rom: Vec<u8>) -> Self {
    Self {
      rom,
      ram: vec![0; 0x8000],
      rom_bank: 1,
      ram_bank: 0,
      ram_enabled: false,
      banking_mode: BankingMode::Simple,
    }
  }

  /// Reads an 8-bit value from the provided address in rom.
  pub fn read_rom(&self, address: u16) -> u8 {
    let bank = if address < 0x4000 { 0 } else { self.rom_bank };
    let offset = (address as usize) & (0x4000 - 1);

    self
      .rom
      .get(bank * 0x4000 + offset)
      .copied()
      .unwrap_or(0xFF)
  }

  /// "Writes" a value to ROM at the provided address.
  pub fn write_rom(&mut self, address: u16, value: u8) {
    if address < 0x2000 {
      self.ram_enabled = value & 0x0F == 0x0A;
    } else if address < 0x4000 {
      let bank = (value as usize & 0x1F).max(1);

      self.rom_bank = (self.rom_bank & 0x60) | bank;
    } else if address < 0x6000 {
      let bits = (value as usize) & 0x03;

      match self.banking_mode {
        BankingMode::Simple => self.rom_bank = (self.rom_bank & 0x1F) | (bits << 5),
        BankingMode::Advanced => self.ram_bank = bits,
      }
    } else if address < 0x8000 {
      self.banking_mode = if value & 0x01 == 0 {
        BankingMode::Advanced
      } else {
        BankingMode::Simple
      };
    }
  }

  /// Reads the 8-bit value at the provided address in RAM.
  pub fn read_ram(&self, address: u16) -> u8 {
    if self.ram_enabled {
      let offset = (address as usize) & (0x2000 - 1);

      self
        .ram
        .get(self.ram_bank * 0x2000 + offset)
        .copied()
        .unwrap_or(0xFF)
    } else {
      0xFF
    }
  }

  /// Writes the 8-bit to RAM at the provided address.
  pub fn write_ram(&mut self, address: u16, value: u8) {
    if !self.ram_enabled {
      return;
    }

    let offset = (address as usize) & (0x2000 - 1);

    if let Some(byte) = self.ram.get_mut(self.ram_bank * 0x2000 + offset) {
      *byte = value;
    }
  }
}

/// A cartridge that only has ROM.
#[derive(Debug)]
pub struct RomOnly {
  /// The ROM of the cartridge.
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
