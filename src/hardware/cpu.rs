use crate::{
  flags::{ConditionalFlag, Flag, add_flag, is_flag_set, remove_flag},
  hardware::{
    Hardware,
    ppu::DmaTransfer,
    registers::{Register, RegisterPair, Registers},
  },
  instructions::{Instruction, Operand},
  interrupts::Interrupt,
};

/// A state that the CPU can be in.
#[derive(Debug, Copy, Clone)]
pub enum CpuState {
  /// The CPU was running.
  Running,
  /// The CPU was marked as halted.
  Halted,
  /// The CPU was marked as stopped.
  Stopped,
}

#[derive(Debug)]
pub struct Cpu {
  /// The set flags.
  flags: u8,
  /// The clock state.
  clock: ClockState,
  /// The registers.
  pub registers: Registers,
  /// The state of the CPU.
  state: CpuState,
  /// Whether the CPU is in a bugged halted state.
  halt_bug: bool,
  /// Master interrupt flag.
  master_interrupt_enabled: bool,
}

impl Cpu {
  /// Creates a new [`Cpu`] in a running state.
  pub fn new() -> Self {
    Self {
      flags: 0,
      master_interrupt_enabled: false,
      state: CpuState::Running,
      clock: ClockState::default(),
      registers: Registers::default(),
      halt_bug: false,
    }
  }

  /// Creates a new [`Cpu`] with the default register values set.
  pub fn with_register_defaults() -> Self {
    let mut cpu = Self::new();

    cpu.set_register_defaults();

    cpu
  }

  /// Executes one cycle, returning the number of T-cycles taken.
  pub fn step(&mut self, hardware: &mut Hardware) -> usize {
    let prev_opcode = self.registers.ir;
    let instruction_byte = self.fetch_instruction(hardware);

    self.registers.ir = instruction_byte;

    let instruction = self.decode_instruction(instruction_byte, hardware);
    // Subtract a byte since we accounted for the instruction byte itself already
    let i_size = instruction.bytes_occupied() - 1;

    self.registers.pc = self.registers.pc.wrapping_add(i_size as u16);

    let before = self.clock.t_cycles;

    self.execute_instruction(hardware, &instruction);

    let cycles_taken = self.clock.t_cycles.wrapping_sub(before);

    // The `EI` instruction has a delay of 4 T-cycles, so enable the master
    // interrupt AFTER the execution of the next instruction
    if prev_opcode == 0xFB {
      self.master_interrupt_enabled = true;
    }

    cycles_taken
  }

  /// Fetches the next instruction byte.
  pub fn fetch_instruction(&mut self, hardware: &Hardware) -> u8 {
    // If we have a DMA transfer and we're not in high ram, then the next instruction
    // byte being fetched is the current byte that is being transferred by the DMA controller
    let next_byte = match hardware.get_dma_transfer() {
      Some(DmaTransfer::Transferring { current_pos: index }) if self.registers.pc < 0xFF80 => {
        // The program counter is still incremented in this case
        self.registers.pc = self.registers.pc.wrapping_add(1);

        (*index as u16) << 8
      }
      _ => {
        let current_pc = self.registers.pc;

        // Don't increment the program counter when we're in a bugged halted state
        if self.halt_bug {
          self.halt_bug = false;
        } else {
          self.registers.pc = self.registers.pc.wrapping_add(1);
        }

        current_pc
      }
    };

    hardware.read_byte(next_byte)
  }

