<div align="center">
  <h1>Gameboy Emulator</h1>
</div>

## Motivation
This project started as a way to learn more about emulator development, specifically the Gameboy 
and its internals.

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
> [!NOTE]
>
> Currently supported games are those up to MBC-1.
