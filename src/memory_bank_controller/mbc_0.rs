/// A memory bank controller with no external ram.
pub struct MemoryBankController0 {
  rom: Vec<u8>,
}

impl MemoryBankController0 {
  pub fn new(rom: Vec<u8>) -> Self {
    Self { rom }
  }

  /// Reads from the rom.
  pub fn read_from(&self, address: u16) -> u8 {
    self.rom[address as usize]
  }
}
