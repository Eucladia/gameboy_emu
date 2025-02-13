use crate::{
  flags::{ConditionalFlag, Flags},
  instructions::{Instruction, Operand},
  memory::Mmu,
  registers::{Register, RegisterPair, Registers},
};

#[derive(Debug)]
pub struct Cpu {
  /// The set flags.
  flags: u8,
  /// The clock state.
  clock: ClockState,
  /// The registers.
  registers: Registers,
  /// Whether the CPU has been halted.
  halted: bool,
  /// Whether the CPU has been "stopped".
  stopped: bool,
  /// Master interrupt flag.
  ime: bool,
}

/// The internal time clock.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct ClockState {
  /// Machine cycles.
  pub m_cycles: usize,
  /// Tick cycles.
  pub t_cycles: usize,
}

impl Cpu {
  pub fn new(mmu: Mmu) -> Self {
    Self {
      flags: 0,
      ime: false,
      stopped: false,
      halted: false,
      clock: ClockState::default(),
      registers: Registers::default(),
    }
  }

  /// Executes one cycle.
  pub fn step(&mut self, mmu: &mut Mmu) {
    let byte = self.fetch_instruction(mmu);

    self.registers.pc += 1;
    self.registers.ir = byte;

    let instruction = self.decode_instruction(byte, mmu);
    // Subtract a byte since we accounted for the instruction byte itself already
    let i_size = instruction.bytes_occupied() - 1;

    self.registers.pc = self.registers.pc.wrapping_add(i_size as u16);

    self.execute_instruction(mmu, &instruction);
  }

  /// Fetches the next instruction byte.
  pub fn fetch_instruction(&self, mmu: &Mmu) -> u8 {
    mmu.read_byte(self.registers.pc)
  }

