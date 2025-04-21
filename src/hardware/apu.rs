mod noise_channel;
mod pulse_channel;
mod pulse_sweep_channel;
mod wave_channel;

use std::{
  collections::VecDeque,
  sync::{Arc, Mutex},
};

use crate::{
  flags::{add_flag, is_flag_set},
  hardware::apu::{
    noise_channel::NoiseChannel, pulse_channel::PulseChannel,
    pulse_sweep_channel::PulseSweepChannel, wave_channel::WaveChannel,
  },
};

#[derive(Debug)]
pub struct Apu {
  channel1: PulseSweepChannel,
  channel2: PulseChannel,
  channel3: WaveChannel,
  channel4: NoiseChannel,

  nr50: u8,
  nr51: u8,
  nr52: u8,

  frame_sequencer_cycles: u16,
  frame_sequencer_step: u8,

  dots: u16,

  volume: f32,

  audio_buffer: Arc<Mutex<VecDeque<AudioSample>>>,
}

impl Apu {
  pub fn new() -> Self {
    Self {
      channel1: PulseSweepChannel::new(),
      channel2: PulseChannel::new(),
      channel3: WaveChannel::new(),
      channel4: NoiseChannel::new(),

      nr50: 0,
      nr51: 0,
      nr52: 0,

      frame_sequencer_cycles: 0,
      frame_sequencer_step: 0,

      dots: 0,

      volume: 0.5,

      audio_buffer: Arc::new(Mutex::new(VecDeque::new())),
    }
  }

  /// Steps the APU.
  pub fn step(&mut self, cycles: usize) {
    if !self.is_enabled() {
      return;
    }

    for _ in 0..cycles {
      self.channel1.step();
      self.channel2.step();
      self.channel3.step();
      self.channel4.step();

      self.step_frame_sequencer();

      self.dots += 1;
    }

    if self.dots >= SAMPLES_PER_CYCLE {
      self.dots -= SAMPLES_PER_CYCLE;

      self.push_audio_sample();
    }
  }

  /// Reads the APU's registers.
  pub fn read_register(&self, address: u16) -> u8 {
    match address {
      // Sound channel 1
      0xFF10..0xFF15 => self.channel1.read_register(address),
      // Undocumented
      0xFF15 => 0xFF,
      // Sound channel 2
      0xFF16..0xFF1A => self.channel2.read_register(address),
      // Sound channel 3
      0xFF1A..0xFF1F => self.channel3.read_register(address),
      // Undocumented
      0xFF1F => 0xFF,
      // Sound channel 4
      0xFF20..0xFF24 => self.channel4.read_register(address),

      // Global registers
      0xFF24 => self.nr50,
      0xFF25 => self.nr51,
      0xFF26 => self.nr52 | self.enabled_channels() | 0b0111_0000,

      // Wave RAM
      0xFF30..0xFF40 => self.channel3.read_wave_ram(address),

      x => unreachable!("apu: tried to read {:02X}", x),
    }
  }

  /// Writes to the APU's registers.
  pub fn write_register(&mut self, address: u16, value: u8) {
    // Only NR52, wave RAM, and the length counters are writable when the APU is disabled.
    let apu_enabled = self.is_enabled();

    match address {
      // Sound channel 1
      0xFF10..0xFF15 => {
        self
          .channel1
          .write_register(apu_enabled, address, value, self.frame_sequencer_step)
      }
      // Undocumented
      0xFF15 => {}
      // Sound channel 2
      0xFF16..0xFF1A => {
        self
          .channel2
          .write_register(apu_enabled, address, value, self.frame_sequencer_step)
      }
      // Sound channel 3
      0xFF1A..0xFF1F => {
        self
          .channel3
          .write_register(apu_enabled, address, value, self.frame_sequencer_step)
      }
      // Undocumented
      0xFF1F => {}
      // Sound channel 4
      0xFF20..0xFF24 => {
        self
          .channel4
          .write_register(apu_enabled, address, value, self.frame_sequencer_step)
      }

      // Global registers
      0xFF24 => {
        if apu_enabled {
          self.nr50 = value
        }
      }
      0xFF25 => {
        if apu_enabled {
          self.nr51 = value
        }
      }
      0xFF26 => {
        const APU_ENABLE_MASK: u8 = 0b1000_0000;

        let turning_off = apu_enabled && !is_flag_set!(value, APU_ENABLE_MASK);
        let turning_on = !apu_enabled && is_flag_set!(value, APU_ENABLE_MASK);

        // If the APU is being turned off, we need to clear its registers.
        if turning_off {
          // Clear sound channels
          self.channel1.clear_registers();
          self.channel2.clear_registers();
          self.channel3.clear_registers();
          self.channel4.clear_registers();

          // Clear global registers
          self.nr50 = 0;
          self.nr51 = 0;
        }

        // There's an edge case when there is a rising edge for the APU's enable bit.
        //
        // That is, if the APU is being turned on, we need to clear the frame sequencer step.
        if turning_on {
          self.frame_sequencer_step = 0;
        }

        // Only the MSB is writeable
        self.nr52 = value & 0x80;
      }

      // Wave RAM
      0xFF30..0xFF40 => self.channel3.write_wave_ram(address, value),

      x => unreachable!("apu: tried to write {:02X}", x),
    }
  }

  /// Increments the master volume by 10%.
  pub fn increment_volume(&mut self) {
    self.set_volume(self.volume() + VOLUME_INCREMENT);
  }

  /// Decrements the master volume by 10%.
  pub fn decrement_volume(&mut self) {
    self.set_volume(self.volume() - VOLUME_INCREMENT)
  }

  /// Sets the master volume.
  pub fn set_volume(&mut self, volume: f32) {
    self.volume = volume.clamp(0.0, 1.0);
  }

