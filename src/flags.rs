/// Checks if a number has the following flag set.
macro_rules! is_flag_set {
  ($num:expr, $mask:expr) => {
    ($num & $mask) == $mask
  };
}

/// Removes a flag from the number.
macro_rules! unset_flag {
  ($num:expr, $mask:expr) => {
    $num & !$mask
  };
}

/// Adds a flag to the number.
macro_rules! add_flag {
  ($num:expr, $mask:expr) => {
    *$num |= $mask
  };
}

/// Sets a flag in a number.
macro_rules! set_flag {
  ($num:expr, $mask:expr) => {
    $num | $mask
  };
}

/// Sets a flag in a number.
macro_rules! remove_flag {
  ($num:expr, $mask:expr) => {
    *$num &= !$mask
  };
}

pub(crate) use add_flag;
pub(crate) use is_flag_set;
pub(crate) use remove_flag;
pub(crate) use set_flag;
pub(crate) use unset_flag;

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
  /// Attempts to convert the 2-bit value into a [`ConditionalFlag`].
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