  /// Executes the [`Instruction`], updating the internal clock state.
  pub fn execute_instruction(&mut self, mmu: &mut Mmu, instruction: &Instruction) {
    use Instruction::*;

    match instruction {
      LD(Operand::Register(dest), Operand::Register(src)) => {
        let value = self.read_register(mmu, *src);
        self.write_register(mmu, *dest, value);
        self.clock.tick();

        // Add another machine cycle if we fetched or wrote to memory
        if matches!(dest, Register::M) || matches!(src, Register::M) {
          self.clock.tick();
        }
      }
      LD(Operand::RegisterPair(rp), Operand::Word(value)) => {
        self.write_register_pair(*rp, *value);
        self.clock.advance(3);
      }
      LD(Operand::RegisterPairMemory(rp), Operand::Register(Register::A)) => {
        mmu.write_byte(self.read_register_pair(*rp), self.registers.a);
        self.clock.advance(2);
      }
      LD(Operand::Register(Register::A), Operand::RegisterPairMemory(rp)) => {
        self.registers.a = mmu.read_byte(self.read_register_pair(*rp));
        self.clock.advance(2);
      }
      LD(Operand::MemoryAddress(value), Operand::RegisterPair(RegisterPair::SP)) => {
        mmu.write_byte(*value, (self.registers.sp & 0xFF) as u8);
        mmu.write_byte(*value + 1, ((self.registers.sp >> 8) & 0xFF) as u8);
        self.clock.advance(5);
      }
      LD(Operand::Register(dest), Operand::Byte(value)) => {
        self.write_register(mmu, *dest, *value);
        self.clock.advance(2);

        // Add another machine cycle if we wrote to memory
        if matches!(dest, Register::M) {
          self.clock.tick();
        }
      }
      LD(Operand::RegisterPair(RegisterPair::HL), Operand::StackOffset(offset)) => {
        // NOTE: The offset can be negative, so do a sign-extend add.
        self.registers.sp = self.registers.sp.wrapping_add(*offset as i8 as u16);
        self.clock.advance(3);
      }
      LD(Operand::RegisterPair(RegisterPair::SP), Operand::RegisterPair(RegisterPair::HL)) => {
        self.registers.sp = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        self.clock.advance(2);
      }
      LD(Operand::MemoryAddress(address), Operand::Register(Register::A)) => {
        mmu.write_byte(*address, self.registers.a);
        self.clock.advance(4);
      }
      LD(Operand::Register(Register::A), Operand::MemoryAddress(address)) => {
        self.registers.a = mmu.read_byte(*address);
        self.clock.advance(4);
      }

      LDI(Operand::RegisterPairMemory(RegisterPair::HL), Operand::Register(Register::A)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;

        mmu.write_byte(address, self.registers.a);

        let inc = address.wrapping_add(1);

        self.registers.h = ((inc >> 8) & 0xFF) as u8;
        self.registers.l = (inc & 0xFF) as u8;

        self.clock.advance(2);
      }
      LDI(Operand::Register(Register::A), Operand::RegisterPairMemory(RegisterPair::HL)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        let value = mmu.read_byte(address);

        self.registers.a = value;

        let inc = address.wrapping_add(1);

        self.registers.h = ((inc >> 8) & 0xFF) as u8;
        self.registers.l = (inc & 0xFF) as u8;

        self.clock.advance(2);
      }
      LDD(Operand::RegisterPairMemory(RegisterPair::HL), Operand::Register(Register::A)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;

        mmu.write_byte(address, self.registers.a);

        let inc = address.wrapping_sub(1);

        self.registers.h = ((inc >> 8) & 0xFF) as u8;
        self.registers.l = (inc & 0xFF) as u8;

        self.clock.advance(2);
      }
      LDD(Operand::Register(Register::A), Operand::RegisterPairMemory(RegisterPair::HL)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        let value = mmu.read_byte(address);

        self.registers.a = value;

        let inc = address.wrapping_sub(1);

        self.registers.h = ((inc >> 8) & 0xFF) as u8;
        self.registers.l = (inc & 0xFF) as u8;

        self.clock.advance(2);
      }
      LDH(Operand::HighMemoryByte(value), Operand::Register(Register::A)) => {
        mmu.write_byte(0xFF00 + *value as u16, self.registers.a);
        self.clock.advance(3);
      }
      LDH(Operand::Register(Register::A), Operand::HighMemoryByte(value)) => {
        self.registers.a = mmu.read_byte(0xFF00 + *value as u16);
        self.clock.advance(3);
      }
      LDH(Operand::HighMemoryRegister(Register::C), Operand::Register(Register::A)) => {
        mmu.write_byte(0xFF00 + self.registers.c as u16, self.registers.a);
        self.clock.advance(2);
      }
      LDH(Operand::Register(Register::A), Operand::HighMemoryRegister(Register::C)) => {
        self.registers.a = mmu.read_byte(0xFF00 + self.registers.c as u16);
        self.clock.advance(2);
      }

      ADC(Operand::Register(Register::A), Operand::Register(src)) => {
        let is_carry_set = self.is_flag_set(Flags::C);
        let reg_value = self.read_register(mmu, *src);
        let res = self
          .registers
          .a
          .wrapping_add(reg_value)
          .wrapping_add(is_carry_set as u8);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(
          Flags::H,
          (self.registers.a & 0x0F) + (reg_value & 0x0F) + (is_carry_set as u8 & 0x0F) > 0x0F,
        );
        self.toggle_flag(
          Flags::C,
          (self.registers.a as u16 + reg_value as u16 + is_carry_set as u16) > u8::MAX as u16,
        );

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(src, Register::M) {
          self.clock.tick();
        }
      }
      ADC(Operand::Register(Register::A), Operand::Byte(byte)) => {
        let is_carry_set = self.is_flag_set(Flags::C);
        let res = self
          .registers
          .a
          .wrapping_add(*byte)
          .wrapping_add(is_carry_set as u8);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(
          Flags::H,
          (self.registers.a & 0x0F) + (*byte & 0x0F) + (is_carry_set as u8 & 0x0F) > 0x0F,
        );
        self.toggle_flag(
          Flags::C,
          (self.registers.a as u16 + *byte as u16 + is_carry_set as u16) > u8::MAX as u16,
        );

        self.clock.advance(2);
      }
      ADD(Operand::Register(Register::A), Operand::Register(src)) => {
        let reg_value = self.read_register(mmu, *src);
        let res = self.registers.a.wrapping_add(reg_value);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(
          Flags::H,
          (self.registers.a & 0x0F) + (reg_value & 0x0F) > 0x0F,
        );
        self.toggle_flag(
          Flags::C,
          (self.registers.a as u16 + reg_value as u16) > u8::MAX as u16,
        );

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(src, Register::M) {
          self.clock.tick();
        }
      }
      ADD(Operand::Register(Register::A), Operand::Byte(byte)) => {
        let res = self.registers.a.wrapping_add(*byte);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, (self.registers.a & 0x0F) + (*byte & 0x0F) > 0x0F);
        self.toggle_flag(
          Flags::C,
          (self.registers.a as u16 + *byte as u16) > u8::MAX as u16,
        );

        self.clock.advance(2);
      }
      ADD(Operand::RegisterPair(RegisterPair::HL), Operand::RegisterPair(src)) => {
        let hl_value = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        let rp_value = self.read_register_pair(*src);
        let res = hl_value.wrapping_add(rp_value);

        self.registers.h = ((res >> 8) & 0xFF) as u8;
        self.registers.l = (res & 0xFF) as u8;

        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, (hl_value & 0x0FFF) + (rp_value & 0x0FFF) > 0x0FFF);
        self.toggle_flag(Flags::C, res < hl_value);

        self.clock.advance(2);
      }
      ADD(Operand::RegisterPair(RegisterPair::SP), Operand::Byte(value)) => {
        // Sign extend the number
        let num = *value as i8 as u16;
        let sp_value = self.registers.sp;

        self.registers.sp = sp_value.wrapping_add(num);

        self.toggle_flag(Flags::Z, false);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, (sp_value & 0x0F) + (num & 0x0F) > 0x0F);
        self.toggle_flag(Flags::C, (sp_value & 0xFF) + (num & 0xFF) > 0xFF);

