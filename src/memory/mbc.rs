use crate::memory::Mbc0;

/// The kind of memory bank controller.
#[derive(Debug)]
pub enum MemoryBankController {
  Zero(Mbc0),
}

impl MemoryBankController {
  pub fn read_rom(&self, address: u16) -> u8 {
    match self {
      MemoryBankController::Zero(mbc) => mbc.read_rom(address),
    }
  }

  pub fn write_rom(&mut self, address: u16, value: u8) {
    match self {
      MemoryBankController::Zero(mbc) => mbc.write_rom(address, value),
    }
  }
}
