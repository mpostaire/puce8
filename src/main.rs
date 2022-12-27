// https://tobiasvl.github.io/blog/write-a-chip-8-emulator/

use std::{env, fs, path::Path};

use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::Event,
    keyboard::{Scancode},
    pixels::PixelFormatEnum,
    rect::Rect,
};

use puce8::{Chip8, Chip8Keys};

fn sdl_scancode_to_chip8_key(key: Scancode) -> Option<usize> {
    match key {
        Scancode::Num1 => Some(Chip8Keys::Num1 as usize),
        Scancode::Num2 => Some(Chip8Keys::Num2 as usize),
        Scancode::Num3 => Some(Chip8Keys::Num3 as usize),
        Scancode::Num4 => Some(Chip8Keys::C as usize),
        Scancode::Q => Some(Chip8Keys::Num4 as usize),
        Scancode::W => Some(Chip8Keys::Num5 as usize),
        Scancode::E => Some(Chip8Keys::Num6 as usize),
        Scancode::R => Some(Chip8Keys::D as usize),
        Scancode::A => Some(Chip8Keys::Num7 as usize),
        Scancode::S => Some(Chip8Keys::Num8 as usize),
        Scancode::D => Some(Chip8Keys::Num9 as usize),
        Scancode::F => Some(Chip8Keys::E as usize),
        Scancode::Z => Some(Chip8Keys::A as usize),
        Scancode::X => Some(Chip8Keys::Num0 as usize),
        Scancode::C => Some(Chip8Keys::B as usize),
        Scancode::V => Some(Chip8Keys::F as usize),
        _ => None,
    }
}

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generate a square wave
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

// TODO test games/programs bins
// TODO wasm port
fn main() -> Result<(), String> {
    let bin_path = env::args().nth(1).expect("Missing bin path");
    let bin = fs::read(&bin_path).expect("Error reading bin");

    let emu_speed = 700; // 700 instructions per second
    let mut emulator = Chip8::new(&bin, emu_speed);

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let audio = sdl.audio()?;

    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1), // mono
        samples: None,     // default sample size
    };
    let device = audio.open_playback(None, &desired_spec, |spec| {
        // initialize the audio callback
        SquareWave {
            phase_inc: 440.0 / spec.freq as f32,
            phase: 0.0,
            volume: 0.05,
        }
    })?;

    let window_title = format!(
        "puce8 -- {}",
        Path::new(&bin_path).file_stem().unwrap().to_str().unwrap()
    );

    let window = video
        .window(window_title.as_str(), 512, 256)
        .position_centered()
        .hidden()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())?;

    // show window only after canvas has been created to avoid window show/hide/show at statup
    canvas.window_mut().show();

    let texture_creator = canvas.texture_creator();

    let mut screen_texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 64, 32)
        .map_err(|e| e.to_string())?;

    let rect = Rect::new(0, 0, 512, 256);

    // TODO AUDIO: generate continuous square wave (with the callback not the queue)
    //      --> pause device when sound timer is == 0
    //      --> play device when sound timer is > 0

    let mut event_pump = sdl.event_pump()?;

    let sleep_duration = ::std::time::Duration::new(0, 1_000_000_000u32 / emu_speed);

    'running: loop {
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    scancode: Some(Scancode::Escape),
                    ..
                } => {
                    break 'running;
                }
                Event::KeyDown {
                    scancode: Some(scancode),
                    repeat: false,
                    ..
                } => {
                    if let Some(key) = sdl_scancode_to_chip8_key(scancode) {
                        emulator.key_press(key);
                    }
                }
                Event::KeyUp {
                    scancode: Some(scancode),
                    repeat: false,
                    ..
                } => {
                    if let Some(key) = sdl_scancode_to_chip8_key(scancode) {
                        emulator.key_release(key);
                    }
                }
                _ => {}
            }
        }

        if emulator.step() {
            screen_texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..32 {
                        for x in 0..64 {
                            let pixel = if emulator.vram[y * 64 + x] { 255 } else { 0 };
                            let offset = y * pitch + x * 3;
                            buffer[offset] = pixel;
                            buffer[offset + 1] = pixel;
                            buffer[offset + 2] = pixel;
                        }
                    }
                })
                .unwrap();

            canvas.clear();
            canvas.copy(&screen_texture, None, rect)?;
            canvas.present();
        }

        if emulator.is_sound_playing() {
            device.resume();
        } else {
            device.pause();
        }

        ::std::thread::sleep(sleep_duration);
    }

    Ok(())
}