        self.clock.advance(4);
      }
      AND(Operand::Register(Register::A), Operand::Register(src_reg)) => {
        let src_value = self.read_register(mmu, *src_reg);
        let res = self.registers.a & src_value;

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, true);
        self.toggle_flag(Flags::C, false);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(src_reg, Register::M) {
          self.clock.tick();
        }
      }
      AND(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a & *value;

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, true);
        self.toggle_flag(Flags::C, false);

        self.clock.advance(2);
      }
      CP(Operand::Register(Register::A), Operand::Register(src_reg)) => {
        let src_value = self.read_register(mmu, *src_reg);
        let res = self.registers.a.wrapping_sub(src_value);

        self.clock.tick();

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, true);
        self.toggle_flag(Flags::H, (self.registers.a & 0x0F) < (src_value & 0x0F));
        self.toggle_flag(Flags::C, self.registers.a < src_value);

        // Add another machine cycle if  we fetched memory
        if matches!(src_reg, Register::M) {
          self.clock.tick();
        }
      }
      CP(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a.wrapping_sub(*value);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, true);
        self.toggle_flag(Flags::H, (self.registers.a & 0x0F) < (*value & 0x0F));
        self.toggle_flag(Flags::C, self.registers.a < *value);

        self.clock.advance(2);
      }
      DEC(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = reg_value.wrapping_sub(1);

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, true);
        self.toggle_flag(Flags::H, (reg_value & 0x0F) == 0x0);

        self.clock.tick();

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      DEC(Operand::RegisterPair(rp)) => {
        let reg_value = self.read_register_pair(*rp);
        let res = reg_value.wrapping_sub(1);

        self.write_register_pair(*rp, res);
        self.clock.advance(2);
      }
      INC(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = reg_value.wrapping_add(1);

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, (reg_value & 0x0F) == 0x0F);

        self.clock.tick();

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      INC(Operand::RegisterPair(rp)) => {
        let reg_value = self.read_register_pair(*rp);
        let res = reg_value.wrapping_add(1);

        self.write_register_pair(*rp, res);
        self.clock.advance(2);
      }
      OR(Operand::Register(Register::A), Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = self.registers.a | reg_value;

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, false);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      OR(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a | *value;

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, false);

        self.clock.advance(2);
      }
      SBC(Operand::Register(Register::A), Operand::Register(reg)) => {
        let is_carry_set = self.is_flag_set(Flags::C) as u8;
        let reg_value = self.read_register(mmu, *reg);
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(reg_value).wrapping_sub(is_carry_set);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, true);
        self.toggle_flag(
          Flags::H,
          (a_value & 0x0F) < ((reg_value & 0x0F) + is_carry_set),
        );
        self.toggle_flag(
          Flags::C,
          (a_value as u16) < (reg_value as u16 + is_carry_set as u16),
        );

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      SBC(Operand::Register(Register::A), Operand::Byte(value)) => {
        let is_carry_set = self.is_flag_set(Flags::C) as u8;
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(*value).wrapping_sub(is_carry_set);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, true);
        self.toggle_flag(
          Flags::H,
          (a_value & 0x0F) < ((*value & 0x0F) + is_carry_set),
        );
        self.toggle_flag(
          Flags::C,
          (a_value as u16) < (*value as u16 + is_carry_set as u16),
        );

        self.clock.advance(2);
      }
      SUB(Operand::Register(Register::A), Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(reg_value);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, true);
        self.toggle_flag(Flags::H, (a_value & 0x0F) < (reg_value & 0x0F));
        self.toggle_flag(Flags::C, a_value < reg_value);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      SUB(Operand::Register(Register::A), Operand::Byte(value)) => {
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(*value);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, true);
        self.toggle_flag(Flags::H, (a_value & 0x0F) < (*value & 0x0F));
        self.toggle_flag(Flags::C, a_value < *value);

        self.clock.advance(2);
      }
      XOR(Operand::Register(Register::A), Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = self.registers.a ^ reg_value;

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, false);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      XOR(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a ^ *value;

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, false);

        self.clock.advance(2);
      }
      DAA => {
        let mut correction = 0;

        let subtracted = self.is_flag_set(Flags::N);
        let half_carried = self.is_flag_set(Flags::H);
        let mut carried = self.is_flag_set(Flags::C);

        // Check the lower nibble
        if half_carried || (!subtracted && (self.registers.a & 0x0F) > 0x09) {
          correction |= 0x06;
        }

        // Check the upper nibble
        if carried || (!subtracted && (self.registers.a >> 4) > 0x09) {
          correction |= 0x60;
          carried = true;
        }

        let res = if subtracted {
          self.registers.a.wrapping_sub(correction)
        } else {
          self.registers.a.wrapping_add(correction)
        };

        self.registers.a = res;

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, carried);

        self.clock.tick();
      }

      CALL(Some(Operand::Conditional(flag)), Operand::Word(address)) => {
        let should_jump = self.is_conditional_flag_set(*flag);

        if should_jump {
          let upper = ((self.registers.pc >> 8) & 0xFF) as u8;
          let lower = (self.registers.pc & 0xFF) as u8;

          mmu.write_byte(self.registers.sp - 1, upper);
          mmu.write_byte(self.registers.sp - 2, lower);

          self.registers.pc = *address;
          self.registers.sp -= 2;
          self.clock.advance(6);
        } else {
          self.clock.advance(3);
        }
      }
      CALL(None, Operand::Word(address)) => {
        let upper = ((self.registers.pc >> 8) & 0xFF) as u8;
        let lower = (self.registers.pc & 0xFF) as u8;

        mmu.write_byte(self.registers.sp - 1, upper);
        mmu.write_byte(self.registers.sp - 2, lower);

        self.registers.pc = *address;
        self.registers.sp -= 2;
        self.clock.advance(6);
      }
      JP(Some(Operand::Conditional(flag)), Operand::Word(address)) => {
        let should_jump = self.is_conditional_flag_set(*flag);

        if should_jump {
          self.registers.pc = *address;
          self.clock.advance(4);
        } else {
          self.clock.advance(3);
        }
      }
      JP(None, Operand::Word(address)) => {
        self.registers.pc = *address;
        self.clock.advance(4);
      }
      JP(None, Operand::RegisterPair(RegisterPair::HL)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;

        self.registers.pc = address;
        self.clock.tick();
      }
      JR(Some(Operand::Conditional(flag)), Operand::Byte(offset)) => {
        let should_jump = self.is_conditional_flag_set(*flag);

        if should_jump {
          // NOTE: The byte can be negative, so sign-extend add the value
          self.registers.pc = self.registers.pc.wrapping_add(*offset as i8 as u16);
          self.clock.advance(3);
        } else {
          self.clock.advance(2);
        }
      }
      JR(None, Operand::Byte(offset)) => {
        // NOTE: The byte can be negative, so sign-extend add the value
        self.registers.pc = self.registers.pc.wrapping_add(*offset as i8 as u16);
        self.clock.advance(3);
      }
      RET(Some(Operand::Conditional(flag))) => {
        let should_jump = self.is_conditional_flag_set(*flag);

        if should_jump {
          let lower = mmu.read_byte(self.registers.sp);
          let upper = mmu.read_byte(self.registers.sp + 1);

          self.registers.pc = ((upper as u16) << 8) | lower as u16;
          self.registers.sp += 2;
          self.clock.advance(5);
        } else {
          self.clock.advance(2);
        }
      }
      RET(None) => {
        let lower = mmu.read_byte(self.registers.sp);
        let upper = mmu.read_byte(self.registers.sp + 1);

        self.registers.pc = ((upper as u16) << 8) | lower as u16;
        self.registers.sp += 2;
        self.clock.advance(4);
      }
      RETI => {
        let lower = mmu.read_byte(self.registers.sp);
        let upper = mmu.read_byte(self.registers.sp + 1);

        self.registers.pc = ((upper as u16) << 8) | lower as u16;
        self.registers.sp += 2;
        self.ime = true;
        self.clock.advance(4);
      }
      RST(Operand::Byte(target)) => {
        let upper = ((self.registers.pc >> 8) & 0xFF) as u8;
        let lower = (self.registers.pc & 0xFF) as u8;

        mmu.write_byte(self.registers.sp - 1, upper);
        mmu.write_byte(self.registers.sp - 2, lower);

        self.registers.h = 0;
        self.registers.l = *target;
        self.registers.sp -= 2;
        self.clock.advance(4);
      }
      STOP(Operand::Byte(_)) => {
        self.stopped = true;
        self.clock.tick();
      }
      HALT => {
        self.halted = true;
        self.clock.tick();
      }
      NOP => {
        self.clock.tick();
      }

      POP(Operand::RegisterPair(rp)) => {
        let lower = mmu.read_byte(self.registers.sp);
        let upper = mmu.read_byte(self.registers.sp + 1);
        let value = ((upper as u16) << 8) | lower as u16;

        self.write_register_pair(*rp, value);

        self.registers.sp += 2;
        self.clock.advance(4);
      }
      PUSH(Operand::RegisterPair(rp)) => {
        let reg_value = self.read_register_pair(*rp);
        let upper = ((reg_value >> 8) & 0xFF) as u8;
        let lower = (reg_value & 0xFF) as u8;

        mmu.write_byte(self.registers.sp - 1, upper);
        mmu.write_byte(self.registers.sp - 2, lower);

        self.registers.sp -= 2;
        self.clock.advance(4);
      }
      CCF => {
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, !self.is_flag_set(Flags::C));

        self.clock.tick();
      }
      CPL => {
        self.registers.a = !self.registers.a;

        self.toggle_flag(Flags::N, true);
        self.toggle_flag(Flags::H, true);

        self.clock.tick();
      }
      DI => {
        self.ime = false;
        self.clock.tick();
      }
      EI => {
        self.ime = true;
        self.clock.tick();
      }
      SCF => {
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, true);

        self.clock.tick();
      }

      RLA => {
        let is_carry_set = self.is_flag_set(Flags::C);
        let a_value = self.registers.a;
        let res = (a_value << 1) | (is_carry_set as u8);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, false);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (a_value >> 7) == 0x1);

        self.clock.tick();
      }
      RLCA => {
        let a_value = self.registers.a;
        let res = a_value.rotate_left(1);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, false);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (a_value >> 7) == 0x1);

        self.clock.tick();
      }
      RRA => {
        let is_carry_set = self.is_flag_set(Flags::C);
        let a_value = self.registers.a;
        let res = (a_value >> 1) | ((is_carry_set as u8) << 7);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, false);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (a_value & 0x1) == 1);

        self.clock.tick();
      }
      RRCA => {
        let a_value = self.registers.a;
        let res = a_value.rotate_right(1);

        self.registers.a = res;

        self.toggle_flag(Flags::Z, false);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (a_value & 0x1) == 1);

        self.clock.tick();
      }

      BIT(Operand::Byte(bit), Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let extracted_bit = (reg_value >> bit) & 1;

        self.toggle_flag(Flags::Z, extracted_bit == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, true);

        self.clock.advance(2);

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      RES(Operand::Byte(bit), Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let new_value = reg_value & !(1 << bit);

        self.write_register(mmu, *reg, new_value);
        self.clock.advance(2);

        // Advance 2 machine cycles since we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      SET(Operand::Byte(bit), Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let new_value = reg_value | (1 << bit);

        self.write_register(mmu, *reg, new_value);
        self.clock.advance(2);

        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      RL(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let is_carry_set = self.is_flag_set(Flags::C) as u8;
        let res = (reg_value << 1) | is_carry_set;

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (reg_value >> 7) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      RLC(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = reg_value.rotate_left(1);

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (reg_value >> 7) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      RR(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let is_carry_set = self.is_flag_set(Flags::C) as u8;
        let res = (reg_value >> 1) | (is_carry_set << 7);

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      RRC(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = reg_value.rotate_right(1);

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      SLA(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = reg_value << 1;

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (reg_value >> 7) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      SRA(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        // SRA preserves the sign bit (MSB)
        let res = (reg_value >> 1) | (reg_value & 0x80);

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      SRL(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let res = reg_value >> 1;

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      SWAP(Operand::Register(reg)) => {
        let reg_value = self.read_register(mmu, *reg);
        let lower = reg_value & 0b00001111;
        let upper = reg_value & 0b11110000;
        let res = (lower << 4) | (upper >> 4);

        self.write_register(mmu, *reg, res);

        self.toggle_flag(Flags::Z, res == 0);
        self.toggle_flag(Flags::N, false);
        self.toggle_flag(Flags::H, false);
        self.toggle_flag(Flags::C, false);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      x => panic!("missing instruction execution for {:?}", x),
    }
  }

  /// Decodes a byte into an [`Instruction`].
  pub fn decode_instruction(&self, byte: u8, mmu: &Mmu) -> Instruction {
    match byte {
      // LD r8, r8
      0x40..0x76 | 0x77..=0x7F => {
        let dest_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::LD(Operand::Register(dest_reg), Operand::Register(src_reg))
      }
      // LD r16, n16
      0x01 | 0x11 | 0x21 | 0x31 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0b11, false).unwrap();
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::LD(Operand::RegisterPair(r16), Operand::Word(n16))
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
      // LD [a16], SP
      0x08 => {
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::LD(
          Operand::MemoryAddress(n16),
          Operand::RegisterPair(RegisterPair::SP),
        )
      }
      // LD r8 | [HL], n8
      0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E => {
        let dest_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::LD(Operand::Register(dest_reg), Operand::Byte(n8))
      }
      // LD HL, SP + n8
      0xF8 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::LD(
          Operand::RegisterPair(RegisterPair::HL),
          Operand::StackOffset(n8),
        )
      }
      // LD SP, HL
      0xF9 => Instruction::LD(
        Operand::RegisterPair(RegisterPair::SP),
        Operand::RegisterPair(RegisterPair::HL),
      ),
      // LD [n16], A
      0xEA => {
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::LD(Operand::MemoryAddress(n16), Operand::Register(Register::A))
      }
      // LD A, [n16]
      0xFA => {
        let n16 = mmu.read_word(self.registers.pc);

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
      // LDH [0xFF00 + n8], A
      0xE0 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::LDH(Operand::HighMemoryByte(n8), Operand::Register(Register::A))
      }
      // LDH A, [0xFF00 + n8]
      0xF0 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::LDH(Operand::Register(Register::A), Operand::HighMemoryByte(n8))
      }
      // LDH [0xFF00 + C], A
      0xE2 => Instruction::LDH(
        Operand::HighMemoryRegister(Register::C),
        Operand::Register(Register::A),
      ),
      // LDH A, [0xFF00 + C]
      0xF2 => Instruction::LDH(
        Operand::Register(Register::A),
        Operand::HighMemoryRegister(Register::C),
      ),
      // ADC A, r8 | [HL]
      0x88..=0x8F => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::ADC(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // ADC A, n8
      0xCE => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::ADC(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // ADD A, r8 | [HL]
      0x80..=0x87 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::ADD(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // ADD A, n8
      0xC6 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::ADD(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // ADD HL, r16
      0x09 | 0x19 | 0x29 | 0x39 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0b11, false).unwrap();

        Instruction::ADD(
          Operand::RegisterPair(RegisterPair::HL),
          Operand::RegisterPair(r16),
        )
      }
      // ADD SP, n8
      0xE8 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::ADD(Operand::RegisterPair(RegisterPair::SP), Operand::Byte(n8))
      }
      // AND A, r8 | [HL]
      0xA0..=0xA7 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::AND(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // AND A, n8
      0xE6 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::AND(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // CP A, r8 | [HL]
      0xB8..=0xBF => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::CP(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // CP A, n8
      0xFE => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::CP(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // DEC r8
      0x05 | 0x15 | 0x25 | 0x35 | 0x0D | 0x1D | 0x2D | 0x3D => {
        let dst_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();

        Instruction::DEC(Operand::Register(dst_reg))
      }
      // DEC r16
      0x0B | 0x1B | 0x2B | 0x3B => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0b11, false).unwrap();

        Instruction::DEC(Operand::RegisterPair(r16))
      }
      // INC r8
      0x04 | 0x14 | 0x24 | 0x34 | 0x0C | 0x1C | 0x2C | 0x3C => {
        let dst_reg = Register::from_bits((byte >> 3) & 0b111).unwrap();

        Instruction::INC(Operand::Register(dst_reg))
      }
      // INC r16
      0x03 | 0x13 | 0x23 | 0x33 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0b11, false).unwrap();

        Instruction::INC(Operand::RegisterPair(r16))
      }
      // OR A, r8 | [HL]
      0xB0..=0xB7 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::OR(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // OR A, n8
      0xF6 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::OR(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // SBC A, r8 | [HL]
      0x98..=0x9F => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::SBC(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // SBC A, n8
      0xDE => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::SBC(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // SUB A, r8 | [HL]
      0x90..=0x97 => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::SUB(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // SUB A, n8
      0xD6 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::SUB(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // XOR A, r8 | [HL]
      0xA8..=0xAF => {
        let src_reg = Register::from_bits(byte & 0b111).unwrap();

        Instruction::XOR(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // XOR A, n8
      0xEE => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::XOR(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // DAA
      0x27 => Instruction::DAA,

      // CALL cf, n16
      0xC4 | 0xD4 | 0xCC | 0xDC => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0b11).unwrap();
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::CALL(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // CALL n16
      0xCD => {
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::CALL(None, Operand::Word(n16))
      }
      // JP cf, n16
      0xC2 | 0xD2 | 0xCA | 0xDA => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0b11).unwrap();
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::JP(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // JP n16
      0xC3 => {
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::JP(None, Operand::Word(n16))
      }
      // JP HL
      0xE9 => Instruction::JP(None, Operand::RegisterPair(RegisterPair::HL)),
      // JR cf, n16
      0x20 | 0x30 | 0x28 | 0x38 => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0b11).unwrap();
        let n16 = mmu.read_word(self.registers.pc);

        Instruction::JR(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // JR n8
      0x18 => {
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::JR(None, Operand::Byte(n8))
      }
      // RET cf
      0xC0 | 0xD0 | 0xC8 | 0xD8 => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0b11).unwrap();

        Instruction::RET(Some(Operand::Conditional(cond_flag)))
      }
      // RET
      0xC9 => Instruction::RET(None),
      // RETI
      0xD9 => Instruction::RETI,
      // RST 0x0 | 0x10 | 0x20 | 0x30 | 0x08 | 0x18 | 0x28 | 0x38
      0xC7 | 0xD7 | 0xE7 | 0xF7 | 0xCF | 0xDF | 0xEF | 0xFF => {
        // The target is encoded in bits 3 through 5.
        let target = byte & 0b00111000;

        Instruction::RST(Operand::Byte(target))
      }
      // STOP n8
      0x10 => {
        // NOTE: `STOP` needs to be followed by another byte.
        let n8 = mmu.read_byte(self.registers.pc);

        Instruction::STOP(Operand::Byte(n8))
      }
      // HALT
      0x76 => Instruction::HALT,
      // NOP
      0x0 => Instruction::NOP,

      // POP r16
      0xC1 | 0xD1 | 0xE1 | 0xF1 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0b11, true).unwrap();

        Instruction::POP(Operand::RegisterPair(r16))
      }
      // PUSH r16
      0xC5 | 0xD5 | 0xE5 | 0xF5 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0b11, true).unwrap();

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
        let next_byte = mmu.read_byte(self.registers.pc);

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

      // Unused opcodes
      0xD3 | 0xE3 | 0xE4 | 0xF4 | 0xDB | 0xEB | 0xEC | 0xFC | 0xDD | 0xED | 0xFD => unreachable!(),
    }
  }

  /// Reads the value of the [`Register`].
  fn read_register(&self, mmu: &Mmu, register: Register) -> u8 {
    match register {
      Register::A => self.registers.a,
      Register::B => self.registers.b,
      Register::C => self.registers.c,
      Register::D => self.registers.d,
      Register::E => self.registers.e,
      Register::H => self.registers.h,
      Register::L => self.registers.l,
      Register::M => {
        let address = (self.registers.h as u16) << 8 | self.registers.l as u16;

        mmu.read_byte(address)
      }
    }
  }

  /// Writes the value to the [`Register`].
  fn write_register(&mut self, mmu: &mut Mmu, register: Register, value: u8) {
    match register {
      Register::A => self.registers.a = value,
      Register::B => self.registers.a = value,
      Register::C => self.registers.a = value,
      Register::D => self.registers.a = value,
      Register::E => self.registers.a = value,
      Register::H => self.registers.a = value,
      Register::L => self.registers.a = value,
      Register::M => {
        let address = (self.registers.h as u16) << 8 | self.registers.l as u16;

        mmu.write_byte(address, value);
      }
    }
  }

  /// Reads the value of the [`RegisterPair`].
  fn read_register_pair(&self, register_pair: RegisterPair) -> u16 {
    match register_pair {
      RegisterPair::AF => (self.registers.a as u16) << 8 | self.flags as u16,
      RegisterPair::BC => (self.registers.b as u16) << 8 | self.registers.c as u16,
      RegisterPair::DE => (self.registers.d as u16) << 8 | self.registers.e as u16,
      RegisterPair::HL => (self.registers.h as u16) << 8 | self.registers.l as u16,
      RegisterPair::SP => self.registers.sp,
    }
  }

  /// Writes the value to the following [`RegisterPair`].
  fn write_register_pair(&mut self, register_pair: RegisterPair, value: u16) {
    match register_pair {
      RegisterPair::AF => {
        self.registers.a = ((value >> 8) & 0xFF) as u8;
        self.flags = (value & 0xFF) as u8;
      }
      RegisterPair::BC => {
        self.registers.b = ((value >> 8) & 0xFF) as u8;
        self.registers.c = (value & 0xFF) as u8;
      }
      RegisterPair::DE => {
        self.registers.d = ((value >> 8) & 0xFF) as u8;
        self.registers.e = (value & 0xFF) as u8;
      }
      RegisterPair::HL => {
        self.registers.h = ((value >> 8) & 0xFF) as u8;
        self.registers.l = (value & 0xFF) as u8;
      }
      RegisterPair::SP => {
        self.registers.sp = value;
      }
    }
  }

  /// Returns whether the following [`ConditionalFlag`] is set.
  fn is_conditional_flag_set(&self, cond_flag: ConditionalFlag) -> bool {
    match cond_flag {
      ConditionalFlag::Z => self.is_flag_set(Flags::Z),
      ConditionalFlag::C => self.is_flag_set(Flags::C),
      ConditionalFlag::NZ => !self.is_flag_set(Flags::Z),
      ConditionalFlag::NC => !self.is_flag_set(Flags::C),
    }
  }

  /// Returns whether or not the [`Flags`] is set.
  fn is_flag_set(&self, flags: Flags) -> bool {
    (self.flags & flags as u8) == flags as u8
  }

  /// Conditionally toggles the flag.
  fn toggle_flag(&mut self, flags: Flags, condition: bool) {
    if condition {
      self.flags |= flags as u8;
    } else {
      self.flags &= !(flags as u8);
    }
  }

  /// Sets the following [`Flags`].
  fn set_flag(&mut self, flags: Flags) {
    self.flags |= flags as u8;
  }
}

impl ClockState {
  /// Advances the internal clock by 1 machine cycle.
  pub fn tick(&mut self) {
    self.advance(1);
  }

  /// Advance the internal clock by the following machine cycles.
  pub fn advance(&mut self, m_cycles: usize) {
    self.m_cycles += m_cycles;
    self.t_cycles = m_cycles * 4;
  }
}
