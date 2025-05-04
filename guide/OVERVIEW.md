# Architecture Overview

This part provides a high-level overview of the Gameboy's hardware architecture. Understanding 
the core components is crucial for developing an emulator.

---

## CPU:

The Gameboy uses a custom SoC named **Sharp LR35902**, which contains the **SM83** CPU core. It 
is an 8-bit processor with a 16-bit address bus that is a hybrid of the Intel 8080 and Zilog Z80. 

### Overview:
- 8-bit general purpose registers: `A`, `B`, `C`, `D`, `E`, `H`, `L`.
- 16-bit special purpose registers: `SP` (stack pointer), `PC` (program counter).
- Register pairs: `AF`, `BC`, `DE`, `HL`.
- Indirect addressing: `(HL)` accesses memory at the address in `HL`.
- Flags: 
    - **Zero** (`Z`): Set if the result produced a zero.
    - **Subtract** (`N`): Set if the result was from a subtraction operation.
    - **Half Carry** (`H`): Set if the result produced a carry over the nibbles.
    - **Carry** (`C`): Set if the result produced a carry.

---

## Memory Map

The Gameboy has a 16-bit address space (`0x0000` to `0xFFFF`). The memory is divided into 
several regions:

| Address Range | Purpose                   |
|---------------|---------------------------|
| `0000-3FFF`   | ROM Bank 0                |
| `4000-7FFF`   | ROM Bank 1-N              |
| `8000-9FFF`   | Video RAM                 |
| `A000-BFFF`   | External RAM              |
| `C000-DFFF`   | Work RAM                  |
| `E000-FDFF`   | Echo RAM                  |
| `FE00-FE9F`   | Sprite Memory             |
| `FEA0-FEFF`   | Unusable                  |
| `FF00-FF7F`   | I/O Registers             |
| `FF80-FFFE`   | High RAM                  |
| `FFFF`        | Interrupt Enable Register |

---

## Interrupts

Interrupts allow the CPU to respond to certain hardware events.

| Bit | Interrupt        | Address  |
|-----|------------------|----------|
| 0   | V-Blank          | `0x0040` |
| 1   | LCD STAT         | `0x0048` |
| 2   | Timer Overflow   | `0x0050` |
| 3   | Serial Transfer  | `0x0058` |
| 4   | Joypad           | `0x0060` |

Interrupt handling can be toggled via the `EI`, `DI`, and `RETI` instructions.

---

## PPU (Pixel Processing Unit)

The PPU is responsible for drawing the background, window\*, and sprites.
- Operates on scanlines.
- Tile-based rendering system.
- Uses tile data and tile maps stored in video RAM.
- Controlled via I/O registers such as `LCDC`, `STAT`, `LY`, `SCY`, `SCX`, etc.


\* The window refers to a small subsection of the screen, that gets displayed over the background.

---

## Timers

The timer system consists of:
- **DIV** (divider): Increments at a rate of 16384 Hz.
- **TIMA** (counter): Increments at a frequency set by **TAC**.
- **TMA** (modulo): Value to reload TIMA on overflow.
- **TAC** (control): Controls the frequency and whether the timer is enabled.

---

## Input (Joypad)

The original Gameboy has a 2 groups of buttons:
- **D-pad**: Up, Down, Left, and Right.
- **Action**:  A, B, Start, and Select.

The state of these buttons are stored in I/O register `0xFF00`.

---

## Boot ROM

During boot up, the Gameboy executes a small program that is stored 
internally (not on the cartridge). It

- Displays the Nintendo logo.
- Performs hardware checks.
- Jumps to cartridge entry point.
