use crate::interrupts::{Interrupt, Interrupts};

/// The pixel processing unit.
#[derive(Debug)]
pub struct Ppu {
  /// The working memory for the PPU.
  memory: [u8; VIDEO_RAM_SIZE as usize],
  /// The object attribute map.
  oam: [u8; OAM_SIZE as usize],
  /// The frame buffer.
  buffer: [[u8; 160]; 144],

  /// The LCD byte that controls what gets shown on the screen.
  lcdc: u8,
  /// The status of the LCD.
  stat: u8,
  /// Scroll Y.
  scy: u8,
  /// Scroll X.
  scx: u8,
  /// LCDC Y-Coordinate aka the current scanline.
  ly: u8,
  /// The window's scanline. See https://gbdev.io/pandocs/Tile_Maps.html#window.
  window_line: u8,
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

  /// The last value set when executing a DMA transfer,
  pub dma: u8,
  /// The current DMA transfer.
  pub dma_transfer: Option<DmaTransfer>,
}

/// The state of a direct memory transfer.
#[derive(Debug, Clone)]
pub enum DmaTransfer {
  /// A DMA transfer was requested.
  Requested,
  /// A DMA transfer is going to begin.
  Starting,
  /// A DMA transfer is in progress.
  Transferring { current_pos: u8 },
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
      window_line: 0,
      lyc: 0,
      bgp: 0,
      obp0: 0,
      obp1: 0,
      wy: 0,
      wx: 0,
      mode: PpuMode::OamScan,
      counter: 0,

      dma: 0,
      dma_transfer: None,

