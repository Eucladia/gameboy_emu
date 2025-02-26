/// The CPU flags that may get affected after executing an instruction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Flag {
  /// The zero flag.
  Z = 1 << 7,
  /// The subtraction flag.
  N = 1 << 6,
  /// The half-carry flag.
  H = 1 << 5,
  /// The carry flag.
  C = 1 << 4,
}

/// A condition that an instruction can use for control flow.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConditionalFlag {
  /// Continues control flow if a zero was produced.
  Z,
  /// Continues control flow if a carry was produced.
  C,
  /// Continues control flow if a carry was not produced.
  NZ,
  /// Continues control flow if a carry was not produced.
  NC,
}

impl ConditionalFlag {
  pub fn from_bits(bits: u8) -> Option<Self> {
    ConditionalFlag::try_from(bits).ok()
  }
}

impl TryFrom<u8> for ConditionalFlag {
  type Error = ();

  /// Attempts to convert the byte into a [`ConditionalFlag`].
  fn try_from(bits: u8) -> Result<Self, Self::Error> {
    Ok(match bits {
      0b00 => ConditionalFlag::NZ,
      0b01 => ConditionalFlag::Z,
      0b10 => ConditionalFlag::NC,
      0b11 => ConditionalFlag::C,
      _ => return Err(()),
    })
  }
}