  /// Returns the current master volume.
  pub fn volume(&self) -> f32 {
    self.volume
  }

  /// Returns the audio buffer.
  pub fn audio_buffer(&self) -> Arc<Mutex<VecDeque<AudioSample>>> {
    Arc::clone(&self.audio_buffer)
  }

  /// Pushes a new audio channel into the audio buffer.
  fn push_audio_sample(&self) {
    let ch1 = self.channel1.get_sample();
    let ch2 = self.channel2.get_sample();
    let ch3 = self.channel3.get_sample();
    let ch4 = self.channel4.get_sample();

    // The system device channel outputs
    let mut left = 0.0;
    let mut right = 0.0;

    // Selectively add the sound channel outputs to the left and right channels
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel1Right as u8) {
      right += ch1 as f32 / 15.0;
    }
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel2Right as u8) {
      right += ch2 as f32 / 15.0;
    }
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel3Right as u8) {
      right += ch3 as f32 / 15.0;
    }
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel4Right as u8) {
      right += ch4 as f32 / 15.0;
    }
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel1Left as u8) {
      left += ch1 as f32 / 15.0;
    }
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel2Left as u8) {
      left += ch2 as f32 / 15.0;
    }
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel3Left as u8) {
      left += ch3 as f32 / 15.0;
    }
    if is_flag_set!(self.nr51, SoundPanningFlags::Channel4Left as u8) {
      left += ch4 as f32 / 15.0;
    }

    // Apply volume scaling for each output channel
    let left_volume = (self.nr50 >> 4) & 0x07;
    let right_volume = self.nr50 & 0x07;

    // Add 1 because the amplifier treats never mutes the output
    left *= (left_volume + 1) as f32 / 8.0;
    right *= (right_volume + 1) as f32 / 8.0;

    // Scale by the master volume and normalize the outputs
    let volume_scale = self.volume / 4.0;

    left *= volume_scale;
    right *= volume_scale;

    self
      .audio_buffer
      .lock()
      .unwrap()
      .push_back(AudioSample { left, right });
  }

  /// Steps the frame sequencer.
  fn step_frame_sequencer(&mut self) {
    self.frame_sequencer_cycles += 1;

    if self.frame_sequencer_cycles == FRAME_SEQEUNCER_CYCLES {
      match self.frame_sequencer_step & (FRAME_SEQUENCER_STEP_COUNT - 1) {
        step @ (0 | 2 | 4 | 6) => {
          // Length counters step every even step
          self.channel1.step_length_timer();
          self.channel2.step_length_timer();
          self.channel3.step_length_timer();
          self.channel4.step_length_timer();

          // Pulse channel steps its sweep every 2nd and 6th step
          if step == 2 || step == 6 {
            self.channel1.step_sweep();
          }
        }

        // Do nothing on 1, 3, and 5
        1 | 3 | 5 => {}

        // Step the envelopes
        7 => {
          self.channel1.step_envelope();
          self.channel2.step_envelope();
          self.channel4.step_envelope();
        }

        _ => unreachable!(),
      }

      self.frame_sequencer_cycles = 0;
      self.frame_sequencer_step = (self.frame_sequencer_step + 1) % FRAME_SEQUENCER_STEP_COUNT;
    }
  }

  /// Returns whether the APU is enabled.
  fn is_enabled(&self) -> bool {
    is_flag_set!(self.nr52, APU_ENABLE_MASK)
  }

  /// Returns a bitfield of the enabled sound channels.
  fn enabled_channels(&self) -> u8 {
    let mut bitfield = 0;

    if self.channel1.enabled() {
      add_flag!(&mut bitfield, EnabledChannels::Channel1 as u8);
    }

    if self.channel2.enabled() {
      add_flag!(&mut bitfield, EnabledChannels::Channel2 as u8);
    }

    if self.channel3.enabled() {
      add_flag!(&mut bitfield, EnabledChannels::Channel3 as u8);
    }

    if self.channel4.enabled() {
      add_flag!(&mut bitfield, EnabledChannels::Channel4 as u8);
    }

    bitfield
  }
}

/// An audio sample with a left and right channel.
#[derive(Debug, Default, Clone)]
pub struct AudioSample {
  /// The left sound channel.
  pub left: f32,
  /// The right sound channel.
  pub right: f32,
}

/// The audio channels' outputs.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum SoundPanningFlags {
  Channel1Right = 1 << 0,
  Channel2Right = 1 << 1,
  Channel3Right = 1 << 2,
  Channel4Right = 1 << 3,

  Channel1Left = 1 << 4,
  Channel2Left = 1 << 5,
  Channel3Left = 1 << 6,
  Channel4Left = 1 << 7,
}

/// The enabled channels for the lower nibble of the NR52 register.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum EnabledChannels {
  Channel1 = 1 << 0,
  Channel2 = 1 << 1,
  Channel3 = 1 << 2,
  Channel4 = 1 << 3,
}

/// The samples per cycle.
const SAMPLES_PER_CYCLE: u16 = (GAMEBOY_CLOCK_SPEED / SAMPLE_RATE) as u16;
/// The Gameboy's clock speed.
const GAMEBOY_CLOCK_SPEED: u32 = 4_194_304;
/// The sample rate.
const SAMPLE_RATE: u32 = 44_100;
/// The number of cycles per frame sequencer step.
const FRAME_SEQEUNCER_CYCLES: u16 = 8192;
/// The step count for the frame sequenecer.
const FRAME_SEQUENCER_STEP_COUNT: u8 = 8;
/// The bitmask for checking whether the APU is enabled.
const APU_ENABLE_MASK: u8 = 0b1000_0000;
/// The increment for adjusting the volume.
const VOLUME_INCREMENT: f32 = 0.10;
