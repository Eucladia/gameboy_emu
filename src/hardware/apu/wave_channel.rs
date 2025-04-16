use crate::flags::is_flag_set;

/// A wave channel, known as sound channel 3.
#[derive(Debug)]
pub struct WaveChannel {
  /// The DAC.
  nr30: u8,
  /// The length timer.
  nr31: u8,
  /// The output level.
  nr32: u8,
  /// The low frequency.
  nr33: u8,
  /// The high frequency and control.
  nr34: u8,

  frequency_timer: u16,
  length_timer: u16,

  enabled: bool,

  wave_ram: [u8; 16],
  wave_ram_index: u8,
}

impl WaveChannel {
  pub fn new() -> Self {
    Self {
      nr30: 0x7F,
      nr31: 0xFF,
      nr32: 0x9F,
      nr33: 0xFF,
      nr34: 0xBF,

      frequency_timer: 0,
      length_timer: 0,
      wave_ram_index: 0,
      enabled: false,

      wave_ram: [0; 16],
    }
  }

  /// Steps the wave channel.
  pub fn step(&mut self) {
    if self.frequency_timer == 0 {
      self.frequency_timer = self.frequency_timer_reload() * DOTS_MULTIPLIER;
      self.wave_ram_index = (self.wave_ram_index + 1) % WAVEFORM_SAMPLE_COUNT;
    }

    self.frequency_timer -= 1;
  }

  /// Steps this channel's length timer.
  pub fn step_length_timer(&mut self) {
    const LENGTH_ENABLE_MASK: u8 = 0b0100_0000;

    if !is_flag_set!(self.nr34, LENGTH_ENABLE_MASK) || self.length_timer == 0 {
      return;
    }

    self.length_timer -= 1;

    if self.length_timer == 0 {
      self.disable();
    }
  }

  /// Reads the channel's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address & 0xFF {
      0x1A => self.nr30 | 0x7F,
      0x1B => 0xFF,
      0x1C => self.nr32 | 0x9F,
      0x1D => 0xFF,
      0x1E => self.nr34 | 0xBF,
      0x30..=0x3F => {
        if self.enabled {
          0xFF
        } else {
          self.wave_ram[address as usize - 0xFF30]
        }
      }

      _ => unreachable!(),
    }
  }

  /// Writes to the channel's registers.
  pub fn write_register(&mut self, address: u16, value: u8) {
    match address & 0xFF {
      0x1A => {
        self.nr30 = value;

        if !self.is_dac_on() {
          self.disable();
        }
      }
      0x1B => {
        self.nr31 = value;
        self.reload_length_timer();
      }
      0x1C => self.nr32 = value,
      0x1D => self.nr33 = value,
      0x1E => {
        self.nr34 = value;

        if is_flag_set!(value, CHANNEL_TRIGGER_BIT_MASK) {
          self.trigger();
        }
      }
      0x30..=0x3F => self.wave_ram[address as usize - 0xFF30] = value,

      _ => unreachable!(),
    }
  }

  /// Gets the current sample.
  pub fn get_sample(&self) -> u8 {
    if !self.enabled {
      return 0;
    }

    let raw_byte = self.wave_ram[(self.wave_ram_index >> 1) as usize];
    let wave_byte = if self.wave_ram_index % 2 == 0 {
      raw_byte >> 4
    } else {
      raw_byte & 0x0F
    };

    // Adjust for the volume
    match (self.nr32 >> 5) & 0x03 {
      // Muted
      0b00 => 0,
      // 100% volume
      0b01 => wave_byte,
      // 50% volume
      0b10 => wave_byte >> 1,
      // 25% volume
      0b11 => wave_byte >> 2,
      _ => unreachable!(),
    }
  }

  /// Triggers this channel.
  fn trigger(&mut self) {
    self.enabled = self.is_dac_on();

    if self.length_timer == 0 {
      self.length_timer = CHANNEL_LENGTH_TIMER_TICKS;
    }

    self.frequency_timer = self.frequency_timer_reload() * DOTS_MULTIPLIER;
    self.wave_ram_index = 0;
  }

  /// Returns the raw 11-bit period value of the channel.
  fn get_period(&self) -> u16 {
    let low = self.nr33 as u16;
    let high = ((self.nr34 & 0x07) as u16) << 8;

    high | low
  }

  /// Returns the frequency timer reload value.
  fn frequency_timer_reload(&self) -> u16 {
    MAX_FREQUENCY - self.get_period()
  }

  /// Returns whether the DAC is enabled.
  fn is_dac_on(&self) -> bool {
    /// The bitmask to check whether the DAC is enabled.
    const DAC_ENABLE_MASK: u8 = 0b1000_0000;

    is_flag_set!(self.nr30, DAC_ENABLE_MASK)
  }

  /// Reloads the length timer.
  fn reload_length_timer(&mut self) {
    self.length_timer = CHANNEL_LENGTH_TIMER_TICKS - self.nr31 as u16;
  }

  /// Disables this sound channel.
  fn disable(&mut self) {
    self.enabled = false;
  }
}

/// The number of ticks for the channel length timer.
const CHANNEL_LENGTH_TIMER_TICKS: u16 = 0b0000_0001_0000_0000;
/// The maximum frequency.
const MAX_FREQUENCY: u16 = 2048;
/// The multiplication factor for checking if the frequency was reached.
const DOTS_MULTIPLIER: u16 = 2;
/// The bitmask for checking if a channel should be triggered.
const CHANNEL_TRIGGER_BIT_MASK: u8 = 0b1000_0000;
/// The number of samples in waveform.
const WAVEFORM_SAMPLE_COUNT: u8 = 32;
