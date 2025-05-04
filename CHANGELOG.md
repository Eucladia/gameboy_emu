# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project does not adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) until v1.0.0.

## [Unreleased]

### Added
- Support for incrementing and decrementing the master volume via `Shift` + `+` / `-`.
- Toggleable volume overlay when pressing `Shift` + `1`.
- Basic guide.

### Fixed
- Implement edge case bugs when triggering sound channels', allowing us to pass 
  Blargg's `03-trigger` test.
- Properly set the internal sweep enabled register on trigger, allowing us to pass
  Blargg's `04-sweep` test.
- Properly clear all APU registers when disabling the APU.

### Changed
- The FPS text now only displays 1 digit after the decimal, 4 digits was unnecessary.
- Added functions to read/write wave channel's wave RAM -- `read_wave_ram` and `write_wave_ram`.
- Joypad's `update_button_state` now takes an enum (`ButtonAction`) to represent
  the possible actions, replacing the less expressive boolean approach.

## [0.1.0] - 2025-04-18

Initial release.

- Capable of running basic games like **Tetris** and **Dr. Mario**.
- Supports games up to MBC 1.
- Passes Blargg's `instr_timing` and `cpu_instrs` tests.
- Audio is functional but not fully accurate due to many edge cases on the Gameboy.
