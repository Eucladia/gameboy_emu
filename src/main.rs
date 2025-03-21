mod emulator;
mod error;
mod flags;
mod hardware;
mod instructions;
mod interrupts;

use emulator::Emulator;
use hardware::{Cpu, Hardware, joypad::Button};

use softbuffer::{Context, Surface};

use std::{
  fs,
  num::NonZeroU32,
  rc::Rc,
  time::{Duration, Instant},
};

use winit::{
  dpi::LogicalSize,
  event::{ElementState, Event, KeyEvent, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  keyboard::{KeyCode, PhysicalKey},
  window::WindowBuilder,
};

const FRAME_TIME: Duration = Duration::from_millis(16);

const GAMEBOY_WIDTH: u32 = 160;
const GAMEBOY_HEIGHT: u32 = 144;

const INITIAL_GAMEBOY_WIDTH: u32 = GAMEBOY_WIDTH * 3;
const INITIAL_GAMEBOY_HEIGHT: u32 = GAMEBOY_HEIGHT * 3;

fn main() {
  let rom = fs::read("./roms/Tetris.gb").unwrap();

  let cpu = Cpu::with_register_defaults();
  let hardware = Hardware::new(rom);
  let mut emulator = Emulator::new(cpu, hardware);

  let event_loop = EventLoop::new().unwrap();
  let window = Rc::new(
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

  let context = Context::new(Rc::clone(&window)).unwrap();
  let mut surface = Surface::new(&context, Rc::clone(&window)).unwrap();

  let mut last_update = Instant::now();
  let mut first_update = true;
  let mut limit_frames = true;

  let mut last_width = INITIAL_GAMEBOY_WIDTH;
  let mut last_height = INITIAL_GAMEBOY_HEIGHT;

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
          window_id,
          event: WindowEvent::CloseRequested,
          ..
        } if window_id == window.id() => elwt.exit(),

        Event::AboutToWait => {
          window.request_redraw();
        }

        Event::WindowEvent {
          window_id,
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
        } if window_id == window.id() => {
          if matches!(physical_key, PhysicalKey::Code(KeyCode::Space))
            && matches!(state, ElementState::Released)
          {
            limit_frames = !limit_frames;

            if limit_frames {
              elwt.set_control_flow(ControlFlow::WaitUntil(last_update + FRAME_TIME));
            } else {
              elwt.set_control_flow(ControlFlow::Poll);
            }

            window.request_redraw();
          } else if let Some(gb_button) = convert_button(&physical_key) {
            emulator
              .hardware
              .update_button(gb_button, matches!(state, ElementState::Pressed))
          }
        }

        Event::WindowEvent {
          window_id,
          event: WindowEvent::RedrawRequested,
          ..
        } if window_id == window.id() => {
          let now = Instant::now();

          if first_update || !limit_frames || now >= last_update + FRAME_TIME {
            let (width, height) = {
              let size = window.inner_size();
              (size.width, size.height)
            };

            if width != last_width || height != last_height {
              surface
                .resize(
                  NonZeroU32::new(width).unwrap(),
                  NonZeroU32::new(height).unwrap(),
                )
                .unwrap();

              window_frame.resize((width * height) as usize, 0);
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

            #[cfg(debug_assertions)]
            // Pre-fill the buffer with green in debug mode
            window_frame.fill(0x0000FF00);
            #[cfg(not(debug_assertions))]
            // Pre-fill the buffer with black in release builds
            window_frame.fill(0x00000000);

            for y in offset_y..offset_y + game_height {
              for x in offset_x..offset_x + game_width {
                let index = width * y + x;
                let src_x = (((x - offset_x) as f64 / scale) as u32).min(GAMEBOY_WIDTH - 1);
                let src_y = (((y - offset_y) as f64 / scale) as u32).min(GAMEBOY_HEIGHT - 1);

                let color = match game_buffer[src_y as usize][src_x as usize] {
                  0 => 0x00FFFFFF,
                  1 => 0x0088C070,
                  2 => 0x00346856,
                  3 => 0x00081820,
                  _ => 0x00FF0000,
                };

                window_frame[index as usize] = color;
              }
            }

            let mut buffer = surface.buffer_mut().unwrap();

            buffer.copy_from_slice(&window_frame);
            buffer.present().unwrap();

            last_update = now;
            first_update = false;
          }
        }

        _ => {}
      }
    })
    .unwrap();
}

/// Computes the scale factor for the game.
fn compute_scale_factor(window_width: u32, window_height: u32) -> f64 {
  let scale_x = window_width as f64 / GAMEBOY_WIDTH as f64;
  let scale_y = window_height as f64 / GAMEBOY_HEIGHT as f64;

  scale_x.min(scale_y).max(1.0)
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
