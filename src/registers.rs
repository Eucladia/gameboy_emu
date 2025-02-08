/// The status of the registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registers {
  /// The `A` register.
  pub a: u8,
  /// The `B` register.
  pub b: u8,
  /// The `C` register.
  pub c: u8,
  /// The `D` register.
  pub d: u8,
  /// The `E` register.
  pub e: u8,
  /// The `H` register.
  pub h: u8,
  /// The `L` register.
  pub l: u8,

  /// The program counter.
  pub pc: u16,
  /// The stack pointer.
  pub sp: u16,

  /// Instruction register.
  pub ir: u8,
  /// Interrupt enable register.
  pub ie: u8,
}

/// A register.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Register {
  A,
  B,
  C,
  D,
  E,
  H,
  L,
}

/// A register pair.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum RegisterPair {
  /// Psuedo-register of the accumulator & flags that can be used in 16-bit contexts.
  AF,
  /// The register `B` paired with the register `C`.
  BC,
  /// The register `D` paired with the register `E`.
  DE,
  /// The register `H` paired with the register `L`.
  HL,
  /// Psuedo-register of the stack pointer.
  SP,
}

impl Default for Registers {
  fn default() -> Self {
    Self {
      a: 0,
      b: 0,
      c: 0,
      d: 0,
      e: 0,
      h: 0,
      l: 0,
      pc: 0,
      sp: u16::MAX,
      ir: 0,
      ie: 0,
    }
  }
}