  /// Executes the [`Instruction`], updating the internal clock state.
  pub fn execute_instruction(&mut self, hardware: &mut Hardware, instruction: &Instruction) {
    use Instruction::*;

    match instruction {
      &LD(Operand::Register(dest), Operand::Register(src)) => {
        let value = self.read_register(hardware, src);
        self.write_register(hardware, dest, value);
        self.clock.tick();

        // Add another machine cycle if we fetched or wrote to memory
        if matches!(dest, Register::M) || matches!(src, Register::M) {
          self.clock.tick();
        }
      }
      &LD(Operand::RegisterPair(rp), Operand::Word(value)) => {
        self.write_register_pair(rp, value);
        self.clock.advance(3);
      }
      &LD(Operand::RegisterPairMemory(rp), Operand::Register(Register::A)) => {
        hardware.write_byte(self.read_register_pair(rp), self.registers.a);
        self.clock.advance(2);
      }
      &LD(Operand::Register(Register::A), Operand::RegisterPairMemory(rp)) => {
        self.registers.a = hardware.read_byte(self.read_register_pair(rp));
        self.clock.advance(2);
      }
      &LD(Operand::MemoryAddress(value), Operand::RegisterPair(RegisterPair::SP)) => {
        let lower = (self.registers.sp & 0xFF) as u8;
        let upper = ((self.registers.sp & 0xFF00) >> 8) as u8;

        hardware.write_byte(value, lower);
        hardware.write_byte(value.wrapping_add(1), upper);

        self.clock.advance(5);
      }
      &LD(Operand::Register(dest), Operand::Byte(value)) => {
        self.write_register(hardware, dest, value);
        self.clock.advance(2);

        // Add another machine cycle if we wrote to memory
        if matches!(dest, Register::M) {
          self.clock.tick();
        }
      }
      &LD(Operand::RegisterPair(RegisterPair::HL), Operand::StackOffset(offset)) => {
        // The offset can be negative, so do a sign-extend add.
        let offset = offset as i8 as u16;
        let sp = self.registers.sp;
        let result = sp.wrapping_add(offset);

        self.registers.h = ((result & 0xFF00) >> 8) as u8;
        self.registers.l = (result & 0xFF) as u8;

        self.toggle_flag(Flag::Z, false);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, (sp & 0x0F) as u8 + ((offset as u8) & 0x0F) > 0x0F);
        self.toggle_flag(Flag::C, ((sp & 0xFF) + (offset & 0xFF)) > 0xFF);

        self.clock.advance(3);
      }
      LD(Operand::RegisterPair(RegisterPair::SP), Operand::RegisterPair(RegisterPair::HL)) => {
        self.registers.sp = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        self.clock.advance(2);
      }
      &LD(Operand::MemoryAddress(address), Operand::Register(Register::A)) => {
        hardware.write_byte(address, self.registers.a);
        self.clock.advance(4);
      }
      &LD(Operand::Register(Register::A), Operand::MemoryAddress(address)) => {
        self.registers.a = hardware.read_byte(address);
        self.clock.advance(4);
      }

      LDI(Operand::RegisterPairMemory(RegisterPair::HL), Operand::Register(Register::A)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;

        hardware.write_byte(address, self.registers.a);

        let res = address.wrapping_add(1);

        self.registers.h = ((res >> 8) & 0xFF) as u8;
        self.registers.l = (res & 0xFF) as u8;

        self.clock.advance(2);
      }
      LDI(Operand::Register(Register::A), Operand::RegisterPairMemory(RegisterPair::HL)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        let value = hardware.read_byte(address);

        self.registers.a = value;

        let res = address.wrapping_add(1);

        self.registers.h = ((res >> 8) & 0xFF) as u8;
        self.registers.l = (res & 0xFF) as u8;

        self.clock.advance(2);
      }
      LDD(Operand::RegisterPairMemory(RegisterPair::HL), Operand::Register(Register::A)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;

        hardware.write_byte(address, self.registers.a);

        let res = address.wrapping_sub(1);

        self.registers.h = ((res >> 8) & 0xFF) as u8;
        self.registers.l = (res & 0xFF) as u8;

        self.clock.advance(2);
      }
      LDD(Operand::Register(Register::A), Operand::RegisterPairMemory(RegisterPair::HL)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        let value = hardware.read_byte(address);

        self.registers.a = value;

        let res = address.wrapping_sub(1);

        self.registers.h = ((res >> 8) & 0xFF) as u8;
        self.registers.l = (res & 0xFF) as u8;

        self.clock.advance(2);
      }
      &LDH(Operand::HighMemoryByte(value), Operand::Register(Register::A)) => {
        hardware.write_byte(0xFF00 + value as u16, self.registers.a);
        self.clock.advance(3);
      }
      &LDH(Operand::Register(Register::A), Operand::HighMemoryByte(value)) => {
        self.registers.a = hardware.read_byte(0xFF00 + value as u16);
        self.clock.advance(3);
      }
      LDH(Operand::HighMemoryRegister(Register::C), Operand::Register(Register::A)) => {
        hardware.write_byte(0xFF00 + self.registers.c as u16, self.registers.a);
        self.clock.advance(2);
      }
      LDH(Operand::Register(Register::A), Operand::HighMemoryRegister(Register::C)) => {
        self.registers.a = hardware.read_byte(0xFF00 + self.registers.c as u16);
        self.clock.advance(2);
      }

      &ADC(Operand::Register(Register::A), Operand::Register(src)) => {
        let a_value = self.registers.a;
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
        let reg_value = self.read_register(hardware, src);
        let res = a_value
          .wrapping_add(reg_value)
          .wrapping_add(is_carry_set as u8);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(
          Flag::H,
          (a_value & 0x0F) + (reg_value & 0x0F) + (is_carry_set as u8 & 0x0F) > 0x0F,
        );
        self.toggle_flag(
          Flag::C,
          (a_value as u16 + reg_value as u16 + is_carry_set as u16) > 0xFF,
        );

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(src, Register::M) {
          self.clock.tick();
        }
      }
      &ADC(Operand::Register(Register::A), Operand::Byte(byte)) => {
        let a_value = self.registers.a;
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
        let res = a_value.wrapping_add(byte).wrapping_add(is_carry_set as u8);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(
          Flag::H,
          (a_value & 0x0F) + (byte & 0x0F) + (is_carry_set as u8 & 0x0F) > 0x0F,
        );
        self.toggle_flag(
          Flag::C,
          (a_value as u16 + byte as u16 + is_carry_set as u16) > 0xFF,
        );

        self.clock.advance(2);
      }
      &ADD(Operand::Register(Register::A), Operand::Register(src)) => {
        let a_value = self.registers.a;
        let reg_value = self.read_register(hardware, src);
        let res = a_value.wrapping_add(reg_value);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, (a_value & 0x0F) + (reg_value & 0x0F) > 0x0F);
        self.toggle_flag(Flag::C, (a_value as u16 + reg_value as u16) > 0xFF);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(src, Register::M) {
          self.clock.tick();
        }
      }
      &ADD(Operand::Register(Register::A), Operand::Byte(byte)) => {
        let a_value = self.registers.a;
        let res = a_value.wrapping_add(byte);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, (a_value & 0x0F) + (byte & 0x0F) > 0x0F);
        self.toggle_flag(Flag::C, (a_value as u16 + byte as u16) > 0xFF);

