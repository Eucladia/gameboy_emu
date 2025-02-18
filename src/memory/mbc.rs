use crate::memory::Mbc0;

/// The kind of memory bank controller.
#[derive(Debug)]
pub enum Cartridge {
  Zero(Mbc0),
}

impl Cartridge {
  pub fn read_rom(&self, address: u16) -> u8 {
    match self {
      Cartridge::Zero(mbc) => mbc.read_rom(address),
    }
  }

  pub fn write_rom(&mut self, address: u16, value: u8) {
    match self {
      Cartridge::Zero(mbc) => mbc.write_rom(address, value),
    }
  }
}
