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

pub enum Chip8Keys {
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    A,
    B,
    C,
    D,
    E,
    F,
}

pub struct Chip8 {
    ram: [u8; 4096],
    pub vram: [bool; 2048],
    stack: Vec<u16>,
    keys: [bool; 16],
    just_released_key: Option<u8>,
    speed: u32,
    cycles_count: u32,

    pc: usize,
    i: usize,
    delay_timer: u8,
    sound_timer: u8,
    v: [u8; 16],
}

impl Chip8 {
    pub fn new(bin: &Vec<u8>, speed: u32) -> Self {
        let mut ram = [0; 4096];
        ram[0x050..0x050 + FONT.len()].copy_from_slice(&FONT);
        ram[0x200..0x200 + bin.len()].copy_from_slice(bin);

        Self {
            ram,
            vram: [false; 2048],
            stack: vec![],
            keys: [false; 16],
            just_released_key: None,
            speed,
            cycles_count: 0,
            pc: 0x200,
            i: 0,
            delay_timer: 0,
            sound_timer: 0,
            v: [0; 16],
        }
    }

    pub fn key_press(&mut self, key: usize) {
        self.keys[key] = true;
    }

    pub fn key_release(&mut self, key: usize) {
        self.just_released_key = Some(key as u8);
        self.keys[key] = false;
    }

    pub fn is_sound_playing(&self) -> bool {
        self.sound_timer > 0
    }

    pub fn step(&mut self) -> bool {
        // timers
        self.cycles_count += 1;
        if self.cycles_count > self.speed / 60 {
            self.cycles_count = 0;
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }
            if self.sound_timer > 0 {
                self.sound_timer -= 1;
            }
        }

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
        //     "pc: 0x{:04X}, instr: 0x{:04X} (op: 0x{:X}, x: 0x{:X}, y: 0x{:X}, n: 0x{:X}, nn: 0x{:02X}, nnn: 0x{:02X}), regs: {:02X?}",
        //     self.pc - 2, instr, op, x, y, n, nn, nnn, self.v
        // );

        // execute
        let mut render = false;
        match op {
            0x0 => match nnn {
                0x0E0 => self.vram.iter_mut().for_each(|pixel| *pixel = false),
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
                    self.v[0xF] = if overflow { 0 } else { 1 };
                }
                0x6 => {
                    let tmp = self.v[y];
                    self.v[x] = tmp >> 1;
                    self.v[y] = self.v[y];
                    self.v[0xF] = tmp & 0x1;
                }
                0x7 => {
                    let (res, overflow) = self.v[y].overflowing_sub(self.v[x]);
                    self.v[x] = res;
                    self.v[0xF] = if overflow { 0 } else { 1 };
                }
                0xE => {
                    let tmp = self.v[y];
                    self.v[x] = tmp << 1;
                    self.v[y] = self.v[y];
                    self.v[0xF] = (tmp & 0x80) >> 7;
                }
                _ => panic!("unimplemented op 0x8 with nnn: 0x{:03X}", nnn),
            },
            0x9 => {
                if self.v[x] != self.v[y] {
                    self.pc += 2;
                }
            }
            0xA => self.i = nnn as usize,
            0xB => self.pc = nnn as usize + self.v[0x0] as usize,
            0xC => self.v[x] = rand::random::<u8>() & nn,
            0xD => {
                let x = (self.v[x] & 63) as usize;
                let y = (self.v[y] & 31) as usize;
                self.v[0xF] = 0;

                let sprite_bytes = &self.ram[self.i..self.i + (n as usize)];

                for (r, byte) in sprite_bytes.iter().enumerate() {
                    for c in 0..8 {
                        let offset = ((y + r) % 32) * 64 + ((x + c) % 64);

                        let old_pixel = self.vram[offset];
                        let new_pixel = (((byte >> (7 - c)) & 1) == 1) ^ old_pixel;

                        self.vram[offset] = new_pixel;
                        if old_pixel != new_pixel {
                            self.v[0xF] = 1;
                        }
                    }
                }

                render = true;
            }
            0xE => match nn {
                0x9E => {
                    if self.keys[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                0xA1 => {
                    if !self.keys[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                _ => panic!("unimplemented op 0xE with nnn: 0x{:03X}", nnn),
            },
            0xF => match nn {
                0x07 => self.v[x] = self.delay_timer,
                0x0A => {
                    if let Some(key) = self.just_released_key {
                        self.v[x] = key;
                    } else {
                        self.pc -= 2;
                    }
                }
                0x15 => self.delay_timer = self.v[x],
                0x18 => self.sound_timer = self.v[x],
                0x1E => {
                    self.i += self.v[x] as usize;
                    if self.i > 0x0FFF {
                        self.i -= 0x0FFF;
                        self.v[0xF] = 1;
                    };
                }
                0x29 => self.i = (self.v[x] as usize * 5) + 0x050,
                0x33 => {
                    self.ram[self.i] = (self.v[x] / 100) % 10;
                    self.ram[self.i + 1] = (self.v[x] / 10) % 10;
                    self.ram[self.i + 2] = self.v[x] % 10;
                }
                0x55 => {
                    for offset in 0..=x {
                        self.ram[self.i + offset] = self.v[offset];
                    }
                }
                0x65 => {
                    for offset in 0..=x {
                        self.v[offset] = self.ram[self.i + offset];
                    }
                }
                _ => panic!("unimplemented op 0xF with nnn: 0x{:03X}", nnn),
            },
            _ => panic!("unimplemented op: 0x{:X}", op),
        }

        self.just_released_key = None;
        render
    }
}
