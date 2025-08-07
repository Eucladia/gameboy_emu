use crate::flags::{is_flag_set, is_rising_edge};

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

  frequency_timer: u32,
  volume: u8,
  envelope_timer: u8,
  length_timer: u8,

  lsfr: u16,

  enabled: bool,
}

impl NoiseChannel {
  pub fn new() -> Self {
    Self {
      nr41: 0,
      nr42: 0,
      nr43: 0,
      nr44: 0,

      frequency_timer: 0,
      volume: 0,
      envelope_timer: 0,
      length_timer: 0,

      lsfr: 0,

      enabled: false,
    }
  }

  /// Steps the noise channel.
  pub fn step(&mut self) {
    const LSFR_WIDTH_MODE_MASK: u8 = 0b000_1000;
    const LSFR_SHORT_WIDTH_BIT: u8 = 6;

    if self.frequency_timer == 0 {
      self.frequency_timer = self.get_frequency();

      // XOR the first 2 bits
      let xor = (self.lsfr & 0x01) ^ ((self.lsfr >> 1) & 0x01);

      self.lsfr = (self.lsfr >> 1) | (xor << 14);

      // Set the 6th bit if in smaller width mode
      if is_flag_set!(self.nr43, LSFR_WIDTH_MODE_MASK) {
        self.lsfr = (self.lsfr & !(1 << LSFR_SHORT_WIDTH_BIT)) | (xor << LSFR_SHORT_WIDTH_BIT);
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
    if !is_flag_set!(self.nr44, TIMER_LENGTH_ENABLE_MASK) || self.length_timer == 0 {
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
  pub fn write_register(&mut self, address: u16, value: u8, frame_step: u8) {
    let lower_byte = address & 0xFF;

    match lower_byte {
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
        let old_value = self.nr44;
        let curr_length_enabled = is_flag_set!(value, TIMER_LENGTH_ENABLE_MASK);
        let should_trigger = is_flag_set!(value, CHANNEL_TRIGGER_MASK);

        self.nr44 = value;

        // The timer's length normally only gets clocked on even frame sequencer steps.
        // However, there are edge cases when this step is odd.
        let will_clock_length = frame_step & 1 == 0;

        // There is an edge case when there was a rising edge for the length enable
        // and the length counter isn't 0.
        //
        // When these conditions are met, the length gets clocked. If the clock caused
        // it to be 0, then the channel gets disabled as well.
        if is_rising_edge!(old_value, value, TIMER_LENGTH_ENABLE_MASK)
          && !will_clock_length
          && self.length_timer > 0
        {
          self.length_timer -= 1;

          if self.length_timer == 0 && !should_trigger {
            self.enabled = false;
          }
        }

        let old_length = self.length_timer;

        if should_trigger {
          self.trigger();
        }

        let timer_reloaded = old_length == 0 && self.length_timer == MAX_CHANNEL_TIMER_LENGTH;

        // There is another edge case when the length counter gets reloaded. That is, that
        // the length ends up being clocked.
        //
        // NOTE: The first edge case can cause this edge case to occur, so it's important
        // that we handle this edge case separately *after* calling `trigger`.
        if curr_length_enabled && timer_reloaded && !will_clock_length {
          self.length_timer -= 1;
        }
      }

      _ => unreachable!(),
    }
  }

  /// Clears the audio registers in this channel.
  pub fn clear_registers(&mut self) {
    self.nr41 = 0;
    self.nr42 = 0;
    self.nr43 = 0;
    self.nr44 = 0;

    self.enabled = false;
  }

  /// Returns the current sample.
  pub fn get_sample(&self) -> u8 {
    if !self.enabled {
      return 0;
    }

    if (self.lsfr & 0x01) == 0 {
      self.volume
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
      self.length_timer = MAX_CHANNEL_TIMER_LENGTH;
    }

    self.frequency_timer = self.get_frequency();

    self.envelope_timer = self.nr42 & 0x07;
    self.volume = (self.nr42 >> 4) & 0x0F;
    self.lsfr = 0x7FFF;
  }

  /// Updates the envelope timer.
  fn update_envelope_timer(&mut self) {
    const ENVELOPE_DIRECTION_MASK: u8 = 0b0000_1000;

    self.envelope_timer = self.nr42 & 0x07;

    // Update the volume
    if is_flag_set!(self.nr42, ENVELOPE_DIRECTION_MASK) {
      if self.volume < 0x0F {
        self.volume += 1;
      }
    } else {
      self.volume = self.volume.saturating_sub(1);
    }
  }

  /// Reloads the length timer.
  fn reload_length_timer(&mut self) {
    // The length timer is stored in the first 6 bits
    let length_timer = self.nr41 & 0b0011_1111;

    self.length_timer = MAX_CHANNEL_TIMER_LENGTH - length_timer
  }

  /// Gets the channel's frequency.
  fn get_frequency(&self) -> u32 {
    const DIVISORS: [u8; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

    // NOTE: We use `u32`s instead of `u16`s as with other sound channels because
    // `u16` can't hold all possible frequencies. This causes `dmg_sound`'s `01` test
    // to crash due to an underflow.
    let clock_shift = (self.nr43 >> 4) as u32;
    let clock_divider = DIVISORS[(self.nr43 & 0x07) as usize] as u32;

    clock_divider << clock_shift
  }

  /// Returns whether this channel's DAC is enabled.
  fn is_dac_on(&self) -> bool {
    // Channel 4's DAC is disabled if bits 3-7 are all 0
    (self.nr42 >> 3) != 0
  }
}

/// The number of ticks for the channel length timer.
const MAX_CHANNEL_TIMER_LENGTH: u8 = 64;
/// The bitmask for checking if a channel should be triggered.
const CHANNEL_TRIGGER_MASK: u8 = 0b1000_0000;
/// The bitmask for enabling the length timer.
const TIMER_LENGTH_ENABLE_MASK: u8 = 0b0100_0000;
