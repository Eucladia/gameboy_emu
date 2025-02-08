use crate::memory_bank_controller::{MemoryBankController, MemoryBankController0};

const CARTRIDGE_TYPE: u16 = 0x147;

/// The MMU for the Gameboy.
pub struct Memory {
  /// The memory bank controller this cartridge uses.
  mbc: MemoryBankController,
}

impl Memory {
  /// Creates a new memory manager.
  pub fn new(rom: Vec<u8>) -> Self {
    let mbc = match rom[CARTRIDGE_TYPE as usize] {
      0 => MemoryBankController::Zero(MemoryBankController0::new(rom)),
      b => panic!("got invalid memory cartridge type: {b:02X}"),
    };

    Self { mbc }
  }
}
