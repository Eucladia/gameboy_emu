/// Flags that can get set after an instruction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Flags {
  /// The zero flag.
  Z = 1 << 7,
  /// The subtraction flag.
  N = 1 << 6,
  /// The half carry flag, which is a carry over the nibbles.
  H = 1 << 5,
  /// The carry flag.
  C = 1 << 4,
}

/// Flags that conditional instructions use.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConditionalFlags {
  /// The condtion is true if [`Flags::Z`] is set.
  Z,
  /// The condtion is true if [`Flags::C`] is set.
  C,
  /// The condtion is true if [`Flags::Z`] is not set.
  NZ,
  /// The condtion is true if [`Flags::C`] is not set.
  NC,
}
