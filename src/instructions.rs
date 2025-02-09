use crate::{
  flags::ConditionalFlags,
  registers::{Register, RegisterPair},
};

/// A GBZ80 Assembly instruction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Instruction {
  // Load and store instructions
  // NOTE: Separate `LDD`, `LDI`, and `LDH` from `LD` since they have different semantics.
  /// Load.
  LD(Operand, Operand),
  /// Load and decrement, equivalent to `LD A, [HL-]` or `LD [HL-], A`.
  LDD(Operand, Operand),
  /// Load and increment, equivalent to `LD A, [HL+]` or `LD [HL+], A`.
  LDI(Operand, Operand),
  /// Load high, equivalent to `LD A, [0xFF00 + C]` or `LD [0xFF00 + C], A`.
  LDH(Operand, Operand),

  // Arithmetic and logical instructions
  /// Add with carry.
  ADC(Operand, Operand),
  /// Add.
  ADD(Operand, Operand),
  /// Logical AND.
  AND(Operand, Operand),
  /// Compare.
  CP(Operand, Operand),
  /// Logical OR.
  OR(Operand, Operand),
  /// Subtract with carry.
  SBC(Operand, Operand),
  /// Subtract.
  SUB(Operand, Operand),
  /// Logical XOR.
  XOR(Operand, Operand),
  /// Decimal adjust accumulator.
  DAA,

  // Control flow instructions
  /// Call.
  CALL(Operand, Operand),
  /// Jump.
  // We can have something like `JP HL`, so make an argument optional.
  JP(Option<Operand>, Operand),
  /// Jump relative.
  // We can have something like `JR e8`, so make an argument optional.
  JR(Option<Operand>, Operand),
  /// Return.
  // We can have something like `RET`, so make the argument optional.
  RET(Option<Operand>),
  /// Return and enable interrupts.
  RETI,
  /// Restart.
  RST(Operand),
  /// Stop.
  // NOTE: `STOP` needs to be followed by any byte, usually 0x0.
  STOP(Operand),
  /// Halt.
  HALT,
  /// No operation.
  NOP,

  // Stack instructions
  /// Pop.
  POP(Operand),
  /// Push.
  PUSH(Operand),

  // Flag instructions
  /// Complement carry flag.
  CCF,
  /// Complement accumulator.
  CPL,
  /// Disable interrupts.
  DI,
  /// Enable interrupts.
  EI,
  /// Set carry flag.
  SCF,

  // Bit manipulation instructions
  /// Rotate left accumulator.
  RLA,
  /// Rotate left circular accumulator.
  RLCA,
  /// Rotate right accumulator.
  RRA,
  /// Rotate right circular accumulator.
  RRCA,

  // Extended instruction set
  /// Tests a bit.
  BIT(Operand, Operand),
  /// Reset a bit.
  RES(Operand, Operand),
  /// Set a bit.
  SET(Operand, Operand),
  /// Rotate left.
  RL(Operand),
  /// Rotate left circular.
  RLC(Operand),
  /// Rotate right.
  RR(Operand),
  /// Rotate right circular.
  RRC(Operand),
  /// Shift left arithmetic.
  SLA(Operand),
  /// Shift right arithmetic.
  SRA(Operand),
  /// Shift right logical.
  SRL(Operand),
  /// Swap upper and lower nibbles.
  SWAP(Operand),
}

/// An operand inside an instruction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Operand {
  /// An 8-bit literal number.
  Byte(u8),
  /// A 16-bit literal number.
  Word(u16),
  /// A register.
  Register(Register),
  /// A register pair.  
  RegisterPair(RegisterPair),
  /// A value stored in memory at the value of the register.
  RegisterMemory(Register),
  /// A value stored in memory at the value of the register pair.
  RegisterPairMemory(RegisterPair),
  /// A value stored in memory at the address.
  MemoryAddress(u16),
  /// A conditional flag.
  Conditional(ConditionalFlags),
}
