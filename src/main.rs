#[allow(unused_imports)]
use std::{env, fs};

use sdl2::{
    audio::{AudioCallback, AudioSpecDesired},
    event::Event,
    keyboard::Scancode,
    pixels::PixelFormatEnum,
    rect::Rect,
};

use lazy_static::lazy_static;
use std::{process, sync::Mutex};

use puce8::{Chip8, Chip8Keys};

// This is very hacky but I can't figure how to communicate button inputs from js to rust...
lazy_static! {
    static ref EMULATOR: Mutex<Vec<Chip8>> = Mutex::new(vec![]);
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

// taken from https://github.com/Gigoteur/PX8/blob/master/src/px8/emscripten.rs
#[cfg(target_os = "emscripten")]
pub mod emscripten {
    use std::cell::RefCell;
    use std::os::raw::{c_int, c_void};
    use std::ptr::null_mut;

    #[allow(non_camel_case_types)]
    type em_callback_func = unsafe extern "C" fn();

    extern "C" {
        // void emscripten_set_main_loop(em_callback_func func, int fps, int simulate_infinite_loop)
        pub fn emscripten_set_main_loop(
            func: em_callback_func,
            fps: c_int,
            simulate_infinite_loop: c_int,
        );
    }

    thread_local!(static MAIN_LOOP_CALLBACK: RefCell<*mut c_void> = RefCell::new(null_mut()));

    pub fn set_main_loop_callback<F>(callback: F, fps: c_int)
    where
        F: FnMut(),
    {
        MAIN_LOOP_CALLBACK.with(|log| {
            *log.borrow_mut() = &callback as *const _ as *mut c_void;
        });

        unsafe {
            emscripten_set_main_loop(wrapper::<F>, fps, 1);
        }

        unsafe extern "C" fn wrapper<F>()
        where
            F: FnMut(),
        {
            MAIN_LOOP_CALLBACK.with(|z| {
                let closure = *z.borrow_mut() as *mut F;
                (*closure)();
            });
        }
    }
}

fn run_at_speed(bin: Vec<u8>, emu_speed: u32) {
    let sdl = sdl2::init().unwrap();
    let video = sdl.video().unwrap();
    let audio = sdl.audio().unwrap();

    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1), // mono
        samples: None,     // default sample size
    };
    let audio_device = audio
        .open_playback(None, &desired_spec, |spec| {
            // initialize the audio callback
            SquareWave {
                phase_inc: 440.0 / spec.freq as f32,
                phase: 0.0,
                volume: 0.05,
            }
        })
        .unwrap();

    let window = video
        .window("puce8", 512, 256)
        .position_centered()
        .hidden()
        .build()
        .map_err(|e| e.to_string())
        .unwrap();

    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .accelerated()
        .build()
        .map_err(|e| e.to_string())
        .unwrap();

    // show window only after canvas has been created to avoid window show/hide/show at statup
    canvas.window_mut().show();

    let tecture_creator = canvas.texture_creator();
    let mut screen_texture = tecture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 64, 32)
        .map_err(|e| e.to_string())
        .unwrap();

    let rect = Rect::new(0, 0, 512, 256);

    let fps = 60;

    let mut event_pump = sdl.event_pump().unwrap();

    EMULATOR.lock().unwrap().push(Chip8::new(&bin, emu_speed));
    let emulator = &mut EMULATOR.lock().unwrap()[0];

    #[allow(unused_mut)]
    let mut main_loop = || {
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    scancode: Some(Scancode::Escape),
                    ..
                } => {
                    // break 'running;
                    process::exit(0);
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

        if emulator.run_frame(fps) {
            let _ = screen_texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
                for y in 0..32 {
                    for x in 0..64 {
                        let pixel = if emulator.vram[y * 64 + x] { 255 } else { 0 };
                        let offset = y * pitch + x * 3;
                        buffer[offset] = pixel;
                        buffer[offset + 1] = pixel;
                        buffer[offset + 2] = pixel;
                    }
                }
            });

            canvas.clear();
            let _ = canvas.copy(&screen_texture, None, rect);
            canvas.present();
        }

        if emulator.is_sound_playing() {
            audio_device.resume();
        } else {
            audio_device.pause();
        }
    };

    #[cfg(not(target_os = "emscripten"))]
    {
        let sleep_duration = ::std::time::Duration::new(0, 1_000_000_000u32 / fps);
        loop {
            main_loop();
            ::std::thread::sleep(sleep_duration);
        }
    }

    #[cfg(target_os = "emscripten")]
    emscripten::set_main_loop_callback(main_loop, fps as i32);
}

fn run(bin: Vec<u8>) {
    run_at_speed(bin, 700);
}

#[cfg(target_os = "emscripten")]
#[no_mangle]
pub fn load_bin(bin: &[u8]) {
    run(bin.to_vec());
}

#[cfg(target_os = "emscripten")]
#[no_mangle]
pub fn on_gui_key_press(key: usize) {
    let mut emu_container = EMULATOR.lock().unwrap();
    if emu_container.len() > 0 {
        emu_container[0].key_press(key);
    }
}

#[cfg(target_os = "emscripten")]
#[no_mangle]
pub fn on_gui_key_release(key: usize) {
    let mut emu_container = EMULATOR.lock().unwrap();
    if emu_container.len() > 0 {
        emu_container[0].key_release(key);
    }
}

fn main() {
    #[cfg(not(target_os = "emscripten"))]
    {
        let bin_path = env::args().nth(1).expect("Missing bin path");
        let bin = fs::read(&bin_path).expect("Error reading bin");
        run(bin);
    }
}
