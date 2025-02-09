use crate::memory::Mbc0;

/// The kind of memory bank controller.
#[derive(Debug)]
pub enum MemoryBankController {
  Zero(Mbc0),
}
