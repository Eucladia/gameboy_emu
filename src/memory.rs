/// Memory for the Gameboy
pub struct Memory {
  data: Box<[u8; u16::MAX as usize + 1]>,
}

impl Memory {
  /// Creates a new block of memory.
  pub fn new() -> Self {
    Self {
      data: Box::new([0; u16::MAX as usize + 1]),
    }
  }

  /// Gets the byte at the following address.
  pub fn byte_at(&self, address: u16) -> u8 {
    *self.data.get(address as usize).unwrap()
  }

  /// Reads a word, in little endian format.
  pub fn word_at(&self, address: u16) -> u16 {
    let b1 = self.byte_at(address);
    let b2 = self.byte_at(address + 1);

    (b2 as u16) << 8 | b1 as u16
  }

  /// Writes the byte to the address in little endian format.
  pub fn write_byte(&mut self, address: u16, value: u8) {
    *self.data.get_mut(address as usize).unwrap() = value;
  }

  /// Writes the word to the address in little endian format.
  pub fn write_word(&mut self, address: u16, value: u16) {
    let lower = (value & 0xFF) as u8;
    let upper = (value >> 8) as u8;

    self.write_byte(address, lower);
    self.write_byte(address + 1, upper);
  }
}
