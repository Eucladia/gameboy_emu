mod cartridge_header;
mod cpu;
mod emulator;
mod error;
mod flags;
mod instructions;
mod memory;
mod registers;

use cartridge_header::CartridgeHeader;
pub use error::*;

use cpu::Cpu;
use std::fs;

fn main() {
  let rom = fs::read("./roms/Tetris.gb").unwrap();
  let header = CartridgeHeader::new(&rom).unwrap();

  dbg!(&header);

  // let memory = Memory::new(file);
  // let cpu = Cpu::new(memory);
}
