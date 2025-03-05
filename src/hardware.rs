pub mod cartridge;
pub mod cpu;
pub mod joypad;
pub mod ppu;
pub mod registers;
pub mod timer;

pub use cpu::Cpu;
pub use joypad::Joypad;
pub use timer::Timer;

use crate::{
  hardware::cartridge::{Cartridge, RomOnly},
  hardware::ppu::{DmaTransfer, Ppu},
  interrupts::Interrupts,
};

#[derive(Debug)]
pub struct Hardware {
  /// The internal memory available.
  memory: [u8; MEMORY_SIZE as usize],
  /// The high ram.
  high_ram: [u8; HIGH_RAM_SIZE as usize],
  /// The input joypad.
  joypad: Joypad,
  /// The game cartridge.
  cartridge: Cartridge,
  /// The timer.
  timer: Timer,
  /// The pixel processing unit.
  ppu: Ppu,
  /// The set interrupts
  interrupts: Interrupts,
}

impl Hardware {
  pub fn new(bytes: Vec<u8>) -> Self {
    let cartridge = match bytes[CARTRIDGE_TYPE as usize] {
      0 => Cartridge::RomOnly(RomOnly::new(bytes)),
      b => panic!("got invalid memory cartridge type: {b:02X}"),
    };

    Self {
      memory: [0; MEMORY_SIZE as usize],
      high_ram: [0; HIGH_RAM_SIZE as usize],
      joypad: Joypad::new(),
      timer: Timer::new(),
      ppu: Ppu::new(),
      interrupts: Interrupts::new(),
      cartridge,
    }
  }

  /// Reads 8 bits of memory from the given address.
  pub fn read_byte(&self, address: u16) -> u8 {
    match address {
      // ROM
      ROM_BANK_0_START..ROM_BANK_0_END => self.cartridge.read_rom(address),
      // ROM, bank N
      ROM_BANK_N_START..ROM_BANK_N_END => self.cartridge.read_rom(address),
      // Video RAM
      VIDEO_RAM_START..VIDEO_RAM_END => self.ppu.read_ram(address),
      // External RAM
      EXTERNAL_RAM_START..EXTERNAL_RAM_END => self.cartridge.read_ram(address),
      // Work RAM
      WORK_RAM_START..WORK_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - WORK_RAM_START)) as usize]
      }
      // Echo RAM
      ECHO_RAM_START..ECHO_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - ECHO_RAM_START)) as usize]
      }
      // Sprite memory
      OAM_START..OAM_END => self.ppu.read_oam(address),
      // Unused
      UNUSED_START..UNUSED_END => 0xFF,
      // I/O Registers
      IO_REGISTER_START..IO_REGISTER_END => self.read_io_register(address),
      // High RAM
      HIGH_RAM_START..HIGH_RAM_END => self.high_ram[(address - HIGH_RAM_START) as usize],
      // Interrupt enable register
      INTERRUPT_ENABLE_REGISTER => self.interrupts.enabled_bitfield(),
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
      ROM_BANK_0_START..ROM_BANK_0_END => self.cartridge.write_rom(address, value),
      // Switchable ROM bank
      ROM_BANK_N_START..ROM_BANK_N_END => self.cartridge.write_rom(address, value),
      // Video RAM
      VIDEO_RAM_START..VIDEO_RAM_END => self.ppu.write_ram(address, value),
      // External RAM
      EXTERNAL_RAM_START..EXTERNAL_RAM_END => self.cartridge.write_ram(address, value),
      // Work RAM
      WORK_RAM_START..WORK_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - WORK_RAM_START)) as usize] = value
      }
      // Echo RAM
      ECHO_RAM_START..ECHO_RAM_END => {
        self.memory[(WORK_RAM_OFFSET + (address - ECHO_RAM_START)) as usize] = value
      }
      // Sprite memory
      OAM_START..OAM_END => self.ppu.write_oam(address, value),
      // Unused
      UNUSED_START..UNUSED_END => {}
      // I/O Registers
      IO_REGISTER_START..IO_REGISTER_END => self.write_io_register(address, value),
      // High RAM
      HIGH_RAM_START..HIGH_RAM_END => self.high_ram[(address - HIGH_RAM_START) as usize] = value,
      // Interrupt enable register
      INTERRUPT_ENABLE_REGISTER => self.interrupts.set_enabled(value),
    }
  }

  pub fn update_dma_transfer(&mut self, cycles: usize) {
    match self.ppu.dma_transfer {
      Some(DmaTransfer::Requested) => {
        // There's a delay of 1 M-cycle when executing DMA, so we have a filler state
        self.ppu.dma_transfer = Some(DmaTransfer::Starting);
      }
      Some(DmaTransfer::Starting) => {
        self.ppu.dma_transfer = Some(DmaTransfer::Transferring { current_pos: 0 });
      }
      Some(DmaTransfer::Transferring { current_pos }) => {
        let mut index = current_pos as u16;
        let starting_address = (self.ppu.dma as u16) << 8;
        let ending_address = starting_address + 160;
        let remaining_bytes = ending_address - index;
        let iterations = (cycles as u16 / 4).min(remaining_bytes);

        for _ in 0..iterations {
          let src = self.read_byte(starting_address + index);

          self.write_byte(0xFE00 + index, src);

          index += 1;
        }

        if index >= 160 {
          self.ppu.dma_transfer = None;
        } else {
          self.ppu.dma_transfer = Some(DmaTransfer::Transferring {
            current_pos: index as u8,
          })
        }
      }
      None => {}
    }
  }

  /// Gets the active DMA transfer.
  pub fn get_dma_transfer(&self) -> Option<&DmaTransfer> {
    self.ppu.dma_transfer.as_ref()
  }

  fn read_io_register(&self, address: u16) -> u8 {
    match address {
      JOYPAD_REGISTER => self.joypad.read_register(),
      TIMER_REGISTER_START..TIMER_REGISTER_END => self.timer.read_register(address),
      PPU_REGISTER_START..PPU_REGISTER_END => self.ppu.read_register(address),
      INTERRUPT_FLAG => self.interrupts.requested_bitfield(),
      _ => todo!("Other registers"),
    }
  }

  fn write_io_register(&mut self, address: u16, value: u8) {
    match address {
      JOYPAD_REGISTER => self.joypad.write_register(value),
      TIMER_REGISTER_START..TIMER_REGISTER_END => self.timer.write_register(address, value),
      PPU_REGISTER_START..PPU_REGISTER_END => self.ppu.write_register(address, value),
      INTERRUPT_FLAG => self.interrupts.set_requested(value),
      _ => todo!("Other registers"),
    }
  }
}

