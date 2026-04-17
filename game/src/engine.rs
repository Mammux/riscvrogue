use crate::input::{decode_key, GameKey};
use crate::io::Console;

pub enum TickResult {
    Continue,
    Quit,
}

pub trait GameState {
    fn tick<C: Console + ?Sized>(&mut self, ctx: &mut BTerm<'_, C>) -> TickResult;
}

pub struct BTerm<'a, C: Console + ?Sized> {
    console: &'a mut C,
}

impl<'a, C: Console + ?Sized> BTerm<'a, C> {
    pub fn new(console: &'a mut C) -> Self {
        Self { console }
    }

    pub fn cls(&mut self) {
        self.console.write_str("\x1b[2J\x1b[H");
    }

    pub fn print(&mut self, text: &str) {
        self.console.write_str(text);
    }

    pub fn put_char(&mut self, byte: u8) {
        self.console.write_byte(byte);
    }

    pub fn set_sgr(&mut self, code: u8) {
        crate::cprint!(self.console, "\x1b[{}m", code);
    }

    pub fn reset_style(&mut self) {
        self.set_sgr(0);
    }

    /// Send a private font-select escape understood by the kernel's
    /// framebuffer console parser.
    pub fn set_font(&mut self, font_index: u8) {
        crate::cprint!(self.console, "\x1b[{}z", font_index);
    }

    pub fn console_mut(&mut self) -> &mut C {
        self.console
    }

    pub fn key(&mut self) -> GameKey {
        decode_key(self.console.read_byte_blocking())
    }
}

pub fn main_loop<C: Console + ?Sized, S: GameState>(console: &mut C, state: &mut S) {
    let mut ctx = BTerm::new(console);
    loop {
        if matches!(state.tick(&mut ctx), TickResult::Quit) {
            return;
        }
    }
}
