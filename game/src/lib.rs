//! A tiny text-mode roguelike.
//!
//! The game is written as a `no_std` library so it can be embedded in
//! the `kernel` crate and run on bare-metal RISC-V. All I/O goes
//! through the [`io::Console`] trait, which the kernel implements on
//! top of the SBI legacy console.
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
mod map;

use io::Console;
use map::Map;

/// Run the roguelike on the given console. This function never
/// returns in normal play – the caller decides what to do when the
/// player quits (the kernel shuts the machine down).
pub fn run<C: Console + ?Sized>(console: &mut C) {
    let map = Map::demo();
    let (mut px, mut py) = map.spawn();

    loop {
        render(console, &map, px, py);
        cprintln!(console, "Move: h/j/k/l, arrows, keypad 1-9   Quit: q");

        let key = console.read_byte_blocking();
        if matches!(key, b'q' | 0x03 /* Ctrl-C */ | 0x04 /* Ctrl-D */) {
            cprintln!(console, "\nThanks for playing!");
            return;
        }

        let (dx, dy) = match movement_delta(key) {
            Some(delta) => delta,
            None => continue,
        };

        let nx = px as i32 + dx;
        let ny = py as i32 + dy;
        if map.is_walkable(nx, ny) {
            px = nx as usize;
            py = ny as usize;
        }
    }
}

fn movement_delta(key: u8) -> Option<(i32, i32)> {
    match key {
        b'h' | b'4' => Some((-1, 0)),
        b'l' | b'6' => Some((1, 0)),
        b'k' | b'8' => Some((0, -1)),
        b'j' | b'2' => Some((0, 1)),
        b'y' | b'7' => Some((-1, -1)),
        b'u' | b'9' => Some((1, -1)),
        b'b' | b'1' => Some((-1, 1)),
        b'n' | b'3' => Some((1, 1)),
        _ => None,
    }
}

fn render<C: Console + ?Sized>(console: &mut C, map: &Map, px: usize, py: usize) {
    // Clear screen + home cursor using ANSI escapes. The QEMU serial
    // console understands these.
    console.write_str("\x1b[2J\x1b[H");
    console.write_str("riscvrogue -- a roguelike on bare-metal RISC-V\n\n");

    for y in 0..map.height() {
        for x in 0..map.width() {
            let ch = if x == px && y == py {
                b'@'
            } else {
                map.tile(x, y)
            };
            console.write_byte(ch);
        }
        console.write_byte(b'\n');
    }
    console.write_byte(b'\n');
}
