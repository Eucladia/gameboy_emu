mod emulator;
mod error;
mod flags;
mod hardware;
mod instructions;
mod interrupts;

use emulator::Emulator;
use flags::is_flag_set;
use hardware::{Cpu, Hardware, joypad::Button};
use pixels::{Pixels, SurfaceTexture, wgpu::PresentMode};

use std::{
  fs,
  sync::Arc,
  time::{Duration, Instant},
};

use winit::{
  dpi::{LogicalSize, PhysicalSize},
  event::{ElementState, Event, KeyEvent, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  keyboard::{KeyCode, PhysicalKey},
  window::WindowBuilder,
};

/// The Gameboy runs at 59.7275 frames per second.
const FRAME_TIME: Duration = Duration::from_micros(16_740);

const GAMEBOY_WIDTH: u32 = 160;
const GAMEBOY_HEIGHT: u32 = 144;

const INITIAL_GAMEBOY_WIDTH: u32 = GAMEBOY_WIDTH * 3;
const INITIAL_GAMEBOY_HEIGHT: u32 = GAMEBOY_HEIGHT * 3;

fn main() {
  let mut args = std::env::args();

  // The first argument is usually the executable name
  args.next();

  let Some(game_rom) = args.next() else {
    eprintln!("Expected a game to be passed as an argument!");
    return;
  };

  let rom_bytes = fs::read(&game_rom).unwrap();

  let cpu = Cpu::with_register_defaults();
  let hardware = Hardware::new(rom_bytes);
  let mut emulator = Emulator::new(cpu, hardware);

  let event_loop = EventLoop::new().unwrap();
  let window = Arc::new(
    WindowBuilder::new()
      .with_min_inner_size(LogicalSize::new(GAMEBOY_WIDTH, GAMEBOY_HEIGHT))
      .with_inner_size(LogicalSize::new(
        INITIAL_GAMEBOY_WIDTH,
        INITIAL_GAMEBOY_HEIGHT,
      ))
      .with_title("Gameboy")
      .build(&event_loop)
      .unwrap(),
  );

  let mut pixels = {
    let PhysicalSize { width, height } = window.inner_size();
    let surface_texture = SurfaceTexture::new(width, height, Arc::clone(&window));

    Pixels::new(width, height, surface_texture).unwrap()
  };

  pixels.set_present_mode(PresentMode::Immediate);

  let mut last_update = Instant::now();
  let mut first_update = true;
  let mut limit_frames = true;

  let mut last_width = INITIAL_GAMEBOY_WIDTH;
  let mut last_height = INITIAL_GAMEBOY_HEIGHT;

  let mut show_fps = false;
  let mut fps = 0.0;
  let mut num_frames = 0;
  let mut last_fps_update = last_update;

  let mut is_shift_held = false;

  let mut window_frame = vec![0; (last_width * last_height) as usize];

  event_loop
    .run(move |event, elwt| {
      if limit_frames {
        let next_frame_time = last_update + FRAME_TIME;

        elwt.set_control_flow(ControlFlow::WaitUntil(next_frame_time));
      } else {
        elwt.set_control_flow(ControlFlow::Poll);
      }

      match event {
        Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
        } => elwt.exit(),

        Event::AboutToWait => {
          window.request_redraw();
        }

        Event::WindowEvent {
          event:
            WindowEvent::KeyboardInput {
              event:
                KeyEvent {
                  physical_key,
                  state,
                  ..
                },
              ..
            },
          ..
        } => match physical_key {
          PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {
            is_shift_held = matches!(state, ElementState::Pressed);
          }
          PhysicalKey::Code(KeyCode::Digit1)
            if is_shift_held && matches!(state, ElementState::Pressed) =>
          {
            show_fps = !show_fps;
          }
          PhysicalKey::Code(KeyCode::Space) if matches!(state, ElementState::Released) => {
            limit_frames = !limit_frames;

            if limit_frames {
              elwt.set_control_flow(ControlFlow::WaitUntil(last_update + FRAME_TIME));
            } else {
              elwt.set_control_flow(ControlFlow::Poll);
            }

            window.request_redraw();
          }
          key => {
            if let Some(gb_button) = convert_button(&key) {
              emulator
                .hardware
                .update_button(gb_button, matches!(state, ElementState::Pressed))
            }
          }
        },

        Event::WindowEvent {
          event: WindowEvent::RedrawRequested,
          ..
        } => {
          let now = Instant::now();

          if first_update || !limit_frames || now >= last_update + FRAME_TIME {
            let (width, height) = {
              let size = window.inner_size();
              (size.width, size.height)
            };

            if width != last_width || height != last_height {
              pixels.resize_buffer(width, height).unwrap();
              pixels.resize_surface(width, height).unwrap();

              last_width = width;
              last_height = height;
            }

            emulator.step();

            let scale = compute_scale_factor(width, height);
            let game_width = (GAMEBOY_WIDTH as f64 * scale) as u32;
            let game_height = (GAMEBOY_HEIGHT as f64 * scale) as u32;

            // Make sure that the game is in the center of the screen
            let offset_x = (width - game_width) / 2;
            let offset_y = (height - game_height) / 2;

            let game_buffer = emulator.hardware.frame_buffer();

            let buffer = pixels.frame_mut();

            buffer.fill(0x00);

            // #[cfg(debug_assertions)]
            // // Pre-fill the buffer with green in debug mode
            // buffer.fill(0x00FF00FF);
            // #[cfg(not(debug_assertions))]
            // // Pre-fill the buffer with black in release builds
            // buffer.fill(0x000000FF);

            for y in offset_y..offset_y + game_height {
              for x in offset_x..offset_x + game_width {
                let index = 4 * (width * y + x) as usize;
                let src_x = (((x - offset_x) as f64 / scale) as u32).min(GAMEBOY_WIDTH - 1);
                let src_y = (((y - offset_y) as f64 / scale) as u32).min(GAMEBOY_HEIGHT - 1);

                let color = match game_buffer[src_y as usize][src_x as usize] {
                  0 => [0xFF, 0xFF, 0xFF, 0xFF],
                  1 => [0x88, 0xC0, 0x70, 0xFF],
                  2 => [0x34, 0x68, 0x56, 0xFF],
                  3 => [0x08, 0x18, 0x20, 0xFF],
                  _ => [0xFF, 0x00, 0x00, 0xFF],
                };

                buffer[index..index + 4].copy_from_slice(&color);
              }
            }

            num_frames += 1;

            let delta = now.duration_since(last_fps_update).as_secs_f64();

            if delta >= 1.0 {
              fps = num_frames as f64 / delta;
              last_fps_update = now;
              num_frames = 0;
            }

            if show_fps {
              const FPS_X_POS: u32 = 2;
              const FPS_Y_POS: u32 = 2;
              const FPS_TEXT_COLOR: u32 = 0xFF0000FF;

              let fps_text = format!("FPS: {:.4}", fps);

              draw_text(
                &fps_text,
                buffer,
                width,
                FPS_X_POS,
                FPS_Y_POS,
                FPS_TEXT_COLOR,
                scale as u32,
              );
            }

            pixels.render().unwrap();

            last_update = now;
            first_update = false;
          }
        }

        _ => {}
      }
    })
    .unwrap();
}

