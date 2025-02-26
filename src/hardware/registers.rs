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
}

/// A register.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Register {
  /// The register `A`.
  A,
  /// The register `B`.
  B,
  /// The register `C`.
  C,
  /// The register `D`.
  D,
  /// The register `E`.
  E,
  /// The register `H`.
  H,
  /// The register `L`.
  L,
  /// Psuedo-register that points to the memory at the address of the register pair `HL`.
  M,
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

impl Register {
  /// Returns a register from its encoded 3 bits.
  pub fn from_bits(bits: u8) -> Option<Self> {
    Register::try_from(bits).ok()
  }
}

impl RegisterPair {
  // Gets a register pair from 2 bits. If `psw` is true, then `0x2` will
  // return [`RegisterPair::AF`] instead of [`RegisterPair::SP`].
  pub fn from_bits(bits: u8, use_psw: bool) -> Option<Self> {
    Some(match bits {
      0b00 => RegisterPair::BC,
      0b01 => RegisterPair::DE,
      0b10 => RegisterPair::HL,
      0b11 if use_psw => RegisterPair::AF,
      0b11 => RegisterPair::SP,
      _ => return None,
    })
  }
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
    }
  }
}

impl TryFrom<u8> for Register {
  type Error = ();

  /// Attempts to convert the byte into a [`Register`].
  fn try_from(value: u8) -> Result<Self, Self::Error> {
    Ok(match value {
      0b000 => Register::B,
      0b001 => Register::C,
      0b010 => Register::D,
      0b011 => Register::E,
      0b100 => Register::H,
      0b101 => Register::L,
      0b110 => Register::M,
      0b111 => Register::A,
      _ => return Err(()),
    })
  }
}
