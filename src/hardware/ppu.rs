use crate::{
  flags::{add_flag, is_flag_set, remove_flag},
  interrupts::{Interrupt, Interrupts},
};
use arrayvec::ArrayVec;

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
  /// The window's scanline.
  ///
  /// See https://gbdev.io/pandocs/Tile_Maps.html#window for more.
  wly: u8,
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

  /// Internal counter for tracking cycles.
  counter: usize,

  /// The last value set when executing a DMA transfer,
  dma: u8,
  /// The current DMA transfer.
  pub dma_transfer: Option<DmaTransfer>,
  /// A restarted DMA transfer.
  pub restarted_dma_transfer: Option<RestartedDmaTransfer>,
}

/// The state of a direct memory transfer.
#[derive(Debug, Clone)]
pub struct DmaTransfer {
  /// The source address of where to copy from, for this the DMA transfer.
  pub source: u8,
  /// The progress of the DMA transfer.
  pub progress: DmaTransferProgress,
}

/// The progress of an existing DMA transfer.
#[derive(Debug, Clone)]
pub enum DmaTransferProgress {
  /// A DMA transfer was requested and is going to begin after an M-cycle has elapsed.
  Requested { delay_ticks: u8 },
  /// A DMA transfer that is in progress with the following dots.
  Transferring { ticks: u16 },
}

/// A DMA transfer when one is already running.
#[derive(Debug, Clone)]
pub struct RestartedDmaTransfer {
  /// The source address of where to copy from, for this the DMA transfer.
  pub source: u8,
  /// The number of T-cycles since this restarted DMA transfer was requested.
  pub delay_ticks: u8,
}

impl Ppu {
  /// Creates a new [`Ppu`].
  pub fn new() -> Self {
    Self {
      lcdc: 0x0,
      // The default mode is the OAM search
      stat: PpuMode::OamScan as u8,
      scy: 0,
      scx: 0,
      ly: 0,
      wly: 0,
      lyc: 0,
      bgp: 0,
      obp0: 0,
      obp1: 0,
      wy: 0,
      wx: 0,

      counter: 0,

      dma: 0,
      dma_transfer: None,
      restarted_dma_transfer: None,

      memory: [0; VIDEO_RAM_SIZE as usize],
      oam: [0; OAM_SIZE as usize],
      buffer: [[0; 160]; 144],
    }
  }

