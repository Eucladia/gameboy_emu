# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project does not adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) until v1.0.0.

## [Unreleased]

### Added
- T-cycle accuracy.
- Support for incrementing and decrementing the master volume via `Shift` + `+` / `-`.
- Toggleable volume overlay when pressing `Shift` + `1`.
- Basic guide.

### Fixed
- Properly fetch the byte being transferred by a DMA transfer in the CPU when the PC isn't 
  in high RAM.
- Implemented OAM DMA blocking (Mooneye's `oam_dma_timing`).
- Fixed a bug where erroneous instructions would be executed when halted (Mooneye's `di_timing-GS`,
  `ei_timing`, `halt_ime0_ei`, `halt_ime0_nointr_timing`, `halt_ime1_timing`, `halt_ime1_timging2-GS`,
  `rapid_di_ei`).
- Fixed a bug when buttons from both button groups were pressed.
- Fix an issue with the noise channel not being able to handle all possible frequencies.
- Implement edge case bugs when triggering sound channels' (Blargg's `dmg_sound` test 03).
- Properly set the internal sweep enabled register on trigger (Blargg's `dmg_sound` test 04).
- Properly clear all APU registers when disabling the APU.
- Store the sample as a temporary buffer in the wave channel (Blargg's `dmg_sound` test 09/12).
- Implemented wave RAM corruption (Blargg's `dmg_sound` test 10).

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
