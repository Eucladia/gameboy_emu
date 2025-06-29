use crate::{
  flags::{ConditionalFlag, Flag, add_flag, is_flag_set, remove_flag},
  hardware::{
    Hardware,
    registers::{self, Registers},
  },
  interrupts::Interrupt,
};
use macros::*;

/// A state that the CPU can be in.
#[derive(Debug, Copy, Clone)]
pub enum CpuState {
  /// The CPU is processing instructions.
  Running,
  /// The CPU is halted.
  Halted,
  /// The CPU is stopped.
  Stopped,
  /// The CPU is processing interrupts.
  HandlingInterrupts,
}

#[derive(Debug)]
pub struct Cpu {
  /// The enabled flags.
  flags: u8,
  /// The set of registers.
  pub registers: Registers,
  /// The state of the CPU.
  state: CpuState,
  /// Whether the CPU is in a bugged halt state.
  halt_bug: bool,
  /// Master interrupt flag.
  interrupt_master_enabled: bool,
  // The amount of T-cycles elapsed.
  t_cycles: usize,

  // Stuff for T-cycle accuracy
  /// The current cycle of the CPU during execution.
  cycle: CpuCycle,
  /// Whether the CPU should handle interrupts on the next M-cycle.
  should_handle_interrupts: bool,
  /// Whether the next instruction should be parsed from the extended instruction set.
  saw_prefix_opcode: bool,
  /// The last executed instruction.
  last_instruction: u8,
  /// Whether the initial instruction was fetched.
  initial_fetch: bool,
  /// Temporary storage to store things in-between M-cycles when executing instructions.
  data_buffer: [u8; 2],
}

/// A machine cycle when stepping the CPU's instruction or interrupt handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CpuCycle {
  // Machine cycle 1.
  M1,
  // Machine cycle 2.
  M2,
  // Machine cycle 3.
  M3,
  // Machine cycle 4.
  M4,
  // Machine cycle 5.
  M5,
  // Machine cycle 6.
  M6,
}

impl Cpu {
  /// Creates a new [`Cpu`] in a running state.
  pub fn new() -> Self {
    Self {
      flags: 0,
      state: CpuState::Running,
      registers: Registers::default(),
      halt_bug: false,
      interrupt_master_enabled: false,
      t_cycles: 0,

      cycle: CpuCycle::M1,
      should_handle_interrupts: false,
      last_instruction: 0x00,
      initial_fetch: false,
      data_buffer: [0; 2],
      saw_prefix_opcode: false,
    }
  }

  /// Creates a new [`Cpu`] with the default register values set.
  pub fn with_register_defaults() -> Self {
    let mut cpu = Self::new();

    cpu.set_register_defaults();

    cpu
  }

  // TODO: Once we have a boot rom, we shouldn't need this function.
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

  /// Steps the CPU by 1 T-cycle.
  pub fn step(&mut self, hardware: &mut Hardware) {
    self.t_cycles = self.t_cycles.wrapping_add(1);

    match self.t_cycles % 4 {
      1 | 2 => {}
      3 => {
        // Perform an initial fetch to avoid the assumption that the first instruction
        // at address 0x0100 will always be a `NOP`.
        if !self.initial_fetch {
          self.fetch_cycle(hardware);
          self.initial_fetch = true;
        }

        // The check for interrupts supposedly occur during T3 from the end of the
        // previous instruction's fetch, so lets transition into the appropriate
        // state if we need to handle interrupts.
        if self.should_handle_interrupts {
          self.state = CpuState::HandlingInterrupts;
        }
      }
      0 => {
        // The `EI` instruction has a delay of 4 T-cycles.
        if self.last_instruction == 0xFB {
          self.interrupt_master_enabled = true;
        }

        match self.state {
          CpuState::Running => self.step_instruction(hardware),
          CpuState::HandlingInterrupts => self.step_interrupts(hardware),
          CpuState::Halted => {
            if hardware.has_pending_interrupts() {
              // If the CPU was successfully halted and there weren't any immediate
              // interrupts following the completion of the `HALT` instruction, and
              // we now have some pending interrupts, then we should start handling
              // interrupts if the IME is set.
              //
              // If the IME is not set, then we should exit out of the halted state,
              // since it should have been 4 T-cycles by now and we have pending interrupts.
              if self.interrupt_master_enabled {
                self.should_handle_interrupts = true;
              } else {
                self.state = CpuState::Running;
              }
            }
          }
          CpuState::Stopped => {}
        }
      }
      _ => unreachable!(),
    }
  }

