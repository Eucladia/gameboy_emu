mod cartridge_header;
mod emulator;
mod error;
mod flags;
mod hardware;
mod instructions;

use cartridge_header::CartridgeHeader;
pub use error::*;

use std::fs;

fn main() {
  let rom = fs::read("./roms/Tetris.gb").unwrap();
  let header = CartridgeHeader::new(&rom).unwrap();

  dbg!(&header);

  // let memory = Memory::new(file);
  // let cpu = Cpu::new(memory);
}
