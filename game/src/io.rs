//! I/O abstractions used by the game.
//!
//! The game never touches hardware directly – it only knows how to
//! read and write bytes through a [`Console`] implementor. On QEMU
//! this is backed by SBI; on real hardware it will be backed by a
//! 16550 UART (or whatever the board provides).

use core::fmt;

/// A byte-oriented, blocking console.
pub trait Console {
    /// Write a single byte. Implementations may translate line
    /// endings if it makes sense for the underlying transport.
    fn write_byte(&mut self, byte: u8);

    /// Block until a byte is available and return it.
    fn read_byte_blocking(&mut self) -> u8;

    /// Convenience: write a whole string.
    fn write_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            self.write_byte(b);
        }
    }

    /// Convenience: format arguments into the console.
    ///
    /// Any errors from `core::fmt` are silently swallowed, which
    /// matches how a serial console behaves in practice.
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) {
        let mut adapter = Adapter { inner: self };
        let _ = fmt::Write::write_fmt(&mut adapter, args);
    }
}

/// Private bridge from [`Console`] to [`core::fmt::Write`].
struct Adapter<'a, C: Console + ?Sized> {
    inner: &'a mut C,
}

impl<'a, C: Console + ?Sized> fmt::Write for Adapter<'a, C> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Console::write_str(self.inner, s);
        Ok(())
    }
}

/// Write formatted text to a `Console`.
#[macro_export]
macro_rules! cprint {
    ($console:expr, $($arg:tt)*) => {{
        $crate::io::Console::write_fmt($console, core::format_args!($($arg)*));
    }};
}

/// Like [`cprint!`] but appends a newline.
#[macro_export]
macro_rules! cprintln {
    ($console:expr $(,)?) => {{
        $crate::io::Console::write_byte($console, b'\n');
    }};
    ($console:expr, $($arg:tt)*) => {{
        $crate::io::Console::write_fmt($console, core::format_args!($($arg)*));
        $crate::io::Console::write_byte($console, b'\n');
    }};
}
