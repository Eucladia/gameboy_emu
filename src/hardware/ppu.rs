use crate::interrupts::{Interrupt, Interrupts};

/// The pixel processing unit.
#[derive(Debug)]
pub struct Ppu {
  /// The working memory for the PPU.
  memory: [u8; VIDEO_RAM_SIZE as usize],
  /// The object attribute map.
  oam: [u8; OAM_SIZE as usize],

  /// The LCD byte that controls what gets shown on the screen.
  lcdc: u8,
  /// The status of the LCD.
  stat: u8,
  /// Scroll Y.
  scy: u8,
  /// Scroll X.
  scx: u8,
  /// LCDC Y-Coordinate aka the current scan line.
  ly: u8,
  /// LY Compare.
  lyc: u8,
  /// Background and Window palette.
  bgp: u8,
  /// Object palette 0.
  obp0: u8,
  /// Object palette 1.
  obp1: u8,
  /// Window Y position.
  wy: u8,
  /// Window X position.
  wx: u8,
  /// The last value set when executing a DMA transfer,
  dma: u8,
  /// Internal counter for tracking cycles.
  counter: usize,

  /// The mode that the PPU is in.
  mode: PpuMode,
}

/// The different modes the PPU can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum PpuMode {
  HBlank = 0,
  VBlank = 1,
  OamScan = 2,
  PixelTransfer = 3,
}

impl Ppu {
  pub fn new() -> Self {
    Self {
      lcdc: 0,
      stat: 0,
      scy: 0,
      scx: 0,
      ly: 0,
      lyc: 0,
      bgp: 0,
      obp0: 0,
      obp1: 0,
      wy: 0,
      wx: 0,

      dma: 0,

      counter: 0,
      mode: PpuMode::OamScan,

      memory: [0; VIDEO_RAM_SIZE as usize],
      oam: [0; OAM_SIZE as usize],
    }
  }

  /// Steps a cycle.
  pub fn step(&mut self, cycles: usize, interrupts: &mut Interrupts) {
    self.counter += cycles;

    match self.mode {
      // OAM scan lasts for 80 cycles
      PpuMode::OamScan => {
        if self.counter >= OAM_CYCLE_COUNT {
          self.counter -= OAM_CYCLE_COUNT;
          self.mode = PpuMode::PixelTransfer;
        }
      }
      // Pixel transfer lasts for 172 cycles
      PpuMode::PixelTransfer => {
        if self.counter >= PIXEL_TRANSFER_CYCLE_COUNT {
          self.counter -= OAM_CYCLE_COUNT;
          self.mode = PpuMode::HBlank;
        }
      }
      // HBlank last for 204 cycles
      PpuMode::HBlank => {
        if self.counter >= H_BLANK_CYCLE_COUNT {
          self.counter -= H_BLANK_CYCLE_COUNT;
          self.ly += 1;

          // Enter VBlank on the last line
          if self.ly == 144 {
            self.mode = PpuMode::VBlank;
            interrupts.request_interrupt(Interrupt::VBlank);
          } else {
            self.mode = PpuMode::OamScan;
          }
        }
      }
      // VBlank last for 456 cycles
      PpuMode::VBlank => {
        if self.counter >= V_BLANK_CYCLE_COUNT {
          self.counter -= V_BLANK_CYCLE_COUNT;
          self.ly += 1;

          // Check for the end of the VBlank
          if self.ly > 153 {
            self.ly = 0;
            self.mode = PpuMode::OamScan;
          }
        }
      }
    }

    // Bits 0 & 1 are the mode
    self.stat = (self.stat & 0b1111_1100) | (self.mode as u8);

    if self.ly == self.lyc {
      // Set the coincidence flag, which is bit 2
      self.stat |= 0b100;

      if self.stat & 0b0100_0000 == 1 {
        interrupts.request_interrupt(Interrupt::LCD);
      }
    } else {
      self.stat &= !0b100;
    }
  }

  /// Reads the value of the register referencing the address.
  pub fn read_register(&self, address: u16) -> u8 {
    match address {
      0xFF40 => self.lcdc,
      0xFF41 => self.stat,
      0xFF42 => self.scy,
      0xFF43 => self.scx,
      0xFF44 => self.ly,
      0xFF45 => self.lyc,
      0xFF46 => self.dma,
      0xFF47 => self.bgp,
      0xFF48 => self.obp0,
      0xFF49 => self.obp1,
      0xFF4A => self.wy,
      0xFF4B => self.wx,
      _ => unreachable!(),
    }
  }

  /// Writes the value of the register referencing the address.
  pub fn write_register(&mut self, address: u16, value: u8) {
    match address {
      0xFF40 => self.lcdc = value,
      0xFF41 => self.stat = value,
      0xFF42 => self.scy = value,
      0xFF43 => self.scx = value,
      // Writing to LY resets it
      0xFF44 => self.ly = 0,
      0xFF45 => self.lyc = value,
      0xFF46 => self.dma = value,
      0xFF47 => self.bgp = value,
      0xFF48 => self.obp0 = value,
      0xFF49 => self.obp1 = value,
      0xFF4A => self.wy = value,
      0xFF4B => self.wx = value,
      _ => unreachable!(),
    }
  }

  /// Reads the 8-bit value in memory at the provided address.
  pub fn read_ram(&self, address: u16) -> u8 {
    self.memory[address as usize]
  }

  /// Writes 8-bits of memory to the provided address.
  pub fn write_ram(&mut self, address: u16, value: u8) {
    self.memory[address as usize] = value;
  }

  /// Reads 8-bits of OAM memory at the provided address.
  pub fn read_oam(&self, address: u16) -> u8 {
    self.oam[address as usize]
  }

  /// Writes 8-bits of OAM memory to the provided address.
  pub fn write_oam(&mut self, address: u16, value: u8) {
    self.oam[address as usize] = value;
  }
}

/// The amount of memory available to the PPU.
const VIDEO_RAM_SIZE: u16 = 0x2000;
/// The amount of memory availabkle for the sprites
const OAM_SIZE: u16 = 0xA0;
/// The number of cycles it takes for OAM.
const OAM_CYCLE_COUNT: usize = 80;
/// The number of cycles it takes for a pixel transfer.
const PIXEL_TRANSFER_CYCLE_COUNT: usize = 172;
/// The number of cycles it takes for an HBlank.
const H_BLANK_CYCLE_COUNT: usize = 204;
/// The number of cycles it takes for a VBlank.
const V_BLANK_CYCLE_COUNT: usize = 456;
