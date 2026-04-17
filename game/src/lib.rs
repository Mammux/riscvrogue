//! A tiny text-mode roguelike.
//!
//! The game is written as a `no_std` library so it can be embedded in
//! the `kernel` crate and run on bare-metal RISC-V. All I/O goes
//! through the [`io::Console`] trait, which the kernel implements on
//! top of the SBI legacy console.
//!
//! This crate now also exposes a tiny bracket-like API surface:
//! [`engine::BTerm`], [`engine::GameState`], and [`engine::main_loop`].
//!
//! For now this is just a walking-around demo: an `@` on a small map
//! that the player can move with vi keys, arrow keys, or numpad-style
//! directions, and quit with `q`. It is deliberately kept minimal –
//! the plan is to grow it
//! into a proper dungeon crawler once the kernel has enough services
//! (timers, RNG from the platform, a framebuffer or a proper TTY).

#![no_std]

extern crate alloc;

#[macro_use]
pub mod io;
pub mod dungeon;
pub mod engine;
pub mod input;
mod map;

use dungeon::DungeonState;
use engine::{BTerm, GameState, TickResult, main_loop};
use input::GameKey;
use io::Console;
use map::Map;

pub mod prelude {
    pub use crate::dungeon::DungeonState;
    pub use crate::engine::{BTerm, GameState, TickResult, main_loop};
    pub use crate::input::GameKey;
}

struct DemoState {
    map: Map,
    px: usize,
    py: usize,
}

impl DemoState {
    fn new() -> Self {
        let map = Map::demo();
        let (px, py) = map.spawn();
        Self { map, px, py }
    }
}

impl GameState for DemoState {
    fn tick<C: Console + ?Sized>(&mut self, ctx: &mut BTerm<'_, C>) -> TickResult {
        render(ctx, &self.map, self.px, self.py);
        cprintln!(ctx.console_mut(), "Move: h/j/k/l, arrows, keypad 1-9   Quit: q");

        match ctx.key() {
            GameKey::Quit => {
                cprintln!(ctx.console_mut(), "\nThanks for playing!");
                TickResult::Quit
            }
            GameKey::Move { dx, dy } => {
                let nx = self.px as i32 + dx;
                let ny = self.py as i32 + dy;
                if self.map.is_walkable(nx, ny) {
                    self.px = nx as usize;
                    self.py = ny as usize;
                }
                TickResult::Continue
            }
            GameKey::Options | GameKey::Char(_) => TickResult::Continue,
            GameKey::Unknown => TickResult::Continue,
        }
    }
}

/// Run the roguelike on the given console. This function never
/// returns in normal play – the caller decides what to do when the
/// player quits (the kernel shuts the machine down).
pub fn run<C: Console + ?Sized>(console: &mut C) {
    let mut state = DemoState::new();
    main_loop(console, &mut state);
}

/// Run the procedural dungeon sample state.
///
/// This is a more "bracket-like" starting point than the fixed demo map.
pub fn run_dungeon<C: Console + ?Sized>(console: &mut C) {
    let mut state = DungeonState::new();
    main_loop(console, &mut state);
}

/// Run the procedural dungeon sample state with an explicit RNG seed.
///
/// Useful when the caller wants different layouts per boot while still being
/// able to reproduce runs by logging the seed.
pub fn run_dungeon_with_seed<C: Console + ?Sized>(console: &mut C, seed: u64) {
    let mut state = DungeonState::new_with_seed(seed);
    main_loop(console, &mut state);
}

fn render<C: Console + ?Sized>(ctx: &mut BTerm<'_, C>, map: &Map, px: usize, py: usize) {
    ctx.cls();
    ctx.print("riscvrogue -- a roguelike on bare-metal RISC-V\n\n");

    for y in 0..map.height() {
        for x in 0..map.width() {
            let ch = if x == px && y == py {
                b'@'
            } else {
                map.tile(x, y)
            };
            ctx.put_char(ch);
        }
        ctx.put_char(b'\n');
    }
    ctx.put_char(b'\n');
}
