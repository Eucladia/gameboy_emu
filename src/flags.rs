/// Checks if a number has the following flag set.
macro_rules! is_flag_set {
  ($num:expr, $mask:expr) => {
    ($num & $mask) == $mask
  };
}

/// Adds a flag to the number.
macro_rules! add_flag {
  ($num:expr, $mask:expr) => {
    *$num |= $mask
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
