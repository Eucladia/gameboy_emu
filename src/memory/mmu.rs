use crate::memory::{Mbc0, MemoryBankController};

const CARTRIDGE_TYPE: u16 = 0x147;

/// The memory management unit for the Gameboy.
#[derive(Debug)]
pub struct Mmu {
  /// The available RAM .
  ram: Vec<u8>,
  /// The available ROM.
  rom: Vec<u8>,
  /// The memory bank controller this cartridge uses.
  mbc: MemoryBankController,
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
      ram: Vec::new(),
      rom: Vec::new(),
    }
  }
}