      memory: [0; VIDEO_RAM_SIZE as usize],
      oam: [0; OAM_SIZE as usize],
      buffer: [[0; 160]; 144],
    }
  }

  /// Steps a cycle.
  pub fn step(&mut self, interrupts: &mut Interrupts, cycles: usize) {
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
          self.render_scanline();
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
      // Set the coincidence flag
      self.stat |= 0x4;

      if self.stat & 0b0100_0000 == 1 {
        interrupts.request_interrupt(Interrupt::LCD);
      }
    } else {
      // Unset the coincidence flag
      self.stat &= !0x4;
    }

    // Increment the window scanline only if it's visible
    if self.ly >= self.wy {
      self.window_line = self.window_line.wrapping_add(1);
    } else {
      self.window_line = 0;
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
      0xFF46 => {
        self.dma = value;

        if address == 0xFF46 {
          self.dma_transfer = Some(DmaTransfer::Requested);
        }
      }
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
    self.memory[(address - 0x8000) as usize]
  }

  /// Writes 8-bits of memory to the provided address.
  pub fn write_ram(&mut self, address: u16, value: u8) {
    self.memory[(address - 0x8000) as usize] = value;
  }

  /// Reads 8-bits of OAM memory at the provided address.
  pub fn read_oam(&self, address: u16) -> u8 {
    self.oam[(address - 0xFE00) as usize]
  }

  /// Writes 8-bits of OAM memory to the provided address.
  pub fn write_oam(&mut self, address: u16, value: u8) {
    self.oam[(address - 0xFE00) as usize] = value;
  }

  /// Renders a scanline
  fn render_scanline(&mut self) {
    // The MSB of `lcdc` determines whether there should be any output to the window.
    if self.lcdc & 0x80 == 0 {
      return;
    }

    let mut scanline = [0; 160];

    // The LSB determines whether the background should be drawn
    if self.lcdc & 0x01 == 1 {
      self.render_background(&mut scanline);
    }

    // The 5th bit determines whether the window should be drawn
    if self.lcdc & 0x20 == 1 {
      self.render_window(&mut scanline);
    }

    // The 2nd bit determines whether the sprites should be drawn
    if self.lcdc & 0x02 == 1 {
      self.render_sprites(&mut scanline);
    }

    self.buffer[self.ly as usize] = scanline;
  }

  fn render_background(&self, scanline: &mut [u8; 160]) {
    let base_tilemap = if self.lcdc & 0x08 == 0 {
      0x9800
    } else {
      0x9C00
    };

    let y = self.ly.wrapping_add(self.scy) as usize;
    let tile_row = (y / 8) * 32;

    for x in 0_u8..160 {
      let x_pos = x.wrapping_add(self.scx) as usize;
      let tile_col = x_pos / 8;
      let tile_index = self.memory[base_tilemap - 0x8000 + tile_row + tile_col] as usize;

      let tile_addr = 0x8000 + (tile_index * 16);
      let row_offset = (y % 8) * 2;

      let low = self.memory[tile_addr - 0x8000 + row_offset];
      let high = self.memory[tile_addr - 0x8000 + row_offset + 1];

      let bit = 7 - (x_pos % 8);
      let color = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);

      scanline[x as usize] = color;
    }
  }

  fn render_window(&self, scanline: &mut [u8; 160]) {
    // Don't draw if its out of the bounds of the current scanline
    if self.ly < self.wy {
      return;
    }

    let base_tilemap = if self.lcdc & 0x40 == 0 {
      0x9800
    } else {
      0x9C00
    };

    // The window is drawn starting from `WX - 7`
    let window_x_start = self.wx.saturating_sub(7);
    let window_y = self.window_line;
    // Tiles are arranged in rows of 32 for the window.
    let tile_row_offset = (window_y as usize / 8) * 32;

    for screen_x in window_x_start..160 {
      let window_x = screen_x - window_x_start;
      let tile_col = (window_x as usize) / 8;
      let tilemap_index = (base_tilemap - 0x8000) + tile_row_offset + tile_col;
      let tile_index = self.memory[tilemap_index] as usize;

      // Each tile takes 16 bytes (8x8 pixels, 2 bytes per row)
      let tile_addr = 0x8000 + (tile_index * 16);

      let row_offset = (window_y as usize % 8) * 2;

      let low = self.memory[tile_addr - 0x8000 + row_offset];
      let high = self.memory[tile_addr - 0x8000 + row_offset + 1];

      let bit = 7 - (window_x % 8);
      let color = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);

      scanline[screen_x as usize] = color;
    }
  }

  fn render_sprites(&self, scanline: &mut [u8; 160]) {
    // TODO: Handle sprite attributes
    // TODO: We're supposed to only handle 10 sprite objects. Invisible objects due to an
    // x-coordinate being off-screen still counts! Only out of bound y-coordinate sprite
    // objects don't count.
    for i in 0..40 {
      let sprite_index = i * 4;
      let y = self.oam[sprite_index] as i16 - 16;
      let x = self.oam[sprite_index + 1] as i16 - 8;
      let tile_index = self.oam[sprite_index + 2] as usize;

      if self.ly < y as u8 || self.ly >= y as u8 + 8 {
        continue;
      }

      let tile_addr = 0x8000 + (tile_index * 16);
      let row_offset = ((self.ly - y as u8) * 2) as usize;
      let low = self.memory[tile_addr - 0x8000 + row_offset];
      let high = self.memory[tile_addr - 0x8000 + row_offset + 1];

      for x_offset in 0..8 {
        let bit = 7 - x_offset;
        let color = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);

        if color == 0 {
          continue;
        }

        let screen_x = x + x_offset as i16;
        if screen_x < 0 || screen_x >= 160 {
          continue;
        }

        scanline[screen_x as usize] = color;
      }
    }
  }
}

/// The amount of memory available to the PPU.
const VIDEO_RAM_SIZE: u16 = 0x2000;
/// The amount of memory availabkle for the sprites.
const OAM_SIZE: u16 = 0xA0;
/// The number of cycles it takes for OAM.
const OAM_CYCLE_COUNT: usize = 80;
/// The number of cycles it takes for a pixel transfer.
const PIXEL_TRANSFER_CYCLE_COUNT: usize = 172;
/// The number of cycles it takes for an HBlank.
const H_BLANK_CYCLE_COUNT: usize = 204;
/// The number of cycles it takes for a VBlank.
const V_BLANK_CYCLE_COUNT: usize = 456;
