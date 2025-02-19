/// The starting address for ROM bank 0.
pub const ROM_BANK_0_START: u16 = 0;
/// The ending address for ROM bank 0.
pub const ROM_BANK_0_END: u16 = 0x4000;
/// The starting address for the switchable ROM bank.
pub const ROM_BANK_N_START: u16 = 0x4000;
/// The ending address  for the switchable ROM bank.
pub const ROM_BANK_N_END: u16 = 0x8000;
/// The starting address for VRAM.
pub const VIDEO_RAM_START: u16 = 0x8000;
/// The ending address  for VRAM.
pub const VIDEO_RAM_END: u16 = 0xA000;
/// The starting address for the cartridges RAM.
pub const EXTERNAL_RAM_START: u16 = 0xA000;
/// The ending address for the cartridges  RAM.
pub const EXTERNAL_RAM_END: u16 = 0xC000;
/// The starting address for work RAM.
pub const WORK_RAM_START: u16 = 0xC000;
/// The ending address for work  RAM.
pub const WORK_RAM_END: u16 = 0xE000;
/// The starting address for echo RAM.
pub const ECHO_RAM_START: u16 = 0xE000;
/// The ending address for echo RAM.
pub const ECHO_RAM_END: u16 = 0xFE00;
/// The starting address for the OAM (sprite attribute memory).
pub const OAM_START: u16 = 0xFE00;
/// The ending address for the OAM (sprite attribute memory).
pub const OAM_END: u16 = 0xFEA0;
/// The starting address for unused memory.
pub const UNUSED_START: u16 = 0xFEA0;
/// The ending address for unused memory.
pub const UNUSED_END: u16 = 0xFF00;
/// The starting address for I/O registers.
pub const IO_REGISTER_START: u16 = 0xFF00;
/// The ending address for I/O registers.
pub const IO_REGISTER_END: u16 = 0xFF80;
/// The starting address for HRAM.
pub const HIGH_RAM_START: u16 = 0xFF80;
/// The ending address for HRAM.
pub const HIGH_RAM_END: u16 = 0xFFFF;
/// The interrupt enable register.
pub const INTERRUPT_ENABLE_REGISTER: u16 = 0xFFFF;
