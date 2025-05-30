use crate::flags::is_flag_set;

/// A pulse channel, known as channel 2.
#[derive(Debug)]
pub struct PulseChannel {
  /// The sound length and wave pattern duty.
  nr21: u8,
  /// The envelope.
  nr22: u8,
  /// The low frequency.
  nr23: u8,
  /// The low frequency.
  nr24: u8,

  frequency_timer: u16,
  duty_step: u8,
  volume: u8,
  length_timer: u8,
  envelope_timer: u8,

  enabled: bool,
}

impl PulseChannel {
  pub fn new() -> Self {
    Self {
      nr21: 0,
      nr22: 0,
      nr23: 0,
      nr24: 0,

      frequency_timer: 0,
      duty_step: 0,
      volume: 0,
      length_timer: 0,
      envelope_timer: 0,

      enabled: false,
    }
  }

  /// Steps the pulse channel.
  pub fn step(&mut self) {
    // Reload the frequency timer
    if self.frequency_timer == 0 {
      self.frequency_timer = self.frequency_timer_reload() * DOTS_MULTIPLIER;
      self.duty_step = (self.duty_step + 1) % WAVEFORM_SAMPLE_COUNT;
    }

    self.frequency_timer -= 1;
  }

  /// Steps the length timer.
  pub fn step_length_timer(&mut self) {
    const LENGTH_ENABLE_MASK: u8 = 0b0100_0000;

    if !is_flag_set!(self.nr24, LENGTH_ENABLE_MASK) || self.length_timer == 0 {
      return;
    }

    self.length_timer -= 1;

    if self.length_timer == 0 {
      self.enabled = false;
    }
  }

  /// Steps the envelope.
  pub fn step_envelope(&mut self) {
    let sweep_pace = self.nr22 & 0x07;

    if sweep_pace == 0 || self.envelope_timer == 0 {
      return;
    }

    self.envelope_timer -= 1;

    if self.envelope_timer == 0 {
      self.update_envelope_timer();
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

    let wave_duty = self.nr21 >> 6;

    DUTY_TABLE[wave_duty as usize][self.duty_step as usize] * self.volume
  }

  /// Reads the channel's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address & 0xFF {
      0x16 => self.nr21 | 0x3F,
      0x17 => self.nr22,
      0x18 => 0xFF,
      0x19 => self.nr24 | 0xBF,

      _ => unreachable!(),
    }
  }

  /// Writes to the channel's registers.
  pub fn write_register(&mut self, apu_enabled: bool, address: u16, value: u8, frame_step: u8) {
    let lower_byte = address & 0xFF;

    // Writes aren't allowed when the APU is turned off, unless we're writing to the
    // length counter.
    if !apu_enabled && lower_byte != 0x16 {
      return;
    }

    match lower_byte {
      0x16 => {
        // If the APU is disabled, then ONLY read the length bits
        self.nr21 = if apu_enabled { value } else { value & 0x3F };
        self.reload_length_timer();
      }
      0x17 => {
        self.nr22 = value;

        if !self.is_dac_on() {
          self.enabled = false;
        }
      }
      0x18 => self.nr23 = value,
      0x19 => {
        let prev_length_enabled = is_flag_set!(self.nr24, TIMER_LENGTH_ENABLE_MASK);
        let curr_length_enabled = is_flag_set!(value, TIMER_LENGTH_ENABLE_MASK);
        let should_trigger = is_flag_set!(value, CHANNEL_TRIGGER_MASK);

        self.nr24 = value;

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
    self.nr21 = 0;
    self.nr22 = 0;
    self.nr23 = 0;
    self.nr24 = 0;

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
    self.envelope_timer = self.nr22 & 0x07;
    self.volume = self.nr22 >> 4;
    self.duty_step = 0;
  }

  /// Reloads the length timer.
  fn reload_length_timer(&mut self) {
    // The length timer is stored in the first 6 bits
    let length_timer = self.nr21 & 0b0011_1111;

    self.length_timer = MAX_CHANNEL_TIMER_LENGTH - length_timer
  }

  /// Updates the envelope timer.
  fn update_envelope_timer(&mut self) {
    const ENVELOPE_DIRECTION_MASK: u8 = 0b1000;

    self.envelope_timer = self.nr22 & 0x07;

    // Update the volume, bounding it to [0, 15]
    if is_flag_set!(self.nr22, ENVELOPE_DIRECTION_MASK) {
      if self.volume < 0x0F {
        self.volume += 1;
      }
    } else {
      self.volume = self.volume.saturating_sub(1);
    }
  }

  /// Returns the raw 11-bit period value of the channel.
  fn get_period(&self) -> u16 {
    let low = self.nr23;
    // Only take the lower 3 bits from NR14
    let high = self.nr24 & 0x07;

    ((high as u16) << 8) | (low as u16)
  }

  /// Returns the frequency timer reload value.
  fn frequency_timer_reload(&self) -> u16 {
    MAX_FREQUENCY - self.get_period()
  }

  /// Returns whether this channel's DAC is enabled.
  fn is_dac_on(&self) -> bool {
    // Channel 2's DAC is disabled if bits 3-7 are all 0
    (self.nr22 >> 3) != 0
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
