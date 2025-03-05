use crate::{
  interrupts::{Interrupt, Interrupts},
  is_flag_set,
};

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

/// Attributes that sprites can have.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum SpriteAttributes {
  /// The color palette for the sprite (DMG).
  DmgPalette = 1 << 4,
  /// Whether the sprite should be flipped vertically.
  XFlip = 1 << 5,
  /// Whether the sprite should be flipped horizontally.
  YFlip = 1 << 6,
  /// The priority of this sprite. 0 indicates lower priority over the
  /// background and window colors.
  Priority = 1 << 7,
}

/// The LCD control byte.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
enum LcdControl {
  /// Whether the background should be displayed.
  BackgroundDisplay = 1 << 0,
  /// Whether the sprites should be displayed.
  SpriteDisplay = 1 << 1,
  /// The dimensions of the sprite. 0 indicates 8x8 and 1 indicates 8x16.
  SpriteDimensions = 1 << 2,
  /// The background tile map. 0 indicates address space [0x9800, 0x9C00) and 1 indicates
  /// [0x9C00, 0xA000).
  BackgroundTileMap = 1 << 3,
  /// The background tile data. 0 indicates address space [0x8800, 0x9800) and 1 indicates
  /// [0x8000, 0x9000).
  BackgroundTileData = 1 << 4,
  /// Whether the window should be displayed.
  WindowDisplay = 1 << 5,
  /// The window tile map map. 0 indicates address space [0x9800, 0x9C00) and 1 indicates
  /// [0x9C00, 0xA000).
  WindowTileMap = 1 << 6,
  /// Whether the LCD should be on.
  LcdDisplay = 1 << 7,
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
          self.counter -= PIXEL_TRANSFER_CYCLE_COUNT;
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
        self.dma_transfer = Some(DmaTransfer::Requested);
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

  /// Fetches a pixel from the given tile index, row (0-7 inclusive),
  /// and X-coordinate (0-7 inclusive).
  fn get_tile_pixel(&self, tile_index: u8, row: u8, x: u8) -> u8 {
    let (base_addr, tile_index) = if is_flag_set!(self.lcdc, LcdControl::BackgroundTileData as u8) {
      (0x8000, tile_index)
    } else {
      // Tile indices in this region are signed, so we need to offset them accordingly
      //
      // Tile indices [0, 128) reference sprites starting from 0x9000 and
      // tile indices [128, 255) reference sprites starting from 0x8800
      //
      // See https://gbdev.io/pandocs/Tile_Data.html#vram-tile-data for more info.
      if tile_index < 128 {
        (0x9000, tile_index)
      } else {
        (0x8800, (tile_index - 128))
      }
    };

    // Tiles are stored in 16 bytes
    let tile_offset = tile_index as u16 * 16;
    let row_offset = row as u16 * 2;
    let lower = self.read_ram(base_addr + tile_offset + row_offset);
    let upper = self.read_ram(base_addr + tile_offset + row_offset + 1);
    let bit = 7 - (x % 8);

    ((upper >> bit) & 1) << 1 | ((lower >> bit) & 1)
  }

  /// Fetches a sprite's pixel from the given tile index, row (0, 7 inclusive),
  /// and X-coordinate (0-7 inclusive).
  fn get_sprite_pixel(&self, tile_index: u8, row: u8, x: u8, flip_x: bool) -> u8 {
    // Sprites are stored in 16 bytes
    let tile_offset = tile_index as u16 * 16;
    let row_offset = row as u16 * 2;
    let lower = self.read_ram(0x8000 + tile_offset + row_offset);
    let upper = self.read_ram(0x8000 + tile_offset + row_offset + 1);
    let bit = if flip_x { x } else { 7 - x };

    (((upper >> bit) & 1) << 1) | ((lower >> bit) & 1)
  }

  /// Renders a complete scanline into the frame buffer.
  fn render_scanline(&mut self) {
    // Only render if the LCD is enabled.
    if !is_flag_set!(self.lcdc, LcdControl::LcdDisplay as u8) {
      return;
    }

    let mut scanline = [0; 160];

    // Render background if enabled
    if is_flag_set!(self.lcdc, LcdControl::BackgroundDisplay as u8) {
      self.render_background(&mut scanline);
    }

    // Render window if enabled
    if is_flag_set!(self.lcdc, LcdControl::WindowDisplay as u8) {
      self.render_window(&mut scanline);
    }

    // Render sprites if enabled
    if is_flag_set!(self.lcdc, LcdControl::SpriteDisplay as u8) {
      self.render_sprites(&mut scanline);
    }

    self.buffer[self.ly as usize] = scanline;
  }