/// Draws the text into the buffer at the following x and y position.
fn draw_text(
  text: &str,
  buffer: &mut [u8],
  buffer_width: u32,
  x_pos: u32,
  y_pos: u32,
  color: u32,
  scale: u32,
) {
  for (character_x_pos, bitmap) in text.as_bytes().iter().enumerate().flat_map(|(index, b)| {
    get_character_bitmap(*b).map(|row| {
      (
        x_pos + (index as u32 * DEFAULT_CHARACTER_WIDTH * scale),
        row,
      )
    })
  }) {
    for (row, bits) in bitmap.iter().enumerate() {
      for col in 0..DEFAULT_CHARACTER_WIDTH {
        let mask = 1 << (DEFAULT_CHARACTER_WIDTH - 1 - col);

        if is_flag_set!(bits, mask) {
          for dx in 0..scale {
            for dy in 0..scale {
              let draw_x = character_x_pos + col * scale + dx;
              let draw_y = y_pos + row as u32 * scale + dy;
              let buffer_index = (draw_y * buffer_width + draw_x) as usize * 4;

              if buffer_index < buffer.len() as usize {
                buffer[buffer_index..buffer_index + 4].copy_from_slice(&color.to_be_bytes());
              }
            }
          }
        }
      }
    }
  }
}

const DEFAULT_CHARACTER_WIDTH: u32 = 7;
const DEFAULT_CHARACTER_HEIGHT: u32 = 8;

