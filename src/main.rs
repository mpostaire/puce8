// https://tobiasvl.github.io/blog/write-a-chip-8-emulator/

use std::{env, fs};

use sdl2::{event::Event, keyboard::Keycode, pixels::PixelFormatEnum, rect::Rect};

const FONT: [u8; 0x50] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

struct Chip8 {
    ram: [u8; 4096],
    pixels: [bool; 2048],
    stack: Vec<u16>,

    pc: usize,
    i: usize,
    delay_timer: u8,
    sound_timer: u8,
    v: [u8; 16],
}

impl Chip8 {
    fn new(bin: &Vec<u8>) -> Self {
        let mut ram = [0; 4096];
        ram[0x050..0x050 + FONT.len()].copy_from_slice(&FONT);
        ram[0x200..0x200 + bin.len()].copy_from_slice(bin);

        Self {
            ram,
            pixels: [false; 2048],
            stack: vec![],
            pc: 0x200,
            i: 0,
            delay_timer: 0, // TODO decrement 60Hz
            sound_timer: 0, // TODO decrement 60Hz and make beep sound as long as it is above 0
            v: [0; 16],
        }
    }

    fn step(&mut self) -> bool {
        // fetch
        let instr = (self.ram[self.pc] as u16) << 8 | (self.ram[self.pc + 1] as u16);
        self.pc += 2;

        // decode
        let op = ((instr & 0xF000) >> 12) as u8;
        let x = ((instr & 0x0F00) >> 8) as usize;
        let y = ((instr & 0x00F0) >> 4) as usize;
        let n = (instr & 0x000F) as u8;
        let nn = (instr & 0x00FF) as u8;
        let nnn = instr & 0x0FFF;

        // println!(
        //     "[step] pc=0x{:04X}, instr=0x{:04X} (op=0x{:X}, x=0x{:X}, y=0x{:X}, n=0x{:X}, nn=0x{:02X}, nnn=0x{:02X})",
        //     self.pc - 2, instr, op, x, y, n, nn, nnn
        // );

        // execute
        match op {
            0x0 => match nnn {
                0x0E0 => self.pixels.iter_mut().for_each(|x| *x = false),
                0x0EE => {
                    self.pc = self.stack.pop().unwrap() as usize;
                }
                _ => panic!("unimplemented op 0x0 with nnn: 0x{:03X}", nnn),
            },
            0x1 => self.pc = nnn as usize,
            0x2 => {
                self.stack.push(self.pc as u16);
                self.pc = nnn as usize;
            }
            0x3 => {
                if self.v[x] == nn {
                    self.pc += 2;
                }
            }
            0x4 => {
                if self.v[x] != nn {
                    self.pc += 2;
                }
            }
            0x5 => {
                if self.v[x] == self.v[y] {
                    self.pc += 2;
                }
            }
            0x6 => self.v[x] = nn,
            0x7 => self.v[x] = self.v[x].wrapping_add(nn),
            0x8 => match n {
                0x0 => self.v[x] = self.v[y],
                0x1 => self.v[x] |= self.v[y],
                0x2 => self.v[x] &= self.v[y],
                0x3 => self.v[x] ^= self.v[y],
                0x4 => {
                    let (res, overflow) = self.v[x].overflowing_add(self.v[y]);
                    self.v[x] = res;
                    self.v[0xF] = if overflow { 1 } else { 0 };
                }
                0x5 => {
                    let (res, overflow) = self.v[x].overflowing_sub(self.v[y]);
                    self.v[x] = res;
                    self.v[0xF] = if overflow { 1 } else { 0 };
                }
                0x6 => {
                    self.v[x] = self.v[y];
                    self.v[0xF] = self.v[x] & 0x1;
                    self.v[x] >>= 1;
                }
                0x7 => {
                    let (res, overflow) = self.v[y].overflowing_sub(self.v[x]);
                    self.v[x] = res;
                    self.v[0xF] = if overflow { 1 } else { 0 };
                }
                0xE => {
                    self.v[x] = self.v[y];
                    self.v[0xF] = self.v[x] & 0x1;
                    self.v[x] <<= 1;
                }
                _ => panic!("unimplemented op 0x8 with nnn: 0x{:03X}", nnn),
            },
            0x9 => {
                if self.v[x] != self.v[y] {
                    self.pc += 2;
                }
            }
            0xA => self.i = nnn as usize,
            0xD => {
                let x = (self.v[x] & 63) as usize;
                let y = (self.v[y] & 31) as usize;
                self.v[0xF] = 0;

                let sprite_byte = &self.ram[self.i..self.i + (n as usize)];

                for (r, byte) in sprite_byte.iter().enumerate() {
                    for c in 0..8 {
                        let offset = ((y + r) % 32) * 64 + ((x + c) % 64);
                        if offset >= 2048 {
                            break;
                        }

                        let old_pixel = self.pixels[offset];
                        let mut new_pixel = ((byte >> (7 - c)) & 1) == 1;

                        if new_pixel && old_pixel {
                            new_pixel = false;
                            self.v[0xF] = 1;
                        }

                        self.pixels[offset] = new_pixel;
                    }
                }

                return true;
            }
            0xF => match nn {
                0x07 => self.v[x] = self.delay_timer,
                0x15 => self.delay_timer = self.v[x],
                0x18 => self.sound_timer = self.v[x],
                0x33 => {
                    self.ram[self.i] = self.v[x] / 100;
                    self.ram[self.i + 1] = self.v[x] % 100 / 10;
                    self.ram[self.i + 2] = self.v[x] % 10;
                }
                0x55 => {
                    for offset in 0..=x {
                        self.ram[self.i + offset] = self.v[x];
                    }
                }
                0x65 => {
                    for offset in 0..=x {
                        self.v[x] = self.ram[self.i + offset];
                    }
                }
                _ => panic!("unimplemented op 0xF with nnn: 0x{:03X}", nnn),
            },
            _ => panic!("unimplemented op: 0x{:X}", op),
        }

        false
    }
}

fn main() -> Result<(), String> {
    let bin_path = env::args().nth(1).expect("Missing bin path");
    let bin = fs::read(bin_path).expect("Error reading bin");

    let mut emulator = Chip8::new(&bin);

    let sdl = sdl2::init()?;
    let video = sdl.video()?;

    let window = video
        .window("game tutorial", 512, 256)
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

    let mut event_pump = sdl.event_pump()?;

    let sleep_duration = ::std::time::Duration::new(0, 1_000_000_000u32 / 60);

    'running: loop {
        // Handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                _ => {}
            }
        }

        if emulator.step() {
            screen_texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    for y in 0..32 {
                        for x in 0..64 {
                            let pixel = if emulator.pixels[y * 64 + x] { 255 } else { 0 };
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

        ::std::thread::sleep(sleep_duration);
    }

    Ok(())
}
