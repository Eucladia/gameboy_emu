/// Possible flags that get set after executing an instruction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Flags {
  /// The zero flag.
  Z = 1 << 7,
  /// The subtraction flag.
  N = 1 << 6,
  /// The half-carry flag, indicating whether there was a carry over the nibbles.
  H = 1 << 5,
  /// The carry flag.
  C = 1 << 4,
}

/// Flags that conditional instructions use.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConditionalFlags {
  /// The condtion is true if [`Flags::Z`] is set, aka a zero was produced.
  Z,
  /// The condtion is true if [`Flags::C`] is set, aka a carry was produced.
  C,
  /// The condtion is true if [`Flags::Z`] is not set, aka a zero was not produced.
  NZ,
  /// The condtion is true if [`Flags::C`] is not set, aka a carry was not produced.
  NC,
}