/// Converts the ASCII byte to a 7x8 bitmap.
#[rustfmt::skip]
const fn get_character_bitmap(byte: u8) -> Option<[u8; DEFAULT_CHARACTER_HEIGHT as usize]> {
  match byte {
    b'0' => Some([
      0b0111100,
      0b1000010,
      0b1000010,
      0b1000010,
      0b1000010,
      0b1000010,
      0b1000010,
      0b0111100,
    ]),
    b'1' => Some([
      0b0011000,
      0b0101000,
      0b0001000,
      0b0001000,
      0b0001000,
      0b0001000,
      0b0001000,
      0b0111100,
    ]),
    b'2' => Some([
      0b0111100,
      0b1000010,
      0b0000010,
      0b0000100,
      0b0001000,
      0b0010000,
      0b0100000,
      0b1111110,
    ]),
    b'3' => Some([
      0b0111100,
      0b1000010,
      0b0000010,
      0b0011100,
      0b0000010,
      0b0000010,
      0b1000010,
      0b0111100,
    ]),
    b'4' => Some([
      0b0000100,
      0b0001100,
      0b0010100,
      0b0100100,
      0b1000100,
      0b1111110,
      0b0000100,
      0b0000100,
    ]),
    b'5' => Some([
      0b1111110,
      0b1000000,
      0b1000000,
      0b1111100,
      0b0000010,
      0b0000010,
      0b1000010,
      0b0111100,
    ]),
    b'6' => Some([
      0b0111100,
      0b1000010,
      0b1000000,
      0b1111100,
      0b1000010,
      0b1000010,
      0b1000010,
      0b0111100,
    ]),
    b'7' => Some([
      0b1111110,
      0b0000010,
      0b0000100,
      0b0001000,
      0b0010000,
      0b0010000,
      0b0010000,
      0b0010000,
    ]),
    b'8' => Some([
      0b0111100,
      0b1000010,
      0b1000010,
      0b0111100,
      0b1000010,
      0b1000010,
      0b1000010,
      0b0111100,
    ]),
    b'9' => Some([
      0b0111100,
      0b1000010,
      0b1000010,
      0b0111110,
      0b0000010,
      0b0000010,
      0b1000010,
      0b0111100,
    ]),
    b'F' => Some([
      0b1111110,
      0b1000000,
      0b1000000,
      0b1111100,
      0b1000000,
      0b1000000,
      0b1000000,
      0b1000000,
    ]),
    b'P' => Some([
      0b1111100,
      0b1000010,
      0b1000010,
      0b1111100,
      0b1000000,
      0b1000000,
      0b1000000,
      0b1000000,
    ]),
    b'S' => Some([
      0b0111100,
      0b1000010,
      0b1000000,
      0b0111100,
      0b0000010,
      0b0000010,
      0b1000010,
      0b0111100,
    ]),
    b':' => Some([
      0b0000000,
      0b0011000,
      0b0011000,
      0b0000000,
      0b0000000,
      0b0011000,
      0b0011000,
      0b0000000,
    ]),
    b'.' => Some([
      0b0000000,
      0b0000000,
      0b0000000,
      0b0000000,
      0b0000000,
      0b0000000,
      0b0011000,
      0b0011000,
    ]),
    b' ' => Some([
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000000,
      0b00000000,
    ]),
    _ => None,
  }
}

/// Converts a winit key into a Gameboy button.
fn convert_button(physical_key: &PhysicalKey) -> Option<Button> {
  Some(match physical_key {
    PhysicalKey::Code(KeyCode::KeyW) => Button::Up,
    PhysicalKey::Code(KeyCode::KeyS) => Button::Down,
    PhysicalKey::Code(KeyCode::KeyA) => Button::Left,
    PhysicalKey::Code(KeyCode::KeyD) => Button::Right,

    PhysicalKey::Code(KeyCode::KeyZ) => Button::A,
    PhysicalKey::Code(KeyCode::KeyX) => Button::B,

    PhysicalKey::Code(KeyCode::Enter) => Button::Start,
    PhysicalKey::Code(KeyCode::Backspace) => Button::Select,

    _ => return None,
  })
}

/// Computes the scale factor for the game.
fn compute_scale_factor(window_width: u32, window_height: u32) -> f64 {
  let scale_x = window_width as f64 / GAMEBOY_WIDTH as f64;
  let scale_y = window_height as f64 / GAMEBOY_HEIGHT as f64;

  scale_x.min(scale_y).max(1.0)
}
