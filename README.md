<div align="center">
  <h1>Gameboy Emulator</h1>
</div>

## Motivation
This project started as a way to learn more about emulator development, specifically the Gameboy and its internals. A key part of emulation is accurately replicating the CPU’s behavior, which requires understanding its instruction set.

While working on this, I realized that modern Assembly can be pretty daunting for beginners due to its complexity. In contrast, the Gameboy’s Assembly language is smaller and much more approachable. So, I’d like this project to also serve as a learning resource – something that makes understanding the basics of Assembly a little less scary.

## Installation
### Prerequisites
To build and run this emulator, you need to have Rust installed. The minimum supported Rust version (MSRV) is **1.85**.

### Install Rust
The easiest way to install Rust is to use `rustup`. You can find detailed instructions on how to do so [here](https://www.rust-lang.org/tools/install).

After installation, ensure that Rust 1.85+ is available by running:
```sh
$ rustc --version
```

### Clone and Build
```sh
$ git clone https://github.com/Eucladia/gameboy_emu
$ cd gameboy_emu
$ cargo build --release
```

### Running a ROM
```sh
$ cargo run --release -- path/to/rom.gb
```
> **Note:** Currently, this emulator only supports ROMs that do **not** require a memory bank controller.

## Assembly Guide
TODO: Include rationale for this and a `guide` folder with learning resources.