  /// Steps an instruction by 1 M-cycle.
  pub fn step_instruction(&mut self, hardware: &mut Hardware) {
    use CpuCycle::*;

    let opcode = self.registers.ir;

    match (self.saw_prefix_opcode, opcode) {
      // LD r8 | [HL], r8 | [HL]
      (false, 0x40..0x76 | 0x77..=0x7F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) || is_dest_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => write_to_register!(
                &mut self.registers, extract_dest_register!(opcode), reg_value
              )
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          if is_src_register_memory!(opcode) {
            let src_value = self.read_memory_register(hardware);

            write_to_register!(
              &mut self.registers,
              extract_dest_register!(opcode),
              src_value
            );
          } else {
            // We're writing to register M here
            perform_with_register!(
              &self.registers,
               extract_src_register!(opcode),
              (reg_value) => self.write_memory_register(hardware, reg_value)
            );
          }

          self.fetch_cycle(hardware);
        }
      }
      // LD r16, imm16
      (false, 0x01 | 0x11 | 0x21 | 0x31) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            self.registers.c = lower;
          } else if rp == registers::REGISTER_PAIR_DE {
            self.registers.e = lower;
          } else if rp == registers::REGISTER_PAIR_HL {
            self.registers.l = lower;
          } else if rp == registers::REGISTER_PAIR_SP {
            self.registers.sp = (self.registers.sp & 0xFF00) | (lower as u16)
          }

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            self.registers.b = upper;
          } else if rp == registers::REGISTER_PAIR_DE {
            self.registers.d = upper;
          } else if rp == registers::REGISTER_PAIR_HL {
            self.registers.h = upper;
          } else if rp == registers::REGISTER_PAIR_SP {
            self.registers.sp = (self.registers.sp & 0x00FF) | ((upper as u16) << 8);
          }

          self.fetch_cycle(hardware);
        }
      }
      // LD [r16], A
      (false, 0x02 | 0x12) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            let address = ((self.registers.b as u16) << 8) | (self.registers.c as u16);

            hardware.write_byte(address, self.registers.a);
          } else if rp == registers::REGISTER_PAIR_DE {
            let address = ((self.registers.d as u16) << 8) | (self.registers.e as u16);

            hardware.write_byte(address, self.registers.a);
          }

          self.fetch_cycle(hardware);
        }
      }
      // LD A, [r16]
      (false, 0x0A | 0x1A) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            let address = ((self.registers.b as u16) << 8) | (self.registers.c as u16);
            let value = hardware.read_byte(address);

            self.registers.a = value;
          } else if rp == registers::REGISTER_PAIR_DE {
            let address = ((self.registers.d as u16) << 8) | (self.registers.e as u16);
            let value = hardware.read_byte(address);

            self.registers.a = value;
          }

          self.fetch_cycle(hardware);
        }
      }
      // LD [imm16], SP
      (false, 0x08) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);

          self.data_buffer[0] = lower;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);

          self.data_buffer[1] = upper;

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);
          let lower_sp = (self.registers.sp & 0x00FF) as u8;

          hardware.write_byte(address, lower_sp);

          self.cycle = M5;
        } else if matches!(self.cycle, M5) {
          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);
          let upper_sp = (self.registers.sp >> 8) as u8;

          hardware.write_byte(address.wrapping_add(1), upper_sp);

          self.fetch_cycle(hardware);
        }
      }
      // LD r8 | [HL], imm8
      (false, 0x06 | 0x16 | 0x26 | 0x36 | 0x0E | 0x1E | 0x2E | 0x3E) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);

          if is_dest_register_memory!(opcode) {
            self.data_buffer[0] = imm8;

            self.cycle = M3;
          } else {
            write_to_register!(&mut self.registers, extract_dest_register!(opcode), imm8);

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M3) {
          let imm8 = self.data_buffer[0];

          self.write_memory_register(hardware, imm8);

          self.fetch_cycle(hardware);
        }
      }
      // LD HL, SP + imm8
      (false, 0xF8) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);

          self.data_buffer[0] = imm8;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          // The offset can be negative, so do a sign-extend add.
          let offset = self.data_buffer[0] as i8 as u16;
          let sp = self.registers.sp;
          let result = sp.wrapping_add(offset);

          self.registers.h = (result >> 8) as u8;
          self.registers.l = (result & 0x00FF) as u8;

          self.toggle_flag(Flag::Z, false);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, (sp & 0x0F) as u8 + ((offset as u8) & 0x0F) > 0x0F);
          self.toggle_flag(Flag::C, ((sp & 0xFF) + (offset & 0xFF)) > 0xFF);

          self.fetch_cycle(hardware);
        }
      }
      // LD SP, HL
      (false, 0xF9) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          self.registers.sp = ((self.registers.h as u16) << 8) | self.registers.l as u16;

          self.fetch_cycle(hardware);
        }
      }
      // LD [imm16], A
      (false, 0xEA) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);

          self.data_buffer[0] = lower;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);

          self.data_buffer[1] = upper;

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);

          hardware.write_byte(address, self.registers.a);

          self.fetch_cycle(hardware);
        }
      }
      // LD A, [imm16]
      (false, 0xFA) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);

          self.data_buffer[0] = lower;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);

          self.data_buffer[1] = upper;

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);

          self.registers.a = hardware.read_byte(address);

          self.fetch_cycle(hardware);
        }
      }

      // LDI [HL], A
      (false, 0x22) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let hl_value = ((self.registers.h as u16) << 8) | self.registers.l as u16;

          hardware.write_byte(hl_value, self.registers.a);

          // Increment HL and write it back
          let res = hl_value.wrapping_add(1);

          self.registers.h = (res >> 8) as u8;
          self.registers.l = (res & 0x00FF) as u8;

          self.fetch_cycle(hardware);
        }
      }
      // LDI A, [HL]
      (false, 0x2A) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let hl_value = ((self.registers.h as u16) << 8) | self.registers.l as u16;

          self.registers.a = hardware.read_byte(hl_value);

          let res = hl_value.wrapping_add(1);

          self.registers.h = (res >> 8) as u8;
          self.registers.l = (res & 0x00FF) as u8;

          self.fetch_cycle(hardware);
        }
      }
      // LDD [HL], A
      (false, 0x32) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let hl_value = ((self.registers.h as u16) << 8) | self.registers.l as u16;

          hardware.write_byte(hl_value, self.registers.a);

          let res = hl_value.wrapping_sub(1);

          self.registers.h = (res >> 8) as u8;
          self.registers.l = (res & 0x00FF) as u8;

          self.fetch_cycle(hardware);
        }
      }
      // LDD A, [HL]
      (false, 0x3A) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let hl_value = ((self.registers.h as u16) << 8) | self.registers.l as u16;
          let value = hardware.read_byte(hl_value);

          self.registers.a = value;

          let res = hl_value.wrapping_sub(1);

          self.registers.h = (res >> 8) as u8;
          self.registers.l = (res & 0x00FF) as u8;

          self.fetch_cycle(hardware);
        }
      }
      // LDH [0xFF00 + imm8], A
      (false, 0xE0) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);

          self.data_buffer[0] = imm8;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let imm8 = self.data_buffer[0];

          hardware.write_byte(0xFF00 + imm8 as u16, self.registers.a);

          self.fetch_cycle(hardware);
        }
      }
      // LDH A, [0xFF00 + imm8]
      (false, 0xF0) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);

          self.data_buffer[0] = imm8;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let imm8 = self.data_buffer[0];

          self.registers.a = hardware.read_byte(0xFF00 + imm8 as u16);

          self.fetch_cycle(hardware);
        }
      }
      // LDH [0xFF00 + C], A
      (false, 0xE2) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          hardware.write_byte(0xFF00 + self.registers.c as u16, self.registers.a);

          self.fetch_cycle(hardware);
        }
      }
      // LDH A, [0xFF00 + C]
      (false, 0xF2) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          self.registers.a = hardware.read_byte(0xFF00 + self.registers.c as u16);

          self.fetch_cycle(hardware);
        }
      }

      // ADC A, r | [HL]
      (false, 0x88..=0x8F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let a_value = self.registers.a;
            let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);

            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
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
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let a_value = self.registers.a;
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
          let reg_value = self.read_memory_register(hardware);
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

          self.fetch_cycle(hardware);
        }
      }
      // ADC A, imm8
      (false, 0xCE) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);
          let a_value = self.registers.a;
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
          let res = a_value.wrapping_add(imm8).wrapping_add(is_carry_set as u8);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(
            Flag::H,
            (a_value & 0x0F) + (imm8 & 0x0F) + (is_carry_set as u8 & 0x0F) > 0x0F,
          );
          self.toggle_flag(
            Flag::C,
            (a_value as u16 + imm8 as u16 + is_carry_set as u16) > 0xFF,
          );

          self.fetch_cycle(hardware);
        }
      }
      // ADD A, r8 | [HL]
      (false, 0x80..=0x87) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let a_value = self.registers.a;
                let res = a_value.wrapping_add(reg_value);

                self.registers.a = res;

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, (a_value & 0x0F) + (reg_value & 0x0F) > 0x0F);
                self.toggle_flag(Flag::C, (a_value as u16 + reg_value as u16) > 0xFF);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let a_value = self.registers.a;
          let reg_value = self.read_memory_register(hardware);
          let res = a_value.wrapping_add(reg_value);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, (a_value & 0x0F) + (reg_value & 0x0F) > 0x0F);
          self.toggle_flag(Flag::C, (a_value as u16 + reg_value as u16) > 0xFF);

          self.fetch_cycle(hardware);
        }
      }
      // ADD A, imm8
      (false, 0xC6) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);
          let a_value = self.registers.a;
          let res = a_value.wrapping_add(imm8);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, (a_value & 0x0F) + (imm8 & 0x0F) > 0x0F);
          self.toggle_flag(Flag::C, (a_value as u16 + imm8 as u16) > 0xFF);

          self.fetch_cycle(hardware);
        }
      }
      // ADD HL, r16
      (false, 0x09 | 0x19 | 0x29 | 0x39) => {
        if matches!(self.cycle, M1) {
          let l_value = self.registers.l;
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            let src_lower_byte = self.registers.c;
            let result = l_value.wrapping_add(src_lower_byte);

            self.registers.l = result;

            self.toggle_flag(Flag::H, ((l_value & 0x0F) + (src_lower_byte & 0x0F)) > 0x0F);
            self.toggle_flag(Flag::C, (l_value as u16 + src_lower_byte as u16) > 0xFF);
          } else if rp == registers::REGISTER_PAIR_DE {
            let src_lower_byte = self.registers.e;
            let result = l_value.wrapping_add(src_lower_byte);

            self.registers.l = result;

            self.toggle_flag(Flag::H, ((l_value & 0x0F) + (src_lower_byte & 0x0F)) > 0x0F);
            self.toggle_flag(Flag::C, (l_value as u16 + src_lower_byte as u16) > 0xFF);
          } else if rp == registers::REGISTER_PAIR_HL {
            let src_lower_byte = self.registers.l;
            let result = l_value.wrapping_add(src_lower_byte);

            self.registers.l = result;

            self.toggle_flag(Flag::H, ((l_value & 0x0F) + (src_lower_byte & 0x0F)) > 0x0F);
            self.toggle_flag(Flag::C, (l_value as u16 + src_lower_byte as u16) > 0xFF);
          } else if rp == registers::REGISTER_PAIR_SP {
            let src_lower_byte = (self.registers.sp & 0x00FF) as u8;
            let result = l_value.wrapping_add(src_lower_byte);

            self.registers.l = result;

            self.toggle_flag(Flag::H, ((l_value & 0x0F) + (src_lower_byte & 0x0F)) > 0x0F);
            self.toggle_flag(Flag::C, (l_value as u16 + src_lower_byte as u16) > 0xFF);
          }

          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let rp = extract_register_pair!(opcode);
          let h_value = self.registers.h;
          let carry_value = is_flag_set!(self.flags, Flag::C as u8) as u8;

          if rp == registers::REGISTER_PAIR_BC {
            let src_upper_byte = self.registers.b;
            let result = h_value
              .wrapping_add(src_upper_byte)
              .wrapping_add(carry_value);

            self.registers.h = result;

            self.toggle_flag(
              Flag::H,
              ((h_value & 0x0F) + (src_upper_byte & 0x0F) + carry_value) > 0x0F,
            );
            self.toggle_flag(
              Flag::C,
              (h_value as u16 + src_upper_byte as u16 + carry_value as u16) > 0xFF,
            );
          } else if rp == registers::REGISTER_PAIR_DE {
            let src_upper_byte = self.registers.d;
            let result = h_value
              .wrapping_add(src_upper_byte)
              .wrapping_add(carry_value);

            self.registers.h = result;

            self.toggle_flag(
              Flag::H,
              ((h_value & 0x0F) + (src_upper_byte & 0x0F) + carry_value) > 0x0F,
            );
            self.toggle_flag(
              Flag::C,
              (h_value as u16 + src_upper_byte as u16 + carry_value as u16) > 0xFF,
            );
          } else if rp == registers::REGISTER_PAIR_HL {
            let src_upper_byte = self.registers.h;
            let result = h_value
              .wrapping_add(src_upper_byte)
              .wrapping_add(carry_value);

            self.registers.h = result;

            self.toggle_flag(
              Flag::H,
              ((h_value & 0x0F) + (src_upper_byte & 0x0F) + carry_value) > 0x0F,
            );
            self.toggle_flag(
              Flag::C,
              (h_value as u16 + src_upper_byte as u16 + carry_value as u16) > 0xFF,
            );
          } else if rp == registers::REGISTER_PAIR_SP {
            let src_upper_byte = (self.registers.sp >> 8) as u8;
            let result = h_value
              .wrapping_add(src_upper_byte)
              .wrapping_add(carry_value);

            self.registers.h = result;

            self.toggle_flag(
              Flag::H,
              ((h_value & 0x0F) + (src_upper_byte & 0x0F) + carry_value) > 0x0F,
            );
            self.toggle_flag(
              Flag::C,
              (h_value as u16 + src_upper_byte as u16 + carry_value as u16) > 0xFF,
            );
          }

          // The `N` flag is unconditionally set to false
          self.toggle_flag(Flag::N, false);

          self.fetch_cycle(hardware);
        }
      }
      // ADD SP, imm8
      (false, 0xE8) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);

          self.data_buffer[0] = imm8;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let imm8 = self.data_buffer[0];
          let sp_low = (self.registers.sp & 0x00FF) as u8;
          let new_sp_low = sp_low.wrapping_add(imm8);

          self.data_buffer[1] = new_sp_low;

          self.toggle_flag(Flag::Z, false);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, (sp_low & 0x0F) + (imm8 & 0x0F) > 0x0F);
          self.toggle_flag(Flag::C, (sp_low as u16) + (imm8 as u16) > 0xFF);

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let imm8 = self.data_buffer[0];
          let upper_sp = (self.registers.sp >> 8) as u8;
          // Adjustment value if the MSB is set, thus negative since the imm8 is signed.
          let adj = if is_flag_set!(imm8, 0x80) { 0xFF } else { 0 };
          let carry = is_flag_set!(self.flags, Flag::C as u8) as u8;
          let new_upper_sp = upper_sp.wrapping_add(adj).wrapping_add(carry);
          let new_lower_sp = self.data_buffer[1];

          self.registers.sp = ((new_upper_sp as u16) << 8) | (new_lower_sp as u16);

          self.fetch_cycle(hardware);
        }
      }
      // AND A, r8 | [HL]
      (false, 0xA0..=0xA7) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let res = self.registers.a & reg_value;

                self.registers.a = res;

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, true);
                self.toggle_flag(Flag::C, false);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let src_value = self.read_memory_register(hardware);
          let res = self.registers.a & src_value;

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, true);
          self.toggle_flag(Flag::C, false);

          self.fetch_cycle(hardware);
        }
      }
      // AND A, imm8
      (false, 0xE6) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);
          let res = self.registers.a & imm8;

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, true);
          self.toggle_flag(Flag::C, false);

          self.fetch_cycle(hardware);
        }
      }
      // CP A, r8 | [HL]
      (false, 0xB8..=0xBF) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let res = self.registers.a.wrapping_sub(reg_value);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, true);
                self.toggle_flag(Flag::H, (self.registers.a & 0x0F) < (reg_value& 0x0F));
                self.toggle_flag(Flag::C, self.registers.a < reg_value);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let src_value = self.read_memory_register(hardware);
          let res = self.registers.a.wrapping_sub(src_value);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, true);
          self.toggle_flag(Flag::H, (self.registers.a & 0x0F) < (src_value & 0x0F));
          self.toggle_flag(Flag::C, self.registers.a < src_value);

          self.fetch_cycle(hardware);
        }
      }
      // CP A, imm8
      (false, 0xFE) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);
          let res = self.registers.a.wrapping_sub(imm8);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, true);
          self.toggle_flag(Flag::H, (self.registers.a & 0x0F) < (imm8 & 0x0F));
          self.toggle_flag(Flag::C, self.registers.a < imm8);

          self.fetch_cycle(hardware);
        }
      }
      // DEC r8 | [HL]
      (false, 0x05 | 0x15 | 0x25 | 0x35 | 0x0D | 0x1D | 0x2D | 0x3D) => {
        if matches!(self.cycle, M1) {
          if is_dest_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let dest_reg = extract_dest_register!(opcode);

            perform_with_register!(
              &self.registers,
              dest_reg,
              (reg_value) => {
                let res = reg_value.wrapping_sub(1);

                write_to_register!(&mut self.registers, dest_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, true);
                self.toggle_flag(Flag::H, (reg_value & 0x0F) < 1);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);

          self.data_buffer[0] = reg_value;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_value = self.data_buffer[0];
          let res = reg_value.wrapping_sub(1);

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, true);
          self.toggle_flag(Flag::H, (reg_value & 0x0F) < 1);

          self.fetch_cycle(hardware);
        }
      }
      // DEC r16
      (false, 0x0B | 0x1B | 0x2B | 0x3B) => {
        if matches!(self.cycle, M1) {
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            let value = ((self.registers.b as u16) << 8) | (self.registers.c as u16);
            let res = value.wrapping_sub(1);

            self.registers.b = (res >> 8) as u8;
            self.registers.c = (res & 0x00FF) as u8;
          } else if rp == registers::REGISTER_PAIR_DE {
            let value = ((self.registers.d as u16) << 8) | (self.registers.e as u16);
            let res = value.wrapping_sub(1);

            self.registers.d = (res >> 8) as u8;
            self.registers.e = (res & 0x00FF) as u8;
          } else if rp == registers::REGISTER_PAIR_HL {
            let value = ((self.registers.h as u16) << 8) | (self.registers.l as u16);
            let res = value.wrapping_sub(1);

            self.registers.h = (res >> 8) as u8;
            self.registers.l = (res & 0x00FF) as u8;
          } else if rp == registers::REGISTER_PAIR_SP {
            self.registers.sp = self.registers.sp.wrapping_sub(1);
          }

          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          self.fetch_cycle(hardware);
        }
      }
      // INC r8 | [HL]
      (false, 0x04 | 0x14 | 0x24 | 0x34 | 0x0C | 0x1C | 0x2C | 0x3C) => {
        if matches!(self.cycle, M1) {
          if is_dest_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let dest_reg = extract_dest_register!(opcode);

            perform_with_register!(
              &self.registers,
              dest_reg,
              (reg_value) => {
                let res = reg_value.wrapping_add(1);

                write_to_register!(&mut self.registers, dest_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, (reg_value & 0x0F) == 0x0F);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let value = self.read_memory_register(hardware);

          self.data_buffer[0] = value;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_value = self.data_buffer[0];
          let res = reg_value.wrapping_add(1);

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, (reg_value & 0x0F) == 0x0F);

          self.fetch_cycle(hardware);
        }
      }
      // INC r16
      (false, 0x03 | 0x13 | 0x23 | 0x33) => {
        if matches!(self.cycle, M1) {
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            let value = ((self.registers.b as u16) << 8) | (self.registers.c as u16);
            let res = value.wrapping_add(1);

            self.registers.b = (res >> 8) as u8;
            self.registers.c = (res & 0x00FF) as u8;
          } else if rp == registers::REGISTER_PAIR_DE {
            let value = ((self.registers.d as u16) << 8) | (self.registers.e as u16);
            let res = value.wrapping_add(1);

            self.registers.d = (res >> 8) as u8;
            self.registers.e = (res & 0x00FF) as u8;
          } else if rp == registers::REGISTER_PAIR_HL {
            let value = ((self.registers.h as u16) << 8) | (self.registers.l as u16);
            let res = value.wrapping_add(1);

            self.registers.h = (res >> 8) as u8;
            self.registers.l = (res & 0x00FF) as u8;
          } else if rp == registers::REGISTER_PAIR_SP {
            self.registers.sp = self.registers.sp.wrapping_add(1);
          }

          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          self.fetch_cycle(hardware);
        }
      }
      // OR A, r8 | [HL]
      (false, 0xB0..=0xB7) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let res = self.registers.a | reg_value;

                self.registers.a = res;

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, false);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let res = self.registers.a | reg_value;

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, false);

          self.fetch_cycle(hardware);
        }
      }
      // OR A, imm8
      (false, 0xF6) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);
          let res = self.registers.a | imm8;

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, false);

          self.fetch_cycle(hardware);
        }
      }
      // SBC A, r8 | [HL]
      (false, 0x98..=0x9F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
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
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let a_value = self.registers.a;
          let reg_value = self.read_memory_register(hardware);
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
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

          self.fetch_cycle(hardware);
        }
      }
      // SBC A, imm8
      (false, 0xDE) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let a_value = self.registers.a;
          let imm8 = self.fetch_byte(hardware);
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
          let res = a_value.wrapping_sub(imm8).wrapping_sub(is_carry_set);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, true);
          self.toggle_flag(Flag::H, (a_value & 0x0F) < ((imm8 & 0x0F) + is_carry_set));
          self.toggle_flag(
            Flag::C,
            (a_value as u16) < (imm8 as u16 + is_carry_set as u16),
          );

          self.fetch_cycle(hardware);
        }
      }
      // SUB A, r8 | [HL]
      (false, 0x90..=0x97) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let a_value = self.registers.a;
                let res = a_value.wrapping_sub(reg_value);

                self.registers.a = res;

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, true);
                self.toggle_flag(Flag::H, (a_value & 0x0F) < (reg_value & 0x0F));
                self.toggle_flag(Flag::C, a_value < reg_value);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let a_value = self.registers.a;
          let reg_value = self.read_memory_register(hardware);
          let res = a_value.wrapping_sub(reg_value);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, true);
          self.toggle_flag(Flag::H, (a_value & 0x0F) < (reg_value & 0x0F));
          self.toggle_flag(Flag::C, a_value < reg_value);

          self.fetch_cycle(hardware);
        }
      }
      // SUB A, imm8
      (false, 0xD6) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);
          let a_value = self.registers.a;
          let res = a_value.wrapping_sub(imm8);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, true);
          self.toggle_flag(Flag::H, (a_value & 0x0F) < (imm8 & 0x0F));
          self.toggle_flag(Flag::C, a_value < imm8);

          self.fetch_cycle(hardware);
        }
      }
      // XOR A, r8 | [HL]
      (false, 0xA8..=0xAF) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let res = self.registers.a ^ reg_value;

                self.registers.a = res;

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, false);
              }
            );

            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let res = self.registers.a ^ reg_value;

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, false);

          self.fetch_cycle(hardware);
        }
      }
      // XOR A, imm8
      (false, 0xEE) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);
          let res = self.registers.a ^ imm8;

          self.registers.a = res;

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, false);

          self.fetch_cycle(hardware);
        }
      }
      // DAA
      (false, 0x27) => {
        if matches!(self.cycle, M1) {
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

          self.fetch_cycle(hardware);
        }
      }

      // CALL cc, imm16
      (false, 0xC4 | 0xD4 | 0xCC | 0xDC) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);

          self.data_buffer[0] = lower;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);

          self.data_buffer[1] = upper;

          let cc = get_conditional_flag!(opcode);

          if self.is_conditional_flag_set(cc) {
            self.cycle = M4;
          } else {
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M4) {
          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M5;
        } else if matches!(self.cycle, M5) {
          let upper_pc = (self.registers.pc >> 8) as u8;

          hardware.write_byte(self.registers.sp, upper_pc);

          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M6;
        } else if matches!(self.cycle, M6) {
          let lower_pc = (self.registers.pc & 0x00FF) as u8;

          hardware.write_byte(self.registers.sp, lower_pc);

          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);

          self.registers.pc = address;

          self.fetch_cycle(hardware);
        }
      }
      // CALL imm16
      (false, 0xCD) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);

          self.data_buffer[0] = lower;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);

          self.data_buffer[1] = upper;

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M5;
        } else if matches!(self.cycle, M5) {
          let upper_pc = (self.registers.pc >> 8) as u8;

          hardware.write_byte(self.registers.sp, upper_pc);

          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M6;
        } else if matches!(self.cycle, M6) {
          let lower_pc = (self.registers.pc & 0x00FF) as u8;

          hardware.write_byte(self.registers.sp, lower_pc);

          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);

          self.registers.pc = address;

          self.fetch_cycle(hardware);
        }
      }
      // JP cc, imm16
      (false, 0xC2 | 0xD2 | 0xCA | 0xDA) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);

          self.data_buffer[0] = lower;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);

          self.data_buffer[1] = upper;

          let cc = get_conditional_flag!(opcode);

          if self.is_conditional_flag_set(cc) {
            self.cycle = M4;
          } else {
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M4) {
          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);

          self.registers.pc = address;

          self.fetch_cycle(hardware);
        }
      }
      // JP imm16
      (false, 0xC3) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower = self.fetch_byte(hardware);

          self.data_buffer[0] = lower;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper = self.fetch_byte(hardware);

          self.data_buffer[1] = upper;

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let address = ((self.data_buffer[1] as u16) << 8) | (self.data_buffer[0] as u16);

          self.registers.pc = address;

          self.fetch_cycle(hardware);
        }
      }
      // JP HL
      (false, 0xE9) => {
        if matches!(self.cycle, M1) {
          let address = ((self.registers.h as u16) << 8) | self.registers.l as u16;

          self.registers.pc = address;

          self.fetch_cycle(hardware);
        }
      }
      // JR cc, imm8
      (false, 0x20 | 0x30 | 0x28 | 0x38) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);

          self.data_buffer[0] = imm8;

          let cc = get_conditional_flag!(opcode);

          if self.is_conditional_flag_set(cc) {
            self.cycle = M3;
          } else {
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M3) {
          // The offset can be negative, so sign-extend the value
          let offset = self.data_buffer[0] as i8 as u16;

          self.registers.pc = self.registers.pc.wrapping_add(offset);

          self.fetch_cycle(hardware);
        }
      }
      // JR imm8
      (false, 0x18) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let imm8 = self.fetch_byte(hardware);

          self.data_buffer[0] = imm8;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          // The offset can be negative, so sign-extend the value
          let offset = self.data_buffer[0] as i8 as u16;

          self.registers.pc = self.registers.pc.wrapping_add(offset);

          self.fetch_cycle(hardware);
        }
      }
      // RET cc
      (false, 0xC0 | 0xD0 | 0xC8 | 0xD8) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let cc = get_conditional_flag!(opcode);

          if self.is_conditional_flag_set(cc) {
            self.cycle = M3;
          } else {
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M3) {
          let lower_pc = hardware.read_byte(self.registers.sp);

          self.data_buffer[0] = lower_pc;
          self.registers.sp = self.registers.sp.wrapping_add(1);

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let upper_pc = hardware.read_byte(self.registers.sp);

          self.data_buffer[1] = upper_pc;
          self.registers.sp = self.registers.sp.wrapping_add(1);

          self.cycle = M5;
        } else if matches!(self.cycle, M5) {
          let upper_pc = self.data_buffer[1];
          let lower_pc = self.data_buffer[0];

          self.registers.pc = ((upper_pc as u16) << 8) | (lower_pc as u16);

          self.fetch_cycle(hardware);
        }
      }
      // RET
      (false, 0xC9) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower_pc = hardware.read_byte(self.registers.sp);

          self.data_buffer[0] = lower_pc;
          self.registers.sp = self.registers.sp.wrapping_add(1);

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper_pc = hardware.read_byte(self.registers.sp);

          self.data_buffer[1] = upper_pc;
          self.registers.sp = self.registers.sp.wrapping_add(1);

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let upper_pc = self.data_buffer[1];
          let lower_pc = self.data_buffer[0];

          self.registers.pc = ((upper_pc as u16) << 8) | (lower_pc as u16);

          self.fetch_cycle(hardware);
        }
      }
      // RETI
      (false, 0xD9) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower_pc = hardware.read_byte(self.registers.sp);

          self.data_buffer[0] = lower_pc;
          self.registers.sp = self.registers.sp.wrapping_add(1);

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper_pc = hardware.read_byte(self.registers.sp);

          self.data_buffer[1] = upper_pc;
          self.registers.sp = self.registers.sp.wrapping_add(1);

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let upper_pc = self.data_buffer[1];
          let lower_pc = self.data_buffer[0];

          self.registers.pc = ((upper_pc as u16) << 8) | (lower_pc as u16);

          self.interrupt_master_enabled = true;

          self.fetch_cycle(hardware);
        }
      }
      // RST vector
      (false, 0xC7 | 0xD7 | 0xE7 | 0xF7 | 0xCF | 0xDF | 0xEF | 0xFF) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper_pc = (self.registers.pc >> 8) as u8;

          hardware.write_byte(self.registers.sp, upper_pc);

          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let lower_pc = (self.registers.pc & 0x00FF) as u8;

          hardware.write_byte(self.registers.sp, lower_pc);

          // The target is encoded in bits 3 through 5.
          let address = opcode & 0b0011_1000;

          self.registers.pc = address as u16;

          self.fetch_cycle(hardware);
        }
      }
      // STOP
      (false, 0x10) => {
        if matches!(self.cycle, M1) {
          // NOTE: `STOP` needs to be followed by another byte
          self.fetch_byte(hardware);

          self.state = CpuState::Stopped;

          self.fetch_cycle(hardware);
        }
      }
      // HALT
      (false, 0x76) => {
        if matches!(self.cycle, M1) {
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
          if !self.interrupt_master_enabled && hardware.has_pending_interrupts() {
            self.halt_bug = true;
            // The CPU doesn't actually enter a halted state in the case of a bugged
            // halt instruction.
            self.state = CpuState::Running;
          } else {
            self.state = CpuState::Halted;
          }

          self.fetch_cycle(hardware);
        }
      }
      // NOP
      (false, 0x00) => {
        if matches!(self.cycle, M1) {
          self.fetch_cycle(hardware);
        }
      }

      // POP r16
      (false, 0xC1 | 0xD1 | 0xE1 | 0xF1) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          let lower_byte = hardware.read_byte(self.registers.sp);

          self.registers.sp = self.registers.sp.wrapping_add(1);

          self.data_buffer[0] = lower_byte;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let upper_byte = hardware.read_byte(self.registers.sp);

          self.registers.sp = self.registers.sp.wrapping_add(1);

          let lower_byte = self.data_buffer[0];
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            self.registers.b = upper_byte;
            self.registers.c = lower_byte;
          } else if rp == registers::REGISTER_PAIR_DE {
            self.registers.d = upper_byte;
            self.registers.e = lower_byte;
          } else if rp == registers::REGISTER_PAIR_HL {
            self.registers.h = upper_byte;
            self.registers.l = lower_byte;
          } else if rp == registers::REGISTER_PAIR_AF {
            self.registers.a = upper_byte;
            self.flags = lower_byte & 0xF0;
          }

          self.fetch_cycle(hardware);
        }
      }
      // PUSH r16
      (false, 0xC5 | 0xD5 | 0xE5 | 0xF5) => {
        if matches!(self.cycle, M1) {
          self.cycle = M2;
        } else if matches!(self.cycle, M2) {
          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            hardware.write_byte(self.registers.sp, self.registers.b);
          } else if rp == registers::REGISTER_PAIR_DE {
            hardware.write_byte(self.registers.sp, self.registers.d);
          } else if rp == registers::REGISTER_PAIR_HL {
            hardware.write_byte(self.registers.sp, self.registers.h);
          } else if rp == registers::REGISTER_PAIR_AF {
            hardware.write_byte(self.registers.sp, self.registers.a);
          }

          self.registers.sp = self.registers.sp.wrapping_sub(1);

          self.cycle = M4;
        } else if matches!(self.cycle, M4) {
          let rp = extract_register_pair!(opcode);

          if rp == registers::REGISTER_PAIR_BC {
            hardware.write_byte(self.registers.sp, self.registers.c);
          } else if rp == registers::REGISTER_PAIR_DE {
            hardware.write_byte(self.registers.sp, self.registers.e);
          } else if rp == registers::REGISTER_PAIR_HL {
            hardware.write_byte(self.registers.sp, self.registers.l);
          } else if rp == registers::REGISTER_PAIR_AF {
            hardware.write_byte(self.registers.sp, self.flags);
          }

          self.fetch_cycle(hardware);
        }
      }
      // CCF
      (false, 0x3F) => {
        if matches!(self.cycle, M1) {
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, !is_flag_set!(self.flags, Flag::C as u8));

          self.fetch_cycle(hardware);
        }
      }
      // CPL
      (false, 0x2F) => {
        if matches!(self.cycle, M1) {
          self.registers.a = !self.registers.a;

          self.toggle_flag(Flag::N, true);
          self.toggle_flag(Flag::H, true);

          self.fetch_cycle(hardware);
        }
      }
      // DI
      (false, 0xF3) => {
        if matches!(self.cycle, M1) {
          self.interrupt_master_enabled = false;

          // Use `complete_cycle` instead of `fetch_cycle` since the IME is disabled,
          // thus no interrupts can occur.
          self.complete_cycle(hardware);
        }
      }
      // EI
      (false, 0xFB) => {
        if matches!(self.cycle, M1) {
          // We shouldn't actually update the master interrupt flag immediately
          // because this instruction seems to have a delay of 4 T-cycles
          self.fetch_cycle(hardware);
        }
      }
      // SCF
      (false, 0x37) => {
        if matches!(self.cycle, M1) {
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, true);

          self.fetch_cycle(hardware);
        }
      }

      // RLA
      (false, 0x17) => {
        if matches!(self.cycle, M1) {
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
          let a_value = self.registers.a;
          let res = (a_value << 1) | (is_carry_set as u8);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, false);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, (a_value >> 7) == 0x1);

          self.fetch_cycle(hardware);
        }
      }
      // RLCA
      (false, 0x07) => {
        if matches!(self.cycle, M1) {
          let a_value = self.registers.a;
          let res = a_value.rotate_left(1);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, false);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, (a_value >> 7) == 0x1);

          self.fetch_cycle(hardware);
        }
      }
      // RRA
      (false, 0x1F) => {
        if matches!(self.cycle, M1) {
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8);
          let a_value = self.registers.a;
          let res = (a_value >> 1) | ((is_carry_set as u8) << 7);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, false);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, (a_value & 0x1) == 1);

          self.fetch_cycle(hardware);
        }
      }
      // RRCA
      (false, 0x0F) => {
        if matches!(self.cycle, M1) {
          let a_value = self.registers.a;
          let res = a_value.rotate_right(1);

          self.registers.a = res;

          self.toggle_flag(Flag::Z, false);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, (a_value & 0x1) == 1);

          self.fetch_cycle(hardware);
        }
      }

      // Unused opcodes
      (false, 0xD3 | 0xE3 | 0xE4 | 0xF4 | 0xDB | 0xEB | 0xEC | 0xFC | 0xDD | 0xED | 0xFD) => {
        // Unused opcodes are actually supposed to hang the CPU, but it may be a sign
        // that there's a bug some where, so lets panic in debug builds!
        debug_assert!(
          false,
          "{:04X}: got invalid opcode {:02X}",
          self.registers.pc, opcode
        );
      }

      // PREFIX
      (false, 0xCB) => {
        self.saw_prefix_opcode = true;

        // Use `complete_cycle` instead of `fetch_cycle` since interrupts cannot
        // occur in-between this prefix byte and the next instruction.
        self.complete_cycle(hardware);
      }

      // Extended Instruction Set

      // BIT 0..8, r8 | [HL}]
      (true, 0x40..=0x7F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let bit = (opcode >> 3) & 0x07;

            perform_with_register!(
              &self.registers,
              extract_src_register!(opcode),
              (reg_value) => {
                let extracted_bit = (reg_value >> bit) & 1;

                self.toggle_flag(Flag::Z, extracted_bit == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, true);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let bit = (opcode >> 3) & 0x07;
          let reg_value = self.read_memory_register(hardware);
          let extracted_bit = (reg_value >> bit) & 1;

          self.toggle_flag(Flag::Z, extracted_bit == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, true);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // RES 0..8, r8 | [HL]
      (true, 0x80..=0xBF) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);
            let bit = (opcode >> 3) & 0x07;

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                 let new_value = reg_value & !(1 << bit);

                 write_to_register!(&mut self.registers, src_reg, new_value);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let bit = (opcode >> 3) & 0x07;
          let reg_value = self.read_memory_register(hardware);
          let new_value = reg_value & !(1 << bit);

          self.data_buffer[0] = new_value;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let new_value = self.data_buffer[0];

          self.write_memory_register(hardware, new_value);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // SET 0..8, r8 | [HL]
      (true, 0xC0..=0xFF) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);
            let bit = (opcode >> 3) & 0x07;

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let new_value = reg_value | (1 << bit);

                write_to_register!(&mut self.registers, src_reg, new_value);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let bit = (opcode >> 3) & 0x07;
          let reg_value = self.read_memory_register(hardware);
          let new_value = reg_value | (1 << bit);

          self.data_buffer[0] = new_value;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let new_value = self.data_buffer[0];

          self.write_memory_register(hardware, new_value);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // RL r8 | [HL]
      (true, 0x10..=0x17) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
                let res = (reg_value << 1) | is_carry_set;

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, (reg_value >> 7) == 1);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
          let res = (reg_value << 1) | is_carry_set;

          // Store the MSB of [HL]
          self.data_buffer[0] = reg_value >> 7;
          self.data_buffer[1] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_msb = self.data_buffer[0];
          let res = self.data_buffer[1];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, reg_msb == 1);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // RLC r8 | [HL]
      (true, 0x00..=0x07) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let res = reg_value.rotate_left(1);

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, (reg_value >> 7) == 1);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let res = reg_value.rotate_left(1);

          // Store the MSB of [HL]
          self.data_buffer[0] = reg_value >> 7;
          self.data_buffer[1] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_msb = self.data_buffer[0];
          let res = self.data_buffer[1];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, reg_msb == 1);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // RR r8 | [HL]
      (true, 0x18..=0x1F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
                let res = (reg_value >> 1) | (is_carry_set << 7);

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);

              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let is_carry_set = is_flag_set!(self.flags, Flag::C as u8) as u8;
          let res = (reg_value >> 1) | (is_carry_set << 7);

          // Store the LSB of [HL]
          self.data_buffer[0] = reg_value & 0x1;
          self.data_buffer[1] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_lsb = self.data_buffer[0];
          let res = self.data_buffer[1];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, reg_lsb == 1);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // RRC r8 | [HL]
      (true, 0x08..=0x0F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let res = reg_value.rotate_right(1);

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let res = reg_value.rotate_right(1);

          // Store the LSB of [HL]
          self.data_buffer[0] = reg_value & 0x1;
          self.data_buffer[1] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_lsb = self.data_buffer[0];
          let res = self.data_buffer[1];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, reg_lsb == 1);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // SLA r8 | [HL]
      (true, 0x20..=0x27) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let res = reg_value << 1;

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, (reg_value >> 7) == 1);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let res = reg_value << 1;

          self.data_buffer[0] = reg_value >> 7;
          self.data_buffer[1] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_msb = self.data_buffer[0];
          let res = self.data_buffer[1];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, reg_msb == 1);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // SRA r8 | [HL]
      (true, 0x28..=0x2F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                // SRA preserves the sign bit (MSB)
                let res = (reg_value >> 1) | (reg_value & 0x80);

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          // SRA preserves the sign bit (MSB)
          let res = (reg_value >> 1) | (reg_value & 0x80);

          self.data_buffer[0] = reg_value & 0x1;
          self.data_buffer[1] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_lsb = self.data_buffer[0];
          let res = self.data_buffer[1];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, reg_lsb == 1);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // SRL r8 | [HL]
      (true, 0x38..=0x3F) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let res = reg_value >> 1;

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, (reg_value & 0x1) == 1);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let res = reg_value >> 1;

          self.data_buffer[0] = reg_value & 0x1;
          self.data_buffer[1] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let reg_lsb = self.data_buffer[0];
          let res = self.data_buffer[1];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, reg_lsb == 1);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
      // SWAP r8 | [HL]
      (true, 0x30..=0x37) => {
        if matches!(self.cycle, M1) {
          if is_src_register_memory!(opcode) {
            self.cycle = M2;
          } else {
            let src_reg = extract_src_register!(opcode);

            perform_with_register!(
              &self.registers,
              src_reg,
              (reg_value) => {
                let lower = reg_value & 0x0F;
                let upper = reg_value & 0xF0;
                let res = (lower << 4) | (upper >> 4);

                write_to_register!(&mut self.registers, src_reg, res);

                self.toggle_flag(Flag::Z, res == 0);
                self.toggle_flag(Flag::N, false);
                self.toggle_flag(Flag::H, false);
                self.toggle_flag(Flag::C, false);
              }
            );

            self.saw_prefix_opcode = false;
            self.fetch_cycle(hardware);
          }
        } else if matches!(self.cycle, M2) {
          let reg_value = self.read_memory_register(hardware);
          let lower = reg_value & 0x0F;
          let upper = reg_value & 0xF0;
          let res = (lower << 4) | (upper >> 4);

          self.data_buffer[0] = res;

          self.cycle = M3;
        } else if matches!(self.cycle, M3) {
          let res = self.data_buffer[0];

          self.write_memory_register(hardware, res);

          self.toggle_flag(Flag::Z, res == 0);
          self.toggle_flag(Flag::N, false);
          self.toggle_flag(Flag::H, false);
          self.toggle_flag(Flag::C, false);

          self.saw_prefix_opcode = false;
          self.fetch_cycle(hardware);
        }
      }
    }
  }

  /// Steps 1 M-cycle of handling interrupts.
  pub fn step_interrupts(&mut self, hardware: &mut Hardware) {
    use CpuCycle::*;

    if matches!(self.cycle, M1) {
      // Undo the increment from the end of the previous instruction
      self.registers.pc = self.registers.pc.wrapping_sub(1);

      self.cycle = M2;
    } else if matches!(self.cycle, M2) {
      self.registers.sp = self.registers.sp.wrapping_sub(1);

      self.cycle = M3;
    } else if matches!(self.cycle, M3) {
      let pc_high = (self.registers.pc >> 8) as u8;

      hardware.write_byte(self.registers.sp, pc_high);

      self.registers.sp = self.registers.sp.wrapping_sub(1);

      self.cycle = M4;
    } else if matches!(self.cycle, M4) {
      // The vector address for when there is no longer an interrupt to handle.
      const INTERRUPT_CANCELLATION_VECTOR: u16 = 0x0000;

      let pc_low = (self.registers.pc & 0x00FF) as u8;

      // Keep track of the pending interrupt before the call to write to the stack because
      // that can overwrite the `IE` and `IF` registers, thus potentially changing the next
      // interrupt that should be handled.
      let prev_interrupt = hardware.next_pending_interrupt();

      // Write the lower byte of the PC to the stack
      hardware.write_byte(self.registers.sp, pc_low);

      // Check *again* for the next interrupt. This can differ from the previous interrupt
      // if the write to the stack changed the `IE` or `IF` registers.
      let curr_interrupt = hardware.next_pending_interrupt();

      // There are 4 cases that to consider when handling interrupts in a cycle-accurate manner:
      //
      // 1) There was no previous interrupt and now there is one. If so, handle the newer one.
      //    This happens when the `IE`/`IF` registers got changed when writing the upper byte
      //    of the PC to the stack.
      //
      // 2) There was a previous interrupt, but none now. If so, handle the older one.
      //    This happens when the `IE`/`IF` registers got changed when writing the lower byte
      //    of the PC to the stack.
      //
      // 3) There was a previous interrupt and there is a new interrupt. If so, get the
      //    interrupt with higher priority.
      //    This can occur when interrupts get enabled or disabled when writing the
      //    lower byte of the PC to the stack, so we need to make sure that we handle
      //    the interrupt with the highest priority.
      //
      // 4) There was no previous interrupt and there still is none. If so, go to the
      //    cancellation vector, which is 0x0000.
      //    This occurs when the `IE`/`IF` registers got changed when writing the upper byte
      //    of the PC to the stack.
      let interrupt = match (prev_interrupt, curr_interrupt) {
        (None, Some(new)) => Some(new),
        (Some(old), None) => Some(old),
        // NOTE: I could implement `Ord` for `Interrupt` and get rid of this. In fact,
        // this ENTIRE match could be removed, but I prefer this way since it's more explicit
        // and the cases can be seen more clearly.
        (Some(old), Some(new)) => Some(Interrupt::prioritize(old, new)),
        (None, None) => None,
      };

      self.registers.pc = interrupt.map_or(INTERRUPT_CANCELLATION_VECTOR, Interrupt::to_vector);

      // Make sure we mark the interrupt as handled in the IF register.
      if let Some(interrupt) = interrupt {
        hardware.clear_interrupt(interrupt);
      }

      self.interrupt_master_enabled = false;

      self.cycle = M5;
    } else if matches!(self.cycle, M5) {
      self.state = CpuState::Running;

      self.fetch_cycle(hardware);
    }
  }

  /// Fetches the next byte.
  pub fn fetch_byte(&mut self, hardware: &Hardware) -> u8 {
    let byte = hardware.read_byte(self.registers.pc);

    // The program counter shouldn't be incremented when we're in a bugged halt state.
    if self.halt_bug {
      self.halt_bug = false;
    } else {
      self.registers.pc = self.registers.pc.wrapping_add(1);
    }

    byte
  }

  /// Marks the completion of the current execution.
  fn complete_cycle(&mut self, hardware: &mut Hardware) {
    self.cycle = CpuCycle::M1;

    self.last_instruction = self.registers.ir;

    // The CPU indefinitely fetches the next instruction byte, even if there are interrupts.
    self.registers.ir = self.fetch_byte(hardware);
  }

  /// Marks the completion of the current execution and fetches the next cycle.
  fn fetch_cycle(&mut self, hardware: &mut Hardware) {
    self.complete_cycle(hardware);

    // Only process interrupts when the IME is also enabled.
    self.should_handle_interrupts =
      self.interrupt_master_enabled && hardware.has_pending_interrupts();
  }

  /// Reads a value from the memory register.
  fn read_memory_register(&self, hardware: &Hardware) -> u8 {
    let address = ((self.registers.h as u16) << 8) | (self.registers.l as u16);

    hardware.read_byte(address)
  }

  /// Writes the value to the memory register.
  fn write_memory_register(&mut self, hardware: &mut Hardware, value: u8) {
    let address = ((self.registers.h as u16) << 8) | (self.registers.l as u16);

    hardware.write_byte(address, value);
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

mod macros {
  /// Calls a function passing the value of the register.
  macro_rules! perform_with_register {
    ($registers:expr, $register_operand:expr, ($value:ident) => $action:expr) => {
      if $register_operand == registers::REGISTER_A {
        let $value = $registers.a;
        $action;
      } else if $register_operand == registers::REGISTER_B {
        let $value = $registers.b;
        $action;
      } else if $register_operand == registers::REGISTER_C {
        let $value = $registers.c;
        $action;
      } else if $register_operand == registers::REGISTER_D {
        let $value = $registers.d;
        $action;
      } else if $register_operand == registers::REGISTER_E {
        let $value = $registers.e;
        $action;
      } else if $register_operand == registers::REGISTER_H {
        let $value = $registers.h;
        $action;
      } else if $register_operand == registers::REGISTER_L {
        let $value = $registers.l;
        $action;
      } else if $register_operand == registers::REGISTER_M {
        debug_assert!(false, "passed register M to perform_with_register");
      }
    };
  }

  /// Writes to the value to the register.
  macro_rules! write_to_register {
    ($registers:expr, $dest_register:expr, $value:expr) => {
      if $dest_register == registers::REGISTER_A {
        $registers.a = $value;
      } else if $dest_register == registers::REGISTER_B {
        $registers.b = $value;
      } else if $dest_register == registers::REGISTER_C {
        $registers.c = $value;
      } else if $dest_register == registers::REGISTER_D {
        $registers.d = $value;
      } else if $dest_register == registers::REGISTER_E {
        $registers.e = $value;
      } else if $dest_register == registers::REGISTER_H {
        $registers.h = $value;
      } else if $dest_register == registers::REGISTER_L {
        $registers.l = $value;
      } else if $dest_register == registers::REGISTER_M {
        debug_assert!(false, "cannot write to register M");
      }
    };
  }

  // Extracts the conditional flag, stored in bits 4 and 5, from an opcode.
  macro_rules! get_conditional_flag {
    ($opcode:expr) => {
      match ($opcode >> 3) & 0x03 {
        0b00 => ConditionalFlag::NZ,
        0b01 => ConditionalFlag::Z,
        0b10 => ConditionalFlag::NC,
        0b11 => ConditionalFlag::C,
        _ => unreachable!(),
      }
    };
  }

  // Extracts the destination register bits from an opcode.
  macro_rules! extract_dest_register {
    ($opcode:expr) => {
      ($opcode >> 3) & 0x07
    };
  }

  // Extracts the source register bits from an opcode.
  macro_rules! extract_src_register {
    ($opcode:expr) => {
      $opcode & 0x07
    };
  }

  // Extracts the register pair bits from an opcode.
  macro_rules! extract_register_pair {
    ($opcode:expr) => {
      ($opcode >> 4) & 0x03
    };
  }

  // Checks whether the destination register, in an opcode, is the memory register.
  macro_rules! is_dest_register_memory {
    ($opcode:expr) => {
      extract_dest_register!($opcode) == registers::REGISTER_M
    };
  }

  // Checks whether the source register, in an opcode, is the memory register.
  macro_rules! is_src_register_memory {
    ($opcode:expr) => {
      extract_src_register!($opcode) == registers::REGISTER_M
    };
  }

  pub(crate) use extract_dest_register;
  pub(crate) use extract_register_pair;
  pub(crate) use extract_src_register;
  pub(crate) use get_conditional_flag;
  pub(crate) use is_dest_register_memory;
  pub(crate) use is_src_register_memory;
  pub(crate) use perform_with_register;
  pub(crate) use write_to_register;
}
