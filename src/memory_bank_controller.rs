mod mbc_0;

pub use mbc_0::*;

/// The kind of memory bank controller.
pub enum MemoryBankController {
  Zero(mbc_0::MemoryBankController0),
}