/// The address of the joypad register.
const JOYPAD_REGISTER: u16 = 0xFF00;
/// The address of the interrupt flag.
const INTERRUPT_FLAG: u16 = 0xFF0F;

/// The starting address for ROM bank 0.
const ROM_BANK_0_START: u16 = 0;
/// The ending address for ROM bank 0.
const ROM_BANK_0_END: u16 = 0x4000;
/// The starting address for the switchable ROM bank.
const ROM_BANK_N_START: u16 = 0x4000;
/// The ending address  for the switchable ROM bank.
const ROM_BANK_N_END: u16 = 0x8000;
/// The starting address for VRAM.
const VIDEO_RAM_START: u16 = 0x8000;
/// The ending address  for VRAM.
const VIDEO_RAM_END: u16 = 0xA000;
/// The starting address for the cartridges RAM.
const EXTERNAL_RAM_START: u16 = 0xA000;
/// The ending address for the cartridges  RAM.
const EXTERNAL_RAM_END: u16 = 0xC000;
/// The starting address for work RAM.
const WORK_RAM_START: u16 = 0xC000;
/// The ending address for work  RAM.
const WORK_RAM_END: u16 = 0xE000;
/// The starting address for echo RAM.
const ECHO_RAM_START: u16 = 0xE000;
/// The ending address for echo RAM.
const ECHO_RAM_END: u16 = 0xFE00;
/// The starting address for the OAM (sprite attribute memory).
const OAM_START: u16 = 0xFE00;
/// The ending address for the OAM (sprite attribute memory).
const OAM_END: u16 = 0xFEA0;
/// The starting address for unused memory.
const UNUSED_START: u16 = 0xFEA0;
/// The ending address for unused memory.
const UNUSED_END: u16 = 0xFF00;
/// The starting address for I/O registers.
const IO_REGISTER_START: u16 = 0xFF00;
/// The ending address for I/O registers.
const IO_REGISTER_END: u16 = 0xFF80;
/// The starting address for HRAM.
const HIGH_RAM_START: u16 = 0xFF80;
/// The ending address for HRAM.
const HIGH_RAM_END: u16 = 0xFFFF;
/// The interrupt enable register.
const INTERRUPT_ENABLE_REGISTER: u16 = 0xFFFF;

/// The starting address of the timer register.
const TIMER_REGISTER_START: u16 = 0xFF04;
/// The ending address fo the timer register.
const TIMER_REGISTER_END: u16 = 0xFF08;
/// The starting address of the PPU register.
const PPU_REGISTER_START: u16 = 0xFF40;
/// The ending address of the PPU register.
const PPU_REGISTER_END: u16 = 0xFF4B;

const VIDEO_RAM_SIZE: u16 = VIDEO_RAM_END - VIDEO_RAM_START;
const WORK_RAM_SIZE: u16 = WORK_RAM_END - WORK_RAM_START;
const OAM_SIZE: u16 = OAM_END - OAM_START;
const HIGH_RAM_SIZE: u16 = HIGH_RAM_END - HIGH_RAM_START;
const INTERRUPT_ENABLE_REGISTER_SIZE: u16 = 1;

const MEMORY_SIZE: u16 = WORK_RAM_SIZE + HIGH_RAM_SIZE;

const VIDEO_RAM_OFFSET: u16 = 0;
const WORK_RAM_OFFSET: u16 = VIDEO_RAM_OFFSET + VIDEO_RAM_SIZE;
const OAM_OFFSET: u16 = WORK_RAM_OFFSET + WORK_RAM_SIZE;
const HIGH_RAM_OFFSET: u16 = OAM_OFFSET + OAM_SIZE;
const INTERRUPT_ENABLE_OFFSET: u16 = HIGH_RAM_OFFSET + HIGH_RAM_SIZE;

const CARTRIDGE_TYPE: u16 = 0x147;