        self.clock.advance(2);
      }
      &ADD(Operand::RegisterPair(RegisterPair::HL), Operand::RegisterPair(src)) => {
        let hl_value = ((self.registers.h as u16) << 8) | self.registers.l as u16;
        let rp_value = self.read_register_pair(src);
        let res = hl_value.wrapping_add(rp_value);

        self.registers.h = ((res >> 8) & 0xFF) as u8;
        self.registers.l = (res & 0xFF) as u8;

        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, (hl_value & 0x0FFF) + (rp_value & 0x0FFF) > 0x0FFF);
        self.toggle_flag(Flag::C, res < hl_value);

        self.clock.advance(2);
      }
      &ADD(Operand::RegisterPair(RegisterPair::SP), Operand::Byte(value)) => {
        // Sign extend the number
        let num = value as i8 as u16;
        let sp_value = self.registers.sp;

        self.registers.sp = sp_value.wrapping_add(num);

        self.toggle_flag(Flag::Z, false);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, (sp_value & 0x0F) + (num & 0x0F) > 0x0F);
        self.toggle_flag(Flag::C, (sp_value & 0xFF) + (num & 0xFF) > 0xFF);

        self.clock.advance(4);
      }
      &AND(Operand::Register(Register::A), Operand::Register(src_reg)) => {
        let src_value = self.read_register(hardware, src_reg);
        let res = self.registers.a & src_value;

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, true);
        self.toggle_flag(Flag::C, false);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(src_reg, Register::M) {
          self.clock.tick();
        }
      }
      &AND(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a & value;

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, true);
        self.toggle_flag(Flag::C, false);

        self.clock.advance(2);
      }
      &CP(Operand::Register(Register::A), Operand::Register(src_reg)) => {
        let src_value = self.read_register(hardware, src_reg);
        let res = self.registers.a.wrapping_sub(src_value);

        self.clock.tick();

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, true);
        self.toggle_flag(Flag::H, (self.registers.a & 0x0F) < (src_value & 0x0F));
        self.toggle_flag(Flag::C, self.registers.a < src_value);

        // Add another machine cycle if  we fetched memory
        if matches!(src_reg, Register::M) {
          self.clock.tick();
        }
      }
      &CP(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a.wrapping_sub(value);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, true);
        self.toggle_flag(Flag::H, (self.registers.a & 0x0F) < (value & 0x0F));
        self.toggle_flag(Flag::C, self.registers.a < value);

        self.clock.advance(2);
      }
      &DEC(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = reg_value.wrapping_sub(1);

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, true);
        self.toggle_flag(Flag::H, (reg_value & 0x0F) < 1);

        self.clock.tick();

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &DEC(Operand::RegisterPair(rp)) => {
        let reg_value = self.read_register_pair(rp);
        let res = reg_value.wrapping_sub(1);

        self.write_register_pair(rp, res);
        self.clock.advance(2);
      }
      &INC(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = reg_value.wrapping_add(1);

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, (reg_value & 0x0F) == 0x0F);

        self.clock.tick();

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &INC(Operand::RegisterPair(rp)) => {
        let reg_value = self.read_register_pair(rp);
        let res = reg_value.wrapping_add(1);

        self.write_register_pair(rp, res);
        self.clock.advance(2);
      }
      &OR(Operand::Register(Register::A), Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = self.registers.a | reg_value;

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, false);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      &OR(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a | value;

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, false);

        self.clock.advance(2);
      }
      &SBC(Operand::Register(Register::A), Operand::Register(reg)) => {
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
        let reg_value = self.read_register(hardware, reg);
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(reg_value).wrapping_sub(is_carry_set);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, true);
        self.toggle_flag(
          Flag::H,
          (a_value & 0x0F) < ((reg_value & 0x0F) + is_carry_set),
        );
        self.toggle_flag(
          Flag::C,
          (a_value as u16) < (reg_value as u16 + is_carry_set as u16),
        );

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      &SBC(Operand::Register(Register::A), Operand::Byte(value)) => {
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(value).wrapping_sub(is_carry_set);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, true);
        self.toggle_flag(Flag::H, (a_value & 0x0F) < ((value & 0x0F) + is_carry_set));
        self.toggle_flag(
          Flag::C,
          (a_value as u16) < (value as u16 + is_carry_set as u16),
        );

        self.clock.advance(2);
      }
      &SUB(Operand::Register(Register::A), Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(reg_value);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, true);
        self.toggle_flag(Flag::H, (a_value & 0x0F) < (reg_value & 0x0F));
        self.toggle_flag(Flag::C, a_value < reg_value);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      &SUB(Operand::Register(Register::A), Operand::Byte(value)) => {
        let a_value = self.registers.a;
        let res = a_value.wrapping_sub(value);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, true);
        self.toggle_flag(Flag::H, (a_value & 0x0F) < (value & 0x0F));
        self.toggle_flag(Flag::C, a_value < value);

        self.clock.advance(2);
      }
      &XOR(Operand::Register(Register::A), Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = self.registers.a ^ reg_value;

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, false);

        self.clock.tick();

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      &XOR(Operand::Register(Register::A), Operand::Byte(value)) => {
        let res = self.registers.a ^ value;

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, false);

        self.clock.advance(2);
      }
      DAA => {
        let mut correction = 0;

        let subtracted = is_flag_set!(self.flags, Flag::N as u8);
        let half_carried = is_flag_set!(self.flags, Flag::H as u8);
        let mut carried = is_flag_set!(self.flags, Flag::C as u8);

        // Check the lower nibble
        if half_carried || (!subtracted && (self.registers.a & 0x0F) > 0x09) {
          correction |= 0x06;
        }

        // Check the upper nibble
        if carried || (!subtracted && self.registers.a > 0x99) {
          correction |= 0x60;
          carried = true;
        }

        let res = if subtracted {
          self.registers.a.wrapping_sub(correction)
        } else {
          self.registers.a.wrapping_add(correction)
        };

        self.registers.a = res;

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, carried);

        self.clock.tick();
      }

      &CALL(Some(Operand::Conditional(flag)), Operand::Word(address)) => {
        let should_jump = self.is_conditional_flag_set(flag);

        if should_jump {
          self.push_stack_word(hardware, self.registers.pc);

          self.registers.pc = address;
          self.clock.advance(6);
        } else {
          self.clock.advance(3);
        }
      }
      &CALL(None, Operand::Word(address)) => {
        self.push_stack_word(hardware, self.registers.pc);

        self.registers.pc = address;
        self.clock.advance(6);
      }
      &JP(Some(Operand::Conditional(flag)), Operand::Word(address)) => {
        let should_jump = self.is_conditional_flag_set(flag);

        if should_jump {
          self.registers.pc = address;
          self.clock.advance(4);
        } else {
          self.clock.advance(3);
        }
      }
      &JP(None, Operand::Word(address)) => {
        self.registers.pc = address;
        self.clock.advance(4);
      }
      JP(None, Operand::RegisterPair(RegisterPair::HL)) => {
        let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;

        self.registers.pc = address;
        self.clock.tick();
      }
      &JR(Some(Operand::Conditional(flag)), Operand::Byte(offset)) => {
        let should_jump = self.is_conditional_flag_set(flag);

        if should_jump {
          // The byte can be negative, so sign-extend add the value
          self.registers.pc = self.registers.pc.wrapping_add(offset as i8 as u16);
          self.clock.advance(3);
        } else {
          self.clock.advance(2);
        }
      }
      &JR(None, Operand::Byte(offset)) => {
        // The byte can be negative, so sign-extend add the value
        self.registers.pc = self.registers.pc.wrapping_add(offset as i8 as u16);
        self.clock.advance(3);
      }
      &RET(Some(Operand::Conditional(flag))) => {
        let should_jump = self.is_conditional_flag_set(flag);

        if should_jump {
          let addr = self.pop_stack_word(hardware);

          self.registers.pc = addr;
          self.clock.advance(5);
        } else {
          self.clock.advance(2);
        }
      }
      RET(None) => {
        let addr = self.pop_stack_word(hardware);

        self.registers.pc = addr;
        self.clock.advance(4);
      }
      RETI => {
        let addr = self.pop_stack_word(hardware);

        self.registers.pc = addr;
        self.master_interrupt_enabled = true;
        self.clock.advance(4);
      }
      &RST(Operand::Byte(target)) => {
        self.push_stack_word(hardware, self.registers.pc);

        self.registers.pc = target as u16;
        self.clock.advance(4);
      }
      STOP(Operand::Byte(_)) => {
        self.state = CpuState::Stopped;
        self.clock.tick();
      }
      HALT => {
        // The Gameboy has a hardware bug when executing the `HALT` instruction.
        //
        // That is, when the master interrupt flag isn't enabled and the program
        // tries to halt while there is an interrupt pending, it will fail to
        // halt and enter a bugged state.
        //
        // In this bugged state, the program counter is NOT incremented after
        // fetching the next byte.
        //
        // See https://gbdev.io/pandocs/halt.html for more.
        if !self.master_interrupt_enabled && hardware.has_pending_interrupts() {
          self.halt_bug = true;
          // The CPU doesn't actually enter a halted state in the case of a bugged
          // halt instruction.
          self.state = CpuState::Running;
        } else {
          self.state = CpuState::Halted;
        }

        self.clock.tick();
      }
      NOP => {
        self.clock.tick();
      }

      &POP(Operand::RegisterPair(rp)) => {
        let value = self.pop_stack_word(hardware);

        self.write_register_pair(rp, value);

        self.clock.advance(3);
      }
      &PUSH(Operand::RegisterPair(rp)) => {
        let reg_value = self.read_register_pair(rp);

        self.push_stack_word(hardware, reg_value);

        self.clock.advance(4);
      }
      CCF => {
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, !is_flag_set!(self.flags, Flag::C as u8));

        self.clock.tick();
      }
      CPL => {
        self.registers.a = !self.registers.a;

        self.toggle_flag(Flag::N, true);
        self.toggle_flag(Flag::H, true);

        self.clock.tick();
      }
      DI => {
        self.master_interrupt_enabled = false;
        self.clock.tick();
      }
      EI => {
        // We shouldn't actually update the master interrupt flag immediately
        // because this instruction seems to have a delay of 4 T-cycles
        self.clock.tick();
      }
      SCF => {
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, true);

        self.clock.tick();
      }

      RLA => {
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
        let a_value = self.registers.a;
        let res = (a_value << 1) | (is_carry_set as u8);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, false);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (a_value >> 7) == 0x1);

        self.clock.tick();
      }
      RLCA => {
        let a_value = self.registers.a;
        let res = a_value.rotate_left(1);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, false);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (a_value >> 7) == 0x1);

        self.clock.tick();
      }
      RRA => {
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
        let a_value = self.registers.a;
        let res = (a_value >> 1) | ((is_carry_set as u8) << 7);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, false);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (a_value & 0x1) == 1);

        self.clock.tick();
      }
      RRCA => {
        let a_value = self.registers.a;
        let res = a_value.rotate_right(1);

        self.registers.a = res;

        self.toggle_flag(Flag::Z, false);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (a_value & 0x1) == 1);

        self.clock.tick();
      }

      &BIT(Operand::Byte(bit), Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let extracted_bit = (reg_value >> bit) & 1;

        self.toggle_flag(Flag::Z, extracted_bit == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, true);

        self.clock.advance(2);

        // Add another machine cycle if we fetched memory
        if matches!(reg, Register::M) {
          self.clock.tick();
        }
      }
      &RES(Operand::Byte(bit), Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let new_value = reg_value & !(1 << bit);

        self.write_register(hardware, reg, new_value);
        self.clock.advance(2);

        // Advance 2 machine cycles since we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &SET(Operand::Byte(bit), Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let new_value = reg_value | (1 << bit);

        self.write_register(hardware, reg, new_value);
        self.clock.advance(2);

        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &RL(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
        let res = (reg_value << 1) | is_carry_set;

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (reg_value >> 7) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &RLC(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = reg_value.rotate_left(1);

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (reg_value >> 7) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &RR(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
        let res = (reg_value >> 1) | (is_carry_set << 7);

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &RRC(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = reg_value.rotate_right(1);

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &SLA(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = reg_value << 1;

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (reg_value >> 7) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &SRA(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        // SRA preserves the sign bit (MSB)
        let res = (reg_value >> 1) | (reg_value & 0x80);

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &SRL(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let res = reg_value >> 1;

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);

        self.clock.advance(2);

        // Add 2 machine cycles if we fetched and wrote to memory
        if matches!(reg, Register::M) {
          self.clock.advance(2);
        }
      }
      &SWAP(Operand::Register(reg)) => {
        let reg_value = self.read_register(hardware, reg);
        let lower = reg_value & 0x0F;
        let upper = reg_value & 0xF0;
        let res = (lower << 4) | (upper >> 4);

        self.write_register(hardware, reg, res);

        self.toggle_flag(Flag::Z, res == 0);
        self.toggle_flag(Flag::N, false);
        self.toggle_flag(Flag::H, false);
        self.toggle_flag(Flag::C, false);

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
  pub fn decode_instruction(&self, byte: u8, hardware: &Hardware) -> Instruction {
    match byte {
      // LD r8, r8
      0x40..0x76 | 0x77..=0x7F => {
        let dest_reg = Register::from_bits((byte >> 3) & 0x7).unwrap();
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::LD(Operand::Register(dest_reg), Operand::Register(src_reg))
      }
      // LD r16, n16
      0x01 | 0x11 | 0x21 | 0x31 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0x3, false).unwrap();
        let n16 = hardware.read_word(self.registers.pc);

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
        let n16 = hardware.read_word(self.registers.pc);

        Instruction::LD(
          Operand::MemoryAddress(n16),
          Operand::RegisterPair(RegisterPair::SP),
        )
      }
      // LD r8 | [HL], n8
      0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E => {
        let dest_reg = Register::from_bits((byte >> 3) & 0x7).unwrap();
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::LD(Operand::Register(dest_reg), Operand::Byte(n8))
      }
      // LD HL, SP + n8
      0xF8 => {
        let n8 = hardware.read_byte(self.registers.pc);

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
        let n16 = hardware.read_word(self.registers.pc);

        Instruction::LD(Operand::MemoryAddress(n16), Operand::Register(Register::A))
      }
      // LD A, [n16]
      0xFA => {
        let n16 = hardware.read_word(self.registers.pc);

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
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::LDH(Operand::HighMemoryByte(n8), Operand::Register(Register::A))
      }
      // LDH A, [0xFF00 + n8]
      0xF0 => {
        let n8 = hardware.read_byte(self.registers.pc);

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
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::ADC(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // ADC A, n8
      0xCE => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::ADC(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // ADD A, r8 | [HL]
      0x80..=0x87 => {
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::ADD(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // ADD A, n8
      0xC6 => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::ADD(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // ADD HL, r16
      0x09 | 0x19 | 0x29 | 0x39 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0x3, false).unwrap();

        Instruction::ADD(
          Operand::RegisterPair(RegisterPair::HL),
          Operand::RegisterPair(r16),
        )
      }
      // ADD SP, n8
      0xE8 => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::ADD(Operand::RegisterPair(RegisterPair::SP), Operand::Byte(n8))
      }
      // AND A, r8 | [HL]
      0xA0..=0xA7 => {
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::AND(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // AND A, n8
      0xE6 => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::AND(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // CP A, r8 | [HL]
      0xB8..=0xBF => {
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::CP(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // CP A, n8
      0xFE => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::CP(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // DEC r8
      0x05 | 0x15 | 0x25 | 0x35 | 0x0D | 0x1D | 0x2D | 0x3D => {
        let dst_reg = Register::from_bits((byte >> 3) & 0x7).unwrap();

        Instruction::DEC(Operand::Register(dst_reg))
      }
      // DEC r16
      0x0B | 0x1B | 0x2B | 0x3B => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0x3, false).unwrap();

        Instruction::DEC(Operand::RegisterPair(r16))
      }
      // INC r8
      0x04 | 0x14 | 0x24 | 0x34 | 0x0C | 0x1C | 0x2C | 0x3C => {
        let dst_reg = Register::from_bits((byte >> 3) & 0x7).unwrap();

        Instruction::INC(Operand::Register(dst_reg))
      }
      // INC r16
      0x03 | 0x13 | 0x23 | 0x33 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0x3, false).unwrap();

        Instruction::INC(Operand::RegisterPair(r16))
      }
      // OR A, r8 | [HL]
      0xB0..=0xB7 => {
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::OR(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // OR A, n8
      0xF6 => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::OR(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // SBC A, r8 | [HL]
      0x98..=0x9F => {
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::SBC(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // SBC A, n8
      0xDE => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::SBC(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // SUB A, r8 | [HL]
      0x90..=0x97 => {
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::SUB(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // SUB A, n8
      0xD6 => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::SUB(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // XOR A, r8 | [HL]
      0xA8..=0xAF => {
        let src_reg = Register::from_bits(byte & 0x7).unwrap();

        Instruction::XOR(Operand::Register(Register::A), Operand::Register(src_reg))
      }
      // XOR A, n8
      0xEE => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::XOR(Operand::Register(Register::A), Operand::Byte(n8))
      }
      // DAA
      0x27 => Instruction::DAA,

      // CALL cf, n16
      0xC4 | 0xD4 | 0xCC | 0xDC => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0x3).unwrap();
        let n16 = hardware.read_word(self.registers.pc);

        Instruction::CALL(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // CALL n16
      0xCD => {
        let n16 = hardware.read_word(self.registers.pc);

        Instruction::CALL(None, Operand::Word(n16))
      }
      // JP cf, n16
      0xC2 | 0xD2 | 0xCA | 0xDA => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0x3).unwrap();
        let n16 = hardware.read_word(self.registers.pc);

        Instruction::JP(Some(Operand::Conditional(cond_flag)), Operand::Word(n16))
      }
      // JP n16
      0xC3 => {
        let n16 = hardware.read_word(self.registers.pc);

        Instruction::JP(None, Operand::Word(n16))
      }
      // JP HL
      0xE9 => Instruction::JP(None, Operand::RegisterPair(RegisterPair::HL)),
      // JR cf, n8
      0x20 | 0x30 | 0x28 | 0x38 => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0x3).unwrap();
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::JR(Some(Operand::Conditional(cond_flag)), Operand::Byte(n8))
      }
      // JR n8
      0x18 => {
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::JR(None, Operand::Byte(n8))
      }
      // RET cf
      0xC0 | 0xD0 | 0xC8 | 0xD8 => {
        let cond_flag = ConditionalFlag::from_bits((byte >> 3) & 0x3).unwrap();

        Instruction::RET(Some(Operand::Conditional(cond_flag)))
      }
      // RET
      0xC9 => Instruction::RET(None),
      // RETI
      0xD9 => Instruction::RETI,
      // RST 0x0 | 0x10 | 0x20 | 0x30 | 0x08 | 0x18 | 0x28 | 0x38
      0xC7 | 0xD7 | 0xE7 | 0xF7 | 0xCF | 0xDF | 0xEF | 0xFF => {
        // The target is encoded in bits 3 through 5.
        let target = byte & 0b11_1000;

        Instruction::RST(Operand::Byte(target))
      }
      // STOP n8
      0x10 => {
        // NOTE: `STOP` needs to be followed by another byte.
        let n8 = hardware.read_byte(self.registers.pc);

        Instruction::STOP(Operand::Byte(n8))
      }
      // HALT
      0x76 => Instruction::HALT,
      // NOP
      0x0 => Instruction::NOP,

      // POP r16
      0xC1 | 0xD1 | 0xE1 | 0xF1 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0x3, true).unwrap();

        Instruction::POP(Operand::RegisterPair(r16))
      }
      // PUSH r16
      0xC5 | 0xD5 | 0xE5 | 0xF5 => {
        let r16 = RegisterPair::from_bits((byte >> 4) & 0x3, true).unwrap();

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
        let next_byte = hardware.read_byte(self.registers.pc);

        match next_byte {
          // BIT 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7, r8 | [HL]
          0x40..=0x7F => {
            let bit_num = (next_byte >> 3) & 0x7;
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::BIT(Operand::Byte(bit_num), Operand::Register(src_reg))
          }
          // RES 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7, r8 | [HL]
          0x80..=0xBF => {
            let bit_num = (next_byte >> 3) & 0x7;
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::RES(Operand::Byte(bit_num), Operand::Register(src_reg))
          }
          // SET 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7, r8 | [HL]
          0xC0..=0xFF => {
            let bit_num = (next_byte >> 3) & 0x7;
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::SET(Operand::Byte(bit_num), Operand::Register(src_reg))
          }
          // RL r8 | [HL]
          0x10..=0x17 => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::RL(Operand::Register(src_reg))
          }
          // RLC r8 | [HL]
          0x00..=0x07 => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::RLC(Operand::Register(src_reg))
          }
          // RR r8 | [HL]
          0x18..=0x1F => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::RR(Operand::Register(src_reg))
          }
          // RRC r8 | [HL]
          0x08..=0x0F => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::RRC(Operand::Register(src_reg))
          }
          // SLA r8 | [HL]
          0x20..=0x27 => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::SLA(Operand::Register(src_reg))
          }
          // SRA r8 | [HL]
          0x28..=0x2F => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::SRA(Operand::Register(src_reg))
          }
          // SRL r8 | [HL]
          0x38..=0x3F => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::SRL(Operand::Register(src_reg))
          }
          // SWAP r8 | [HL]
          0x30..=0x37 => {
            let src_reg = Register::from_bits(next_byte & 0x7).unwrap();

            Instruction::SWAP(Operand::Register(src_reg))
          }
        }
      }

      // Unused opcodes
      0xD3 | 0xE3 | 0xE4 | 0xF4 | 0xDB | 0xEB | 0xEC | 0xFC | 0xDD | 0xED | 0xFD => {
        // NOTE: Is mapping to a `NOP` correct? Supposedly unknown opcodes hang the CPU?
        Instruction::NOP
      }
    }
  }

  /// Handles any of the currently requested interrupts.
  pub fn handle_interrupts(&mut self, hardware: &mut Hardware) {
    // Interrupts with a smaller bit value have higher priority.
    //
    // The master interrupt is disabled before calling the handler of the interrupt.
    //
    // See https://gbdev.io/pandocs/Interrupts.html for more.
    if hardware.is_interrupt_requested(Interrupt::VBlank) {
      self.state = CpuState::Running;

      if self.master_interrupt_enabled {
        self.master_interrupt_enabled = false;

        hardware.clear_interrupt(Interrupt::VBlank);

        self.push_stack_word(hardware, self.registers.pc);

        self.registers.pc = 0x40;
      }
    } else if hardware.is_interrupt_requested(Interrupt::Lcd) {
      self.state = CpuState::Running;

      if self.master_interrupt_enabled {
        self.master_interrupt_enabled = false;

        hardware.clear_interrupt(Interrupt::Lcd);

        self.push_stack_word(hardware, self.registers.pc);

        self.registers.pc = 0x48;
      }
    } else if hardware.is_interrupt_requested(Interrupt::Timer) {
      self.state = CpuState::Running;

      if self.master_interrupt_enabled {
        self.master_interrupt_enabled = false;

        hardware.clear_interrupt(Interrupt::Timer);

        self.push_stack_word(hardware, self.registers.pc);

        self.registers.pc = 0x50;
      }
    } else if hardware.is_interrupt_requested(Interrupt::Serial) {
      self.state = CpuState::Running;

      if self.master_interrupt_enabled {
        self.master_interrupt_enabled = false;

        hardware.clear_interrupt(Interrupt::Serial);

        self.push_stack_word(hardware, self.registers.pc);

        self.registers.pc = 0x58;
      }
    } else if hardware.is_interrupt_requested(Interrupt::Joypad) {
      self.state = CpuState::Running;

      if self.master_interrupt_enabled {
        self.master_interrupt_enabled = false;

        hardware.clear_interrupt(Interrupt::Joypad);

        self.push_stack_word(hardware, self.registers.pc);

        self.registers.pc = 0x60;
      }
    }
  }

  /// Sets the default register values.
  pub fn set_register_defaults(&mut self) {
    // These values were taken from "The Cycle-Accurate Game Boy Docs"
    self.registers.a = 0x01;
    self.flags = 0xB0;

    self.registers.b = 0x00;
    self.registers.c = 0x13;

    self.registers.d = 0x00;
    self.registers.e = 0xD8;

    self.registers.h = 0x01;
    self.registers.l = 0x4D;

    self.registers.sp = 0xFFFE;

    self.registers.pc = 0x100;
  }

  /// The state of the CPU.
  pub fn state(&self) -> CpuState {
    self.state
  }

  /// Reads the value of the [`Register`].
  fn read_register(&self, hardware: &Hardware, register: Register) -> u8 {
    match register {
      Register::A => self.registers.a,
      Register::B => self.registers.b,
      Register::C => self.registers.c,
      Register::D => self.registers.d,
      Register::E => self.registers.e,
      Register::H => self.registers.h,
      Register::L => self.registers.l,
      Register::M => {
        let address = ((self.registers.h as u16) << 8) | (self.registers.l as u16);

        hardware.read_byte(address)
      }
    }
  }

  /// Writes the value to the [`Register`].
  fn write_register(&mut self, hardware: &mut Hardware, register: Register, value: u8) {
    match register {
      Register::A => self.registers.a = value,
      Register::B => self.registers.b = value,
      Register::C => self.registers.c = value,
      Register::D => self.registers.d = value,
      Register::E => self.registers.e = value,
      Register::H => self.registers.h = value,
      Register::L => self.registers.l = value,
      Register::M => {
        let address = ((self.registers.h as u16) << 8) | (self.registers.l as u16);

        hardware.write_byte(address, value);
      }
    }
  }

  /// Reads the value of the [`RegisterPair`].
  fn read_register_pair(&self, register_pair: RegisterPair) -> u16 {
    match register_pair {
      RegisterPair::AF => ((self.registers.a as u16) << 8) | (self.flags as u16),
      RegisterPair::BC => ((self.registers.b as u16) << 8) | (self.registers.c as u16),
      RegisterPair::DE => ((self.registers.d as u16) << 8) | (self.registers.e as u16),
      RegisterPair::HL => ((self.registers.h as u16) << 8) | (self.registers.l as u16),
      RegisterPair::SP => self.registers.sp,
    }
  }

  /// Writes the value to the following [`RegisterPair`].
  fn write_register_pair(&mut self, register_pair: RegisterPair, value: u16) {
    match register_pair {
      RegisterPair::AF => {
        self.registers.a = ((value >> 8) & 0xFF) as u8;
        self.flags = (value & 0xF0) as u8;
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

  /// Pops 16-bits of memory from the stack.
  fn pop_stack_word(&mut self, hardware: &Hardware) -> u16 {
    let lower = hardware.read_byte(self.registers.sp);
    let upper = hardware.read_byte(self.registers.sp.wrapping_add(1));
    let word = ((upper as u16) << 8) | lower as u16;

    self.registers.sp = self.registers.sp.wrapping_add(2);

    word
  }

  /// Pushes the 16-bit value on to the stack.
  fn push_stack_word(&mut self, hardware: &mut Hardware, value: u16) {
    let upper = ((value >> 8) & 0xFF) as u8;
    let lower = (value & 0xFF) as u8;

    hardware.write_byte(self.registers.sp.wrapping_sub(1), upper);
    hardware.write_byte(self.registers.sp.wrapping_sub(2), lower);

    self.registers.sp = self.registers.sp.wrapping_sub(2);
  }

  /// Returns whether the following [`ConditionalFlag`] is set.
  fn is_conditional_flag_set(&self, cond_flag: ConditionalFlag) -> bool {
    match cond_flag {
      ConditionalFlag::Z => is_flag_set!(self.flags, Flag::Z as u8),
      ConditionalFlag::C => is_flag_set!(self.flags, Flag::C as u8),
      ConditionalFlag::NZ => !is_flag_set!(self.flags, Flag::Z as u8),
      ConditionalFlag::NC => !is_flag_set!(self.flags, Flag::C as u8),
    }
  }

  /// Conditionally toggles the flag.
  fn toggle_flag(&mut self, flag: Flag, condition: bool) {
    if condition {
      add_flag!(&mut self.flags, flag as u8);
    } else {
      remove_flag!(&mut self.flags, flag as u8);
    }
  }
}

/// The internal time clock.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct ClockState {
  /// Machine cycles.
  pub m_cycles: usize,
  /// Tick cycles.
  pub t_cycles: usize,
}

impl ClockState {
  /// Advances the internal state by 1 M-cycle.
  pub fn tick(&mut self) {
    self.advance(1);
  }

  /// Advance the internal state by the following amount of M-cycles.
  pub fn advance(&mut self, m_cycles: usize) {
    self.m_cycles = self.m_cycles.wrapping_add(m_cycles);
    self.t_cycles = self.t_cycles.wrapping_add(m_cycles * 4);
  }
}
