use crate::interrupts::{Interrupt, Interrupts};

/// The pixel processing unit.
pub struct Ppu {
  /// The LCD byte that controls what gets shown on the screen.
  lcdc: u8,
  /// The status of the LCD.
  stat: u8,
  /// Scroll Y.
  scy: u8,
  /// Scroll X.
  scx: u8,
  /// LCDC Y-Coordinate, which is the current scan line.
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
  /// The mode that the PPU is in.
  mode: PpuMode,
  /// Internal counter for tracking cycles.
  counter: usize,
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
      counter: 0,
      mode: PpuMode::OamScan,
    }
  }

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
}

/// The number of cycles it takes for OAM.
const OAM_CYCLE_COUNT: usize = 80;
/// The number of cycles it takes for a pixel transfer.
const PIXEL_TRANSFER_CYCLE_COUNT: usize = 172;
/// The number of cycles it takes for an HBlank.
const H_BLANK_CYCLE_COUNT: usize = 204;
/// The number of cycles it takes for a VBlank.
const V_BLANK_CYCLE_COUNT: usize = 456;
