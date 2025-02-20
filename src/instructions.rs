use crate::{
  flags::ConditionalFlag,
  hardware::registers::{Register, RegisterPair},
};

/// A GBZ80 Assembly instruction.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms, non_camel_case_types)]
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
  // Decrement.
  DEC(Operand),
  // Increment.
  INC(Operand),
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
  // We can have something like `CALL n16`, so make an argument optional.
  CALL(Option<Operand>, Operand),
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
  /// A value stored in memory at the value of the register pair.
  RegisterPairMemory(RegisterPair),
  /// A value stored in memory located at the address `0xFF00 + reg`.
  HighMemoryRegister(Register),
  /// A value stored in memory located at the address `0xFF00 + byte`.
  HighMemoryByte(u8),
  /// A value offset to the value of the stack pointer.
  StackOffset(u8),
  /// A value stored in memory at the address.
  MemoryAddress(u16),
  /// A conditional flag.
  Conditional(ConditionalFlag),
}

impl Instruction {
  pub fn bytes_occupied(&self) -> u8 {
    use Instruction::*;

    match self {
      // `LD r8 | [HL], r8 | [HL]`
      LD(Operand::Register(_), Operand::Register(_)) => 1,
      // `LD r16, n16`
      LD(Operand::RegisterPair(_), Operand::Word(_)) => 3,
      // `LD [r16], A` and `LD A, [r16]`
      LD(Operand::RegisterPairMemory(_), Operand::Register(Register::A))
      | LD(Operand::Register(Register::A), Operand::RegisterPairMemory(_)) => 1,
      // `LD [n16], SP`
      LD(Operand::MemoryAddress(_), Operand::RegisterPair(RegisterPair::SP)) => 3,
      // `LD r8 | [HL], n8`
      LD(Operand::Register(_), Operand::Byte(_)) => 2,
      // `LD HL, SP + n8`
      LD(Operand::RegisterPair(RegisterPair::HL), Operand::StackOffset(_)) => 2,
      // `LD SP, HL`
      LD(Operand::RegisterPair(RegisterPair::SP), Operand::RegisterPair(RegisterPair::HL)) => 1,
      // `LD [n16], A` | `LD A, [n16]`
      LD(Operand::MemoryAddress(_), Operand::Register(Register::A))
      | LD(Operand::Register(Register::A), Operand::MemoryAddress(_)) => 3,

      // `LDI [HL], A` | `LDI A, [HL]`
      LDI(Operand::RegisterPairMemory(RegisterPair::HL), Operand::Register(Register::A))
      | LDI(Operand::Register(Register::A), Operand::RegisterPairMemory(RegisterPair::HL)) => 1,
      // `LDD [HL], A` | `LDD A, [HL]`
      LDD(Operand::RegisterPairMemory(RegisterPair::HL), Operand::Register(Register::A))
      | LDD(Operand::Register(Register::A), Operand::RegisterPairMemory(RegisterPair::HL)) => 1,

      // `LDH [0xFF00 + n8], A` | `LDH A, [0xFF00 + n8]`
      LDH(Operand::HighMemoryByte(_), Operand::Register(Register::A))
      | LDH(Operand::Register(Register::A), Operand::HighMemoryByte(_)) => 2,
      // `LDH [0xFF00 + C], A` | `LDH A, [0xFF00 + C]`
      LDH(Operand::HighMemoryRegister(Register::C), Operand::Register(Register::A))
      | LDH(Operand::Register(Register::A), Operand::HighMemoryRegister(Register::C)) => 1,

      // `ADC A, r8 | [HL]`
      ADC(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `ADC A, n8`
      ADC(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `ADD A, r8 | [HL]`
      ADD(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `ADD A, n8`
      ADD(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `ADD HL, r16`
      ADD(Operand::RegisterPair(RegisterPair::HL), Operand::RegisterPair(_)) => 1,
      // `ADD SP, n8`
      ADD(Operand::RegisterPair(RegisterPair::SP), Operand::Byte(_)) => 2,
      // `AND A, r8 | [HL]`
      AND(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `AND A, n8`
      AND(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `CP A, r8 | [HL]`
      CP(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `CP A, n8`
      CP(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `DEC r8` | `DEC r16`
      DEC(Operand::Register(_) | Operand::RegisterPair(_)) => 1,
      // `INC r8` | `INC r16`
      INC(Operand::Register(_) | Operand::RegisterPair(_)) => 1,
      // `OR A, r8 | [HL]`
      OR(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `OR A, n8`
      OR(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `SBC A, r8 | [HL]`
      SBC(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `SBC A, n8`
      SBC(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `SUB A, r8 | [HL]`
      SUB(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `SUB A, n8`
      SUB(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `XOR A, r8 | [HL]`
      XOR(Operand::Register(Register::A), Operand::Register(_)) => 1,
      // `XOR A, n8`
      XOR(Operand::Register(Register::A), Operand::Byte(_)) => 2,
      // `DAA`
      DAA => 1,

      // `CALL cf, n16` | `CALL n16`
      CALL(Some(Operand::Conditional(_)) | None, Operand::Word(_)) => 3,
      // `JP cf, n16` | `JP n16`
      JP(Some(Operand::Conditional(_)) | None, Operand::Word(_)) => 3,
      // `JP HL`
      JP(None, Operand::RegisterPair(RegisterPair::HL)) => 1,

      // `JR cf, n8` | `JR n8`
      JR(Some(Operand::Conditional(_)) | None, Operand::Byte(_)) => 2,
      // `RET cf` | `RET`
      RET(Some(Operand::Conditional(_)) | None) => 1,
      // `RETI`
      RETI => 1,
      // `RST 0x0 | 0x10 | 0x20 | 0x30 | 0x08 | 0x18 | 0x28 | 0x38`
      RST(Operand::Byte(_)) => 1,

      // `STOP n8`
      STOP(Operand::Byte(_)) => 2,
      // `HALT`
      HALT => 1,
      // `NOP`
      NOP => 1,

      // `POP r16`
      POP(Operand::RegisterPair(_)) => 1,
      // `PUSH r16`
      PUSH(Operand::RegisterPair(_)) => 1,

      // `CCF`
      CCF => 1,
      // `CPL`
      CPL => 1,
      // `DI`
      DI => 1,
      // `EI`
      EI => 1,
      // `SCF`
      SCF => 1,

      // `RLA`
      RLA => 1,
      // `RLCA`
      RLCA => 1,
      // `RRA`
      RRA => 1,
      // `RRCA`
      RRCA => 1,

      // `BIT n8, r8 | [HL]`
      BIT(Operand::Byte(_), Operand::Register(_)) => 2,
      // `RES n8, r8 | [HL]`
      RES(Operand::Byte(_), Operand::Register(_)) => 2,
      // `SET n8, r8 | [HL]`
      SET(Operand::Byte(_), Operand::Register(_)) => 2,
      // `RL r8 | [HL]`
      RL(Operand::Register(_)) => 2,
      // `RLC r8 | [HL]`
      RLC(Operand::Register(_)) => 2,
      // `RR r8 | [HL]`
      RR(Operand::Register(_)) => 2,
      // `RRC r8 | [HL]`
      RRC(Operand::Register(_)) => 2,
      // `SLA r8 | [HL]`
      SLA(Operand::Register(_)) => 2,
      // `SRA r8 | [HL]`
      SRA(Operand::Register(_)) => 2,
      // `SRL r8 | [HL]`
      SRL(Operand::Register(_)) => 2,
      // `SWAP r8 | [HL]`
      SWAP(Operand::Register(_)) => 2,

      x => panic!("missing number of bytes for: {:?}", x),
    }
  }
}
