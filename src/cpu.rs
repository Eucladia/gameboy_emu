use crate::{
  flags::ConditionalFlags,
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
  /// Whether the CPU has been halted.
  halted: bool,
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
      flags: 0,
      halted: false,
      clock: ClockState::default(),
      registers: Registers::default(),
    }
  }

  pub fn execute_instruction(&mut self, mmu: &Mmu) {
    let instruction = self.parse_instruction(mmu);

    self.registers.pc += 1;
  }

  pub fn parse_instruction(&mut self, mmu: &Mmu) -> Instruction {
    let byte = mmu.read_byte(self.registers.pc);

    match byte {
      // LD r8, r8
      x if (0x40..=0x7F).contains(&x) && x != 0x76 => {
        let dest_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::LD(Operand::Register(dest_reg), Operand::Register(src_reg))
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

        Instruction::LD(Operand::RegisterPair(dest_reg_pair), Operand::Word(n16))
      }
      // LD [r16], A
      0x02 | 0x12 => {
        let dest_reg_pair = if byte == 0x02 {
          RegisterPair::BC
        } else {
          RegisterPair::DE
        };

        Instruction::LD(
          Operand::RegisterPairMemory(dest_reg_pair),
          Operand::Register(Register::A),
        )
      }
      // LD A, [r16]
      0x0A | 0x1A => {
        let src_reg_pair = if byte == 0x0A {
          RegisterPair::BC
        } else {
          RegisterPair::DE
        };

        Instruction::LD(
          Operand::Register(Register::A),
          Operand::RegisterPairMemory(src_reg_pair),
        )
      }
      // LD r8 | [HL], n8
      0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E => {
        let dest_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::LD(Operand::Register(dest_reg), Operand::Byte(n8))
      }
      // LD HL, SP + n8
      0xF8 => {
        // Cast to i8 to sign extend later when casting to u16
        let n8 = mmu.read_byte(self.registers.pc + 1) as i8;

        self.registers.pc += 1;

        Instruction::LD(
          Operand::RegisterPair(RegisterPair::HL),
          Operand::Word(self.registers.sp.wrapping_add(n8 as u16)),
        )
      }
      // LD SP, HL
      0xF9 => Instruction::LD(
        Operand::RegisterPair(RegisterPair::SP),
        Operand::RegisterPair(RegisterPair::HL),
      ),

      // LD [n16], A
      0xEA => {
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::LD(Operand::MemoryAddress(n16), Operand::Register(Register::A))
      }
      // LD A, [n16]
      0xFA => {
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::LD(Operand::Register(Register::A), Operand::MemoryAddress(n16))
      }
      // LDI [HL], A
      0x22 => Instruction::LDI(
        Operand::RegisterPairMemory(RegisterPair::HL),
        Operand::Register(Register::A),
      ),
      // LDI A, [HL]
      0x2A => Instruction::LDI(
        Operand::Register(Register::A),
        Operand::RegisterPairMemory(RegisterPair::HL),
      ),
      // LDD [HL], A
      0x32 => Instruction::LDD(
        Operand::RegisterPairMemory(RegisterPair::HL),
        Operand::Register(Register::A),
      ),
      // LDD A, [HL]
      0x3A => Instruction::LDD(
        Operand::Register(Register::A),
        Operand::RegisterPairMemory(RegisterPair::HL),
      ),
      // LDH [0xFF00 + C], A
      0xE2 => Instruction::LDH(
        Operand::RegisterMemory(Register::C),
        Operand::Register(Register::A),
      ),
      // LDH A, [0xFF00 + C]
      0xF2 => Instruction::LDH(
        Operand::Register(Register::A),
        Operand::RegisterMemory(Register::C),
      ),
      // ADC A, r8 | [HL]
      0x88..=0x8F => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::ADC(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // ADC A, n8
      0xCE => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::ADC(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // ADD A, r8 | [HL]
      0x80..=0x87 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::ADD(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // ADD A, n8
      0xC6 => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::ADD(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // ADD SP, n8
      0xE8 => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::ADD(Operand::RegisterPair(RegisterPair::SP), Operand::Byte(n8))
      }
      // AND A, r8 | [HL]
      0xA0..=0xA7 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::AND(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // AND A, n8
      0xE6 => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::AND(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // CP A, r8 | [HL]
      0xB8..=0xBF => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::CP(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // CP A, n8
      0xFE => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::CP(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // OR A, r8 | [HL]
      0xB0..=0xB7 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::OR(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // OR A, n8
      0xF6 => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::OR(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // SBC A, r8 | [HL]
      0x98..=0x9F => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::SBC(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // SBC A, n8
      0xDE => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::SBC(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // SUB A, r8 | [HL]
      0x90..=0x97 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::SUB(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // SUB A, n8
      0xD6 => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::SUB(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // XOR A, r8 | [HL]
      0xA8..=0xAF => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::XOR(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // XOR A, n8
      0xEE => {
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::XOR(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // DAA
      0x27 => Instruction::DAA,

      // CALL cf, n16
      0xC4 | 0xD4 | 0xCC | 0xDC => {
        let cond_flag = ConditionalFlags::from_bits((byte >> 3) & 0b11).unwrap();
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::CALL(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // CALL n16
      0xCD => {
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::CALL(None, Operand::Word(n16))
      }
      // JP cf, n16
      0xC2 | 0xD2 | 0xCA | 0xDA => {
        let cond_flag = ConditionalFlags::from_bits((byte >> 3) & 0b11).unwrap();
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::JP(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // JP n16
      0xE9 => {
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::JP(None, Operand::Word(n16))
      }
      // JR cf, n16
      0x20 | 0x30 | 0x28 | 0x38 => {
        let cond_flag = ConditionalFlags::from_bits((byte >> 3) & 0b11).unwrap();
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::JR(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // JR n16
      0x18 => {
        let n16 = mmu.read_word(self.registers.pc + 1);

        self.registers.pc += 2;

        Instruction::JR(None, Operand::Word(n16))
      }
      // RET cf
      0xC0 | 0xD0 | 0xC8 | 0xD8 => {
        let cond_flag = ConditionalFlags::from_bits((byte >> 3) & 0b11).unwrap();

        Instruction::RET(Some(Operand::Conditional(cond_flag)))
      }
      // RET
      0xC9 => Instruction::RET(None),
      // RETI
      0xD9 => Instruction::RETI,
      // RST 0x0 | 0x10 | 0x20 | 0x30 | 0x08 | 0x18 | 0x28 | 0x38
      0xC7 | 0xD7 | 0xE7 | 0xF7 | 0xCF | 0xDF | 0xEF | 0xFF => {
        let target = ((byte >> 3) & 0b111) * 8;

        Instruction::RST(Operand::Byte(target))
      }
      // STOP n8
      0x10 => {
        // NOTE: `STOP` needs to be followed by another byte.
        let n8 = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        Instruction::STOP(Operand::Byte(n8))
      }
      // HALT
      0x76 => Instruction::HALT,
      // NOP
      0x0 => Instruction::NOP,

      // POP r16
      0xC1 | 0xD1 | 0xE1 | 0xF1 => {
        let r16 = match (byte >> 4) & 0b11 {
          0b00 => RegisterPair::BC,
          0b01 => RegisterPair::DE,
          0b10 => RegisterPair::HL,
          0b11 => RegisterPair::AF,
          _ => unreachable!("invalid byte passed to pop"),
        };

        Instruction::POP(Operand::RegisterPair(r16))
      }
      // PUSH r16
      0xC5 | 0xD5 | 0xE5 | 0xF5 => {
        let r16 = match (byte >> 4) & 0b11 {
          0b00 => RegisterPair::BC,
          0b01 => RegisterPair::DE,
          0b10 => RegisterPair::HL,
          0b11 => RegisterPair::AF,
          _ => unreachable!("invalid byte passed to push"),
        };

        Instruction::PUSH(Operand::RegisterPair(r16))
      }

      // CCF
      0x3F => Instruction::CCF,
      // CPL
      0x2F => Instruction::CPL,
      // DI
      0xF3 => Instruction::DI,
      // EI
      0xFB => Instruction::EI,
      // SCF
      0x37 => Instruction::SCF,

      // RLA
      0x17 => Instruction::RLA,
      // RLCA
      0x07 => Instruction::RLCA,
      // RRA
      0x1F => Instruction::RRA,
      // RRCA
      0x0F => Instruction::RRCA,

      // Extended instruction set
      0xCB => {
        let next_byte = mmu.read_byte(self.registers.pc + 1);

        self.registers.pc += 1;

        match next_byte {
          // BIT 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7, r8 | [HL]
          0x40..=0x7F => {
            let bit_num = (next_byte >> 3) & 0b111;
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::BIT(Operand::Byte(bit_num), Operand::Register(src_reg))
          }
          // RES 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7, r8 | [HL]
          0x80..=0xBF => {
            let bit_num = (next_byte >> 3) & 0b111;
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::RES(Operand::Byte(bit_num), Operand::Register(src_reg))
          }
          // SET 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7, r8 | [HL]
          0xC0..=0xFF => {
            let bit_num = (next_byte >> 3) & 0b111;
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::SET(Operand::Byte(bit_num), Operand::Register(src_reg))
          }
          // RL r8 | [HL]
          0x10..=0x17 => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::RL(Operand::Register(src_reg))
          }
          // RLC r8 | [HL]
          0x00..=0x07 => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::RLC(Operand::Register(src_reg))
          }
          // RR r8 | [HL]
          0x18..=0x1F => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::RR(Operand::Register(src_reg))
          }
          // RRC r8 | [HL]
          0x08..=0x0F => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::RRC(Operand::Register(src_reg))
          }
          // SLA r8 | [HL]
          0x20..=0x27 => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::SLA(Operand::Register(src_reg))
          }
          // SRA r8 | [HL]
          0x28..=0x2F => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::SRA(Operand::Register(src_reg))
          }
          // SRL r8 | [HL]
          0x38..=0x3F => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::SRL(Operand::Register(src_reg))
          }
          // SWAP r8 | [HL]
          0x30..=0x37 => {
            let src_reg = Register::from_bits(next_byte & 0b111).unwrap();

            Instruction::SWAP(Operand::Register(src_reg))
          }
        }
      }

      byte => panic!("unimplemented: {byte} ({byte:02X})"),
    }
  }
}
