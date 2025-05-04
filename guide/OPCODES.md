# Gameboy Instruction Set

This document provides a categorized list of Gameboy CPU instructions. Each entry includes:
- A general form of the instruction with placeholders (e.g., `r8`, `r16`).
- The range or list of opcodes used for each form.
- Which flags the instruction affects.
- A short description of the instruction's behavior.

---

## Legend

- `r8`: 8-bit register (`A`, `B`, `C`, `D`, `E`, `H`, `L`).
- `r16`: 16-bit register (`AF`, `BC`, `DE`, `HL`, `SP`).
- `imm8`: 8-bit immediate value.
- `imm16`: 16-bit immediate value.
- `(HL)`: Memory pointed to by register `HL`.
- `SP`: Stack Pointer.
- `PC`: Program Counter.
- `cc`: condition codes (`NZ`, `Z`, `NC`, `C`)

---

## Arithmetic & Logic

| Instruction             | Opcode(s)                                      | Flags            | Description                                                |
|-------------------------|------------------------------------------------|------------------|------------------------------------------------------------|
| `INC r8`                | `04`, `0C`, `14`, `1C`, `24`, `2C`, `3C`       | Z, H, N=0        | Increment 8-bit register.                                  |
| `DEC r8`                | `05`, `0D`, `15`, `1D`, `25`, `2D`, `3D`       | Z, H, N=1        | Decrement 8-bit register.                                  |
| `ADD A, r8`             | `80`, `81`, `82`, `83`, `84`, `85`, `86`, `87` | Z, N=0, H, C     | Add register to A.                                         |
| `ADD HL, r16`           | `09`, `19`, `29`, `39`                         | Z, N=0, H, C     | Add the 16-bit register pair to HL.                        |
| `ADC A, r8`             | `88`, `89`, `8A`, `8B`, `8C`, `8D`, `8E`, `8F` | Z, N=0, H, C     | Add register to A with carry.                              |
| `SUB r8`                | `90`, `91`, `92`, `93`, `94`, `95`, `96`, `97` | Z, N=1, H, C     | Subtract register from A.                                  |
| `SBC A, r8`             | `98`, `99`, `9A`, `9B`, `9C`, `9D`, `9E`, `9F` | Z, N=1, H, C     | Subtract register and carry from A.                        |
| `AND r8`                | `A0`, `A1`, `A2`, `A3`, `A4`, `A5`, `A6`, `A7` | Z, N=0, H=1, C=0 | Logical AND.                                               |
| `OR r8`                 | `B0`, `B1`, `B2`, `B3`, `B4`, `B5`, `B6`, `B7` | Z, N=0, H=0, C=0 | Logical OR.                                                |
| `XOR r8`                | `A8`, `A9`, `AA`, `AB`, `AC`, `AD`, `AE`, `AF` | Z, N=0, H=0, C=0 | Logical XOR.                                               |
| `CP r8`                 | `B8`, `B9`, `BA`, `BB`, `BC`, `BD`, `BE`, `BF` | Z, N=1, H, C     | Compare register with A.                                   |
| `INC (HL)`              | `34`                                           | Z, H, N=0        | Increment value at memory location (HL).                   |
| `DEC (HL)`              | `35`                                           | Z, H, N=1        | Decrement value at memory location (HL).                   |

---

## Load & Store

| Instruction             | Opcode(s)                                      | Flags            | Description                                                |
|-------------------------|------------------------------------------------|------------------|------------------------------------------------------------|
| `LD r8, r8`             | `40–7F` (excluding 76)                         | None             | Copy one 8-bit register into another.                      |
| `LD r8, imm8`           | `06`, `0E`, `16`, `1E`, `26`, `2E`, `3E`       | None             | Load immediate into register.                              |
| `LD r16, imm16`         | `01`, `11`, `21`, `31`                         | None             | Load 16-bit immediate into register.                       |
| `LD (BC), A`            | `02`                                           | None             | Store A into memory pointed by BC.                         |
| `LD (DE), A`            | `12`                                           | None             | Store A into memory pointed by DE.                         |
| `LD (HL), A`            | `77`                                           | None             | Store A into memory pointed by HL.                         |
| `LD A, (BC)`            | `0A`                                           | None             | Load A from memory pointed by BC.                          |
| `LD A, (DE)`            | `1A`                                           | None             | Load A from memory pointed by DE.                          |
| `LD A, (HL)`            | `7E`                                           | None             | Load A from memory pointed by HL.                          |
| `LD (imm16), A`         | `EA`                                           | None             | Store A at immediate address.                              |
| `LD A, (imm16)`         | `FA`                                           | None             | Load A from immediate address.                             |
| `LD (HL+), A`           | `22`                                           | None             | Store A at (HL), then increment HL.                        |
| `LD A, (HL+)`           | `2A`                                           | None             | Load A from (HL), then increment HL.                       |
| `LD (HL-), A`           | `32`                                           | None             | Store A at HL, then decrement HL.                          |
| `LD A, (HL-)`           | `3A`                                           | None             | Load A from HL, then decrement HL.                         |
| `LD SP, HL`             | `F9`                                           | None             | Copy HL into SP.                                           |
| `LD (imm16), SP`        | `08`                                           | None             | Store SP at the address.                                   |
| `LDH (imm8), A`         | `E0`                                           | None             | Store A at address 0xFF00 + imm8.                          |
| `LDH A, (imm8)`         | `F0`                                           | None             | Load A from address 0xFF00 + imm8.                         |
| `LDH (C), A`            | `E2`                                           | None             | Store A at 0xFF00 + C.                                     |
| `LDH A, (C)`            | `F2`                                           | None             | Load A from 0xFF00 + C.                                    |

