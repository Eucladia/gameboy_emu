use crate::flags::is_flag_set;

/// The noise channel, known as channel 4.
#[derive(Debug)]
pub struct NoiseChannel {
  /// The length timer.
  nr41: u8,
  /// The volume and envelope.
  nr42: u8,
  /// The frequency and randomness
  nr43: u8,
  /// The control.
  nr44: u8,

  frequency_timer: u16,
  amplitude: u8,
  envelope_timer: u8,
  length_timer: u8,

  lsfr: u16,

  enabled: bool,
}

impl NoiseChannel {
  pub fn new() -> Self {
    Self {
      nr41: 0xFF,
      nr42: 0x00,
      nr43: 0x00,
      nr44: 0xBF,

      frequency_timer: 0,
      amplitude: 0,
      envelope_timer: 0,
      length_timer: 0,

      lsfr: 0x7FFF,

      enabled: false,
    }
  }

  /// Steps the noise channel.
  pub fn step(&mut self) {
    const LSFR_WIDTH_MODE_MASK: u8 = 0b000_1000;

    if self.frequency_timer == 0 {
      self.frequency_timer = self.get_frequency();

      // XOR the first 2 bits
      let xor = (self.lsfr & 0x01) ^ ((self.lsfr >> 1) & 0x01);

      self.lsfr = (self.lsfr >> 1) | (xor << 14);

      // Set the 7th bit if in smaller width mode
      if is_flag_set!(self.nr43, LSFR_WIDTH_MODE_MASK) {
        self.lsfr = (self.lsfr & 0b1111_1111_1011_1111) | xor;
      }
    }

    self.frequency_timer -= 1;
  }

  /// Steps the envelope.
  pub fn step_envelope(&mut self) {
    let sweep_pace = self.nr42 & 0x07;

    if sweep_pace == 0 || self.envelope_timer == 0 {
      return;
    }

    self.envelope_timer -= 1;

    if self.envelope_timer == 0 {
      self.update_envelope_timer();
    }
  }

  /// Steps the length timer.
  pub fn step_length_timer(&mut self) {
    const LENGTH_ENABLE_MASK: u8 = 0b0100_0000;

    if !is_flag_set!(self.nr44, LENGTH_ENABLE_MASK) || self.length_timer == 0 {
      return;
    }

    self.length_timer -= 1;

    if self.length_timer == 0 {
      self.enabled = false;
    }
  }

  /// Reads this channel's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address & 0xFF {
      0x20 => 0xFF,
      0x21 => self.nr42,
      0x22 => self.nr43,
      0x23 => self.nr44 | 0xBF,

      _ => unreachable!(),
    }
  }

  /// Writes to this channel's registers.
  pub fn write_register(&mut self, address: u16, value: u8) {
    match address & 0xFF {
      0x20 => {
        self.nr41 = value;
        self.reload_length_timer();
      }
      0x21 => {
        self.nr42 = value;

        if !self.is_dac_on() {
          self.enabled = false;
        }
      }
      0x22 => self.nr43 = value,
      0x23 => {
        self.nr44 = value;

        if is_flag_set!(value, CHANNEL_TRIGGER_BIT_MASK) {
          self.trigger();
        }
      }

      _ => unreachable!(),
    }
  }

  /// Returns the current sample.
  pub fn get_sample(&self) -> u8 {
    if !self.enabled {
      return 0;
    }

    if (self.lsfr & 0x01) == 0 {
      self.amplitude
    } else {
      0
    }
  }

  /// Returns whether this sound channel is enabled.
  pub fn enabled(&self) -> bool {
    self.enabled
  }

  /// Triggers this sound channel.
  fn trigger(&mut self) {
    self.enabled = self.is_dac_on();

    if self.length_timer == 0 {
      self.length_timer = CHANNEL_LENGTH_TIMER_TICKS;
    }

    self.envelope_timer = self.nr42 & 0x07;
    self.amplitude = (self.nr42 >> 4) & 0x0F;

    self.lsfr = 0x7FFF;
  }

  /// Updates the envelope timer.
  fn update_envelope_timer(&mut self) {
    const ENVELOPE_DIRECTION_MASK: u8 = 0b0000_1000;

    self.envelope_timer = self.nr42 & 0x07;

    // Update the volume
    if is_flag_set!(self.nr42, ENVELOPE_DIRECTION_MASK) {
      if self.amplitude < 0x0F {
        self.amplitude += 1;
      }
    } else {
      self.amplitude = self.amplitude.saturating_sub(1);
    }
  }

  /// Reloads the length timer.
  fn reload_length_timer(&mut self) {
    // The length timer is stored in the first 6 bits
    let length_timer = self.nr41 & 0b0011_1111;

    self.length_timer = CHANNEL_LENGTH_TIMER_TICKS - length_timer
  }

  /// Gets the channel's frequency.
  fn get_frequency(&self) -> u16 {
    let clock_shift = (self.nr43 >> 4) as u16;
    let mut clock_divider = (self.nr43 & 0x07) as u16;

    if clock_divider == 0 {
      clock_divider = 8;
    } else {
      clock_divider <<= 4;
    }

    clock_divider << clock_shift
  }

  /// Returns whether this channel's DAC is enabled.
  fn is_dac_on(&self) -> bool {
    // Channel 4's DAC is disabled if bits 3-7 are all 0
    (self.nr42 >> 3) != 0
  }
}

/// The number of ticks for the channel length timer.
const CHANNEL_LENGTH_TIMER_TICKS: u8 = 64;
/// The bitmask for checking if a channel should be triggered.
const CHANNEL_TRIGGER_BIT_MASK: u8 = 0b1000_0000;
