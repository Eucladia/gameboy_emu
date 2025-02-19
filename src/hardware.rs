use crate::addresses;
use crate::joypad::Joypad;
use crate::memory::{Cartridge, Mbc0};

const VIDEO_RAM_SIZE: u16 = addresses::VIDEO_RAM_END - addresses::VIDEO_RAM_START;
const WORK_RAM_SIZE: u16 = addresses::WORK_RAM_END - addresses::WORK_RAM_START;
const OAM_SIZE: u16 = addresses::OAM_END - addresses::OAM_START;
const IO_REGISTER_SIZE: u16 = addresses::IO_REGISTER_END - addresses::IO_REGISTER_START;
const HIGH_RAM_SIZE: u16 = addresses::HIGH_RAM_END - addresses::HIGH_RAM_START;
const INTERRUPT_ENABLE_REGISTER_SIZE: u16 = 1;

const MEMORY_SIZE: u16 = VIDEO_RAM_SIZE
  + WORK_RAM_SIZE
  + OAM_SIZE
  // TODO: IO stuff should be separate
  + IO_REGISTER_SIZE
  + HIGH_RAM_SIZE
  + INTERRUPT_ENABLE_REGISTER_SIZE;

const VIDEO_RAM_OFFSET: u16 = 0;
const WORK_RAM_OFFSET: u16 = VIDEO_RAM_OFFSET + VIDEO_RAM_SIZE;
const OAM_OFFSET: u16 = WORK_RAM_OFFSET + WORK_RAM_SIZE;
const IO_REGISTER_OFFSET: u16 = OAM_OFFSET + OAM_SIZE;
const HIGH_RAM_OFFSET: u16 = IO_REGISTER_OFFSET + IO_REGISTER_SIZE;
const INTERRUPT_ENABLE_OFFSET: u16 = HIGH_RAM_OFFSET + HIGH_RAM_SIZE;

const JOYPAD_REGISTER: u16 = 0xFF00;
const CARTRIDGE_TYPE: u16 = 0x147;

/// The types of interrupts.
#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Interrupt {
  VBlank = 1 << 0,
  LCD = 1 << 1,
  Timer = 1 << 2,
  Serial = 1 << 3,
  Joypad = 1 << 4,
}

#[derive(Debug)]
pub struct Hardware {
  /// The internal memory available.
  memory: [u8; MEMORY_SIZE as usize],
  /// The input joypad.
  joypad: Joypad,
  /// The game cartridge.
  cartridge: Cartridge,
}

impl Hardware {
  pub fn new(bytes: Vec<u8>) -> Self {
    let cartridge = match bytes[CARTRIDGE_TYPE as usize] {
      0 => Cartridge::Zero(Mbc0::new(bytes)),
      b => panic!("got invalid memory cartridge type: {b:02X}"),
    };

    Self {
      memory: [0; MEMORY_SIZE as usize],
      joypad: Joypad::new(),
      cartridge,
    }
  }

  /// Reads 8 bits of memory from the given address.
  pub fn read_byte(&self, address: u16) -> u8 {
    match address {
      // ROM
      addresses::ROM_BANK_0_START..addresses::ROM_BANK_0_END => self.cartridge.read_rom(address),
      // ROM, bank N
      addresses::ROM_BANK_N_START..addresses::ROM_BANK_N_END => self.cartridge.read_rom(address),
      // Video RAM
      addresses::VIDEO_RAM_START..addresses::VIDEO_RAM_END => {
        self.memory[(VIDEO_RAM_OFFSET + (address - addresses::VIDEO_RAM_START)) as usize]
      }
      // External RAM
      addresses::EXTERNAL_RAM_START..addresses::EXTERNAL_RAM_END => {
        self.cartridge.read_ram(address)
      }
      // Work RAM
      addresses::WORK_RAM_START..addresses::WORK_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - addresses::WORK_RAM_START)) as usize]
      }
      // Echo RAM
      addresses::ECHO_RAM_START..addresses::ECHO_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - addresses::ECHO_RAM_START)) as usize]
      }
      // Sprite memory
      addresses::OAM_START..addresses::OAM_END => {
        self.memory[(OAM_OFFSET + (address - addresses::OAM_START)) as usize]
      }
      // Unused
      addresses::UNUSED_START..addresses::UNUSED_END => 0xFF,
      // I/O Registers
      addresses::IO_REGISTER_START..addresses::IO_REGISTER_END => self.read_io_register(address),
      // High RAM
      addresses::HIGH_RAM_START..addresses::HIGH_RAM_END => {
        self.memory[(HIGH_RAM_OFFSET + (address - addresses::HIGH_RAM_START)) as usize]
      }
      // Interrupt enable register
      addresses::INTERRUPT_ENABLE_REGISTER => self.memory[INTERRUPT_ENABLE_OFFSET as usize],
    }
  }

  fn read_io_register(&self, address: u16) -> u8 {
    match address {
      JOYPAD_REGISTER => {
        let joypad_state = self.memory[(IO_REGISTER_OFFSET + (address - JOYPAD_REGISTER)) as usize];

        self.joypad.read(joypad_state)
      }
      _ => todo!("Other registers"),
    }
  }

  /// Reads 16-bits in memory, in little endian, from the given address.
  pub fn read_word(&self, address: u16) -> u16 {
    let lower = self.read_byte(address) as u16;
    let upper = self.read_byte(address + 1) as u16;

    (upper << 8) | lower
  }

  /// Writes 8-bits to memory at the specified address.
  pub fn write_byte(&mut self, address: u16, value: u8) {
    match address {
      // ROM
      addresses::ROM_BANK_0_START..addresses::ROM_BANK_0_END => {
        self.cartridge.write_rom(address, value)
      }
      // Switchable ROM bank
      addresses::ROM_BANK_N_START..addresses::ROM_BANK_N_END => {
        self.cartridge.write_rom(address, value)
      }
      // Video RAM
      addresses::VIDEO_RAM_START..addresses::VIDEO_RAM_END => {
        self.memory[(VIDEO_RAM_OFFSET + (address - addresses::VIDEO_RAM_START)) as usize] = value
      }
      // External RAM
      addresses::EXTERNAL_RAM_START..addresses::EXTERNAL_RAM_END => {
        self.cartridge.write_ram(address, value)
      }
      // Work RAM
      addresses::WORK_RAM_START..addresses::WORK_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - addresses::WORK_RAM_START)) as usize] = value
      }
      // Echo RAM
      addresses::ECHO_RAM_START..addresses::ECHO_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - addresses::ECHO_RAM_START)) as usize] = value
      }
      // Sprite memory
      addresses::OAM_START..addresses::OAM_END => {
        self.memory[(OAM_OFFSET + (address - addresses::OAM_START)) as usize] = value
      }
      // Unused
      addresses::UNUSED_START..addresses::UNUSED_END => {}
      // I/O Registers
      addresses::IO_REGISTER_START..addresses::IO_REGISTER_END => {
        self.memory[(IO_REGISTER_OFFSET + (address - addresses::IO_REGISTER_START)) as usize] =
          value
      }
      // High RAM
      addresses::HIGH_RAM_START..addresses::HIGH_RAM_END => {
        self.memory[(HIGH_RAM_OFFSET + (address - addresses::HIGH_RAM_START)) as usize] = value
      }
      // Interrupt register
      addresses::INTERRUPT_ENABLE_REGISTER => self.memory[INTERRUPT_ENABLE_OFFSET as usize] = value,
    }
  }
}
