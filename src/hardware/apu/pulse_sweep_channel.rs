use crate::flags::is_flag_set;

/// A sweeping pulse channel, known as channel 1.
#[derive(Debug)]
pub struct PulseSweepChannel {
  /// The sweep register.
  nr10: u8,
  /// The sound length and wave pattern duty.
  nr11: u8,
  /// The envelope.
  nr12: u8,
  /// The low frequency.
  nr13: u8,
  /// The high frequency and control.
  nr14: u8,

  frequency_timer: u16,

  duty_step: u8,
  volume: u8,
  length_timer: u8,
  envelope_timer: u8,

  enabled: bool,

  shadow_frequency: u16,
  sweep_timer: u8,
  sweep_enabled: bool,

  /// Whether there was a negative sweep calculation since the last trigger.
  had_negative_sweep_calc: bool,
}

impl PulseSweepChannel {
  pub fn new() -> Self {
    Self {
      nr10: 0,
      nr11: 0,
      nr12: 0,
      nr13: 0,
      nr14: 0,

      frequency_timer: 0,
      duty_step: 0,
      volume: 0,
      length_timer: 0,
      envelope_timer: 0,

      enabled: false,

      shadow_frequency: 0,
      sweep_timer: 0,
      sweep_enabled: false,

      had_negative_sweep_calc: false,
    }
  }

  /// Steps the sweeping pulse channel.
  pub fn step(&mut self) {
    // Reload the frequency timer.
    if self.frequency_timer == 0 {
      self.frequency_timer = self.frequency_timer_reload() * DOTS_MULTIPLIER;
      self.duty_step = (self.duty_step + 1) % WAVEFORM_SAMPLE_COUNT;
    }

    self.frequency_timer -= 1;
  }

  /// Steps the envelope.
  pub fn step_envelope(&mut self) {
    let sweep_pace = self.nr12 & 0x07;

    // A sweep pace of 0 disables the envelope
    if sweep_pace == 0 || self.envelope_timer == 0 {
      return;
    }

    self.envelope_timer -= 1;

    if self.envelope_timer == 0 {
      self.update_envelope_timer();
    }
  }

  /// Steps the sweep.
  pub fn step_sweep(&mut self) {
    if self.sweep_timer == 0 {
      return;
    }

    self.sweep_timer -= 1;

    if self.sweep_timer == 0 {
      self.update_sweep_timer();
    }
  }

  /// Steps the length timer.
  pub fn step_length_timer(&mut self) {
    const LENGTH_ENABLE_MASK: u8 = 0b0100_0000;

    if !is_flag_set!(self.nr14, LENGTH_ENABLE_MASK) || self.length_timer == 0 {
      return;
    }

    self.length_timer -= 1;

    if self.length_timer == 0 {
      self.enabled = false;
    }
  }

  /// Returns the current sample.
  pub fn get_sample(&self) -> u8 {
    if !self.enabled {
      return 0;
    }

    const DUTY_TABLE: [[u8; 8]; 4] = [
      [0, 0, 0, 0, 0, 0, 0, 1],
      [0, 0, 0, 0, 0, 0, 1, 1],
      [0, 0, 0, 0, 1, 1, 1, 1],
      [1, 1, 1, 1, 1, 1, 0, 0],
    ];

    let wave_duty = self.nr11 >> 6;

    DUTY_TABLE[wave_duty as usize][self.duty_step as usize] * self.volume
  }

  /// Reads the channel's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address & 0xFF {
      0x10 => self.nr10 | 0x80,
      0x11 => self.nr11 | 0x3F,
      0x12 => self.nr12,
      0x13 => 0xFF,
      0x14 => self.nr14 | 0xBF,

