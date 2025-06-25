/// The value for the register pair BC.
pub const REGISTER_PAIR_BC: u8 = 0b00;
/// The value for the register pair DE.
pub const REGISTER_PAIR_DE: u8 = 0b01;
/// The value for the register pair HL.
pub const REGISTER_PAIR_HL: u8 = 0b10;
/// The value for the register pair AF.
pub const REGISTER_PAIR_AF: u8 = 0b11;
/// The value for the register pair SP.
pub const REGISTER_PAIR_SP: u8 = 0b11;

/// The `A` register.
pub const REGISTER_A: u8 = 0b111;
/// The `B` register.
pub const REGISTER_B: u8 = 0b000;
/// The `C` register.
pub const REGISTER_C: u8 = 0b001;
/// The `D` register.
pub const REGISTER_D: u8 = 0b010;
/// The `E` register.
pub const REGISTER_E: u8 = 0b011;
/// The `H` register.
pub const REGISTER_H: u8 = 0b100;
/// The `L` register.
pub const REGISTER_L: u8 = 0b101;
/// The memory "register."
pub const REGISTER_M: u8 = 0b110;

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