  /// Steps the PPU by a T-cycle.
  pub fn step(&mut self, interrupts: &mut Interrupts) {
    self.counter += 1;

    // `LY==LYC` needs to be checked every cycle.
    if self.ly == self.lyc {
      if !is_flag_set!(self.stat, StatFlag::Coincidence as u8) {
        add_flag!(&mut self.stat, StatFlag::Coincidence as u8);

        if is_flag_set!(self.stat, StatFlag::LycInterrupt as u8) {
          interrupts.request_interrupt(Interrupt::Lcd);
        }
      }
    } else {
      remove_flag!(&mut self.stat, StatFlag::Coincidence as u8);
    }

    match self.current_mode() {
      // OAM scan lasts for 80 cycles
      PpuMode::OamScan => {
        if self.counter >= 80 {
          self.counter -= 80;
          self.set_current_mode(PpuMode::PixelTransfer);

          if is_flag_set!(self.stat, StatFlag::OamInterrupt as u8) {
            interrupts.request_interrupt(Interrupt::Lcd)
          }
        }
      }
      // Pixel transfer lasts for 172 cycles
      PpuMode::PixelTransfer => {
        if self.counter >= 172 {
          self.counter -= 172;
          self.set_current_mode(PpuMode::HBlank);
          self.render_scanline();

          if is_flag_set!(self.stat, StatFlag::HBlankInterrupt as u8) {
            interrupts.request_interrupt(Interrupt::Lcd);
          }
        }
      }
      // HBlank last for 204 cycles
      PpuMode::HBlank => {
        if self.counter >= 204 {
          self.counter -= 204;
          self.ly = self.ly.wrapping_add(1);

          if self.ly == 144 {
            interrupts.request_interrupt(Interrupt::VBlank);
            self.set_current_mode(PpuMode::VBlank);

            if is_flag_set!(self.stat, StatFlag::VBlankInterrupt as u8) {
              interrupts.request_interrupt(Interrupt::Lcd);
            }
          } else {
            self.set_current_mode(PpuMode::OamScan);
          }
        }
      }
      // VBlank last for 456 cycles
      PpuMode::VBlank => {
        if self.counter >= 456 {
          self.counter -= 456;
          self.ly = self.ly.wrapping_add(1);

          if self.ly > 153 {
            self.ly = 0;
            self.wly = 0;
            self.set_current_mode(PpuMode::OamScan);
          }
        }
      }
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
      0xFF40 => {
        // Reset the line counter when the window gets disabled
        if is_flag_set!(self.lcdc, LcdControl::LcdDisplay as u8)
          && !is_flag_set!(value, LcdControl::LcdDisplay as u8)
        {
          self.ly = 0;
          self.wly = 0;

          remove_flag!(&mut self.stat, StatFlag::Coincidence as u8);
        }

        self.lcdc = value;
      }
      // Preserve the PPU mode in the lower 2 bits
      0xFF41 => self.stat = (value & 0b0111_1100) | self.current_mode() as u8,
      0xFF42 => self.scy = value,
      0xFF43 => self.scx = value,
      // Writes to LY are ignored
      0xFF44 => {}
      0xFF45 => self.lyc = value,
      0xFF46 => {
        self.dma = value;

        if self.dma_transfer.is_some() {
          self.restarted_dma_transfer = Some(RestartedDmaTransfer::new(self.dma));
        } else {
          self.dma_transfer = Some(DmaTransfer::new(self.dma));
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

  /// Returns the current mode that the PPU is in.
  pub fn current_mode(&self) -> PpuMode {
    PpuMode::try_from(self.stat & 0x03).unwrap()
  }

  /// Returns whether the LCD is enabled.
  pub fn display_enabled(&self) -> bool {
    is_flag_set!(self.lcdc, LcdControl::LcdDisplay as u8)
  }

  /// Returns whether the OAM can be accessed by the CPU.
  pub fn can_access_oam(&self) -> bool {
    // The PPU can only read from OAM if the LCD is off or the PPU is not in
    // `OamScan` and not in `PixelTransfer`.
    !self.display_enabled()
      || !matches!(
        self.current_mode(),
        PpuMode::OamScan | PpuMode::PixelTransfer
      )
  }

  /// Returns whether there is a DMA transfer.
  pub fn dma_transfer_exists(&self) -> bool {
    self.dma_transfer.is_some()
  }

  /// Returns whether there is a running DMA transfer, that is transferring bytes.
  pub fn dma_transfer_running(&self) -> bool {
    matches!(
      self.dma_transfer,
      Some(DmaTransfer {
        progress: DmaTransferProgress::Transferring { .. },
        ..
      })
    )
  }

  /// Returns whether the OAM can accessed by the CPU.
  pub fn can_access_vram(&self) -> bool {
    // The PPU can only read VRAM if the LCD is off or the PPU is not in pixel transfer.
    !self.display_enabled() || !matches!(self.current_mode(), PpuMode::PixelTransfer)
  }

  /// Gets the frame buffer.
  pub fn buffer(&self) -> &[[u8; 160]; 144] {
    &self.buffer
  }

  /// Sets the mode of the PPU.
  fn set_current_mode(&mut self, mode: PpuMode) {
    // The 7th bit is unused and the lower 2 bits store the mode
    self.stat = (self.stat & 0b0111_1100) | mode as u8;
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
        (0x8800, tile_index - 128)
      }
    };

    // Tiles are stored in 16 bytes
    let tile_offset = tile_index as u16 * 16;
    let row_offset = row as u16 * 2;
    let lower = self.read_ram(base_addr + tile_offset + row_offset);
    let upper = self.read_ram(base_addr + tile_offset + row_offset + 1);
    let bit = 7 - (x % 8);

    (((upper >> bit) & 1) << 1) | ((lower >> bit) & 1)
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
  fn render_background(&mut self, scanline: &mut [u8; 160]) {
    let bg_tile_map = if is_flag_set!(self.lcdc, LcdControl::BackgroundTileMap as u8) {
      0x9C00
    } else {
      0x9800
    };

    let y = (self.ly as u16).wrapping_add(self.scy as u16);
    // Background tile map have 32 tiles per row
    let tile_row = (y / 8) % 32 * 32;

    for (x, pixel) in scanline.iter_mut().enumerate() {
      let x_pos = (x as u16).wrapping_add(self.scx as u16);
      let tile_col = (x_pos / 8) % 32;
      let tile_index = self.read_ram(bg_tile_map + tile_row + tile_col);
      let raw_pixel = self.get_tile_pixel(tile_index, (y % 8) as u8, (x_pos % 8) as u8);

      *pixel = (self.bgp >> (raw_pixel * 2)) & 0x03;
    }
  }

  /// Renders the window into the scanline.
  fn render_window(&mut self, scanline: &mut [u8; 160]) {
    // The window is only drawn on scanlines at or below the window Y-position
    if self.ly < self.wy {
      return;
    }

    // Offset by -7 because thats where the window starts
    let window_x = self.wx.saturating_sub(7);

    if window_x >= 160 {
      return;
    }

    let window_tile_map = if is_flag_set!(self.lcdc, LcdControl::WindowTileMap as u8) {
      0x9C00
    } else {
      0x9800
    };

    let window_y = self.wly as u16;
    // Window tile map have 32 tiles per row
    let tile_row = (window_y / 8) * 32;

    // Render the rest of the tiles on this scanline
    for x in window_x..160 {
      let window_x = (x - window_x) as u16;
      let tile_col = window_x / 8;
      let tile_index = self.read_ram(window_tile_map + tile_row + tile_col);
      let raw_pixel = self.get_tile_pixel(tile_index, (window_y % 8) as u8, (window_x % 8) as u8);

      scanline[x as usize] = (self.bgp >> (raw_pixel * 2)) & 0x03;
    }

    // The window's internal counter is only incremented after window rendering
    self.wly = self.wly.wrapping_add(1);
  }

  /// Renders sprites into the scanline.
  fn render_sprites(&mut self, scanline: &mut [u8; 160]) {
    // The Gameboy can only draw 10 sprites per scanline.
    const MAX_SCANLINE_SPRITES: usize = 10;

    // Bit 2 determines the sprite's height
    let sprite_height = if is_flag_set!(self.lcdc, LcdControl::SpriteDimensions as u8) {
      16
    } else {
      8
    };

    let mut sprites = ArrayVec::<SpriteEntry, MAX_SCANLINE_SPRITES>::new();

    // TODO: The PPU can be blocked from OAM during DMA transfers as well
    for chunk in self.oam.chunks_exact(4) {
      if sprites.len() >= MAX_SCANLINE_SPRITES {
        break;
      }

      // NOTE: We have to do this because `array_chunks` isn't stable. It's a bit ugly,
      // but its looks better than iterating over the indices and offsetting into the OAM
      let (raw_y, raw_x, tile_index, attributes) = match chunk {
        &[a, b, c, d] => (a, b, c, d),
        _ => unreachable!(),
      };

      // A Y-coordinate of 16 means the sprite is fully visible, so offset it by -16
      // Wrapping subtraction is fine here, since we'll still be out of bounds
      let sprite_y = raw_y.wrapping_sub(16);

      // Ignore invisible sprites (those not within the bounds of the screen or not on
      // the current scanline) and don't count it towards the sprite limit.
      if raw_y == 0 || raw_y >= 160 || self.ly < sprite_y || self.ly >= sprite_y + sprite_height {
        continue;
      }

      // Offset by -8 because a sprite is fully visibile at position 8
      let sprite_x = raw_x.wrapping_sub(8);
      let oam_index = sprites.len();

      sprites.push(SpriteEntry {
        x: sprite_x,
        y: sprite_y,
        tile_index,
        attributes,
        oam_position: oam_index as u8,
      });
    }

    // Sort in reverse order, based on the position it was in OAM if the sprite's X-coordinates
    // were equal, otherwise by the X-coordinates.
    sprites.sort_by(|sprite_a, sprite_b| {
      if sprite_a.x == sprite_b.x {
        sprite_b.oam_position.cmp(&sprite_a.oam_position)
      } else {
        sprite_b.x.cmp(&sprite_a.x)
      }
    });

    // Draw the 10 sprites
    for sprite in &sprites {
      // Get the row where the sprite should be drawn
      let row = {
        let line = self.ly.wrapping_sub(sprite.y);

        if is_flag_set!(sprite.attributes, SpriteAttributes::YFlip as u8) {
          sprite_height - 1 - line
        } else {
          line
        }
      };

      let (tile_to_use, tile_row) = if sprite_height == 16 {
        // For 8×16 sprites, we need to clear the LSB of the top tile
        // and set it for the bottom tile
        let cleared_tile = sprite.tile_index & 0xFE;

        if row < 8 {
          (cleared_tile, row)
        } else {
          (cleared_tile | 0x01, row - 8)
        }
      } else {
        (sprite.tile_index, row)
      };

      // Render the 8 pixels in each tile
      for x_offset in 0..8 {
        let screen_x = sprite.x.wrapping_add(x_offset);

        // Don't draw sprites that are off the screen
        if screen_x >= 160 {
          continue;
        }

        let pixel = &mut scanline[screen_x as usize];

        // Don't draw over the background if the sprite has lower priority.
        if is_flag_set!(sprite.attributes, SpriteAttributes::Priority as u8) && *pixel != 0 {
          continue;
        }

        let flip_x = is_flag_set!(sprite.attributes, SpriteAttributes::XFlip as u8);
        let color = self.get_sprite_pixel(tile_to_use, tile_row, x_offset, flip_x);

        if color == 0 {
          continue;
        }

        let palette = if is_flag_set!(sprite.attributes, SpriteAttributes::DmgPalette as u8) {
          self.obp1
        } else {
          self.obp0
        };

        // Map the raw sprite color using the selected palette.
        *pixel = (palette >> (color * 2)) & 0x03;
      }
    }
  }
}

impl DmaTransfer {
  /// Creates a requested DMA transfer, with the following source address.
  pub fn new(source: u8) -> Self {
    Self {
      source,
      progress: DmaTransferProgress::Requested { delay_ticks: 0 },
    }
  }

  /// Creates a requested DMA transfer with the following ticks.
  pub fn requested_with_ticks(source: u8, ticks: u8) -> Self {
    Self {
      source,
      progress: DmaTransferProgress::Requested { delay_ticks: ticks },
    }
  }

  /// Creates a started DMA transfer, with the following source address.
  pub fn starting(source: u8) -> Self {
    Self {
      source,
      progress: DmaTransferProgress::Transferring { ticks: 0 },
    }
  }

  /// Creates a started DMA transfer, with the following ticks.
  pub fn starting_with_ticks(source: u8, ticks: u16) -> Self {
    Self {
      source,
      progress: DmaTransferProgress::Transferring { ticks },
    }
  }
}

impl RestartedDmaTransfer {
  /// Creates a new restarted DMA transfer, with the following source address.
  pub fn new(source: u8) -> Self {
    Self {
      source,
      delay_ticks: 0,
    }
  }
}

/// A sprite entry from the OAM.
#[derive(Debug, Clone)]
struct SpriteEntry {
  /// The X-position of the sprite.
  pub x: u8,
  /// The Y-position of the sprite.
  pub y: u8,
  /// The tile index of the sprite.
  pub tile_index: u8,
  /// The attributes of the sprite.
  pub attributes: u8,
  /// The position in the OAM of this sprite.
  pub oam_position: u8,
}

/// Flags for the `stat` field.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum StatFlag {
  /// Coincidence flag that is set when `LY` is equal to `LYC`.
  Coincidence = 1 << 2,
  /// HBlank interrupt enable.
  HBlankInterrupt = 1 << 3,
  /// VBlank interrupt enable.
  VBlankInterrupt = 1 << 4,
  /// OAM interrupt enable.
  OamInterrupt = 1 << 5,
  /// Interrupt enable when the coincidence flag is set.
  LycInterrupt = 1 << 6,
}

/// The different modes the PPU can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PpuMode {
  HBlank = 0,
  VBlank = 1,
  OamScan = 2,
  PixelTransfer = 3,
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

impl TryFrom<u8> for PpuMode {
  type Error = ();

  fn try_from(bits: u8) -> Result<Self, Self::Error> {
    Ok(match bits {
      0b00 => Self::HBlank,
      0b01 => Self::VBlank,
      0b10 => Self::OamScan,
      0b11 => Self::PixelTransfer,
      _ => return Err(()),
    })
  }
}

/// The amount of memory available to the PPU.
const VIDEO_RAM_SIZE: u16 = 0x2000;
/// The amount of memory available for the sprites.
const OAM_SIZE: u16 = 0xA0;