      _ => unreachable!(),
    }
  }

  /// Writes to the channel's registers.
  pub fn write_register(&mut self, apu_enabled: bool, address: u16, value: u8, frame_step: u8) {
    let lower_byte = address & 0xFF;

    // Writes aren't allowed when the APU is turned off, unless we're writing to the
    // length counter.
    if !apu_enabled && lower_byte != 0x11 {
      return;
    }

    match lower_byte {
      0x10 => {
        // We went from negative to positive after a negative sweep calculation
        // was done, so disable the channel.
        if self.had_negative_sweep_calc
          && is_flag_set!(self.nr10, SWEEP_DIRECTION_MASK)
          && !is_flag_set!(value, SWEEP_DIRECTION_MASK)
        {
          self.enabled = false;
        }

        self.nr10 = value;
      }
      0x11 => {
        // If the APU is disabled, then ONLY read the length bits
        self.nr11 = if apu_enabled { value } else { value & 0x3F };
        self.reload_length_timer();
      }
      0x12 => {
        self.nr12 = value;

        if !self.is_dac_on() {
          self.enabled = false;
        }
      }
      0x13 => self.nr13 = value,
      0x14 => {
        let prev_length_enabled = is_flag_set!(self.nr14, TIMER_LENGTH_ENABLE_MASK);
        let curr_length_enabled = is_flag_set!(value, TIMER_LENGTH_ENABLE_MASK);
        let should_trigger = is_flag_set!(value, CHANNEL_TRIGGER_MASK);

        self.nr14 = value;

        // The timer's length normally only gets clocked on even frame sequencer steps.
        // However, there are edge cases when this step is odd.
        let will_clock_length = frame_step & 1 == 0;

        // There is an edge case when there was a rising edge for the length enable
        // and the length counter isn't 0.
        //
        // When these conditions are met, the length gets clocked. If the clock caused
        // it to be 0, then the channel gets disabled as well.
        if !prev_length_enabled
          && curr_length_enabled
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
    self.nr10 = 0;
    self.nr11 = 0;
    self.nr12 = 0;
    self.nr13 = 0;
    self.nr14 = 0;

    self.enabled = false;
  }

  /// Returns whether this sound channel is enabled.
  pub fn enabled(&self) -> bool {
    self.enabled
  }

  /// Triggers this channel.
  fn trigger(&mut self) {
    self.enabled = self.is_dac_on();

    if self.length_timer == 0 {
      self.length_timer = MAX_CHANNEL_TIMER_LENGTH;
    }

    self.frequency_timer = self.frequency_timer_reload() * DOTS_MULTIPLIER;
    self.envelope_timer = self.nr12 & 0x07;
    self.volume = self.nr12 >> 4;
    self.duty_step = 0;

    // Update the sweep registers
    let sweep_pace = (self.nr10 >> 4) & 0x07;

    self.shadow_frequency = self.get_period();
    self.sweep_timer = sweep_pace;

    if self.sweep_timer == 0 {
      self.sweep_timer = 8;
    }

    let sweep_step = self.nr10 & 0x07;

    self.sweep_enabled = sweep_pace != 0 || sweep_step != 0;
    // It's important to reset the flag here because the next call to
    // `calculate_next_sweep_frequency` can be a negative sweep calculation.
    self.had_negative_sweep_calc = false;

    if sweep_step != 0 {
      let new_freq = self.calculate_next_sweep_frequency();

      if new_freq > 0x07FF {
        self.enabled = false;
      }
    }
  }

  /// Updates the envelope timer.
  fn update_envelope_timer(&mut self) {
    self.envelope_timer = self.nr12 & 0x07;

    // Update the volume, bounding it to [0, 15]
    if is_flag_set!(self.nr12, ENVELOPE_DIRECTION_MASK) {
      if self.volume < 0x0F {
        self.volume += 1;
      }
    } else {
      self.volume = self.volume.saturating_sub(1);
    }
  }

  /// Updates the sweep timer.
  fn update_sweep_timer(&mut self) {
    let sweep_pace = (self.nr10 >> 4) & 0x07;

    self.sweep_timer = sweep_pace;

    // A period of 0 is treated as 8
    if self.sweep_timer == 0 {
      self.sweep_timer = 8;
    }

    if self.sweep_enabled && sweep_pace != 0 {
      self.update_sweep_frequency();
    }
  }

  /// Calculates the next frequency value.
  fn calculate_next_sweep_frequency(&mut self) -> u16 {
    let sweep_shift = self.nr10 & 0x07;

    if is_flag_set!(self.nr10, SWEEP_DIRECTION_MASK) {
      // NOTE: Place this inside because the GBDev Wiki says that there needs to be have
      // been *at least* one negative sweep calculation done for the channel to be
      // considered for disabling when writing to NR10.
      self.had_negative_sweep_calc = true;

      self.shadow_frequency - (self.shadow_frequency >> sweep_shift)
    } else {
      self.shadow_frequency + (self.shadow_frequency >> sweep_shift)
    }
  }

  /// Updates the sweep frequency.
  fn update_sweep_frequency(&mut self) {
    let new_freq = self.calculate_next_sweep_frequency();
    let sweep_shift = self.nr10 & 0x07;

    // Turn off the channel if the new frequency would overflow
    if new_freq > 0x7FF {
      self.enabled = false;

      return;
    }

    // Update shadow registers
    if sweep_shift != 0 {
      self.shadow_frequency = new_freq;
      self.nr13 = (new_freq & 0xFF) as u8;
      self.nr14 = (self.nr14 & 0b1111_1000) | (new_freq >> 8) as u8 & 0x07;

      let new_freq = self.calculate_next_sweep_frequency();

      if new_freq > 0x7FF {
        self.enabled = false;
      }
    }
  }

  /// Returns the raw 11-bit period value of the channel.
  fn get_period(&self) -> u16 {
    let low = self.nr13;
    // Only take the lower 3 bits from NR14
    let high = self.nr14 & 0x07;

    ((high as u16) << 8) | (low as u16)
  }

  /// Returns the frequency timer reload value.
  fn frequency_timer_reload(&self) -> u16 {
    MAX_FREQUENCY - self.get_period()
  }

  /// Reloads the length timer.
  fn reload_length_timer(&mut self) {
    // The length timer is stored in the first 6 bits
    let length_timer = self.nr11 & 0b0011_1111;

    self.length_timer = MAX_CHANNEL_TIMER_LENGTH - length_timer
  }

  /// Returns whether this channel's DAC is enabled.
  fn is_dac_on(&self) -> bool {
    // Channel 1's DAC is disabled if bits 3-7 are all 0
    (self.nr12 >> 3) != 0
  }
}

/// The number of ticks for the channel length timer.
const MAX_CHANNEL_TIMER_LENGTH: u8 = 64;
/// The maximum frequency.
const MAX_FREQUENCY: u16 = 2048;
/// The multiplication factor for checking if the frequency was reached.
const DOTS_MULTIPLIER: u16 = 4;
/// The bitmask for checking if a channel should be triggered.
const CHANNEL_TRIGGER_MASK: u8 = 0b1000_0000;
/// The number of samples in waveform.
const WAVEFORM_SAMPLE_COUNT: u8 = 8;
/// The bitmask for enabling the length timer.
const TIMER_LENGTH_ENABLE_MASK: u8 = 0b0100_0000;
/// The bitmask for the direction bit for the envelope.
const ENVELOPE_DIRECTION_MASK: u8 = 0b0000_1000;
/// The bitmask for the direction bit for the sweep.
const SWEEP_DIRECTION_MASK: u8 = 0b0000_1000;
