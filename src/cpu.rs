use crate::{
  instructions::{Instruction, Operand},
  memory::Mmu,
  registers::{Register, RegisterPair, Registers},
};

#[derive(Debug)]
pub struct Cpu {
  /// The set flags.
  ///
  /// Note: The upper nibble contains the set flags, the lower nibble is always zeroed.
  flags: u8,
  /// The clock state.
  clock: ClockState,
  /// The registers.
  registers: Registers,
}

/// The internal time clock.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct ClockState {
  /// Machine cycles.
  pub m: u32,
  /// Tick cycles.
  pub t: u32,
}

impl Cpu {
  pub fn new(mmu: Mmu) -> Self {
    Self {
      clock: ClockState::default(),
      flags: 0,
      registers: Registers::default(),
    }
  }

  pub fn execute_instruction(&mut self, mmu: &Mmu) {
    let instruction = self.parse_instruction(mmu);

    self.registers.pc += 1;
  }

  pub fn parse_instruction(&mut self, mmu: &Mmu) -> Option<Instruction> {
    let byte = mmu.read_byte(self.registers.pc);

    match byte {
      // LD r8, r8
      x if (0x40..=0x7F).contains(&x) && x != 0x76 => {
        let dest_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Some(Instruction::LD(
          Operand::Register(dest_reg),
          Operand::Register(src_reg),
        ))
      }
      // LD r16, n16
      0x01 | 0x11 | 0x21 | 0x31 => {
        let dest_reg_pair = match (byte >> 4) & 0b11 {
          0b00 => RegisterPair::BC,
          0b01 => RegisterPair::DE,
          0b10 => RegisterPair::HL,
          0b11 => RegisterPair::SP,
          b => unreachable!("incorrect register pair passed to LD: {b:02X}"),
        };
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Some(Instruction::LD(
          Operand::RegisterPair(dest_reg_pair),
          Operand::Word(n16),
        ))
      }
      // LD [r16], A
      0x02 | 0x12 => {
        let dest_reg_pair = if byte == 0x02 {
          RegisterPair::BC
        } else {
          RegisterPair::DE
        };

        Some(Instruction::LD(
          Operand::RegisterPairMemory(dest_reg_pair),
          Operand::Register(Register::A),
        ))
      }
      // LD A, [r16]
      0x0A | 0x1A => {
        let src_reg_pair = if byte == 0x0A {
          RegisterPair::BC
        } else {
          RegisterPair::DE
        };

        Some(Instruction::LD(
          Operand::Register(Register::A),
          Operand::RegisterPairMemory(src_reg_pair),
        ))
      }
      // LD r8 | [HL], n8
      0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E => {
        let dest_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        // Stinky edge case :/
        let operand = if matches!(dest_reg, Register::M) {
          Operand::RegisterPairMemory(RegisterPair::HL)
        } else {
          Operand::Register(dest_reg)
        };

        Some(Instruction::LD(operand, Operand::Byte(n8)))
      }
      // LD HL, SP + n8
      0xF8 => {
        // Cast to i8 to sign extend later when casting to u16
        let n8 = mmu.read_byte(self.registers.pc + 1) as i8;

        self.registers.pc += 1;

        Some(Instruction::LD(
          Operand::RegisterPair(RegisterPair::HL),
          Operand::Word(self.registers.sp.wrapping_add(n8 as u16)),
        ))
      }
      // LD SP, HL
      0xF9 => Some(Instruction::LD(
        Operand::RegisterPair(RegisterPair::SP),
        Operand::RegisterPair(RegisterPair::HL),
      )),

      // LD [n16], A
      0xEA => {
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Some(Instruction::LD(
          Operand::MemoryAddress(n16),
          Operand::Register(Register::A),
        ))
      }
      // LD A, [n16]
      0xFA => {
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Some(Instruction::LD(
          Operand::Register(Register::A),
          Operand::MemoryAddress(n16),
        ))
      }
      // LDI [HL], A
      0x22 => Some(Instruction::LDI(
        Operand::RegisterPairMemory(RegisterPair::HL),
        Operand::Register(Register::A),
      )),
      // LDI A, [HL]
      0x2A => Some(Instruction::LDI(
        Operand::Register(Register::A),
        Operand::RegisterPairMemory(RegisterPair::HL),
      )),
      // LDD [HL], A
      0x32 => Some(Instruction::LDD(
        Operand::RegisterPairMemory(RegisterPair::HL),
        Operand::Register(Register::A),
      )),
      // LDD A, [HL]
      0x3A => Some(Instruction::LDD(
        Operand::Register(Register::A),
        Operand::RegisterPairMemory(RegisterPair::HL),
      )),
      // LDH [0xFF00 + C], A
      0xE2 => Some(Instruction::LDH(
        Operand::RegisterMemory(Register::C),
        Operand::Register(Register::A),
      )),
      // LDH A, [0xFF00 + C]
      0xF2 => Some(Instruction::LDH(
        Operand::Register(Register::A),
        Operand::RegisterMemory(Register::C),
      )),
      byte => panic!("unimplemented: {byte} ({byte:02X})"),
    }
  }
}
