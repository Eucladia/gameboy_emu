/// A memory bank controller with no external ram.
#[derive(Debug)]
pub struct Mbc0 {
  rom: Vec<u8>,
}

impl Mbc0 {
  pub fn new(rom: Vec<u8>) -> Self {
    Self { rom }
  }

  /// Reads from the ROM.
  pub fn read_rom(&self, address: u16) -> u8 {
    // Return 0xFF for unmapped memory
    self.rom.get(address as usize).copied().unwrap_or(0xFF)
  }

  /// Writes the value to the ROM.
  pub fn write_rom(&mut self, _address: u16, _value: u8) {
    // No op since this controller doesn't write back to ROM
  }
}