  /// Renders the background into the provided scanline.
  fn render_background(&self, scanline: &mut [u8; 160]) {
    let bg_tile_map = if is_flag_set!(self.lcdc, LcdControl::BackgroundTileMap as u8) {
      0x9C00
    } else {
      0x9800
    };

    let y = (self.ly as u16).wrapping_add(self.scy as u16);
    // Background tile map have 32 tiles per row
    let tile_row = (y / 8) * 32;

    for (x, pixel) in scanline.iter_mut().enumerate() {
      let x_pos = (x as u16).wrapping_add(self.scx as u16);
      let tile_col = x_pos / 8;
      let tile_index = self.read_ram(bg_tile_map + tile_row + tile_col);

      *pixel = self.get_tile_pixel(tile_index, (y % 8) as u8, (x_pos % 8) as u8);
    }
  }

  /// Renders the window into the scanline.
  fn render_window(&self, scanline: &mut [u8; 160]) {
    // The window is only drawn on scanlines at or below the window Y-position
    if self.ly < self.wy {
      return;
    }

    let window_tile_map = if is_flag_set!(self.lcdc, LcdControl::WindowTileMap as u8) {
      0x9C00
    } else {
      0x9800
    };

    // Offset by -7 because thats where the window starts
    let window_x_start = self.wx.saturating_sub(7);
    let window_y = self.window_line as u16;
    // Window tile map have 32 tiles per row
    let tile_row = (window_y / 8) * 32;

    // Render window pixels starting at window_x_start.
    for screen_x in window_x_start..160 {
      let window_x = (screen_x - window_x_start) as u16;
      let tile_col = window_x / 8;
      let tile_index = self.read_ram(window_tile_map + tile_row + tile_col);

      scanline[screen_x as usize] =
        self.get_tile_pixel(tile_index, (window_y % 8) as u8, (window_x % 8) as u8);
    }
  }

  /// Renders sprites into the scanline.
  fn render_sprites(&self, scanline: &mut [u8; 160]) {
    // The Gameboy can only draw 10 visible sprites per scanline
    let mut sprites_drawn = 0;
    // Bit 2 determines the sprite's height
    let sprite_height = if is_flag_set!(self.lcdc, LcdControl::SpriteDimensions as u8) {
      16
    } else {
      8
    };

    for chunk in self.oam.chunks_exact(4) {
      if sprites_drawn >= 10 {
        break;
      }

      // NOTE: We have to do this because `array_chunks` isn't stable. It's a bit ugly,
      // but its looks better than iterating over the indices and offset into the OAM
      let (raw_y, raw_x, tile_index, attributes) = match chunk {
        &[a, b, c, d] => (a, b, c, d),
        _ => unreachable!(),
      };

      // A Y-coordinate of 16 means the sprite is fully visible, so offset it by -16
      let sprite_y = raw_y as i16 - 16;

      // Ignore invisible sprites (those not within the bounds of the screen or not on
      // the current scanline) and don't count it towards the sprite limit.
      if raw_y == 0
        || raw_y >= 160
        || (self.ly as i16) < sprite_y
        || (self.ly as i16) >= sprite_y + sprite_height
      {
        continue;
      }

      sprites_drawn += 1;

      // A X-coordinate of 8 means the sprite is fully visible, so offset it by -8
      let sprite_x = raw_x as i16 - 8;

      // Get the row where the sprite should be drawn
      let row = {
        let line = self.ly as i16 - sprite_y;

        if is_flag_set!(attributes, SpriteAttributes::YFlip as u8) {
          (sprite_height - 1 - line) as u8
        } else {
          line as u8
        }
      };

      let (tile_to_use, tile_row) = if sprite_height == 16 {
        // For 8×16 sprites, we need to clear the LSB of the top tile
        // and set it for the bottom tile
        let cleared_tile = tile_index & 0xFE;

        if row < 8 {
          (cleared_tile, row)
        } else {
          (cleared_tile | 0x01, row - 8)
        }
      } else {
        (tile_index, row)
      };

      // Render the 8 pixels in each tile
      for x_offset in 0..8 {
        let screen_x = sprite_x + x_offset as i16;

        // Don't draw sprites that are off the screen
        if screen_x < 0 || screen_x >= 160 {
          continue;
        }

        let pixel = &mut scanline[screen_x as usize];

        // Don't draw over the background if the sprite has lower priority.
        if is_flag_set!(attributes, SpriteAttributes::Priority as u8) && *pixel != 0 {
          continue;
        }

        let flip_x = is_flag_set!(attributes, SpriteAttributes::XFlip as u8);
        let color = self.get_sprite_pixel(tile_to_use, tile_row, x_offset, flip_x);

        if color == 0 {
          continue;
        }

        let palette = if is_flag_set!(attributes, SpriteAttributes::DmgPalette as u8) {
          self.obp1
        } else {
          self.obp0
        };

        // Map the raw sprite color using the selected palette.
        *pixel = (palette >> (color << 1)) & 0x03;
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