---

## Stack Operations

| Instruction             | Opcode(s)                                      | Flags            | Description                                                |
|-------------------------|------------------------------------------------|------------------|------------------------------------------------------------|
| `PUSH r16`              | `C5`, `D5`, `E5`, `F5`                         | None             | Push 16-bit register pair onto the stack (BC, DE, HL, AF). |
| `POP r16`               | `C1`, `D1`, `E1`, `F1`                         | None             | Pop 16-bit value from stack into register pair.            |

---

## Control Flow

| Instruction             | Opcode(s)                                      | Flags            | Description                                                 |
|-------------------------|------------------------------------------------|------------------|-------------------------------------------------------------|
| `JP addr16`             | `C3`                                           | None             | Jump to 16-bit address.                                     |
| `JP cc, addr16`         | `C2`, `CA`, `D2`, `DA`                         | None             | Conditional jump.                                           |
| `JR r8`                 | `18`                                           | None             | Relative jump.                                              |
| `JR cc, r8`             | `20`, `28`, `30`, `38`                         | None             | Conditional relative jump.                                  |
| `CALL addr16`           | `CD`                                           | None             | Call subroutine.                                            |
| `CALL cc, addr16`       | `C4`, `CC`, `D4`, `DC`                         | None             | Conditional call.                                           |
| `RET`                   | `C9`                                           | None             | Return from subroutine.                                     |
| `RET cc`                | `C0`, `C8`, `D0`, `D8`                         | None             | Conditional return.                                         |
| `RETI`                  | `D9`                                           | None             | Return and enable interrupts.                               |
| `RST vec`               | `C7`, `CF`, `D7`, `DF`, `E7`, `EF`, `F7`, `FF` | None             | Call fixed subroutine address.                              |

---

## Bit Instructions

| Instruction             | Opcode(s)                                      | Flags            | Description                                                 |
|-------------------------|------------------------------------------------|------------------|-------------------------------------------------------------|
| `RLC r8`                | `00`–`07`                                      | Z, N=0, H=0, C   | Rotate left through carry.                                  |
| `RRC r8`                | `08`–`0F`                                      | Z, N=0, H=0, C   | Rotate right through carry.                                 |
| `RL r8`                 | `10`–`17`                                      | Z, N=0, H=0, C   | Rotate left through carry flag.                             |
| `RR r8`                 | `18`–`1F`                                      | Z, N=0, H=0, C   | Rotate right through carry flag.                            |
| `SLA r8`                | `20`–`27`                                      | Z, N=0, H=0, C   | Shift left arithmetic.                                      |
| `SRA r8`                | `28`–`2F`                                      | Z, N=0, H=0, C   | Shift right arithmetic.                                     |
| `SRL r8`                | `38`–`3F`                                      | Z, N=0, H=0, C   | Shift right logical.                                        |
| `BIT n, r8`             | `40`–`7F`                                      | Z, N=0, H=1      | Test bit `n` of register.                                   |
| `SET n, r8`             | `C0`–`FF`                                      | None             | Set bit `n`.                                                |
| `RES n, r8`             | `80`–`BF`                                      | None             | Clear bit `n`.                                              |

> [!NOTE]
>
> These instructions must be prefixed with the opcode `0xCB`.

---

## Miscellaneous

| Instruction             | Opcode(s)                                      | Flags           | Description                                                  |
|-------------------------|------------------------------------------------|-----------------|--------------------------------------------------------------|
| `NOP`                   | `00`                                           | None            | Do nothing.                                                  |
| `HALT`                  | `76`                                           | None            | Enter low-power mode until interrupt.                        |
| `STOP`                  | `10`                                           | None            | Halt CPU & LCD.                                              |
| `DI`                    | `F3`                                           | None            | Disable interrupts.                                          |
| `EI`                    | `FB`                                           | None            | Enable interrupts.                                           |
| `SCF`                   | `37`                                           | C=1, N=0, H=0   | Set carry flag.                                              |
| `CCF`                   | `3F`                                           | C=!C, N=0, H=0  | Complement carry flag.                                       |
| `DAA`                   | `27`                                           | Z, N, H, C      | Decimal adjust A after BCD operations.                       |
| `CPL`                   | `2F`                                           | N=1, H=1        | Complement A.                                                |


> [!NOTE]
> The `STOP` instruction is only 1 byte in length, however the CPU ignores the next byte on the 
> original Gameboy.
