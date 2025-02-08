pub type GameBoyResult<T> = std::result::Result<T, GameBoyError>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GameBoyError {
  InvalidHeader,
  InvalidComplementChecksum,
  InvalidChecksum,
}
