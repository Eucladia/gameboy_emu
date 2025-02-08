use crate::{GameBoyError, GameBoyResult};

const NINTENDO_LOGO: [u8; 48] = [
  0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
  0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
  0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

#[derive(Debug)]
pub struct CartridgeHeader {
  title: String,
  licensee: Licensee,
  cartridge_type: CartridgeType,
  is_color: bool,
  sgb_indicator: bool,
}

impl CartridgeHeader {
  pub fn new(rom: &[u8]) -> GameBoyResult<Self> {
    if rom.len() < 0x14F {
      return Err(GameBoyError::InvalidHeader);
    }

    // Check the global and header checksums
    let global_checksum = rom.iter().enumerate().fold(0_u16, |x, (i, &byte)| {
      if i != 0x14E && i != 0x14F {
        x.wrapping_add(byte as u16)
      } else {
        x
      }
    });

    if global_checksum != ((rom[0x14E] as u16) << 8 | rom[0x14F] as u16) {
      return Err(GameBoyError::InvalidChecksum);
    }

    let header_checksum = rom[0x134..=0x14C]
      .iter()
      .fold(0_u8, |x, &byte| x.wrapping_sub(byte).wrapping_sub(1));

    if header_checksum != rom[0x14D] {
      return Err(GameBoyError::InvalidComplementChecksum);
    }

    // Check for the Nintendo logo
    if rom[0x104..=0x133] != NINTENDO_LOGO {
      return Err(GameBoyError::InvalidHeader);
    }

    // Parse the title out of the header
    let mut title = String::with_capacity(16);
    let mut saw_zero = false;

    for &byte in &rom[0x134..=0x142] {
      if byte != 0 {
        // Ensure that the remaining bytes are 0-padded
        if saw_zero {
          return Err(GameBoyError::InvalidHeader);
        }

        title.push(byte as char);
      } else if !saw_zero {
        saw_zero = true;
      }
    }

    let licensee = match rom[0x14B] {
      0x33 => {
        Licensee::New(char::from_u32(((rom[0x144] as u32) << 8) | rom[0x145] as u32).unwrap())
      }
      x => Licensee::Code(x),
    };

    Ok(Self {
      title,
      is_color: rom[0x143] == 1,
      licensee,
      sgb_indicator: rom[0x146] == 0x3,
      cartridge_type: CartridgeType::from(rom[0x147]),
    })
  }
}

#[derive(Debug, Copy, Eq, PartialEq, Clone)]
pub enum Licensee {
  Code(u8),
  New(char),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CartridgeType {
  Rom,
  RomMbc1,
  RomMbc1Ram,
  RomMbc1RamBattery,
}

impl From<u8> for CartridgeType {
  fn from(value: u8) -> Self {
    match value {
      0 => CartridgeType::Rom,
      1 => CartridgeType::RomMbc1,
      2 => CartridgeType::RomMbc1Ram,
      3 => CartridgeType::RomMbc1RamBattery,
      x => unimplemented!("cartridge type {x:02X} is not implemented"),
    }
  }
}
